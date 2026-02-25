#!/usr/bin/env bun
import { extname, join } from "path";
import { constants, brotliCompressSync, gzipSync } from "zlib";
import { $ } from "bun";
/**
 * Pre-compress static assets with maximum compression levels.
 * Run after `bun run build`.
 *
 * Generates .gz, .br, .zst variants for compressible files >= MIN_SIZE bytes.
 * These are embedded alongside originals by rust-embed and served via
 * content negotiation in src/web/assets.rs.
 */
import { readFile, readdir, stat, writeFile } from "fs/promises";

// Must match COMPRESSION_MIN_SIZE in src/web/encoding.rs
const MIN_SIZE = 512;

const COMPRESSIBLE_EXTENSIONS = new Set([
  ".js",
  ".css",
  ".html",
  ".json",
  ".svg",
  ".txt",
  ".xml",
  ".map",
]);

// Check if zstd CLI is available
let hasZstd = false;
try {
  await $`which zstd`.quiet();
  hasZstd = true;
} catch {
  console.warn("Warning: zstd not found, skipping .zst generation");
}

async function* walkDir(dir: string): AsyncGenerator<string> {
  try {
    const entries = await readdir(dir, { withFileTypes: true });
    for (const entry of entries) {
      const path = join(dir, entry.name);
      if (entry.isDirectory()) {
        yield* walkDir(path);
      } else if (entry.isFile()) {
        yield path;
      }
    }
  } catch {
    // Directory doesn't exist, skip
  }
}

async function compressFile(path: string): Promise<void> {
  const ext = extname(path);

  if (!COMPRESSIBLE_EXTENSIONS.has(ext)) return;
  if (path.endsWith(".br") || path.endsWith(".gz") || path.endsWith(".zst")) return;

  const stats = await stat(path);
  if (stats.size < MIN_SIZE) return;

  // Skip if all compressed variants already exist
  const variantsExist = await Promise.all([
    stat(`${path}.br`).then(
      () => true,
      () => false
    ),
    stat(`${path}.gz`).then(
      () => true,
      () => false
    ),
    hasZstd
      ? stat(`${path}.zst`).then(
          () => true,
          () => false
        )
      : Promise.resolve(false),
  ]);

  if (variantsExist.every((exists) => exists || !hasZstd)) {
    return;
  }

  const content = await readFile(path);
  const originalSize = content.length;

  // Brotli (maximum quality = 11)
  const brContent = brotliCompressSync(content, {
    params: {
      [constants.BROTLI_PARAM_QUALITY]: 11,
    },
  });
  await writeFile(`${path}.br`, brContent);

  // Gzip (level 9)
  const gzContent = gzipSync(content, { level: 9 });
  await writeFile(`${path}.gz`, gzContent);

  // Zstd (level 19 - maximum)
  if (hasZstd) {
    try {
      await $`zstd -19 -q -f -o ${path}.zst ${path}`.quiet();
    } catch (e) {
      console.warn(`Warning: Failed to compress ${path} with zstd: ${e}`);
    }
  }

  const brRatio = ((brContent.length / originalSize) * 100).toFixed(1);
  const gzRatio = ((gzContent.length / originalSize) * 100).toFixed(1);
  console.log(`Compressed: ${path} (br: ${brRatio}%, gz: ${gzRatio}%, ${originalSize} bytes)`);
}

async function main() {
  console.log("Pre-compressing static assets...");

  // SSR adapter outputs client assets to build/client/
  const dirs = ["build/client"];
  let scannedFiles = 0;
  let compressedFiles = 0;

  for (const dir of dirs) {
    for await (const file of walkDir(dir)) {
      const ext = extname(file);
      scannedFiles++;

      if (
        COMPRESSIBLE_EXTENSIONS.has(ext) &&
        !file.endsWith(".br") &&
        !file.endsWith(".gz") &&
        !file.endsWith(".zst")
      ) {
        const stats = await stat(file);
        if (stats.size >= MIN_SIZE) {
          await compressFile(file);
          compressedFiles++;
        }
      }
    }
  }

  console.log(`Done! Scanned ${scannedFiles} files, compressed ${compressedFiles} files.`);
}

main().catch((e) => {
  console.error("Compression failed:", e);
  process.exit(1);
});
