<script lang="ts">
import { CAMPUS_GROUPS } from "$lib/labels";

let {
  campus = $bindable<string[]>([]),
}: {
  campus: string[];
} = $props();

// A convenience over the campus codes, which are what the filter actually holds.
const hasCampusStudents = $derived(campus.some((c) => CAMPUS_GROUPS.campusStudents.includes(c)));
const hasOnlinePrograms = $derived(campus.some((c) => CAMPUS_GROUPS.onlinePrograms.includes(c)));

/**
 * Which single group is in effect, or "all" when the filter does not reduce to
 * one of them.
 *
 * Both groups at once is not the same as no filter: an empty list asks for
 * every campus, while naming both still excludes any campus in neither group.
 * The pills are lit from the two flags above rather than from this, so that
 * case shows as both chosen instead of as nothing chosen.
 */
const availabilitySelection = $derived(
  hasCampusStudents === hasOnlinePrograms ? "all" : hasCampusStudents ? "campus" : "online"
);

function selectAvailability(option: "campus" | "online" | "all") {
  if (option === "campus") {
    // Set campus filter to all campus-student-accessible campuses
    campus = [...CAMPUS_GROUPS.campusStudents];
  } else if (option === "online") {
    // Set campus filter to online programs only
    campus = [...CAMPUS_GROUPS.onlinePrograms];
  } else {
    // Clear campus filter to show all
    campus = [];
  }
}

function toggleAvailability(option: "campus" | "online") {
  if (availabilitySelection === option) {
    // Already selected, clear
    selectAvailability("all");
  } else {
    selectAvailability(option);
  }
}
</script>

<div class="flex flex-col gap-2">
  <span class="text-xs font-medium text-muted-foreground select-none">Availability</span>

  <div class="flex flex-wrap gap-1">
    <button
      type="button"
      aria-pressed={hasCampusStudents}
      class="inline-flex items-center rounded-full px-2.5 py-0.5 text-xs font-medium transition-colors cursor-pointer select-none
             {hasCampusStudents
        ? 'bg-primary text-primary-foreground'
        : 'bg-muted text-muted-foreground hover:bg-muted/80'}"
      onclick={() => toggleAvailability("campus")}
      title="Courses available to traditional campus students (Main, Downtown, Southwest, Laredo, and Internet campuses)"
    >
      Campus Students
    </button>

    <button
      type="button"
      aria-pressed={hasOnlinePrograms}
      class="inline-flex items-center rounded-full px-2.5 py-0.5 text-xs font-medium transition-colors cursor-pointer select-none
             {hasOnlinePrograms
        ? 'bg-primary text-primary-foreground'
        : 'bg-muted text-muted-foreground hover:bg-muted/80'}"
      onclick={() => toggleAvailability("online")}
      title="Courses restricted to online degree program students only"
    >
      Online Programs
    </button>
  </div>

  {#if availabilitySelection !== "all"}
    <p class="text-xs text-muted-foreground/70 italic">
      {#if availabilitySelection === "campus"}
        Showing courses available to campus students
      {:else}
        Showing courses for online degree programs only
      {/if}
    </p>
  {/if}
</div>
