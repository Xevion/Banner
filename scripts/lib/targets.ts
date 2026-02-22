/**
 * Shared target resolution for Banner project scripts.
 *
 * All core commands (check, test, lint, format) accept optional targets
 * to scope work to specific subsystems. Targets are comma-delimited
 * and resolved through an alias table.
 *
 * Usage: `just check backend,frontend` or `just check b,f`
 */

export type Subsystem = 'frontend' | 'backend';

const ALIASES: Record<string, Subsystem> = {
	frontend: 'frontend',
	front: 'frontend',
	web: 'frontend',
	f: 'frontend',

	backend: 'backend',
	back: 'backend',
	rust: 'backend',
	b: 'backend',
};

const ALL_SUBSYSTEMS: Subsystem[] = ['frontend', 'backend'];

export interface ResolvedTargets {
	subsystems: Set<Subsystem>;
}

/**
 * Resolve CLI arguments into a set of subsystems.
 *
 * Accepts comma-delimited target names that are resolved through an
 * alias table. Empty input selects all subsystems.
 *
 * @param argv - Arguments from process.argv (after flags are stripped)
 * @returns Resolved subsystems
 *
 * @example
 * resolveTargets([])            // { subsystems: all }
 * resolveTargets(["backend"])   // { subsystems: {backend} }
 * resolveTargets(["web,rust"])  // { subsystems: {frontend, backend} }
 * resolveTargets(["b", "f"])    // { subsystems: {backend, frontend} }
 */
export function resolveTargets(argv: string[]): ResolvedTargets {
	if (argv.length === 0) {
		return { subsystems: new Set(ALL_SUBSYSTEMS) };
	}

	const tokens = argv
		.flatMap((arg) => arg.split(',').map((s) => s.trim().toLowerCase()))
		.filter(Boolean);

	const subsystems = new Set<Subsystem>();

	for (const token of tokens) {
		const resolved = ALIASES[token];
		if (resolved === undefined) {
			const valid = [...new Set(Object.values(ALIASES))].sort().join(', ');
			console.error(`Unknown target: '${token}'\nValid targets: ${valid}`);
			process.exit(1);
		}
		subsystems.add(resolved);
	}

	return { subsystems };
}

/** Check if all subsystems are selected (no filtering needed). */
export function isAll(targets: ResolvedTargets): boolean {
	return targets.subsystems.size === ALL_SUBSYSTEMS.length;
}

/** Human-readable label for the selected targets. */
export function targetLabel(targets: ResolvedTargets): string {
	if (isAll(targets)) return 'all';
	return [...targets.subsystems].sort().join(', ');
}
