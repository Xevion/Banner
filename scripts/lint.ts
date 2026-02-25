/**
 * Run linters across subsystems.
 *
 * Usage: bun scripts/lint.ts [targets...]
 *
 * Targets scope linting to specific subsystems (comma-delimited):
 *   backend, frontend (and aliases: b, f, rust, web, etc.)
 *   Omit targets to lint everything.
 */

import { getCommand } from './lib/commands';
import { c } from './lib/fmt';
import { run } from './lib/proc';
import { isAll, resolveTargets, targetLabel } from './lib/targets';

const argv = process.argv.slice(2);

if (argv[0] === '--help' || argv[0] === '-h') {
	console.log(`Usage: bun scripts/lint.ts [targets...]

Runs linters for one or more subsystems.

Targets (comma-delimited, omit for all):
  backend (b, back, rust)     Clippy (--deny warnings)
  frontend (f, front, web)    Biome lint

Examples:
  bun scripts/lint.ts              Lint everything
  bun scripts/lint.ts backend      Clippy only
  bun scripts/lint.ts f,b          Frontend + backend`);
	process.exit(0);
}

const targets = resolveTargets(argv);

if (!isAll(targets)) {
	process.stdout.write(c('1;36', `➡️ Linting: ${targetLabel(targets)}`) + '\n');
}

if (targets.subsystems.has('frontend')) {
	run(getCommand('frontend', 'lint').cmd);
}
if (targets.subsystems.has('backend')) {
	run(getCommand('backend', 'lint').cmd);
}
