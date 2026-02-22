import type { useClipboard } from "$lib/composables/useClipboard.svelte";
import { createContext } from "svelte";

export interface TableContext {
  clipboard: ReturnType<typeof useClipboard>;
  subjectMap: Record<string, string>;
  maxSubjectLength: number;
}

export const [getTableContext, setTableContext] = createContext<TableContext>();
