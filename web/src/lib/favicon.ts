/**
 * The tab icon's seat-state band.
 *
 * `static/favicon.svg` is the default icon and shows the whole seat scale. On a page that knows
 * the user's live watch state, `setFaviconState` replaces it with a one-band variant built here
 * and handed to the icon link as a data URI, so the tab only gains color when there is something
 * to say.
 */

/** Outlined Inter Bold "B", fitted to cap height 6.5 - 25.5 on the 32-unit viewBox. */
export const B_PATH =
  "M7.6 25.5V6.5H15.15Q17.23 6.5 18.63 7.13Q20.02 7.76 20.72 8.86Q21.41 9.96 21.41 11.38Q21.41 12.51 20.97 13.34Q20.52 14.18 19.75 14.71Q18.98 15.24 18 15.46V15.65Q19.07 15.7 20.02 16.26Q20.97 16.82 21.56 17.84Q22.15 18.86 22.15 20.27Q22.15 21.76 21.42 22.95Q20.68 24.13 19.23 24.82Q17.78 25.5 15.63 25.5ZM11.5 22.3H14.86Q16.57 22.3 17.36 21.64Q18.15 20.97 18.15 19.9Q18.15 19.09 17.76 18.47Q17.37 17.85 16.65 17.5Q15.94 17.14 14.96 17.14H11.5ZM11.5 14.48H14.56Q15.4 14.48 16.06 14.18Q16.72 13.88 17.1 13.33Q17.48 12.78 17.48 12.01Q17.48 10.97 16.74 10.32Q16.01 9.68 14.64 9.68H11.5Z";

/** sRGB equivalents of the seat tokens; favicon rasterizers cannot be relied on to parse oklch(). */
export const FAVICON_STATE_COLORS = {
  idle: "#737373",
  open: "#16a34a",
  low: "#eab308",
  full: "#dc2626",
  over: "#9333ea",
} as const;

export type FaviconState = keyof typeof FAVICON_STATE_COLORS;

const STATIC_ICON = "/favicon.svg";
const SVG_ICON_SELECTOR = 'link[rel="icon"][type="image/svg+xml"]';

// "Open" wins because it is the only state the user is asked to act on.
const URGENCY: readonly FaviconState[] = ["open", "low", "over", "full", "idle"];

/** The state the band should show when several watches disagree. */
export function mostUrgentState(states: Iterable<FaviconState>): FaviconState {
  const present = new Set(states);
  return URGENCY.find((state) => present.has(state)) ?? "idle";
}

export function faviconStateSvg(state: FaviconState): string {
  return (
    '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 32 32">' +
    "<style>svg{--bg:#262626;--ink:#ffffff}" +
    "@media (prefers-color-scheme: light){svg{--bg:#ffffff;--ink:#1a1a1a}}</style>" +
    '<rect width="32" height="32" style="fill:var(--bg)"/>' +
    '<g transform="translate(-1.5 0)">' +
    `<path fill-rule="evenodd" style="fill:var(--ink)" d="${B_PATH}"/></g>` +
    `<rect x="25" width="7" height="32" fill="${FAVICON_STATE_COLORS[state]}"/></svg>`
  );
}

/**
 * Paint the band. Call this where watch state settles, debounced, not per render, and never
 * animate it: the icon is a status surface and the Discord DM is the alert.
 */
export function setFaviconState(state: FaviconState): void {
  const link = document.querySelector<HTMLLinkElement>(SVG_ICON_SELECTOR);
  if (link) link.href = `data:image/svg+xml,${encodeURIComponent(faviconStateSvg(state))}`;
}

/** Go back to the full seat scale, for pages that no longer track live state. */
export function resetFavicon(): void {
  const link = document.querySelector<HTMLLinkElement>(SVG_ICON_SELECTOR);
  if (link) link.href = STATIC_ICON;
}
