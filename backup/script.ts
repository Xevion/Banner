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

import { HeadObjectCommand, S3Client } from "@aws-sdk/client-s3";
import { Upload } from "@aws-sdk/lib-storage";
import { createWriteStream, existsSync, readFileSync } from "fs";
import { mkdir } from "fs/promises";
import { Readable, Transform } from "stream";
import { pipeline } from "stream/promises";
import { createGzip } from "zlib";

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

interface StreamSizes {
	raw: number;
	compressed: number;
}

/** Counts bytes flowing through a stream without buffering them. */
function counter(onChunk: (n: number) => void): Transform {
	return new Transform({
		transform(chunk, _enc, cb) {
			onChunk(chunk.length);
			cb(null, chunk);
		},
	});
}

/** Spawn pg_dump; returns the process and a Node readable over its stdout. */
function spawnDump(): { proc: Bun.Subprocess; source: Readable; stderr: Promise<string> } {
	const proc = Bun.spawn(["pg_dump", "--no-owner", "--no-privileges", DATABASE_URL], {
		stdout: "pipe",
		stderr: "pipe",
	});
	return {
		proc,
		source: Readable.fromWeb(proc.stdout as ReadableStream),
		stderr: new Response(proc.stderr).text(),
	};
}

/** Fail loudly if pg_dump exited non-zero (guards against uploading a truncated dump). */
async function assertDumpOk(proc: Bun.Subprocess, stderr: Promise<string>): Promise<void> {
	await proc.exited;
	if (proc.exitCode !== 0) {
		console.error(`pg_dump failed (exit ${proc.exitCode}):\n${await stderr}`);
		process.exit(1);
	}
}

function logSizes(sizes: StreamSizes, start: number): void {
	const ratio = ((1 - sizes.compressed / sizes.raw) * 100).toFixed(0);
	console.log(
		`dump ${humanBytes(sizes.raw)} -> gz ${humanBytes(sizes.compressed)} (${ratio}% reduction) in ${elapsedSec(start)}s`,
	);
}

/** Stream pg_dump -> gzip -> R2 multipart upload. Never materializes the full dump. */
async function streamToR2(client: S3Client, bucket: string, key: string): Promise<StreamSizes> {
	console.log(`streaming pg_dump -> gzip -> R2: ${key}`);
	const start = Date.now();

	const { proc, source, stderr } = spawnDump();
	const sizes: StreamSizes = { raw: 0, compressed: 0 };
	const gzip = createGzip({ level: 9 });
	const body = source
		.pipe(counter((n) => (sizes.raw += n)))
		.pipe(gzip)
		.pipe(counter((n) => (sizes.compressed += n)));

	const upload = new Upload({
		client,
		params: { Bucket: bucket, Key: key, Body: body, ContentType: "application/gzip" },
	});

	await Promise.all([upload.done(), assertDumpOk(proc, stderr)]);
	logSizes(sizes, start);
	console.log(`upload complete in ${elapsedSec(start)}s`);
	return sizes;
}

/** Stream pg_dump -> gzip -> local file (dry-run path). */
async function streamToFile(outPath: string): Promise<StreamSizes> {
	console.log(`streaming pg_dump -> gzip -> file: ${outPath}`);
	const start = Date.now();

	const { proc, source, stderr } = spawnDump();
	const sizes: StreamSizes = { raw: 0, compressed: 0 };
	const gzip = createGzip({ level: 9 });

	await Promise.all([
		pipeline(
			source,
			counter((n) => (sizes.raw += n)),
			gzip,
			counter((n) => (sizes.compressed += n)),
			createWriteStream(outPath),
		),
		assertDumpOk(proc, stderr),
	]);
	logSizes(sizes, start);
	return sizes;
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
	if (DRY_RUN) {
		const dir = "tmp";
		if (!existsSync(dir)) await mkdir(dir, { recursive: true });
		const outPath = `${dir}/banner-${timestamp()}.sql.gz`;
		await streamToFile(outPath);
		console.log(`dry-run: saved to ${outPath}`);
		return;
	}

	const r2 = requireR2Config();
	const client = buildS3Client(r2);

	const key = `${BACKUP_PREFIX}banner-${timestamp()}.sql.gz`;
	const sizes = await streamToR2(client, r2.bucket, key);
	await verifyUpload(client, r2.bucket, key, sizes.compressed);

	client.destroy();
	console.log("backup complete");
}

main().catch((err) => {
	console.error("backup failed:", err);
	process.exit(1);
});
