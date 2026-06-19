<script lang="ts">
import { setCourseDetailContext } from "$lib/components/course-detail/context";
import type { Snippet } from "svelte";

let { children }: { children: Snippet } = $props();

// CourseDetail and its descendants (RelatedSections) read this page-level
// context via getCourseDetailContext(), which throws when unset. Routes
// provide it in +page.svelte; stories must supply an equivalent.
setCourseDetailContext({ attributeMap: {}, navigateToSection: null });
</script>

{@render children()}
