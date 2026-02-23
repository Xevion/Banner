const originalConsole = {
  log: console.log,
  error: console.error,
  warn: console.warn,
  info: console.info,
  debug: console.debug,
};

const useJson = process.env.LOG_JSON === "true" || process.env.LOG_JSON === "1";

function formatLog(level, args) {
  const message = args
    .map((arg) => (typeof arg === "object" ? JSON.stringify(arg) : String(arg)))
    .join(" ");

  if (useJson) {
    const logEntry = {
      timestamp: new Date().toISOString(),
      level: level,
      message: message,
      target: "bun",
    };
    originalConsole.log(JSON.stringify(logEntry));
  } else {
    const timestamp = new Date().toISOString().split("T")[1].slice(0, 12);
    const levelColors = {
      debug: "\x1b[36m", // cyan
      info: "\x1b[32m", // green
      warn: "\x1b[33m", // yellow
      error: "\x1b[31m", // red
    };
    const color = levelColors[level] || "";
    const reset = "\x1b[0m";
    const gray = "\x1b[90m";

    originalConsole.log(
      `${gray}${timestamp}${reset} ${color}${level.toUpperCase().padEnd(5)}${reset} ${gray}bun${reset}: ${message}`
    );
  }
}

console.log = (...args) => formatLog("info", args);
console.info = (...args) => formatLog("info", args);
console.warn = (...args) => formatLog("warn", args);
console.error = (...args) => formatLog("error", args);
console.debug = (...args) => formatLog("debug", args);
