/**
 * Seamless backend reload for Banner development.
 *
 * Watches Rust source files and recompiles in the background while the
 * existing server keeps running. Only swaps to the new binary after a
 * successful build. On build failure, the old server stays up.
 *
 * State machine:
 *   BUILDING (initial)         → success → RUNNING, failure → IDLE
 *   IDLE                       → file change → BUILDING
 *   RUNNING                    → file change → BUILDING_WITH_SERVER, crash → IDLE
 *   BUILDING_WITH_SERVER       → success → swap → RUNNING, failure → RUNNING (keep old)
 *   SWAPPING                   → SIGTERM → wait → SIGKILL → start → RUNNING
 */

import { watch, type FSWatcher } from "fs";
import { c, elapsed } from "./fmt";

type State =
  | "building"
  | "idle"
  | "running"
  | "building_with_server"
  | "swapping";

export interface BackendWatcherOptions {
  /** Path to compiled binary */
  binPath: string;
  /** Use release profile */
  release: boolean;
  /** Extra cargo build arguments (e.g. --no-default-features) */
  cargoExtra: string[];
  /** Arguments to pass to the server binary */
  args: string[];
  /** Kill the compiler on new changes (true) or wait for it to finish (false) */
  interrupt: boolean;
  /** Stream compilation output inline instead of buffering */
  verboseBuild: boolean;
}

export class BackendWatcher {
  private state: State = "building";
  private serverProc: ReturnType<typeof Bun.spawn> | null = null;
  private buildProc: ReturnType<typeof Bun.spawn> | null = null;
  private buildInterrupted = false;
  private dirty = false;
  private debounceTimer: ReturnType<typeof setTimeout> | null = null;
  private watchers: FSWatcher[] = [];
  private shutdownRequested = false;

  constructor(private opts: BackendWatcherOptions) {}

  /** Begin watching and trigger the initial build. Fire-and-forget. */
  start(): void {
    this.setupWatchers();
    this.triggerBuild();
  }

  /**
   * Synchronous cleanup — kills all child processes immediately.
   * Suitable for signal handlers where async work isn't possible.
   */
  killSync(): void {
    this.shutdownRequested = true;
    if (this.debounceTimer) clearTimeout(this.debounceTimer);
    for (const w of this.watchers) w.close();
    this.watchers = [];
    if (this.buildProc) {
      try {
        this.buildProc.kill("SIGTERM");
      } catch {
        /* already dead */
      }
    }
    if (this.serverProc) {
      try {
        this.serverProc.kill("SIGTERM");
      } catch {
        /* already dead */
      }
    }
  }

  /**
   * Graceful async shutdown — SIGTERM with timeout, then SIGKILL.
   * Closes watchers, kills build, and drains the running server.
   */
  async shutdown(): Promise<void> {
    this.shutdownRequested = true;
    if (this.debounceTimer) {
      clearTimeout(this.debounceTimer);
      this.debounceTimer = null;
    }
    for (const w of this.watchers) w.close();
    this.watchers = [];

    // Kill active build
    if (this.buildProc) {
      try {
        this.buildProc.kill("SIGTERM");
      } catch {
        /* already dead */
      }
      await this.buildProc.exited;
      this.buildProc = null;
    }

    // Graceful server shutdown
    if (this.serverProc) {
      try {
        this.serverProc.kill("SIGTERM");
      } catch {
        /* already dead */
      }
      const exited = await Promise.race([
        this.serverProc.exited.then(() => true as const),
        new Promise<false>((r) => setTimeout(() => r(false), 3000)),
      ]);
      if (!exited) {
        try {
          this.serverProc.kill("SIGKILL");
        } catch {
          /* already dead */
        }
        await this.serverProc.exited;
      }
      this.serverProc = null;
    }
  }

  private setupWatchers(): void {
    // Watch src/ recursively for .rs files
    const srcWatcher = watch(
      "src",
      { recursive: true },
      (_event, filename) => {
        if (filename && filename.toString().endsWith(".rs")) {
          this.onFileChange();
        }
      },
    );
    this.watchers.push(srcWatcher);

    // Watch project root (non-recursive) for Cargo.toml / Cargo.lock
    const manifestWatcher = watch(
      ".",
      { recursive: false },
      (_event, filename) => {
        const name = filename?.toString();
        if (name === "Cargo.toml" || name === "Cargo.lock") {
          this.onFileChange();
        }
      },
    );
    this.watchers.push(manifestWatcher);

    // Watch .sqlx/ cache — offline query validation at compile time
    this.tryWatch(".sqlx", { recursive: true }, (_event, filename) => {
      if (filename && filename.toString().endsWith(".json")) {
        this.onFileChange();
      }
    });

    // Watch migrations — schema changes invalidate the .sqlx cache
    this.tryWatch(
      "migrations",
      { recursive: true },
      (_event, filename) => {
        if (filename && filename.toString().endsWith(".sql")) {
          this.onFileChange();
        }
      },
    );

    // Watch .cargo/config.toml — env vars and build configuration
    this.tryWatch(
      ".cargo",
      { recursive: false },
      (_event, filename) => {
        if (filename?.toString() === "config.toml") {
          this.onFileChange();
        }
      },
    );
  }

