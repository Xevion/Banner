/**
 * Telling a page's own load payload from the one the router is passing through.
 */

/**
 * The payload if it belongs to this route, or null if it belongs to another.
 *
 * SvelteKit addresses load data by branch depth, so a page sitting at the same
 * depth as the route being navigated to is handed that route's payload as its
 * own `data`. It does that before it checks whether the navigation was
 * abandoned, and an abandoned one leaves the foreign payload in place. The
 * declared type says every field is there; at runtime the fields belong to
 * whichever route the router was last headed for.
 *
 * `marker` names a key this route's load always returns. Nothing cheaper
 * separates the two payloads: the route id and the URL both still name this
 * route while the foreign data is in place.
 */
export function ownPayload<T extends object>(data: T, marker: keyof T & string): T | null {
  return marker in data ? data : null;
}
