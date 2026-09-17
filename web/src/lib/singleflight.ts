/**
 * Collapses overlapping calls for one key onto a single operation.
 *
 * A caller arriving while a key is in flight joins the call already running
 * instead of starting a second, and every joiner gets that call's outcome,
 * success or failure. Entries are dropped the moment they settle, so this
 * shares work without ever holding an answer: callers in the same tick cost one
 * call, callers a second apart cost two. Anything that should survive past the
 * request wants a cache, which this deliberately is not.
 */
export class SingleFlight {
  private inFlight = new Map<string, Promise<unknown>>();

  /** Runs `operation`, or joins the one already running under `key`. */
  run<T>(key: string, operation: () => Promise<T>): Promise<T> {
    const existing = this.inFlight.get(key);
    // Callers agree on the key because they want the same thing, which is what
    // makes them agree on its type; the map cannot carry that per entry.
    if (existing) return existing as Promise<T>;

    // A throw before the first await never reaches the map, so the key stays
    // free rather than holding a call that is not running.
    const pending = operation().finally(() => this.inFlight.delete(key));
    this.inFlight.set(key, pending);
    return pending;
  }
}
