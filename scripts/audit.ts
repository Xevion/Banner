/**
 * Run `bun audit` with a shared set of ignored advisories.
 *
 * Both `scripts/check.ts` and `.github/workflows/ci.yml` call this
 * script so the ignore list stays in one place.
 */

const IGNORED_ADVISORIES = [
	"GHSA-3ppc-4f35-3m26", // minimatch ReDoS -- transitive via eslint/@typescript-eslint/vitest, no fix available
];

const cmd = [
	"bun", "audit", "--audit-level=moderate",
	...IGNORED_ADVISORIES.map((id) => `--ignore=${id}`),
];

const proc = Bun.spawnSync(cmd, {
	stdio: ["ignore", "inherit", "inherit"],
	cwd: "web",
});

process.exit(proc.exitCode);
