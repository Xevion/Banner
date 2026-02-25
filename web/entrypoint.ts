#!/usr/bin/env bun
/**
 * Docker entrypoint: orchestrates Rust backend + Bun SSR server.
 *
 * 1. Start Rust backend (port from PORT env, default 8000)
 * 2. Health-check Rust via /api/health
 * 3. Start Bun SSR server (port 3001)
 * 4. Monitor both -- exit if either dies
 */

import { type Subprocess, spawn } from "bun";

const RUST_BINARY = process.env.RUST_BINARY ?? "/app/banner";
const RUST_PORT = process.env.PORT ?? "8000";
const SSR_PORT = "3001";
const HEALTH_URL = `http://localhost:${RUST_PORT}/api/health`;
const HEALTH_TIMEOUT_MS = 15_000;
const HEALTH_INTERVAL_MS = 250;

const LOG_JSON = process.env.LOG_JSON ?? "true";
const LOG_LEVEL = process.env.LOG_LEVEL;
const ORIGIN = process.env.ORIGIN ?? `http://localhost:${RUST_PORT}`;

type LogLevel = "info" | "warn" | "error" | "debug";

function log(level: LogLevel, message: string, fields?: Record<string, unknown>) {
  if (LOG_JSON === "true" || LOG_JSON === "1") {
    const entry = {
      timestamp: new Date().toISOString(),
      level,
      target: "banner::entrypoint",
      message,
      ...fields,
    };
    const out = level === "error" ? process.stderr : process.stdout;
    out.write(`${JSON.stringify(entry)}\n`);
  } else {
    const prefix = level === "error" ? "ERROR: " : "";
    const suffix = fields
      ? ` ${Object.entries(fields)
          .map(([k, v]) => `${k}=${v}`)
          .join(" ")}`
      : "";
    const out = level === "error" ? console.error : console.log;
    out(`[entrypoint] ${prefix}${message}${suffix}`);
  }
}

// Shared env for both subprocesses -- normalizes logging and origin config
const sharedEnv: Record<string, string | undefined> = {
  ...process.env,
  LOG_JSON,
  ORIGIN,
};
if (LOG_LEVEL) {
  sharedEnv.LOG_LEVEL = LOG_LEVEL;
}

// Graceful shutdown on signals
let shuttingDown = false;
function shutdown(rustProc: Subprocess, ssrProc: Subprocess) {
  if (shuttingDown) return;
  shuttingDown = true;
  log("info", "Received shutdown signal, stopping processes...");
  rustProc.kill();
  ssrProc.kill();
  process.exit(0);
}

// Start Rust backend
log("info", "Starting Rust backend", { port: RUST_PORT });
const rust = spawn({
  cmd: [RUST_BINARY],
  env: {
    ...sharedEnv,
    PORT: RUST_PORT,
    SSR_DOWNSTREAM: `http://localhost:${SSR_PORT}`,
  },
  stdout: "inherit",
  stderr: "inherit",
});

// Wait for Rust to be healthy
const startTime = Date.now();
let healthy = false;
while (!healthy) {
  if (Date.now() - startTime > HEALTH_TIMEOUT_MS) {
    log("error", `Rust backend did not become healthy within ${HEALTH_TIMEOUT_MS}ms`);
    rust.kill();
    process.exit(1);
  }

  try {
    const resp = await fetch(HEALTH_URL);
    if (resp.ok) {
      healthy = true;
    }
  } catch {
    // Not ready yet
  }

  if (!healthy) {
    await Bun.sleep(HEALTH_INTERVAL_MS);
  }
}
log("info", "Rust backend is healthy");

// Start Bun SSR server
log("info", "Starting Bun SSR server", { port: SSR_PORT });
const ssr = spawn({
  cmd: ["bun", "--smol", "--preload", "/app/web/console-logger.js", "build/index.js"],
  cwd: "/app/web",
  env: {
    ...sharedEnv,
    PORT: SSR_PORT,
    HOST: "0.0.0.0",
    BACKEND_URL: `http://localhost:${RUST_PORT}`,
  },
  stdout: "inherit",
  stderr: "inherit",
});

// Register signal handlers after both processes are started
process.on("SIGTERM", () => shutdown(rust, ssr));
process.on("SIGINT", () => shutdown(rust, ssr));

log("info", "All processes started");

// Monitor both processes -- exit if either dies
async function monitor(name: string, proc: Subprocess) {
  const exitCode = await proc.exited;
  log("error", `${name} exited`, { exit_code: exitCode });
  return { name, exitCode };
}

const result = await Promise.race([monitor("rust", rust), monitor("ssr", ssr)]);

log("error", "Shutting down", { trigger: result.name });
rust.kill();
ssr.kill();
process.exit(result.exitCode || 1);
