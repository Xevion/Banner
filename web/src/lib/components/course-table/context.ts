import type { useClipboard } from "$lib/composables/useClipboard.svelte";
import { createContext } from "svelte";

export interface TableContext {
  clipboard: ReturnType<typeof useClipboard>;
  subjectMap: Record<string, string>;
  maxSubjectLength: number;
  /** Lets a cell adapt to a neighbour, e.g. the end time joining a visible start. */
  isColumnVisible: (id: string) => boolean;
}

export const [getTableContext, setTableContext] = createContext<TableContext>();
