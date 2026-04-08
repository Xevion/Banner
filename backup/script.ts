/**
 * Database backup: pg_dump -> gzip -> R2 upload.
 *
 * Designed for Railway cron: runs once and exits.
 * Reads DATABASE_URL and R2 credentials from environment (or .env locally).
 *
 * Usage:
 *   bun scripts/backup/script.ts              # Full backup + upload to R2
 *   bun scripts/backup/script.ts --dry-run    # Dump + compress locally, skip upload
 */

import { HeadObjectCommand, PutObjectCommand, S3Client } from "@aws-sdk/client-s3";
import { existsSync, readFileSync } from "fs";
import { mkdir } from "fs/promises";
import { promisify } from "util";
import { gzip as gzipCb } from "zlib";

const gzip = promisify(gzipCb);

/** Load .env when running locally (Railway injects env vars directly) */
function loadEnv(): void {
	const envPath = ".env";
	if (!existsSync(envPath)) return;

	const text = readFileSync(envPath, "utf8");
	for (const line of text.split("\n")) {
		const trimmed = line.trim();
		if (!trimmed || trimmed.startsWith("#")) continue;
		const eq = trimmed.indexOf("=");
		if (eq < 0) continue;
		const key = trimmed.slice(0, eq);
		const val = trimmed.slice(eq + 1);
		// Don't override existing env vars (e.g. Railway-injected ones)
		if (!process.env[key]) process.env[key] = val;
	}
}

loadEnv();

function requireEnv(key: string): string {
	const val = process.env[key];
	if (!val) {
		console.error(`missing required env var: ${key}`);
		process.exit(1);
	}
	return val;
}

const DATABASE_URL = requireEnv("DATABASE_URL");

const DRY_RUN = process.argv.includes("--dry-run");
const BACKUP_PREFIX = "backups/";

interface R2Config {
	accountId: string;
	bucket: string;
	accessKeyId: string;
	secretAccessKey: string;
	endpoint: string;
}

/** Validate all R2 credentials are present. Returns config or exits with a clear error. */
function requireR2Config(): R2Config {
	const keys = ["R2_ACCOUNT_ID", "R2_BUCKET", "R2_ACCESS_KEY_ID", "R2_SECRET_ACCESS_KEY"] as const;
	const missing = keys.filter((k) => !process.env[k]);

	if (missing.length > 0) {
		console.error(`missing R2 credentials: ${missing.join(", ")}`);
		process.exit(1);
	}

	const accountId = process.env.R2_ACCOUNT_ID!;
	return {
		accountId,
		bucket: process.env.R2_BUCKET!,
		accessKeyId: process.env.R2_ACCESS_KEY_ID!,
		secretAccessKey: process.env.R2_SECRET_ACCESS_KEY!,
		endpoint: `https://${accountId}.r2.cloudflarestorage.com`,
	};
}

function buildS3Client(r2: R2Config): S3Client {
	return new S3Client({
		region: "auto",
		endpoint: r2.endpoint,
		credentials: {
			accessKeyId: r2.accessKeyId,
			secretAccessKey: r2.secretAccessKey,
		},
	});
}

function timestamp(): string {
	const d = new Date();
	const pad = (n: number) => String(n).padStart(2, "0");
	return `${d.getUTCFullYear()}${pad(d.getUTCMonth() + 1)}${pad(d.getUTCDate())}T${pad(d.getUTCHours())}${pad(d.getUTCMinutes())}${pad(d.getUTCSeconds())}Z`;
}

function humanBytes(bytes: number): string {
	if (bytes < 1024) return `${bytes} B`;
	if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
	return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function elapsedSec(start: number): string {
	return ((Date.now() - start) / 1000).toFixed(1);
}

async function pgDump(): Promise<Buffer> {
	console.log("running pg_dump...");
	const start = Date.now();

	const proc = Bun.spawn(["pg_dump", "--no-owner", "--no-privileges", DATABASE_URL], {
		stdout: "pipe",
		stderr: "pipe",
	});

	const [stdout, stderr] = await Promise.all([
		new Response(proc.stdout).arrayBuffer(),
		new Response(proc.stderr).text(),
	]);
	await proc.exited;

	if (proc.exitCode !== 0) {
		console.error(`pg_dump failed (exit ${proc.exitCode}):\n${stderr}`);
		process.exit(1);
	}

	const raw = Buffer.from(stdout);
	console.log(`pg_dump complete: ${humanBytes(raw.length)} in ${elapsedSec(start)}s`);
	return raw;
}

async function compress(data: Buffer): Promise<Buffer> {
	console.log("compressing...");
	const start = Date.now();
	const compressed = await gzip(data, { level: 9 });
	const ratio = ((1 - compressed.length / data.length) * 100).toFixed(0);
	console.log(
		`compressed: ${humanBytes(data.length)} -> ${humanBytes(compressed.length)} (${ratio}% reduction) in ${elapsedSec(start)}s`,
	);
	return Buffer.from(compressed);
}

async function uploadToR2(client: S3Client, bucket: string, key: string, data: Buffer): Promise<void> {
	console.log(`uploading to R2: ${key} (${humanBytes(data.length)})...`);
	const start = Date.now();

	await client.send(
		new PutObjectCommand({
			Bucket: bucket,
			Key: key,
			Body: data,
			ContentType: "application/gzip",
		}),
	);

	console.log(`upload complete in ${elapsedSec(start)}s`);
}

async function verifyUpload(client: S3Client, bucket: string, key: string, expectedSize: number): Promise<void> {
	const resp = await client.send(
		new HeadObjectCommand({
			Bucket: bucket,
			Key: key,
		}),
	);

	const remoteSize = resp.ContentLength ?? 0;
	if (remoteSize !== expectedSize) {
		console.error(`verification failed: expected ${expectedSize} bytes, got ${remoteSize}`);
		process.exit(1);
	}

	console.log(`verified: ${key} (${humanBytes(remoteSize)})`);
}

// Force-exit if the script hangs for any reason (e.g. S3 client keeping sockets alive).
// .unref() ensures this timer doesn't itself prevent a clean exit.
setTimeout(() => {
	console.error("timed out after 5 minutes, forcing exit");
	process.exit(2);
}, 5 * 60 * 1000).unref();

async function main(): Promise<void> {
	const raw = await pgDump();
	const compressed = await compress(raw);

	if (DRY_RUN) {
		const dir = "tmp";
		if (!existsSync(dir)) await mkdir(dir, { recursive: true });
		const filename = `banner-${timestamp()}.sql.gz`;
		const outPath = `${dir}/${filename}`;
		await Bun.write(outPath, compressed);
		console.log(`dry-run: saved to ${outPath}`);
		return;
	}

	const r2 = requireR2Config();
	const client = buildS3Client(r2);

	const key = `${BACKUP_PREFIX}banner-${timestamp()}.sql.gz`;
	await uploadToR2(client, r2.bucket, key, compressed);
	await verifyUpload(client, r2.bucket, key, compressed.length);

	client.destroy();
	console.log("backup complete");
}

main().catch((err) => {
	console.error("backup failed:", err);
	process.exit(1);
});
