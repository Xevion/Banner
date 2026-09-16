/**
 * Subject color palette for the timeline chart.
 *
 * Subjects are dynamic (coming from the API), so we assign colors from
 * a fixed palette based on a deterministic hash of the subject code.
 * Known high-enrollment subjects get hand-picked colors for familiarity.
 */

/** Hand-picked colors for common UTSA subject codes. */
const KNOWN_SUBJECT_COLORS: Record<string, string> = {
  CS: "#6366f1", // indigo
  MAT: "#f59e0b", // amber
  BIO: "#10b981", // emerald
  ENG: "#ef4444", // red
  PHY: "#3b82f6", // blue
  HIS: "#8b5cf6", // violet
  CHE: "#f97316", // orange
  PSY: "#ec4899", // pink
  ECE: "#14b8a6", // teal
  ART: "#a855f7", // purple
  ACC: "#84cc16", // lime
  FIN: "#06b6d4", // cyan
  MUS: "#e11d48", // rose
  POL: "#d946ef", // fuchsia
  SOC: "#22d3ee", // sky
  KIN: "#4ade80", // green
  IS: "#fb923c", // light orange
  STA: "#818cf8", // light indigo
  MGT: "#fbbf24", // yellow
  MKT: "#2dd4bf", // teal-light
};

/**
 * Extended palette for subjects that don't have a hand-picked color.
 * These are chosen to be visually distinct from each other.
 */
const FALLBACK_PALETTE: readonly [string, ...string[]] = [
  "#f472b6", // pink-400
  "#60a5fa", // blue-400
  "#34d399", // emerald-400
  "#fbbf24", // amber-400
  "#a78bfa", // violet-400
  "#fb7185", // rose-400
  "#38bdf8", // sky-400
  "#4ade80", // green-400
  "#facc15", // yellow-400
  "#c084fc", // purple-400
  "#f87171", // red-400
  "#2dd4bf", // teal-400
  "#fb923c", // orange-400
  "#818cf8", // indigo-400
  "#a3e635", // lime-400
  "#22d3ee", // cyan-400
];

/** Simple string hash for deterministic color assignment. */
function hashCode(str: string): number {
  let hash = 0;
  for (let i = 0; i < str.length; i++) {
    hash = ((hash << 5) - hash + str.charCodeAt(i)) | 0;
  }
  return Math.abs(hash);
}

/** Cache of assigned colors to avoid re-computing. */
const colorCache = new Map<string, string>();

/** Get a consistent color for any subject code. */
export function getSubjectColor(subject: string): string {
  const cached = colorCache.get(subject);
  if (cached) return cached;

  const fallbackIndex = hashCode(subject) % FALLBACK_PALETTE.length;
  // The palette is non-empty, so the modulo always lands; index 0 satisfies the checker.
  const color =
    KNOWN_SUBJECT_COLORS[subject] ?? FALLBACK_PALETTE[fallbackIndex] ?? FALLBACK_PALETTE[0];

  colorCache.set(subject, color);
  return color;
}
