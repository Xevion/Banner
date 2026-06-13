import { existsSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, statSync, writeFileSync } from "node:fs";
import { readFile, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createInterface } from "node:readline";
import { defineConfig, presets, runners } from "@xevion/tempo";
import { newestMtime } from "@xevion/tempo/preflight";
import { ProcessGroup, run as tempoRun, runPiped } from "@xevion/tempo/proc";
import { resolveTargets } from "@xevion/tempo/targets";
import { BackendWatcher } from "@xevion/tempo/watch";

const BINDINGS_DIR = "web/src/lib/bindings";

// Shared: newest mtime across Rust sources + Cargo files
function rustSrcMtime(): number {
  return Math.max(
    newestMtime("src", "**/*.rs"),
    ...["Cargo.toml", "Cargo.lock"]
      .filter(existsSync)
      .map((f) => statSync(f).mtimeMs),
  );
}

// Generate barrel index.ts for TypeScript bindings
function generateBarrel(): void {
  const types = readdirSync(BINDINGS_DIR)
    .filter((f) => f.endsWith(".ts") && f !== "index.ts")
    .map((f) => f.replace(/\.ts$/, ""))
    .sort();

  writeFileSync(
    join(BINDINGS_DIR, "index.ts"),
    types.map((t) => `export type { ${t} } from "./${t}";`).join("\n") + "\n",
  );
}

// bun audit advisory ignores (transitive/no-fix/not-applicable). Single source of truth,
// shared with CI -- .github/workflows/ci.yml derives the same --ignore flags via jq.
const IGNORED_ADVISORIES: { id: string; reason: string }[] = JSON.parse(
  readFileSync("audit-ignore.json", "utf8"),
);

const bunAuditCmd = [
  "bun", "audit", "--audit-level=moderate",
  ...IGNORED_ADVISORIES.map(({ id }) => `--ignore=${id}`),
].join(" ");

// Database constants
const DB_USER = "banner";
const DB_PASS = "banner";
const DB_NAME = "banner";
const DB_PORT = "59489";

async function updateEnv(): Promise<void> {
  const url = `postgresql://${DB_USER}:${DB_PASS}@localhost:${DB_PORT}/${DB_NAME}`;
  const envFile = ".env";
  try {
    let content = await readFile(envFile, "utf8");
    content = content.includes("DATABASE_URL=")
      ? content.replace(/DATABASE_URL=.*$/m, `DATABASE_URL=${url}`)
      : content.trim() + `\nDATABASE_URL=${url}\n`;
    await writeFile(envFile, content);
  } catch {
    await writeFile(envFile, `DATABASE_URL=${url}\n`);
  }
}

// Words for generating random confirmation tokens (db reset safety)
const WORDS = [
  "apple", "beach", "blend", "camel", "cedar", "coral", "crane", "creek",
  "dance", "depot", "drift", "eagle", "ember", "exile", "fence", "flint",
  "flood", "frost", "giant", "grail", "grain", "grove", "hatch", "haven",
  "horse", "image", "inlet", "ivory", "jewel", "joker", "joust", "knife",
  "kudos", "lance", "lemur", "lodge", "mango", "maple", "merit", "navel",
  "noble", "north", "ocean", "olive", "orbit", "perch", "petal", "plank",
  "polar", "prism", "quest", "quill", "raven", "ridge", "royal", "scone",
  "shelf", "shell", "stark", "straw", "stone", "swift", "taste", "tiger",
  "toast", "tower", "trunk", "ultra", "under", "unify", "valve", "vapor",
  "vinyl", "whale", "winch", "xenon", "yacht", "yearn", "zebra", "amber",
] as const;

function randomToken(): string {
  const pick = () => WORDS[Math.floor(Math.random() * WORDS.length)];
  return `${pick()}-${pick()}-${pick()}`;
}

function readLine(prompt: string): Promise<string> {
  return new Promise((resolve) => {
    const rl = createInterface({ input: process.stdin, output: process.stderr });
    rl.question(prompt, (answer) => {
      rl.close();
      resolve(answer);
    });
  });
}

const rustPreset = presets.rust({ allFeatures: true, bin: "banner" });

