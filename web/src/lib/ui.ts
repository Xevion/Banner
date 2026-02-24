export interface StatusBadge {
  label: string;
  classes: string;
}

export interface FilterCard<S> {
  label: string;
  value: string | undefined;
  stat: keyof S;
  textColor: string;
  ringColor: string;
}

export interface ProgressSegment<S> {
  stat: keyof S;
  color: string;
  label: string;
}

export function getBadge(map: Record<string, StatusBadge>, status: string): StatusBadge {
  return map[status] ?? { label: status, classes: "bg-muted text-muted-foreground" };
}
