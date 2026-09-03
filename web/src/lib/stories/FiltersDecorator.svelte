<script lang="ts">
import { createFilterState, setFiltersContext } from "$lib/stores/search-filters.svelte";
import type { Snippet } from "svelte";

let { children }: { children: Snippet } = $props();

// SearchAutocomplete calls getFiltersContext(), which throws when unset.
setFiltersContext(createFilterState());
</script>

<div class="w-[28rem] max-w-full">
  {@render children()}
</div>
