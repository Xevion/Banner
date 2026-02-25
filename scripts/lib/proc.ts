/**
 * Shared process spawning utilities for Banner project scripts.
 *
 * Provides utilities for synchronous/asynchronous process execution,
 * coordinated process groups, and parallel command execution with
 * result ordering.
 *
 * All spawn functions set CI=1 to prevent interactive prompts in tools
 * like vitest, npm, etc.
 */

import { elapsed } from './fmt';

/** Base environment for all spawned processes - prevents interactive prompts */
const baseEnv = { ...process.env, CI: '1' };

/**
 * Result from collecting process output asynchronously.
 */
export interface CollectResult {
	/** Standard output captured from the process */
	stdout: string;
	/** Standard error captured from the process */
	stderr: string;
	/** Exit code (0 = success, non-zero = failure) */
	exitCode: number;
	/** Formatted elapsed time (e.g., "1.2s") */
	elapsed: string;
}

/**
 * Spawn a command synchronously with inherited stdio.
 *
 * The process will inherit stdout and stderr, preserving color output.
 * Stdin is ignored to prevent interactive prompts from blocking.
 * Exits the parent process if the command fails.
 *
 * @param cmd - Command array [program, ...args]
 * @throws Exits process with child's exit code on failure
 *
 * @example
 * run(["cargo", "build", "--release"]);
 * run(["bun", "run", "test"]);
 */
export function run(cmd: string[], options?: { cwd?: string; env?: Record<string, string> }): void {
	const proc = Bun.spawnSync(cmd, {
		stdio: ['ignore', 'inherit', 'inherit'],
		env: options?.env ? { ...baseEnv, ...options.env } : baseEnv,
		cwd: options?.cwd,
	});
	if (proc.exitCode !== 0) process.exit(proc.exitCode);
}

/**
 * Spawn a command synchronously with captured output.
 *
 * Unlike `run()`, this captures stdout/stderr instead of inheriting
 * stdio. Useful for parsing command output or checking for specific
 * text in the output.
 *
 * @param cmd - Command array [program, ...args]
 * @returns Object with exitCode, stdout, and stderr
 *
 * @example
 * const { exitCode, stdout } = runPiped(["git", "status", "--short"]);
 * if (exitCode === 0) {
 *   const modified = stdout.split("\n").filter(l => l.startsWith(" M"));
 * }
 */
export function runPiped(
	cmd: string[],
	options?: { cwd?: string; env?: Record<string, string> },
): { exitCode: number; stdout: string; stderr: string } {
	const proc = Bun.spawnSync(cmd, {
		stdout: 'pipe',
		stderr: 'pipe',
		env: options?.env ? { ...baseEnv, ...options.env } : baseEnv,
		cwd: options?.cwd,
	});
	return {
		exitCode: proc.exitCode,
		stdout: proc.stdout?.toString() ?? '',
		stderr: proc.stderr?.toString() ?? '',
	};
}

/**
 * Spawn a command asynchronously and collect output.
 *
 * Enables FORCE_COLOR=1 to preserve colored output in piped processes.
 * Sets CI=1 to prevent interactive prompts in tools like vitest.
 * Catches spawn failures (e.g., command not found) and returns them as
 * CollectResult instead of throwing.
 *
 * @param cmd - Command array [program, ...args]
 * @param startTime - Timestamp from Date.now() for elapsed calculation
 * @returns Promise resolving to collected output and exit code
 *
 * @example
 * const start = Date.now();
 * const result = await spawnCollect(["cargo", "clippy"], start);
 * if (result.exitCode !== 0) {
 *   console.error(`clippy failed in ${result.elapsed}s`);
 * }
 */
