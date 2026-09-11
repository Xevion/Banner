import { browser } from "$app/environment";
import { createContext } from "svelte";
import { SvelteMap } from "svelte/reactivity";

/**
 * Slug to display name, for the filter chips that would otherwise show a slug.
 *
 * The map is per-tree rather than per-module because a universal load runs on
 * the server too, where one module instance serves every request: a shared map
 * there would grow for the life of the process and let one request's names
 * answer another's lookups.
 */
export class InstructorNames {
  #names = new SvelteMap<string, string>();

  constructor(seed?: Record<string, string>) {
    // Returning to the page remounts this, and the load that brought us back
    // asked for nothing it already knew. `warm` is empty on the server, so a
    // render there still starts from only what its own load resolved.
    for (const [slug, name] of warm) this.#names.set(slug, name);
    if (seed) this.seed(seed);
  }

  /** Merge in names, keeping any the caller does not mention. */
  seed(entries: Record<string, string>): void {
    for (const [slug, name] of Object.entries(entries)) {
      this.#names.set(slug, name);
    }
    rememberInstructorNames(entries);
  }

  /** The display name, or the slug itself while it is still unresolved. */
  get(slug: string): string {
    return this.#names.get(slug) ?? slug;
  }
}

/**
 * Names already learned in this tab.
 *
 * Only a loader reads this, to skip resolving a slug the user just picked from
 * an autocomplete. It stays empty on the server, so a server render always
 * resolves what it needs and never accumulates.
 */
const warm = new Map<string, string>();

/** Record names a loader will not have to ask for again. */
function rememberInstructorNames(entries: Record<string, string>): void {
  if (!browser) return;
  for (const [slug, name] of Object.entries(entries)) {
    warm.set(slug, name);
  }
}

/** The slugs a load still has to resolve, each asked for once. */
export function unresolvedSlugs(slugs: string[]): string[] {
  const unique = [...new Set(slugs)];
  return browser ? unique.filter((s) => !warm.has(s)) : unique;
}

export const [getInstructorNames, setInstructorNames] = createContext<InstructorNames>();
