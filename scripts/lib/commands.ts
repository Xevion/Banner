/**
 * Centralized command registry for Banner project scripts.
 *
 * Maps (subsystem, action) tuples to command definitions, eliminating
 * duplication across check.ts, format.ts, lint.ts, pre-commit.ts, and test.ts.
 */

import type { Subsystem } from './targets';

export type Action = 'format-check' | 'format-apply' | 'lint' | 'type-check' | 'test' | 'build';

export interface CommandDef {
	/** Command array [program, ...args] */
	cmd: string[];
	/** Working directory relative to project root */
	cwd?: string;
	/** Human-readable description */
	description: string;
}

const REGISTRY: Record<Subsystem, Partial<Record<Action, CommandDef>>> = {
	frontend: {
		'format-check': {
			cmd: ['bun', 'run', '--cwd', 'web', 'format:check'],
			description: 'Biome format check',
		},
		'format-apply': {
			cmd: ['bun', 'run', '--cwd', 'web', 'format'],
			description: 'Biome format',
		},
		lint: {
			cmd: ['bun', 'run', '--cwd', 'web', 'lint'],
			description: 'Biome lint',
		},
		'type-check': {
			cmd: ['bun', 'run', '--cwd', 'web', 'check'],
			description: 'SvelteKit type check',
		},
		test: {
			cmd: ['bun', 'run', '--cwd', 'web', 'test'],
			description: 'Vitest unit tests',
		},
		build: {
			cmd: ['bun', 'run', '--cwd', 'web', 'build'],
			description: 'Vite production build',
		},
	},
	backend: {
		'format-check': {
			cmd: ['cargo', 'fmt', '--all', '--', '--check'],
			description: 'cargo fmt check',
		},
		'format-apply': {
			cmd: ['cargo', 'fmt', '--all'],
			description: 'cargo fmt',
		},
	lint: {
		cmd: ['cargo', 'clippy', '--all-features', '--all-targets', '--', '-D', 'warnings'],
		description: 'Clippy with warnings denied',
	},
		'type-check': {
			cmd: ['cargo', 'check', '--all-features'],
			description: 'cargo check',
		},
		test: {
			cmd: ['cargo', 'nextest', 'run'],
			description: 'cargo nextest',
		},
	},
};

/**
 * Look up a command definition by subsystem and action.
 *
 * @throws Error if the (subsystem, action) combination doesn't exist
 */
export function getCommand(subsystem: Subsystem, action: Action): CommandDef {
	const def = REGISTRY[subsystem]?.[action];
	if (!def) throw new Error(`No command registered for (${subsystem}, ${action})`);
	return def;
}
