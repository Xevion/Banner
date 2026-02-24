import { SvelteMap, SvelteSet } from "svelte/reactivity";

export function useRowHighlight(durationMs = 2000) {
  const changed = new SvelteSet<number>();
  const timeouts = new SvelteMap<number, ReturnType<typeof setTimeout>>();

  function mark(id: number) {
    const existing = timeouts.get(id);
    if (existing) clearTimeout(existing);
    changed.add(id);
    const t = setTimeout(() => {
      changed.delete(id);
      timeouts.delete(id);
    }, durationMs);
    timeouts.set(id, t);
  }

  function clear() {
    for (const t of timeouts.values()) clearTimeout(t);
    timeouts.clear();
    changed.clear();
  }

  function has(id: number) {
    return changed.has(id);
  }

  return { mark, clear, has };
}
