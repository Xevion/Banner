/**
 * The shared tooltip surface: the box, not how it is positioned.
 *
 * `max-w-xs` bounds the measure so `whitespace-pre-line` has a width to wrap
 * against, and `text-balance` evens the lines so a sentence cannot end on a
 * single dangling word.
 */
export const TOOLTIP_SURFACE =
  "z-50 max-w-xs rounded-md border border-border bg-card px-2.5 py-1.5 " +
  "text-left text-xs leading-5 text-balance whitespace-pre-line " +
  "text-card-foreground shadow-sm";
