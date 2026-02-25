/**
 * Pre-flight staleness detection for generated artifacts.
 *
 * Provides a generic mtime-comparison mechanism: if source files are newer
 * than generated artifacts, a regeneration callback runs. Used by check.ts
 * for TypeScript bindings and SQLx metadata.
 */

import { existsSync, statSync } from 'fs';
import { c, elapsed } from './fmt';

/** Newest mtime among files matching a glob pattern, or 0 if none match. */
export function newestMtime(dir: string, pattern: string): number {
	let newest = 0;
	for (const file of new Bun.Glob(pattern).scanSync(dir)) {
		const mt = statSync(`${dir}/${file}`).mtimeMs;
		if (mt > newest) newest = mt;
	}
	return newest;
}

/** Result returned by a preflight regeneration callback. */
export interface PreflightResult {
	/** Detail appended to the output line (e.g. "15 types, 3 changed"). */
	detail?: string;
	/** If set, display as warning (yellow) instead of success (green). */
	warning?: string;
}

export interface PreflightOpts {
	/** Short label for output (e.g. "bindings", "sqlx"). */
	label: string;
	/** Pre-computed source modification time (epoch ms). */
	sourceMtime: number;
	/** Directory containing generated artifacts. */
	artifactDir: string;
	/** Glob pattern for artifact files within artifactDir. */
	artifactGlob: string;
	/** Reason shown in the regenerating message (e.g. "Rust sources changed"). */
	reason: string;
	/**
	 * Called when artifacts are stale. For fatal failures, call
	 * process.exit() directly -- ensureFresh will not continue.
	 */
	regenerate: () => PreflightResult | undefined;
}

/**
 * Regenerate artifacts when sources are newer.
 *
 * Compares the newest source mtime against the newest artifact mtime.
 * If artifacts are missing or older, runs the regeneration callback.
 *
 * @returns true if regeneration was performed.
 */
export function ensureFresh(opts: PreflightOpts): boolean {
	const { label, sourceMtime, artifactDir, artifactGlob, reason, regenerate } = opts;

	const artifactMtime = existsSync(artifactDir) ? newestMtime(artifactDir, artifactGlob) : 0;
	const stale = artifactMtime === 0 || sourceMtime > artifactMtime;

	if (!stale) {
		process.stdout.write(c('2', `* ${label} up-to-date, skipped`) + '\n');
		return false;
	}

	const t = Date.now();
	process.stdout.write(c('1;36', `➡️ Regenerating ${label} (${reason})...`) + '\n');

	const result = regenerate();
	const detail = result?.detail ? `, ${result.detail}` : '';

	if (result?.warning) {
		process.stdout.write(c('33', `⚠️ ${result.warning}`) + ` (${elapsed(t)}s${detail})\n`);
	} else {
		process.stdout.write(c('32', `✅ ${label}`) + ` (${elapsed(t)}s${detail})\n`);
	}

	return true;
}
