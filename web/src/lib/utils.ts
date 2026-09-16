import { type ClassValue, clsx } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

/**
 * Register values the surrounding `$effect` should re-run on but does not otherwise read.
 * Evaluating the arguments inside the effect is what subscribes to them.
 */
export function dependOn(..._values: unknown[]): void {
  // Intentionally empty: evaluating the arguments is the whole effect.
}

/**
 * Drop keys whose value is `undefined`.
 * `exactOptionalPropertyTypes` rejects an explicit `undefined` for an optional field, so
 * an options bag assembled from possibly-absent values has to omit the key instead.
 */
export function compact<T extends object>(obj: T): { [K in keyof T]?: Exclude<T[K], undefined> } {
  return Object.fromEntries(Object.entries(obj).filter(([, v]) => v !== undefined)) as {
    [K in keyof T]?: Exclude<T[K], undefined>;
  };
}

/** Shared tooltip content styling for bits-ui Tooltip.Content */
export const tooltipContentClass =
  "z-50 bg-card text-card-foreground text-xs border border-border rounded-md px-2.5 py-1.5 shadow-md max-w-72";

export interface FormatNumberOptions {
  /** Include sign for positive numbers (default: false) */
  sign?: boolean;
  /** Maximum fraction digits (default: 0 for integers) */
  maximumFractionDigits?: number;
}

/**
 * Format a number with locale-aware thousands separators.
 * Uses browser locale via Intl.NumberFormat.
 */
export function formatNumber(num: number, options: FormatNumberOptions = {}): string {
  const { sign = false, maximumFractionDigits = 0 } = options;
  const formatted = new Intl.NumberFormat(undefined, {
    maximumFractionDigits,
  }).format(num);

  if (sign && num >= 0) {
    return `+${formatted}`;
  }
  return formatted;
}
