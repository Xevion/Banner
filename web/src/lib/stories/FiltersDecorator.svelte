<script lang="ts">
import { InstructorNames, setInstructorNames } from "$lib/stores/instructor-names";
import { createFilterState, setFiltersContext } from "$lib/stores/search-filters.svelte";
import type { Snippet } from "svelte";

let { children }: { children: Snippet } = $props();

// The autocompletes read both contexts, and reading an unset one throws.
setFiltersContext(createFilterState());
setInstructorNames(new InstructorNames());
</script>

<div class="w-[28rem] max-w-full">
  {@render children()}
</div>