export async function spawnCollect(
	cmd: string[],
	startTime: number,
	options?: { cwd?: string },
): Promise<CollectResult> {
	try {
		const proc = Bun.spawn(cmd, {
			env: { ...baseEnv, FORCE_COLOR: '1' },
			stdout: 'pipe',
			stderr: 'pipe',
			cwd: options?.cwd,
		});
		const [stdout, stderr] = await Promise.all([
			new Response(proc.stdout).text(),
			new Response(proc.stderr).text(),
		]);
		await proc.exited;
		return {
			stdout,
			stderr,
			exitCode: proc.exitCode ?? 1,
			elapsed: elapsed(startTime),
		};
	} catch (err) {
		return {
			stdout: '',
			stderr: String(err),
			exitCode: 1,
			elapsed: elapsed(startTime),
		};
	}
}

/**
 * Execute promises in parallel, yielding results in completion order.
 *
 * Unlike Promise.all(), this calls the callback as each promise completes,
 * allowing for progress reporting. Spawn failures are caught and converted
 * to error results using the fallback metadata.
 *
 * @param promises - Array of promises to execute
 * @param fallbacks - Metadata for each promise (used on failure)
 * @param onResult - Callback invoked as each result completes
 *
 * @example
 * const checks = [
 *   spawnCollect(["bun", "run", "check"], Date.now()),
 *   spawnCollect(["cargo", "clippy"], Date.now()),
 * ];
 * await raceInOrder(
 *   checks,
 *   [{ name: "frontend" }, { name: "backend" }],
 *   (result) => console.log(`${result.name}: ${result.exitCode}`)
 * );
 */
export async function raceInOrder<T extends { name: string }>(
	promises: Promise<T & CollectResult>[],
	fallbacks: T[],
	onResult: (r: T & CollectResult) => void,
): Promise<void> {
	const tagged = promises.map((p, i) =>
		p
			.then((r) => ({ i, r }))
			.catch((err) => ({
				i,
				r: {
					...fallbacks[i],
					exitCode: 1,
					stdout: '',
					stderr: String(err),
					elapsed: '?',
				} as T & CollectResult,
			})),
	);

	for (let n = 0; n < promises.length; n++) {
		const { i, r } = await Promise.race(tagged);
		tagged[i] = new Promise(() => {}); // sentinel: never resolves
		onResult(r);
	}
}

/**
 * Managed process group with coordinated lifecycle and cleanup.
 *
 * Spawns multiple processes and ensures they are all killed when:
 * - Any process exits (via waitForFirst / waitForAll)
 * - Parent receives SIGINT/SIGTERM
 * - Explicit killAll() is called
 *
 * @example
 * const group = new ProcessGroup();
 * group.spawn(["bun", "run", "--cwd", "web", "dev"]);
 * group.spawn(["bacon", "--headless", "run"]);
 * const exitCode = await group.waitForFirst();
 * process.exit(exitCode);
 */
export class ProcessGroup {
	private procs: ReturnType<typeof Bun.spawn>[] = [];
	private signalHandlers: { signal: NodeJS.Signals; handler: () => void }[] = [];
	private cleanupFns: (() => void)[] = [];
	private asyncCleanupFns: (() => Promise<void>)[] = [];

	private shuttingDown = false;

	constructor() {
		const cleanup = () => {
			if (this.shuttingDown) return;
			this.shuttingDown = true;
			for (const p of this.procs) {
				try {
					p.kill('SIGTERM');
				} catch {
					// Process may already be dead
				}
			}
			for (const fn of this.cleanupFns) {
				try {
					fn();
				} catch {
					// Best-effort cleanup
				}
			}
			// Don't process.exit() here -- let waitForFirst/waitForAll resolve
			// naturally after children exit. Exiting immediately causes children
			// to keep writing to the terminal after the shell prompt returns.
		};
		for (const sig of ['SIGINT', 'SIGTERM'] as const) {
			process.on(sig, cleanup);
			this.signalHandlers.push({ signal: sig, handler: cleanup });
		}
	}

	/**
	 * Register a synchronous cleanup function called on signal or killAll.
	 */
	onCleanup(fn: () => void): void {
		this.cleanupFns.push(fn);
	}

