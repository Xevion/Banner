/**
 * Dev server orchestrator.
 *
 * Usage: bun scripts/dev.ts [flags] [-- passthrough-args]
 *
 * Flags:
 *   -f, --frontend-only   Frontend only (Vite dev server)
 *   -b, --backend-only    Backend only (watch + seamless reload)
 *   -W, --no-watch        Build once + run (no watch)
 *   -n, --no-build        Run last compiled binary (no rebuild)
 *   -r, --release         Use release profile
 *   -e, --embed           Embed assets (implies -b)
 *   -d, --dev-build       Use dev build for frontend (faster, no minification)
 *   -I, --no-interrupt    Don't kill compiler on new changes; wait, then rebuild
 *   -V, --verbose-build   Stream compilation output inline (default: buffered)
 *   --tracing <fmt>       Tracing format (default: pretty)
 */

import { existsSync } from "fs";
import { parseFlags, c } from "./lib/fmt";
import { run, ProcessGroup } from "./lib/proc";
import { BackendWatcher } from "./lib/watch";

const { flags, passthrough } = parseFlags(
  process.argv.slice(2),
  {
    "frontend-only": "bool",
    "backend-only": "bool",
    "no-watch": "bool",
    "no-build": "bool",
    release: "bool",
    embed: "bool",
    "dev-build": "bool",
    "no-interrupt": "bool",
    "verbose-build": "bool",
    tracing: "string",
  } as const,
  {
    f: "frontend-only",
    b: "backend-only",
    W: "no-watch",
    n: "no-build",
    r: "release",
    e: "embed",
    d: "dev-build",
    I: "no-interrupt",
    V: "verbose-build",
  },
  {
    "frontend-only": false,
    "backend-only": false,
    "no-watch": false,
    "no-build": false,
    release: false,
    embed: false,
    "dev-build": false,
    "no-interrupt": false,
    "verbose-build": false,
    tracing: "pretty",
  },
);

let frontendOnly = flags["frontend-only"];
let backendOnly = flags["backend-only"];
let noWatch = flags["no-watch"];
const noBuild = flags["no-build"];
const release = flags.release;
const embed = flags.embed;
const devBuild = flags["dev-build"];
const tracing = flags.tracing as string;

// -e implies -b
if (embed) backendOnly = true;
// -n implies -W
if (noBuild) noWatch = true;

if (frontendOnly && backendOnly) {
  console.error("Cannot use -f and -b together (or -e implies -b)");
  process.exit(1);
}

const runFrontend = !backendOnly;
const runBackend = !frontendOnly;
const profile = release ? "release" : "dev";
const profileDir = release ? "release" : "debug";
const group = new ProcessGroup();

// Rust proxies non-API requests to the Vite/Bun SSR server
const SSR_PORT = "3001";
process.env.SSR_DOWNSTREAM = `http://localhost:${SSR_PORT}`;

// Build frontend first when embedding assets
if (embed && !noBuild) {
  const buildMode = devBuild ? "development" : "production";
  console.log(c("1;36", `→ Building frontend (${buildMode}, for embedding)...`));
  const buildArgs = ["bun", "run", "--cwd", "web", "build"];
  if (devBuild) buildArgs.push("--", "--mode", "development");
  run(buildArgs);
}

// Frontend: Vite dev server
if (runFrontend) {
  group.spawn(["bun", "run", "--cwd", "web", "dev"]);
}

// Backend
if (runBackend) {
  const backendArgs = ["--tracing", tracing, ...passthrough];
  const bin = `target/${profileDir}/banner`;
  const cargoExtra: string[] = [];
  if (!embed) cargoExtra.push("--no-default-features");

  if (noWatch) {
    if (!noBuild) {
      console.log(c("1;36", `→ Building backend (${profile})...`));
      const cargoArgs = ["cargo", "build", "--bin", "banner", ...cargoExtra];
      if (release) cargoArgs.push("--release");
      run(cargoArgs);
    }

    if (!existsSync(bin)) {
      console.error(`Binary not found: ${bin}`);
      console.error(`Run 'just build${release ? "" : " -d"}' first, or remove -n.`);
      await group.killAll();
      process.exit(1);
    }

    console.log(c("1;36", `→ Running ${bin} (no watch)`));
    group.spawn([bin, ...backendArgs]);
  } else {
    // Seamless watch + reload
    console.log(c("1;36", "→ Starting backend dev server (watch mode)..."));
    const watcher = new BackendWatcher({
      binPath: bin,
      release,
      cargoExtra,
      args: backendArgs,
      interrupt: !flags["no-interrupt"],
      verboseBuild: flags["verbose-build"],
    });
    group.onAsyncCleanup(() => watcher.shutdown());
    watcher.start();
  }
}

const code = await group.waitForFirst();
// 130 = SIGINT (128 + 2), which is a normal dev server shutdown
process.exit(code === 130 ? 0 : code);
