<script lang="ts">
import { setTableContext } from "$lib/components/course-table/context";
import { useClipboard } from "$lib/composables/useClipboard.svelte";
import type { Snippet } from "svelte";

let {
  children,
  subjectMap = {},
  hiddenColumns = [],
}: {
  children: Snippet;
  subjectMap?: Record<string, string>;
  /** Column ids a story wants treated as hidden, for cells that adapt to a neighbour. */
  hiddenColumns?: string[];
} = $props();

const maxSubjectLength = $derived(
  Object.keys(subjectMap).reduce((longest, code) => Math.max(longest, code.length), 0)
);

setTableContext({
  clipboard: useClipboard(),
  get subjectMap() {
    return subjectMap;
  },
  get maxSubjectLength() {
    return maxSubjectLength;
  },
  isColumnVisible: (id: string) => !hiddenColumns.includes(id),
});
</script>

<!-- Cell components render a bare <td>, which needs a table ancestor to lay out. -->
<table class="text-sm">
  <tbody>
    <tr>
      {@render children()}
    </tr>
  </tbody>
</table>
