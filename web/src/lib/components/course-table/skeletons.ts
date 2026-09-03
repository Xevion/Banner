/**
 * One bar per cell, matching the single-line row. Widths approximate each column's
 * typical content so the placeholder does not read as a uniform block.
 */
const CELL_SKELETONS: Record<string, string> = {
  days: `<div class="h-4 w-[110px] rounded-[3px] bg-muted animate-pulse"></div>`,
  time: `<div class="ml-auto h-3.5 w-[118px] rounded bg-muted animate-pulse"></div>`,
  duration: `<div class="ml-auto h-3.5 w-[46px] rounded bg-muted animate-pulse"></div>`,
  course_code: `<div class="h-3.5 w-[88px] rounded bg-muted animate-pulse"></div>`,
  title: `<div class="h-3.5 w-4/5 rounded bg-muted animate-pulse"></div>`,
  instructor: `<div class="h-3.5 w-[120px] rounded bg-muted animate-pulse"></div>`,
  seats: `<div class="h-3.5 w-[70px] rounded bg-muted animate-pulse"></div>`,
};

export function buildSkeletonHtml(colIds: string[], rowCount: number): string {
  const cells = colIds
    .map((id) => {
      const skeleton =
        CELL_SKELETONS[id] ?? `<div class="h-3.5 w-20 rounded bg-muted animate-pulse"></div>`;
      return `<td class="px-2">${skeleton}</td>`;
    })
    .join("");
  const row = `<tr class="h-10 border-b border-border">${cells}</tr>`;
  return row.repeat(rowCount);
}

export function buildCardSkeletonHtml(count: number): string {
  const card = `<div class="rounded-lg border border-border bg-card p-3 animate-pulse"><div class="flex items-baseline justify-between gap-2"><div class="flex items-baseline gap-1.5"><div class="h-4 w-16 bg-muted rounded"></div><div class="h-4 w-32 bg-muted rounded"></div></div><div class="h-4 w-10 bg-muted rounded"></div></div><div class="flex items-center justify-between gap-2 mt-1"><div class="h-3 w-24 bg-muted rounded"></div><div class="h-3 w-20 bg-muted rounded"></div></div></div>`;
  return card.repeat(count);
}
