/**
 * Shared formatting, color, and CLI argument parsing utilities.
 *
 * Provides ANSI color output (TTY-aware), elapsed time formatting,
 * and flexible CLI flag parsing for Banner scripts.
 */

const isTTY = process.stdout.isTTY ?? false;

/** Whether stderr is a TTY (useful for progress spinners and status output) */
export const isStderrTTY = process.stderr.isTTY ?? false;

/**
 * ANSI color wrapper - automatically disables colors when not in TTY.
 *
 * Common color codes:
 * - "31" = red
 * - "32" = green
 * - "33" = yellow
 * - "34" = blue
 * - "36" = cyan
 * - "1;32" = bold green
 * - "2" = dim
 *
 * @param code - ANSI color code (e.g., "32" for green, "1;31" for bold red)
 * @param text - Text to colorize
 * @returns Colored text if stdout is TTY, plain text otherwise
 *
 * @example
 * console.log(c("32", "(ok) Tests passed"));
 * console.error(c("31", "(x) Build failed"));
 */
export function c(code: string, text: string): string {
	return isTTY ? `\x1b[${code}m${text}\x1b[0m` : text;
}

/**
 * Format elapsed time since a start timestamp.
 *
 * @param start - Timestamp from Date.now()
 * @returns Formatted elapsed seconds (e.g., "1.2", "0.5")
 *
 * @example
 * const start = Date.now();
 * await runTests();
 * console.log(`Tests completed in ${elapsed(start)}s`);
 */
export function elapsed(start: number): string {
	return ((Date.now() - start) / 1000).toFixed(1);
}

/**
 * Parse CLI flags from argument array with support for short/long flags.
 *
 * Supports:
 * - Short flags: `-f`, `-b` (can be combined as `-fb`)
 * - Long flags: `--frontend`, `--backend`
 * - String values: `--tracing pretty`, `-p fabric`
 * - Passthrough args: `-- extra args here`
 *
 * Short flags can be combined: `-fbW` expands to `-f -b -W`.
 * The `--` separator terminates flag parsing; remaining args go to passthrough.
 *
 * @param argv - Arguments to parse (typically process.argv.slice(2))
 * @param spec - Flag specification: maps flag names to "bool" or "string"
 * @param shortMap - Maps single characters to flag names (e.g., { f: "frontend" })
 * @param defaults - Default values for all flags
 * @returns Object with parsed flags and passthrough args
 *
 * @example
 * // Parse: scripts/dev.ts -fb --tracing pretty -- extra args
 * const { flags, passthrough } = parseFlags(
 *   process.argv.slice(2),
 *   { frontend: "bool", backend: "bool", tracing: "string" },
 *   { f: "frontend", b: "backend" },
 *   { frontend: false, backend: false, tracing: "pretty" }
 * );
 * // flags = { frontend: true, backend: true, tracing: "pretty" }
 * // passthrough = ["extra", "args"]
 */
export function parseFlags<T extends Record<string, 'bool' | 'string'>>(
	argv: string[],
	spec: T,
	shortMap: Record<string, keyof T>,
	defaults: { [K in keyof T]: T[K] extends 'bool' ? boolean : string },
): { flags: typeof defaults; passthrough: string[] } {
	const flags = { ...defaults };
	const passthrough: string[] = [];
	let i = 0;

	while (i < argv.length) {
		const arg = argv[i];

		// Handle '--' separator
		if (arg === '--') {
			passthrough.push(...argv.slice(i + 1));
			break;
		}

		// Handle long flags (--flag or --flag value)
		if (arg.startsWith('--')) {
			const name = arg.slice(2);
			if (!(name in spec)) {
				console.error(`Unknown flag: ${arg}`);
				process.exit(1);
			}
			if (spec[name] === 'string') {
				i++;
				if (i >= argv.length || argv[i].startsWith('-')) {
					console.error(`Flag ${arg} requires a value`);
					process.exit(1);
				}
				(flags as Record<string, unknown>)[name] = argv[i];
			} else {
				(flags as Record<string, unknown>)[name] = true;
			}
		}
		// Handle short flags (-f or -fb or -p value)
		else if (arg.startsWith('-') && arg.length > 1) {
			const chars = arg.slice(1);
			for (let j = 0; j < chars.length; j++) {
				const ch = chars[j];
				const mapped = shortMap[ch];
				if (!mapped) {
					console.error(`Unknown flag: -${ch}`);
					process.exit(1);
				}
				if (spec[mapped as string] === 'string') {
					i++;
					if (i >= argv.length || argv[i].startsWith('-')) {
						console.error(`Flag -${ch} requires a value`);
						process.exit(1);
					}
					(flags as Record<string, unknown>)[mapped as string] = argv[i];
				} else {
					(flags as Record<string, unknown>)[mapped as string] = true;
				}
			}
		}
		// Non-flag argument (positional)
		else {
			passthrough.push(arg);
		}

		i++;
	}

	return { flags, passthrough };
}

/**
 * Parse a string into an array of space-separated arguments.
 *
 * @param raw - String to parse
 * @returns Array of trimmed, non-empty arguments
 *
 * @example
 * parseArgs("cargo build --release")
 * // ["cargo", "build", "--release"]
 */
export function parseArgs(raw: string): string[] {
	return raw.trim().split(/\s+/).filter(Boolean);
}