export default defineConfig({
  runtime: "bun",
  subsystems: {
    frontend: {
      aliases: ["f", "front", "web"],
      cwd: "web",
      commands: {
        "format-check": "bun run format:check",
        "format-apply": "bun run format",
        lint: "bun run lint",
        "type-check": "bun run check",
        test: "bun run test",
        build: "bun run build",
      },
      autoFix: {
        "format-check": "format-apply",
      },
    },
    backend: {
      ...rustPreset,
      aliases: ["b", "back", "rust"],
      commands: {
        ...rustPreset.commands,
        "format-check": "cargo fmt --all -- --check",
        "format-apply": "cargo fmt --all",
        lint: "cargo clippy --all-features --all-targets -- -D warnings",
        "type-check": "cargo check --all-features",
        test: "cargo nextest run -E 'not test(export_bindings)'",
      },
    },
    security: {
      alwaysRun: true,
      aliases: ["sec", "audit"],
      commands: {
        "cargo-audit": {
          cmd: "cargo audit",
          requires: ["cargo-audit"],
        },
        "bun-audit": {
          cmd: bunAuditCmd,
          cwd: "web",
        },
        actionlint: {
          cmd: "actionlint",
          requires: ["actionlint"],
        },
      },
    },
  },
  preflights: [
    // Ensure frontend dependencies are installed
    (ctx) => {
      if (!existsSync("web/node_modules")) {
        ctx.fail("web/node_modules not found -- run `bun install --cwd web` first");
      }
    },

    // TS bindings: Rust types -> frontend TypeScript (diff-copy + barrel)
    async (ctx) => {
      const srcMtime = rustSrcMtime();
      const artifactMtime = newestMtime(BINDINGS_DIR, "**/*");

      if (artifactMtime >= srcMtime) return;

      ctx.logger.info("Regenerating TypeScript bindings (Rust sources changed)...");
      const tmpDir = mkdtempSync(join(tmpdir(), "banner-bindings-"));

      try {
        const { spawnSync } = await import("node:child_process");

        // Build test binary first
        const build = spawnSync("cargo", ["test", "--lib", "--no-run", "--quiet"], {
          stdio: ["ignore", "pipe", "pipe"],
        });
        if (build.status !== 0) {
          if (build.stderr?.length) process.stderr.write(build.stderr);
          ctx.fail("Failed to build bindings test binary");
          return;
        }

        // Export bindings to temp dir
        const exp = spawnSync("cargo", ["test", "--lib", "export_bindings", "--quiet"], {
          stdio: ["ignore", "pipe", "pipe"],
          env: { ...process.env, TS_RS_EXPORT_DIR: tmpDir },
        });
        if (exp.status !== 0) {
          if (exp.stdout?.length) process.stdout.write(exp.stdout);
          if (exp.stderr?.length) process.stderr.write(exp.stderr);
          ctx.fail("Failed to export bindings");
          return;
        }

        if (!existsSync(BINDINGS_DIR)) {
          mkdirSync(BINDINGS_DIR, { recursive: true });
        }

        const newFiles = new Set(readdirSync(tmpDir).filter((f) => f.endsWith(".ts")));
        const oldFiles = new Set(
          readdirSync(BINDINGS_DIR).filter((f) => f.endsWith(".ts") && f !== "index.ts"),
        );

        let changed = 0;
        for (const file of newFiles) {
          const newContent = readFileSync(join(tmpDir, file));
          const oldPath = join(BINDINGS_DIR, file);
          if (existsSync(oldPath)) {
            const oldContent = readFileSync(oldPath);
            if (Buffer.compare(newContent, oldContent) === 0) continue;
          }
          writeFileSync(oldPath, newContent);
          changed++;
        }

        for (const file of oldFiles) {
          if (!newFiles.has(file)) {
            rmSync(join(BINDINGS_DIR, file));
            changed++;
          }
        }

        generateBarrel();

        ctx.logger.info(`Bindings: ${newFiles.size} types, ${changed} changed`);
      } finally {
        rmSync(tmpDir, { recursive: true, force: true });
      }
    },

    // SQLx query metadata: Rust + migrations -> .sqlx/
    async (ctx) => {
      const srcMtime = Math.max(rustSrcMtime(), newestMtime("migrations", "*.sql"));
      const artifactMtime = newestMtime(".sqlx", "*.json");

      if (artifactMtime >= srcMtime) return;

      ctx.logger.info("Regenerating SQLx metadata (sources changed)...");
      const { spawnSync } = await import("node:child_process");
      const result = spawnSync("cargo", ["sqlx", "prepare"], {
        stdio: ["ignore", "pipe", "pipe"],
      });
      if (result.status !== 0) {
        ctx.logger.warn("sqlx prepare failed (is the database running?)");
        return;
      }
      const count = existsSync(".sqlx")
        ? readdirSync(".sqlx").filter((f) => f.endsWith(".json")).length
        : 0;
      ctx.logger.info(`SQLx: ${count} queries`);
    },
  ],
  hooks: {
    "before:dev": (ctx) => {
      if (ctx.targets.has("frontend") && !existsSync("web/node_modules")) {
        ctx.fail("web/node_modules not found -- run `bun install --cwd web` first");
      }
      if (!existsSync(".env")) {
        ctx.logger.warn(".env not found -- copy .env.example or create one with DATABASE_URL");
      }
    },
  },
  dev: {
    exitBehavior: "first-exits",
    processes: {
      frontend: {
        type: "unmanaged",
        cmd: ["bun", "run", "dev"],
        cwd: "web",
      },
      backend: {
        type: "managed",
        watch: {
          dirs: ["src", "migrations", ".sqlx", ".cargo"],
          exts: [".rs", ".sql", ".json", ".toml"],
          extraPaths: ["Cargo.toml", "Cargo.lock"],
          debounce: 200,
        },
        build: {
          cmd: ["cargo", "build", "--bin", "banner", "--no-default-features"],
        },
        run: {
          cmd: ["./target/debug/banner", "--tracing", "pretty"],
          passthrough: true,
        },
        interrupt: true,
      },
    },
  },
  commands: {
    check: runners.check({
      autoFixStrategy: "fix-first",
      exclude: ["frontend:build", "backend:build"],
    }),
    fmt: runners.sequential("format-apply", {
      description: "Sequential per-subsystem formatting",
      alias: "format",
      autoFixFallback: true,
    }),
    lint: runners.sequential("lint", {
      description: "Sequential per-subsystem linting",
    }),
    dev: {
      description: "Dev server with watch + reload",
      parameters: ["[targets...]", "--", "[passthrough...]"],
      flags: {
        "frontend-only": { type: Boolean, alias: "f", description: "Frontend only" },
        "backend-only": { type: Boolean, alias: "b", description: "Backend only" },
        "no-watch": { type: Boolean, alias: "W", description: "Build once + run (no watch)" },
        "no-build": { type: Boolean, alias: "n", description: "Run last compiled binary (no rebuild)" },
        release: { type: Boolean, alias: "r", description: "Use release profile" },
        embed: { type: Boolean, alias: "e", description: "Embed frontend assets (implies -b)" },
        "dev-build": { type: Boolean, alias: "d", description: "Dev build for frontend (faster, no minification)" },
        "no-interrupt": { type: Boolean, alias: "I", description: "Don't kill compiler on new changes" },
        "verbose-build": { type: Boolean, alias: "V", description: "Stream compilation output inline" },
        tracing: { type: String, description: "Tracing format", default: "pretty" },
      },
      run: async (ctx) => {
        let frontendOnly = ctx.flags["frontend-only"] as boolean;
        let backendOnly = ctx.flags["backend-only"] as boolean;
        let noWatch = ctx.flags["no-watch"] as boolean;
        const noBuild = ctx.flags["no-build"] as boolean;
        const release = ctx.flags.release as boolean;
        const embed = ctx.flags.embed as boolean;
        const devBuild = ctx.flags["dev-build"] as boolean;
        const tracing = (ctx.flags.tracing as string) || "pretty";

        // -e implies -b, -n implies -W
        if (embed) backendOnly = true;
        if (noBuild) noWatch = true;

        if (frontendOnly && backendOnly) {
          console.error("Cannot use -f and -b together (or -e implies -b)");
          return 1;
        }

        const runFrontend = !backendOnly;
        const runBackend = !frontendOnly;
        const profileDir = release ? "release" : "debug";
        const group = new ProcessGroup({ signal: "natural" });

        // Rust proxies non-API requests to the Vite/Bun SSR server
        const SSR_PORT = "3001";
        process.env.SSR_DOWNSTREAM = `http://localhost:${SSR_PORT}`;

        // Build frontend first when embedding assets
        if (embed && !noBuild) {
          const buildMode = devBuild ? "development" : "production";
          console.log(`Building frontend (${buildMode}, for embedding)...`);
          const buildArgs = ["bun", "run", "--cwd", "web", "build"];
          if (devBuild) buildArgs.push("--", "--mode", "development");
          tempoRun(buildArgs);
        }

        // Frontend: Vite dev server
        if (runFrontend) {
          group.spawn(["bun", "run", "--cwd", "web", "dev"]);
        }

        // Backend
        if (runBackend) {
          const backendArgs = ["--tracing", tracing, ...ctx.passthrough];
          const bin = `target/${profileDir}/banner`;
          const cargoExtra: string[] = [];
          if (!embed) cargoExtra.push("--no-default-features");

          if (noWatch) {
            if (!noBuild) {
              console.log(`Building backend (${release ? "release" : "dev"})...`);
              const cargoArgs = ["cargo", "build", "--bin", "banner", ...cargoExtra];
              if (release) cargoArgs.push("--release");
              tempoRun(cargoArgs);
            }

            if (!existsSync(bin)) {
              console.error(`Binary not found: ${bin}`);
              console.error(`Run 'just build${release ? "" : " -d"}' first, or remove -n.`);
              await group.killAll();
              return 1;
            }

            console.log(`Running ${bin} (no watch)`);
            group.spawn([bin, ...backendArgs]);
          } else {
            console.log("Starting backend dev server (watch mode)...");
            const watcher = new BackendWatcher({
              watchDirs: ["src"],
              watchExts: [".rs"],
              extraPaths: ["Cargo.toml", "Cargo.lock"],
              debounce: 200,
              buildCmd: ["cargo", "build", "--bin", "banner", ...cargoExtra, ...(release ? ["--release"] : [])],
              runCmd: [bin, ...backendArgs],
              interrupt: !ctx.flags["no-interrupt"],
              verboseBuild: ctx.flags["verbose-build"] as boolean,
            });
            group.onCleanup(() => watcher.killSync());
            group.onAsyncCleanup(() => watcher.shutdown());
            group.waitOn(watcher.done);
            watcher.start();
          }
        }

        const code = await group.waitForFirst();
        // 130 = SIGINT (128 + 2), normal dev server shutdown
        return code === 130 ? 0 : code;
      },
    },
    "pre-commit": runners.preCommit(),
    test: {
      description: "Run tests",
      parameters: ["[args...]"],
      run: async (ctx) => {
        const input = ctx.args.join(" ").trim();
        if (input === "web") {
          tempoRun(["bun", "run", "--cwd", "web", "test"]);
        } else if (input === "rust") {
          tempoRun(["cargo", "nextest", "run", "-E", "not test(export_bindings)"]);
        } else if (input === "") {
          tempoRun(["cargo", "nextest", "run", "-E", "not test(export_bindings)"]);
          tempoRun(["bun", "run", "--cwd", "web", "test"]);
        } else {
          tempoRun(["cargo", "nextest", "run", ...input.split(/\s+/)]);
        }
        return 0;
      },
    },
    build: {
      description: "Production build",
      parameters: ["[args...]"],
      flags: {
        debug: { type: Boolean, alias: "d", description: "Debug build instead of release" },
        "frontend-only": { type: Boolean, alias: "f", description: "Frontend only" },
        "backend-only": { type: Boolean, alias: "b", description: "Backend only" },
      },
      run: async (ctx) => {
        if (ctx.flags["frontend-only"] && ctx.flags["backend-only"]) {
          console.error("Cannot use -f and -b together");
          return 1;
        }
        const buildFrontend = !ctx.flags["backend-only"];
        const buildBackend = !ctx.flags["frontend-only"];

        if (buildFrontend) {
          console.log("Building frontend...");
          tempoRun(["bun", "run", "--cwd", "web", "build"]);
        }
        if (buildBackend) {
          const profile = ctx.flags.debug ? "debug" : "release";
          console.log(`Building backend (${profile})...`);
          const cmd = ["cargo", "build", "--bin", "banner"];
          if (!ctx.flags.debug) cmd.push("--release");
          tempoRun(cmd);
        }
        return 0;
      },
    },
    bindings: {
      description: "Force-regenerate TypeScript bindings",
      run: async () => {
        const tmpDir = mkdtempSync(join(tmpdir(), "banner-bindings-"));
        try {
          tempoRun(["cargo", "test", "--lib", "--no-run", "--quiet"]);
          const proc = Bun.spawnSync(
            ["cargo", "test", "--lib", "export_bindings", "--quiet"],
            {
              stdio: ["ignore", "inherit", "inherit"],
              env: { ...process.env, TS_RS_EXPORT_DIR: tmpDir },
            },
          );
          if (proc.exitCode !== 0) process.exit(proc.exitCode);

          // Clean slate + copy
          rmSync(BINDINGS_DIR, { recursive: true, force: true });
          mkdirSync(BINDINGS_DIR, { recursive: true });
          for (const file of readdirSync(tmpDir)) {
            writeFileSync(
              join(BINDINGS_DIR, file),
              readFileSync(join(tmpDir, file)),
            );
          }
          generateBarrel();

          const count = readdirSync(BINDINGS_DIR).filter((f) => f.endsWith(".ts") && f !== "index.ts").length;
          console.log(`Generated ${BINDINGS_DIR}/index.ts (${count} types)`);
        } finally {
          rmSync(tmpDir, { recursive: true, force: true });
        }
        return 0;
      },
    },
    db: {
      description: "PostgreSQL Docker Compose management",
      parameters: ["[subcommand]"],
      run: async (ctx) => {
        const subcmd = ctx.args[0] || "start";
        switch (subcmd) {
          case "start":
            tempoRun(["docker", "compose", "up", "-d"]);
            await updateEnv();
            console.log("started");
            break;
          case "reset": {
            // Interactive confirmation -- intentionally non-automatable
            if (!process.stdin.isTTY) {
              process.stderr.write("ERROR: `just db reset` requires an interactive terminal.\n");
              process.stderr.write("Cannot run non-interactively -- must be confirmed by a human.\n");
              return 1;
            }
            const token = randomToken();
            process.stderr.write("\nDATABASE RESET -- permanently drops and recreates the banner database.\n");
            process.stderr.write("All scraped data, user accounts, and sessions will be lost. No undo.\n\n");
            process.stderr.write(`Confirm by typing: ${token}\n`);
            const response = await readLine("> ");
            if (response.trim() !== token) {
              process.stderr.write("Aborted. Database was not modified.\n");
              return 1;
            }
            process.stderr.write("Resetting...\n");
            tempoRun(["docker", "compose", "up", "-d"]);
            await new Promise((r) => setTimeout(r, 2000));
            tempoRun(["docker", "compose", "exec", "postgres", "psql", "-U", DB_USER, "-d", "postgres", "-c", `DROP DATABASE IF EXISTS ${DB_NAME}`]);
            tempoRun(["docker", "compose", "exec", "postgres", "psql", "-U", DB_USER, "-d", "postgres", "-c", `CREATE DATABASE ${DB_NAME}`]);
            await updateEnv();
            console.log("reset");
            break;
          }
          case "rm":
            tempoRun(["docker", "compose", "down"]);
            console.log("removed");
            break;
          default:
            console.error(`Unknown db command: "${subcmd}"\nValid: start, reset, rm`);
            return 1;
        }
        return 0;
      },
    },
    search: {
      description: "Run Banner API search demo (hits live UTSA API)",
      parameters: ["[args...]"],
      run: async (ctx) => {
        tempoRun(["cargo", "run", "-q", "--bin", "search", "--", ...ctx.args]);
        return 0;
      },
    },
  },
});
