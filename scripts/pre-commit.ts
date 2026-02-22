#!/usr/bin/env bun

import { getCommand } from './lib/commands';
import { runPiped } from './lib/proc';

function main() {
	// Get list of staged files
	const stagedResult = runPiped(['git', 'diff', '--cached', '--name-only', '--diff-filter=ACMR']);
	const stagedFiles = new Set(stagedResult.stdout.trim().split('\n').filter(Boolean));

	if (stagedFiles.size === 0) {
		process.exit(0); // Nothing staged, nothing to do
	}

	// Get list of partially staged files (files with both staged and unstaged changes)
	const partiallyStaged = new Set<string>();
	for (const file of stagedFiles) {
		const diffResult = runPiped(['git', 'diff', '--name-only', file]);
		if (diffResult.exitCode === 0 && diffResult.stdout.trim()) {
			partiallyStaged.add(file);
		}
	}

	// Categorize staged files by subsystem
	const frontendFiles = Array.from(stagedFiles).filter((f) => f.startsWith('web/'));
	const backendFiles = Array.from(stagedFiles).filter(
		(f) => f.startsWith('src/') || f === 'Cargo.toml' || f === 'Cargo.lock',
	);

	// Track which subsystems need formatting
	let needsFormatting = false;

	// Run format checks for each subsystem with staged files
	for (const [files, sub] of [
		[frontendFiles, 'frontend'],
		[backendFiles, 'backend'],
	] as const) {
		if (files.length > 0) {
			const def = getCommand(sub, 'format-check');
			const result = runPiped(def.cmd, { cwd: def.cwd });
			if (result.exitCode !== 0) needsFormatting = true;
		}
	}

	// If all format checks passed, exit early
	if (!needsFormatting) {
		process.exit(0);
	}

	console.log('⚠  Formatting issues detected, running auto-format...');

	// Run auto-format for subsystems that need it
	for (const [files, sub] of [
		[frontendFiles, 'frontend'],
		[backendFiles, 'backend'],
	] as const) {
		if (files.length > 0) {
			const def = getCommand(sub, 'format-apply');
			runPiped(def.cmd, { cwd: def.cwd });
		}
	}

	// Get files modified by formatting (unstaged changes after formatting)
	const diffResult = runPiped(['git', 'diff', '--name-only']);
	const modifiedFiles = diffResult.stdout.trim().split('\n').filter(Boolean);

	// Check for conflicts with partially staged files
	const conflicts = modifiedFiles.filter((f) => partiallyStaged.has(f));

	if (conflicts.length > 0) {
		console.error('\n❌ ERROR: Cannot auto-stage formatting changes for partially staged files:');
		for (const file of conflicts) {
			console.error(`  - ${file}`);
		}
		console.error('\nPlease either:');
		console.error('  1. Stage all changes: git add <files>');
		console.error('  2. Stash unstaged changes: git stash push --keep-index');
		process.exit(1);
	}

	// Re-stage formatted files (only those that were originally staged)
	const toStage = modifiedFiles.filter((f) => stagedFiles.has(f));

	if (toStage.length > 0) {
		runPiped(['git', 'add', ...toStage]);
		console.log(`✓ Auto-formatted and re-staged ${toStage.length} file(s)`);
	}

	process.exit(0);
}

main();
