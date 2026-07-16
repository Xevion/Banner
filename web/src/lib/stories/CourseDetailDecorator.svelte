<script lang="ts">
import { setCourseDetailContext } from "$lib/components/course-detail/context";
import type { Snippet } from "svelte";

let { children }: { children: Snippet } = $props();

// RelatedSections calls getCourseDetailContext(), which throws when unset.
setCourseDetailContext({ navigateToSection: null });
</script>

{@render children()}
