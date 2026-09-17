/**
 * Import every module in the SSR build to prove the runtime dependency tree is complete.
 *
 * Page chunks load lazily, so the entry point alone resolves almost nothing. Only unresolved
 * imports fail; a chunk throwing for its own reasons is reported and tolerated.
 */
import { readdirSync } from "node:fs";
import { join } from "node:path";
import { pathToFileURL } from "node:url";

const buildDir = process.argv[2] ?? "build";
const serverDir = join(buildDir, "server");

const modules: string[] = [];
function collect(dir: string): void {
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) collect(path);
    else if (entry.name.endsWith(".js")) modules.push(path);
  }
}
collect(serverDir);
modules.push(join(buildDir, "handler.js"));

let unresolved = 0;
let tolerated = 0;

for (const path of modules) {
  try {
    await import(pathToFileURL(path).href);
  } catch (error) {
    const code = error instanceof Error ? (error as NodeJS.ErrnoException).code : undefined;
    if (code === "ERR_MODULE_NOT_FOUND") {
      unresolved++;
      const detail = error instanceof Error ? error.message.split("\n")[0] : String(error);
      console.error(`unresolved  ${path}  ${detail}`);
    } else {
      tolerated++;
      console.warn(`tolerated(${code ?? "unknown"})  ${path}`);
    }
  }
}

console.log(
  `checked ${modules.length} SSR modules: ${unresolved} unresolved, ${tolerated} tolerated`
);
process.exit(unresolved > 0 ? 1 : 0);
