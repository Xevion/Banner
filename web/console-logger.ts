/**
 * Normalizes SSR console output to the backend's log format.
 *
 * Preloaded with `node --import` so the patch is in place before any application module runs;
 * Node strips the types at load time, so there is no build step. The JSON shape is consumed by
 * VictoriaLogs ingest, which unpacks level/target into queryable fields.
 */
type Level = "debug" | "info" | "warn" | "error";

const originalConsole = {
  log: console.log,
  error: console.error,
  warn: console.warn,
  info: console.info,
  debug: console.debug,
};

const useJson = process.env.LOG_JSON === "true" || process.env.LOG_JSON === "1";

const levelColors: Record<Level, string> = {
  debug: "\x1b[36m", // cyan
  info: "\x1b[32m", // green
  warn: "\x1b[33m", // yellow
  error: "\x1b[31m", // red
};

function formatLog(level: Level, args: unknown[]): void {
  const message = args
    .map((arg) => (typeof arg === "object" ? JSON.stringify(arg) : String(arg)))
    .join(" ");

  if (useJson) {
    originalConsole.log(
      JSON.stringify({
        timestamp: new Date().toISOString(),
        level,
        message,
        target: "ssr",
      })
    );
    return;
  }

  const timestamp = new Date().toISOString().split("T")[1].slice(0, 12);
  const color = levelColors[level];
  const reset = "\x1b[0m";
  const gray = "\x1b[90m";

  originalConsole.log(
    `${gray}${timestamp}${reset} ${color}${level.toUpperCase().padEnd(5)}${reset} ${gray}ssr${reset}: ${message}`
  );
}

console.log = (...args: unknown[]) => formatLog("info", args);
console.info = (...args: unknown[]) => formatLog("info", args);
console.warn = (...args: unknown[]) => formatLog("warn", args);
console.error = (...args: unknown[]) => formatLog("error", args);
console.debug = (...args: unknown[]) => formatLog("debug", args);