	/**
	 * Register an async cleanup function awaited during killAll.
	 *
	 * Use for cleanup that must complete before the process exits,
	 * e.g. waiting for a graceful server shutdown.
	 */
	onAsyncCleanup(fn: () => Promise<void>): void {
		this.asyncCleanupFns.push(fn);
	}

	private removeSignalHandlers(): void {
		for (const { signal, handler } of this.signalHandlers) {
			process.off(signal, handler);
		}
		this.signalHandlers = [];
	}

	/**
	 * Reset terminal to a sane state after killing child processes.
	 *
	 * Killed children may leave the terminal in raw mode, with a hidden cursor,
	 * inside an alternate screen buffer, or mid-escape sequence.
	 */
	static resetTerminal(): void {
		try {
			process.stdout.write('\x1b[0m\x1b[?25h\x1b[?1049l');
		} catch {
			// stdout may already be closed
		}
		try {
			Bun.spawnSync(['stty', 'sane'], { stdio: ['inherit', 'ignore', 'ignore'] });
		} catch {
			// stty may not be available
		}
	}

	/**
	 * Spawn a new process in the group.
	 *
	 * By default, stdin is set to "ignore" to prevent child processes from
	 * blocking on interactive prompts or causing I/O errors on parent exit.
	 */
	spawn(
		cmd: string[],
		options?: { env?: Record<string, string>; cwd?: string; inheritStdin?: boolean },
	): ReturnType<typeof Bun.spawn> {
		const proc = Bun.spawn(cmd, {
			stdio: [options?.inheritStdin ? 'inherit' : 'ignore', 'inherit', 'inherit'],
			env: { ...baseEnv, ...options?.env },
			cwd: options?.cwd,
		});
		this.procs.push(proc);
		return proc;
	}

	/**
	 * Kill all processes in the group and wait for them to exit.
	 */
	async killAll(): Promise<void> {
		// Run sync cleanup fns if signal handler hasn't already
		if (!this.shuttingDown) {
			for (const fn of this.cleanupFns) {
				try {
					fn();
				} catch {
					// Best-effort cleanup
				}
			}
		}

		for (const p of this.procs) {
			try {
				p.kill('SIGTERM');
			} catch {
				// Process may already be dead
			}
		}

		const timeout = 5000;
		const timeoutPromise = new Promise<void>((resolve) => setTimeout(resolve, timeout));

		// Wait for group procs and async cleanup (e.g. watcher shutdown) together
		const exitPromises = this.procs.map((p) => p.exited);
		const asyncCleanup = this.asyncCleanupFns.map((fn) =>
			fn().catch(() => {
				/* best-effort */
			}),
		);
		await Promise.race([Promise.all([...exitPromises, ...asyncCleanup]), timeoutPromise]);

		for (const p of this.procs) {
			try {
				p.kill('SIGKILL');
			} catch {
				// Process may already be dead
			}
		}

		this.removeSignalHandlers();
		ProcessGroup.resetTerminal();
	}

	/**
	 * Wait for any process to exit, then kill the rest.
	 *
	 * Returns the exit code of the first process to exit. Useful for
	 * development servers where any subsystem crash should stop all others.
	 *
	 * @returns Exit code of the first process to exit
	 */
	async waitForFirst(): Promise<number> {
		const results = this.procs.map((p, i) => p.exited.then((code) => ({ i, code })));
		const first = await Promise.race(results);
		await this.killAll();
		return first.code;
	}

	/**
	 * Wait for all processes to complete.
	 *
	 * Returns the highest exit code (0 if all succeeded).
	 *
	 * @returns Highest exit code from all processes (0 = all passed)
	 */
	async waitForAll(): Promise<number> {
		const codes = await Promise.all(this.procs.map((p) => p.exited));
		this.removeSignalHandlers();
		return Math.max(0, ...codes);
	}
}
