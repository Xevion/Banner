export interface StatusBadge {
  label: string;
  classes: string;
}

export interface FilterCard<S, V extends string> {
  label: string;
  value: V | undefined;
  stat: keyof S;
  textColor: string;
  ringColor: string;
}

export interface MatchColumn {
  label: string;
  /** Extra header cell classes, e.g. text alignment. */
  class?: string;
}

export interface ProgressSegment<S> {
  stat: keyof S;
  color: string;
  label: string;
}

/** Every variant of `K` must be present in `map`, so a new status fails the build. */
export function getBadge<K extends string>(map: Record<K, StatusBadge>, status: K): StatusBadge {
  return map[status];
}
