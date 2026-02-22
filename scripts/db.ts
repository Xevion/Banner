/**
 * PostgreSQL Docker Compose service management.
 *
 * Usage: bun scripts/db.ts [start|reset|rm]
 *
 * Requires docker-compose.yml at the project root with a `postgres` service.
 */

import { readFile, writeFile } from 'fs/promises';

const USER = 'banner';
const PASS = 'banner';
const DB = 'banner';
const PORT = '59489';
const ENV_FILE = '.env';

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
	compose('up', '-d');
	// Wait briefly for postgres to be ready
	await new Promise((r) => setTimeout(r, 2000));
	Bun.spawnSync(
		[
			'docker',
			'compose',
			'exec',
			'postgres',
			'psql',
			'-U',
			USER,
			'-d',
			'postgres',
			'-c',
			`DROP DATABASE IF EXISTS ${DB}`,
		],
		{ stdio: ['ignore', 'inherit', 'inherit'] },
	);
	Bun.spawnSync(
		[
			'docker',
			'compose',
			'exec',
			'postgres',
			'psql',
			'-U',
			USER,
			'-d',
			'postgres',
			'-c',
			`CREATE DATABASE ${DB}`,
		],
		{ stdio: ['ignore', 'inherit', 'inherit'] },
	);
	await updateEnv();
	console.log('reset');
} else {
	compose('up', '-d');
	await updateEnv();
	console.log('started');
}
