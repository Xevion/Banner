/**
 * Every column is a stack now, so each skeleton mirrors its cell's line count.
 * A single bar would collapse the row height and make loading jump.
 */
const CELL_SKELETONS: Record<string, string> = {
  time: `<div class="flex w-31 flex-col gap-[5px] animate-pulse"><div class="h-4 w-full rounded bg-muted"></div><div class="h-[9px] w-full rounded-[2px] bg-muted"></div><div class="h-3 w-16 rounded bg-muted"></div></div>`,
  course_code: `<div class="flex flex-col gap-1 animate-pulse"><div class="h-4 w-24 rounded bg-muted"></div><div class="h-3 w-40 rounded bg-muted"></div><div class="h-3 w-12 rounded bg-muted"></div></div>`,
  instructor: `<div class="flex flex-col gap-1 animate-pulse"><div class="h-4 w-20 rounded bg-muted"></div><div class="h-3 w-16 rounded bg-muted"></div></div>`,
  seats: `<div class="flex flex-col items-end gap-1 animate-pulse"><div class="h-5 w-8 rounded bg-muted"></div><div class="h-2.5 w-10 rounded bg-muted"></div></div>`,
};

export function buildSkeletonHtml(colIds: string[], rowCount: number): string {
  const cells = colIds
    .map((id) => {
      const skeleton =
        CELL_SKELETONS[id] ?? `<div class="h-4 w-20 rounded bg-muted animate-pulse"></div>`;
      return `<td class="py-2 px-2">${skeleton}</td>`;
    })
    .join("");
  const row = `<tr class="border-b border-border">${cells}</tr>`;
  return row.repeat(rowCount);
}

export function buildCardSkeletonHtml(count: number): string {
  const card = `<div class="rounded-lg border border-border bg-card p-3 animate-pulse"><div class="flex items-baseline justify-between gap-2"><div class="flex items-baseline gap-1.5"><div class="h-4 w-16 bg-muted rounded"></div><div class="h-4 w-32 bg-muted rounded"></div></div><div class="h-4 w-10 bg-muted rounded"></div></div><div class="flex items-center justify-between gap-2 mt-1"><div class="h-3 w-24 bg-muted rounded"></div><div class="h-3 w-20 bg-muted rounded"></div></div></div>`;
  return card.repeat(count);
}
