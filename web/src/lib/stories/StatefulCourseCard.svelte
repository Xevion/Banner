<script lang="ts">
/**
 * A CourseCard that owns its expanded state, for stories about the interaction
 * rather than about a fixed appearance.
 *
 * Passing `expanded` as a story arg pins the card open or shut, which leaves the
 * header looking clickable while clicking it changes nothing.
 */
import type { CourseResponse } from "$lib/bindings";
import CourseCard from "$lib/components/CourseCard.svelte";

let { course }: { course: CourseResponse } = $props();

let expanded = $state(false);
</script>

<CourseCard {course} {expanded} onToggle={() => (expanded = !expanded)} />
