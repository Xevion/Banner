/**
 * Generate TypeScript bindings from Rust types (ts-rs).
 *
 * Usage: bun scripts/bindings.ts
 */

import { readdirSync, writeFileSync, rmSync } from "fs";
import { run } from "./lib/proc";

const BINDINGS_DIR = "web/src/lib/bindings";

// Build lib test binary first (slow part) — fail before deleting anything
// --lib: only compiles the library's unit tests (where export_bindings live),
// skipping integration test binaries in tests/ which aren't needed here.
run(["cargo", "test", "--lib", "--no-run"]);

// Clean slate
rmSync(BINDINGS_DIR, { recursive: true, force: true });

// Run the export (fast, already compiled)
run(["cargo", "test", "--lib", "export_bindings"]);

// Auto-generate index.ts from emitted .ts files
const types = readdirSync(BINDINGS_DIR)
  .filter((f) => f.endsWith(".ts") && f !== "index.ts")
  .map((f) => f.replace(/\.ts$/, ""))
  .sort();

writeFileSync(
  `${BINDINGS_DIR}/index.ts`,
  types.map((t) => `export type { ${t} } from "./${t}";`).join("\n") + "\n",
);

console.log(`Generated ${BINDINGS_DIR}/index.ts (${types.length} types)`);
