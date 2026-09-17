/**
 * Replaces a header's asc/desc/none behavior, for a column whose header drives a
 * longer cycle or a different key than its own. Returning null keeps the default.
 */
export interface HeaderOverride {
  /** Replaces the column's own label, with the active sort key or a shared heading. */
  label?: string;
  /** Keeps the label for screen readers only, for a column its neighbour heads. */
  labelHidden?: boolean;
  indicator?: "asc" | "desc" | "none";
  /** Native tooltip, describing what the next click does. */
  title?: string;
  /** Replaces the column's own sort toggle. Omitted, the column keeps it. */
  onclick?: () => void;
}
