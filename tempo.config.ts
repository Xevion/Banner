import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { readFile, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { createInterface } from "node:readline";
import type { RunContext } from "@xevion/tempo";
import { defineConfig, presets, task } from "@xevion/tempo";

const BINDINGS_DIR = "web/src/lib/bindings";
const RUST_SOURCES = ["src/**/*.rs", "Cargo.toml", "Cargo.lock"];

/** Rewrite the bindings barrel from the generated per-type files. */
function generateBarrel(root: string): number {
  const dir = join(root, BINDINGS_DIR);
  const types = readdirSync(dir)
    .filter((f) => f.endsWith(".ts") && f !== "index.ts")
    .map((f) => f.replace(/\.ts$/, ""))
    .sort();
  writeFileSync(
    join(dir, "index.ts"),
    `${types.map((t) => `export type { ${t} } from "./${t}";`).join("\n")}\n`,
  );
  return types.length;
}

/**
 * Export ts-rs bindings via a temp dir, then diff-copy into the tree.
 *
 * Unchanged files keep their contents untouched and types deleted from Rust
 * lose their `.ts`, so the barrel always matches what Rust actually exports.
 */
async function regenerateBindings(ctx: RunContext): Promise<number> {
  const build = await ctx.run(["cargo", "test", "--lib", "--no-run", "--quiet"]);
  if (build !== 0) return build;

  const tmp = mkdtempSync(join(tmpdir(), "banner-bindings-"));
  try {
    const exported = await ctx.run(
      ["cargo", "test", "--lib", "export_bindings", "--quiet"],
      { env: { TS_RS_EXPORT_DIR: tmp } },
    );
    if (exported !== 0) return exported;

    const dir = join(ctx.cwd, BINDINGS_DIR);
    mkdirSync(dir, { recursive: true });

    // ts-rs nests some types (serde_json/JsonValue.ts), so recurse.
    const fresh = new Set(
      (readdirSync(tmp, { recursive: true }) as string[]).filter((f) =>
        f.endsWith(".ts"),
      ),
    );
    let changed = 0;
    for (const entry of fresh) {
      const next = readFileSync(join(tmp, entry));
      const dest = join(dir, entry);
      if (existsSync(dest) && Buffer.compare(next, readFileSync(dest)) === 0) {
        continue;
      }
      mkdirSync(dirname(dest), { recursive: true });
      writeFileSync(dest, next);
      changed++;
    }
    for (const stale of readdirSync(dir)) {
      if (!stale.endsWith(".ts") || stale === "index.ts") continue;
      if (!fresh.has(stale)) {
        rmSync(join(dir, stale));
        changed++;
      }
    }

    ctx.log(`${generateBarrel(ctx.cwd)} types, ${changed} changed`);
    return 0;
  } finally {
    rmSync(tmp, { recursive: true, force: true });
  }
}

const DB_URL = "postgresql://banner:banner@localhost:59489/banner";

async function updateEnv(root: string): Promise<void> {
  const envFile = join(root, ".env");
  try {
    const current = await readFile(envFile, "utf8");
    const next = current.includes("DATABASE_URL=")
      ? current.replace(/DATABASE_URL=.*$/m, `DATABASE_URL=${DB_URL}`)
      : `${current.trim()}\nDATABASE_URL=${DB_URL}\n`;
    await writeFile(envFile, next);
  } catch {
    await writeFile(envFile, `DATABASE_URL=${DB_URL}\n`);
  }
}

const WORDS = [
  "apple", "beach", "blend", "camel", "cedar", "coral", "crane", "creek",
  "dance", "depot", "drift", "eagle", "ember", "exile", "fence", "flint",
  "flood", "frost", "giant", "grail", "grain", "grove", "hatch", "haven",
  "horse", "image", "inlet", "ivory", "jewel", "joker", "joust", "knife",
  "lance", "lemur", "lodge", "mango", "maple", "merit", "navel", "noble",
  "ocean", "olive", "orbit", "perch", "petal", "plank", "polar", "prism",
  "quest", "quill", "raven", "ridge", "royal", "shelf", "shell", "stark",
  "stone", "swift", "tiger", "toast", "tower", "trunk", "valve", "vapor",
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

// bun audit advisory ignores, shared with CI: .github/workflows/ci.yml derives
// the same --ignore flags from this file via jq.
const IGNORED_ADVISORIES: { id: string; reason: string }[] = JSON.parse(
  readFileSync(join(import.meta.dirname, "audit-ignore.json"), "utf8"),
);

// Deploy target. Kept out of tracked config since this repo is public; set these
// in .env rather than hardcoding a real host.
const DEPLOY_HOST = process.env.DEPLOY_HOST;
const DEPLOY_PATH = process.env.DEPLOY_PATH ?? "~/banner-src";
const DEPLOY_IMAGE = process.env.DEPLOY_IMAGE ?? "ghcr.io/xevion/banner";
const DEPLOY_RELEASE = process.env.DEPLOY_RELEASE ?? "banner";
// Read at frontend build time to widen the CSP, so it is a Dockerfile ARG rather
// than a runtime variable: changing it needs a rebuild, not a restart.
const PUBLIC_POSTHOG_HOST = process.env.PUBLIC_POSTHOG_HOST;

// Allowlist, never "everything in .env": DATABASE_URL points at the local
// compose Postgres, and POSTGRES_*/S3_* are managed out of band.
const SYNCED_SECRET_KEYS = [
  "BOT_TOKEN",
  "BOT_TARGET_GUILD",
  "DISCORD_CLIENT_ID",
  "DISCORD_CLIENT_SECRET",
] as const;

// Synced only when .env has them; the deployment runs without any of them.
const OPTIONAL_SECRET_KEYS = [
  "ADMIN_DISCORD_ID",
  "BANNER_BASE_URL",
  "POSTHOG_KEY",
  "POSTHOG_HOST",
  "POSTHOG_PROJECT_ID",
  "POSTHOG_PERSONAL_API_KEY",
  "PUBLIC_POSTHOG_KEY",
] as const;

const ssh = (ctx: RunContext, command: string) =>
  ctx.run(["ssh", DEPLOY_HOST as string, command]);

/**
 * Merge the allowlisted .env values into the cluster Secret.
 *
 * .env holds development credentials, including a Discord application distinct from
 * the production one, so this overwrites the running bot's identity. Run it only when
 * the local values are meant to become the deployed ones; deploys do not call it.
 *
 * A merge patch, so keys managed out of band survive untouched. The patch would
 * reach the remote process list on argv, so it travels on stdin instead.
 */
async function syncSecrets(ctx: RunContext): Promise<number> {
  const missing = SYNCED_SECRET_KEYS.filter(
    (k) => !(process.env[k] ?? "").trim(),
  );
  if (missing.length > 0) ctx.fail(`missing in .env: ${missing.join(", ")}`);

  const keys = [
    ...SYNCED_SECRET_KEYS,
    ...OPTIONAL_SECRET_KEYS.filter((k) => (process.env[k] ?? "").trim()),
  ];
  const stringData = Object.fromEntries(
    keys.map((k) => [k, process.env[k] as string]),
  );
  const patch = await Bun.spawn(
    [
      "ssh",
      DEPLOY_HOST as string,
      `sudo k3s kubectl patch secret ${DEPLOY_RELEASE}-secrets --type=merge --patch-file=/dev/stdin`,
    ],
    { stdin: new TextEncoder().encode(JSON.stringify({ stringData })) },
  ).exited;
  if (patch !== 0) return patch;

  ctx.log(`synced ${keys.length} secrets: ${keys.join(", ")}`);
  return 0;
}

/**
 * Report the CORS origin the rolled-out deployment actually received.
 *
 * A wrong-but-valid origin is invisible from outside: the site keeps working
 * while every cross-origin caller is refused. Printing it makes it noticeable.
 */
async function reportPublicOrigin(ctx: RunContext): Promise<number> {
  const jsonpath = `{.spec.template.spec.containers[0].env[?(@.name=="PUBLIC_ORIGIN")].value}`;
  const { stdout } = await ctx.capture([
    "ssh",
    DEPLOY_HOST as string,
    `sudo k3s kubectl get deployment/${DEPLOY_RELEASE} -o jsonpath='${jsonpath}'`,
  ]);
  const origin = stdout.trim();
  if (!origin) {
    ctx.fail(
      "PUBLIC_ORIGIN missing from the rolled-out deployment: CORS admits local dev origins only",
    );
  }
  ctx.log(`CORS origin: ${origin}`);
  return 0;
}

export default defineConfig({
  runtime: "bun",
  tasks: [
    task({
      name: "web:deps",
      cwd: "web",
      body: (ctx) => {
        if (!existsSync(join(ctx.cwd, "node_modules"))) {
          ctx.fail("web/node_modules not found -- run `bun install --cwd web` first");
        }
      },
    }),

    // Rust types to frontend TypeScript. The frontend imports the barrel, so
    // every frontend check orders behind it.
    task({
      name: "backend:bindings",
      body: regenerateBindings,
      inputs: RUST_SOURCES,
      outputs: [`${BINDINGS_DIR}/index.ts`],
    }),

    // Offline query metadata for sqlx's compile-time checking.
    task({
      name: "backend:sqlx",
      body: async (ctx) => {
        if ((await ctx.run(["cargo", "sqlx", "prepare"])) !== 0) {
          ctx.log("sqlx prepare failed -- is the database running?");
        }
      },
      tags: ["check"],
      inputs: [...RUST_SOURCES, "migrations/*.sql"],
      outputs: [".sqlx/*.json"],
      requires: [{ tool: "cargo-sqlx", hint: "cargo install sqlx-cli" }],
    }),

    task({
      name: "web:format",
      cwd: "web",
      body: "bun run format:check",
      tags: ["check"],
      needs: ["web:deps"],
    }),
    task({
      name: "web:format-fix",
      cwd: "web",
      body: "bun run format",
      tags: ["format"],
      needs: ["web:deps"],
    }),
    task({
      name: "web:lint",
      cwd: "web",
      body: "bun run lint",
      tags: ["check", "lint"],
      needs: ["web:deps"],
    }),
    task({
      name: "web:type-check",
      cwd: "web",
      body: "bun run check",
      tags: ["check"],
      needs: ["web:deps", "backend:bindings"],
    }),
    task({
      name: "web:test",
      cwd: "web",
      body: "bun run test",
      tags: ["check", "test"],
      needs: ["web:deps", "backend:bindings"],
    }),
    task({
      name: "web:build",
      cwd: "web",
      body: "bun run build",
      tags: ["build"],
      needs: ["web:deps", "backend:bindings"],
    }),

    ...presets.rust({
      name: "backend",
      allFeatures: true,
      bin: "banner",
      override: {
        format: "cargo fmt --all -- --check",
        "format-fix": "cargo fmt --all",
        lint: "cargo clippy --all-features --all-targets -- -D warnings",
        test: "cargo nextest run -E 'not test(export_bindings)'",
      },
    }),
    task({
      name: "backend:type-check",
      body: "cargo check --all-features",
      tags: ["check"],
      // Ordering only: .sqlx is checked in, so a missing sqlx-cli must not
      // block the compile that reads it.
      after: ["backend:sqlx"],
    }),

    task({
      name: "security:cargo-audit",
      body: "cargo audit",
      tags: ["check"],
      requires: [{ tool: "cargo-audit", hint: "cargo install cargo-audit" }],
    }),
    task({
      name: "security:bun-audit",
      cwd: "web",
      body: [
        "bun", "audit", "--audit-level=moderate",
        ...IGNORED_ADVISORIES.map(({ id }) => `--ignore=${id}`),
      ],
      tags: ["check"],
      needs: ["web:deps"],
    }),
    task({
      name: "security:actionlint",
      body: "actionlint",
      tags: ["check"],
      requires: [{ tool: "actionlint" }],
    }),

    task({
      name: "web:dev",
      cwd: "web",
      body: ["bun", "run", "dev"],
      tags: ["dev"],
      persistent: true,
      needs: ["web:deps"],
    }),
    // Rust proxies non-API requests to the Vite dev server.
    task({
      name: "backend:dev",
      body: ["./target/debug/banner", "--tracing", "pretty"],
      tags: ["dev"],
      persistent: true,
      passthrough: true,
      env: { SSR_DOWNSTREAM: "http://localhost:3001" },
      needs: ["backend:dev-build"],
      // interrupt: cargo rewrites target/debug/banner while it is executing.
      watch: {
        paths: ["src", "migrations", ".sqlx", ".cargo", "Cargo.toml", "Cargo.lock"],
        exts: [".rs", ".sql", ".json", ".toml"],
        debounce: 200,
        interrupt: true,
      },
    }),
    task({
      name: "backend:dev-build",
      body: ["cargo", "build", "--bin", "banner", "--no-default-features"],
    }),

    task({
      name: "backend:search",
      body: ["cargo", "run", "-q", "--bin", "search", "--"],
      passthrough: true,
    }),

    task({
      name: "db",
      passthrough: true,
      body: async (ctx) => {
        const sub = ctx.args[0] ?? "start";
        if (sub === "rm") {
          const code = await ctx.run(["docker", "compose", "down"]);
          ctx.log("removed");
          return code;
        }
        if (sub === "start") {
          const code = await ctx.run(["docker", "compose", "up", "-d"]);
          await updateEnv(ctx.cwd);
          ctx.log("started");
          return code;
        }
        if (sub !== "reset") ctx.fail(`unknown db command "${sub}" (start, reset, rm)`);

        // Deliberately non-automatable: a human has to type the token.
        if (!process.stdin.isTTY) {
          ctx.fail("db reset requires an interactive terminal");
        }
        const token = randomToken();
        process.stderr.write(
          "\nDATABASE RESET -- permanently drops and recreates the banner database.\n" +
            "All scraped data, user accounts, and sessions will be lost. No undo.\n\n" +
            `Confirm by typing: ${token}\n`,
        );
        if ((await readLine("> ")).trim() !== token) {
          ctx.fail("aborted -- database was not modified");
        }

        await ctx.run(["docker", "compose", "up", "-d"]);
        await new Promise((r) => setTimeout(r, 2000));
        const psql = (sql: string) =>
          ctx.run([
            "docker", "compose", "exec", "postgres",
            "psql", "-U", "banner", "-d", "postgres", "-c", sql,
          ]);
        await psql("DROP DATABASE IF EXISTS banner");
        await psql("CREATE DATABASE banner");
        await updateEnv(ctx.cwd);
        ctx.log("reset");
        return 0;
      },
    }),

    task({
      name: "sync-secrets",
      body: async (ctx) => {
        if (!DEPLOY_HOST) ctx.fail("DEPLOY_HOST not set -- add it to .env");
        return syncSecrets(ctx);
      },
    }),

    task({
      name: "deploy",
      body: async (ctx) => {
        if (!DEPLOY_HOST) ctx.fail("DEPLOY_HOST not set -- add it to .env");
        if (!PUBLIC_POSTHOG_HOST) {
          ctx.fail("PUBLIC_POSTHOG_HOST not set -- baked into the CSP at build time");
        }

        // Deliberately does not sync secrets. .env is a development environment -- its
        // Discord token, client id and guild belong to a separate test application -- so
        // pushing it at deploy time silently swaps production onto the wrong bot. The
        // cluster Secret is the source of truth; `sync-secrets` pushes local values only
        // when asked for explicitly.

        await ctx.run([
          "rsync", "-az", "--delete",
          "--exclude", ".git", "--exclude", "target",
          "--exclude", "node_modules", "--exclude", ".svelte-kit",
          "--exclude", "build", "--exclude", "sim", "--exclude", "*.dump",
          "./", `${DEPLOY_HOST}:${DEPLOY_PATH}/`,
        ]);

        // The default buildx driver builds inside the local daemon, so every
        // uniquely-tagged build stays in the image store forever with no GC.
        await ssh(
          ctx,
          `docker buildx inspect ci >/dev/null 2>&1 || docker buildx create --name ci --driver docker-container --config ${DEPLOY_PATH}/deploy/buildkitd.toml --bootstrap`,
        );
        await ssh(ctx, "docker buildx use ci");

        const sha = (await ctx.capture(["git", "rev-parse", "HEAD"])).stdout.trim();
        const short = (await ctx.capture(["git", "rev-parse", "--short=12", "HEAD"])).stdout.trim();
        const dirty = (await ctx.capture(["git", "status", "--porcelain"])).stdout.trim() !== "";
        // A changing tag is what makes the pod spec change, which is what
        // triggers the rollout; a dirty tree still gets a distinct tag.
        const tag = dirty ? `${short}-dirty.${Math.floor(Date.now() / 1000)}` : short;
        ctx.log(`deploying ${DEPLOY_IMAGE}:${tag}`);

        // The host is aarch64, so --platform keeps a developer on x86 from
        // pushing an amd64 image the node cannot run.
        await ssh(
          ctx,
          `cd ${DEPLOY_PATH} && docker buildx build --platform linux/arm64 --build-arg PUBLIC_POSTHOG_HOST=${PUBLIC_POSTHOG_HOST} --build-arg GIT_COMMIT_SHA=${sha} -t ${DEPLOY_IMAGE}:${tag} --push .`,
        );
        await ssh(
          ctx,
          `cd ${DEPLOY_PATH} && sudo -E env KUBECONFIG=/etc/rancher/k3s/k3s.yaml helm upgrade --install ${DEPLOY_RELEASE} ./charts/banner --set image.tag=${tag}`,
        );
        // Migrations run at boot behind a startupProbe allowing 5 minutes, so
        // the rollout deadline has to clear that.
        await ssh(ctx, `sudo k3s kubectl rollout status deployment/${DEPLOY_RELEASE} --timeout=360s`);

        return reportPublicOrigin(ctx);
      },
    }),
  ],

  commands: {
    check: { description: "Run every check", tags: ["check"] },
    fmt: { description: "Apply every formatter", tags: ["format"], concurrency: 1 },
    lint: { description: "Lint every subsystem", tags: ["lint"] },
    test: { description: "Run every test suite", tags: ["test"] },
    build: { description: "Production build", tags: ["build"] },
    dev: {
      description: "Dev servers with watch and reload",
      tags: ["dev"],
      exitBehavior: "first-exits",
    },
    bindings: {
      description: "Regenerate TypeScript bindings from Rust types",
      tasks: ["backend:bindings"],
    },
    search: {
      description: "Run the Banner API search demo against the live UTSA API",
      tasks: ["backend:search"],
      passthrough: true,
    },
    db: { description: "PostgreSQL compose management", tasks: ["db"], passthrough: true },
    "sync-secrets": {
      description: "Push allowlisted .env (development) secrets into the cluster Secret",
      tasks: ["sync-secrets"],
    },
    deploy: {
      description: "Sync, build+push the ARM64 image, and roll out k3s",
      tasks: ["deploy"],
    },
  },
});
