/**
 * PostgreSQL Docker Compose service management.
 *
 * Usage: bun scripts/db.ts [start|reset|rm]
 *
 * Requires docker-compose.yml at the project root with a `postgres` service.
 */

import { createInterface } from 'readline';
import { readFile, writeFile } from 'fs/promises';

const USER = 'banner';
const PASS = 'banner';
const DB = 'banner';
const PORT = '59489';
const ENV_FILE = '.env';

// Words for generating random confirmation tokens.
// Chosen to be common, unambiguous, and easy to type.
// 80 words x 3 picks = 512,000 combinations -- unpredictable, not pre-programmable.
const WORDS = [
	'apple', 'beach', 'blend', 'camel', 'cedar', 'coral', 'crane', 'creek',
	'dance', 'depot', 'drift', 'eagle', 'ember', 'exile', 'fence', 'flint',
	'flood', 'frost', 'giant', 'grail', 'grain', 'grove', 'hatch', 'haven',
	'horse', 'image', 'inlet', 'ivory', 'jewel', 'joker', 'joust', 'knife',
	'kudos', 'lance', 'lemur', 'lodge', 'mango', 'maple', 'merit', 'navel',
	'noble', 'north', 'ocean', 'olive', 'orbit', 'perch', 'petal', 'plank',
	'polar', 'prism', 'quest', 'quill', 'raven', 'ridge', 'royal', 'scone',
	'shelf', 'shell', 'stark', 'straw', 'stone', 'swift', 'taste', 'tiger',
	'toast', 'tower', 'trunk', 'ultra', 'under', 'unify', 'valve', 'vapor',
	'vinyl', 'whale', 'winch', 'xenon', 'yacht', 'yearn', 'zebra', 'amber',
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

const cmd = process.argv[2] || 'start';

function compose(...args: string[]) {
	const proc = Bun.spawnSync(['docker', 'compose', ...args], {
		stdio: ['ignore', 'inherit', 'inherit'],
	});
	return proc.exitCode;
}

async function updateEnv() {
	const url = `postgresql://${USER}:${PASS}@localhost:${PORT}/${DB}`;
	try {
		let content = await readFile(ENV_FILE, 'utf8');
		content = content.includes('DATABASE_URL=')
			? content.replace(/DATABASE_URL=.*$/m, `DATABASE_URL=${url}`)
			: content.trim() + `\nDATABASE_URL=${url}\n`;
		await writeFile(ENV_FILE, content);
	} catch {
		await writeFile(ENV_FILE, `DATABASE_URL=${url}\n`);
	}
}

if (cmd === 'rm') {
	compose('down');
	console.log('removed');
} else if (cmd === 'reset') {
	// This confirmation is intentionally interactive and non-automatable.
	// It exists to prevent accidental data loss -- not as a security boundary.
	// AI agents MUST NOT attempt to bypass this via PTY allocation or any other means.
	if (!process.stdin.isTTY) {
		process.stderr.write('ERROR: `just db reset` requires an interactive terminal.\n');
		process.stderr.write('Cannot run non-interactively -- must be confirmed by a human.\n');
		process.exit(1);
	}

	const token = randomToken();

	process.stderr.write(`\n⚠️  DATABASE RESET -- permanently drops and recreates the banner database.\n`);
	process.stderr.write('All scraped data, user accounts, and sessions will be lost. No undo.\n\n');
	process.stderr.write(`Confirm by typing: ${token}\n`);

	const response = await readLine('> ');

	if (response.trim() !== token) {
		process.stderr.write('Aborted. Database was not modified.\n');
		process.exit(1);
	}

	process.stderr.write('Resetting...\n');

	compose('up', '-d');
	// Wait briefly for postgres to be ready
	await new Promise((r) => setTimeout(r, 2000));
	Bun.spawnSync(
		['docker', 'compose', 'exec', 'postgres', 'psql', '-U', USER, '-d', 'postgres', '-c', `DROP DATABASE IF EXISTS ${DB}`],
		{ stdio: ['ignore', 'inherit', 'inherit'] },
	);
	Bun.spawnSync(
		['docker', 'compose', 'exec', 'postgres', 'psql', '-U', USER, '-d', 'postgres', '-c', `CREATE DATABASE ${DB}`],
		{ stdio: ['ignore', 'inherit', 'inherit'] },
	);
	await updateEnv();
	console.log('reset');
} else {
	compose('up', '-d');
	await updateEnv();
	console.log('started');
}