  /** Watch a path if it exists — some directories (e.g., .sqlx) may not exist yet. */
  private tryWatch(
    path: string,
    options: { recursive: boolean },
    cb: (event: string, filename: string | Buffer | null) => void,
  ): void {
    try {
      this.watchers.push(watch(path, options, cb));
    } catch {
      // Directory doesn't exist — skip silently
    }
  }

  private onFileChange(): void {
    if (this.shutdownRequested) return;

    if (this.debounceTimer) clearTimeout(this.debounceTimer);
    this.debounceTimer = setTimeout(() => {
      this.debounceTimer = null;
      this.handleChange();
    }, 200);
  }

  private handleChange(): void {
    if (this.shutdownRequested) return;

    switch (this.state) {
      case "idle":
        this.triggerBuild();
        break;

      case "running":
        this.state = "building_with_server";
        this.triggerBuild();
        break;

      case "building":
      case "building_with_server":
        if (this.opts.interrupt && this.buildProc) {
          console.log(
            c("1;33", "→ Change detected, restarting compilation..."),
          );
          this.buildInterrupted = true;
          try {
            this.buildProc.kill("SIGTERM");
          } catch {
            /* already dead */
          }
        } else {
          console.log(
            c(
              "1;33",
              "→ Change detected, will rebuild after current compilation",
            ),
          );
          this.dirty = true;
        }
        break;

      case "swapping":
        // Mid-swap — rebuild after swap completes
        this.dirty = true;
        break;
    }
  }

  private async triggerBuild(): Promise<void> {
    if (this.shutdownRequested) return;

    const hadServer =
      this.state === "building_with_server" || this.state === "running";
    this.state = hadServer ? "building_with_server" : "building";
    this.dirty = false;

    console.log(c("1;36", "→ Compiling backend..."));
    const startTime = Date.now();

    const cargoArgs = ["cargo", "build", "--bin", "banner", ...this.opts.cargoExtra];
    if (this.opts.release) cargoArgs.push("--release");

    const piped = !this.opts.verboseBuild;
    const buildProc = Bun.spawn(cargoArgs, {
      stdout: piped ? "pipe" : "inherit",
      stderr: piped ? "pipe" : "inherit",
      env: { ...process.env, CI: "1" },
    });
    this.buildProc = buildProc;

    // Consume pipes to prevent blocking (even if we don't display stdout)
    let stderr = "";
    if (piped) {
      const [, stderrText] = await Promise.all([
        buildProc.stdout ? new Response(buildProc.stdout).text() : "",
        buildProc.stderr ? new Response(buildProc.stderr).text() : "",
      ]);
      stderr = stderrText;
    }

    const exitCode = await buildProc.exited;
    this.buildProc = null;

    if (this.shutdownRequested) return;

    // Interrupted by a newer change — restart immediately
    if (this.buildInterrupted) {
      this.buildInterrupted = false;
      this.triggerBuild();
      return;
    }

    if (exitCode === 0) {
      console.log(c("1;32", `→ Backend compiled (${elapsed(startTime)})`));

      if (this.dirty) {
        this.triggerBuild();
        return;
      }

      if (this.state === "building_with_server") {
        await this.swapServer();
      } else {
        await this.startServer();
      }
    } else {
      console.log(c("1;31", `→ Build failed (${elapsed(startTime)}):`));
      if (piped && stderr) {
        process.stderr.write(stderr);
      }

      if (this.state === "building_with_server") {
        console.log(c("1;33", "→ Keeping previous server running"));
        this.state = "running";
      } else {
        console.log(c("1;33", "→ Waiting for changes..."));
        this.state = "idle";
      }

      if (this.dirty) {
        this.triggerBuild();
      }
    }
  }

  private async startServer(): Promise<void> {
    if (this.shutdownRequested) return;

    const proc = Bun.spawn([this.opts.binPath, ...this.opts.args], {
      stdio: ["ignore", "inherit", "inherit"],
      env: { ...process.env },
    });
    this.serverProc = proc;
    this.state = "running";
    console.log(c("1;32", `→ Backend running (pid ${proc.pid})`));

    // Monitor for unexpected exit (crash)
    proc.exited.then((code) => {
      if (this.serverProc !== proc) return; // stale reference
      this.serverProc = null;
      if (this.shutdownRequested) return;

      console.log(c("1;31", `→ Backend exited (code ${code})`));
      if (this.state === "building_with_server") {
        this.state = "building";
        console.log(
          c("1;33", "→ Build in progress, will start server on completion"),
        );
      } else {
        this.state = "idle";
        console.log(c("1;33", "→ Waiting for changes..."));
      }
    });
  }

  private async swapServer(): Promise<void> {
    if (this.shutdownRequested) return;

    this.state = "swapping";

    if (this.serverProc) {
      console.log(c("1;36", "→ Restarting backend..."));
      const oldProc = this.serverProc;
      this.serverProc = null;

      try {
        oldProc.kill("SIGTERM");
      } catch {
        /* already dead */
      }

      const exited = await Promise.race([
        oldProc.exited.then(() => true as const),
        new Promise<false>((r) => setTimeout(() => r(false), 3000)),
      ]);

      if (!exited) {
        try {
          oldProc.kill("SIGKILL");
        } catch {
          /* already dead */
        }
        await oldProc.exited;
      }
    }

    if (this.shutdownRequested) return;

    await this.startServer();

    if (this.dirty) {
      this.handleChange();
    }
  }
}
