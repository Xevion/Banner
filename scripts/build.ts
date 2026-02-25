/**
 * Production build.
 *
 * Usage: bun scripts/build.ts [flags]
 *
 * Flags:
 *   -d, --debug           Debug build instead of release
 *   -f, --frontend-only   Frontend only
 *   -b, --backend-only    Backend only
 */

import { parseFlags, c } from "./lib/fmt";
import { run } from "./lib/proc";

const { flags } = parseFlags(
  process.argv.slice(2),
  {
    debug: "bool",
    "frontend-only": "bool",
    "backend-only": "bool",
  } as const,
  { d: "debug", f: "frontend-only", b: "backend-only" },
  { debug: false, "frontend-only": false, "backend-only": false },
);

if (flags["frontend-only"] && flags["backend-only"]) {
  console.error("Cannot use -f and -b together");
  process.exit(1);
}

const buildFrontend = !flags["backend-only"];
const buildBackend = !flags["frontend-only"];
const profile = flags.debug ? "debug" : "release";

if (buildFrontend) {
  console.log(c("1;36", "➡️ Building frontend..."));
  run(["bun", "run", "--cwd", "web", "build"]);
}

if (buildBackend) {
  console.log(c("1;36", `➡️ Building backend (${profile})...`));
  const cmd = ["cargo", "build", "--bin", "banner"];
  if (!flags.debug) cmd.push("--release");
  run(cmd);
}
