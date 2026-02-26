<script lang="ts">
import type { CourseResponse } from "$lib/bindings";
import { formatISOTime, formatMeetingDaysLong } from "$lib/course";
import { formatDate } from "$lib/date";
import { Calendar, Download, ExternalLink, MapPin } from "@lucide/svelte";
import { DropdownMenu } from "bits-ui";

let { course }: { course: CourseResponse } = $props();

let sharedDateRange = $derived.by(() => {
  if (course.meetingTimes.length === 0) return null;
  const first = course.meetingTimes[0].dateRange;
  const allSame = course.meetingTimes.every(
    (mt) => mt.dateRange.start === first.start && mt.dateRange.end === first.end
  );
  if (allSame) return first;
  return null;
});

let hasCalendar = $derived(course.meetingTimes.length > 0);
</script>

<div>
  <div class="flex items-center justify-between mb-1.5">
    <h4 class="text-xs font-medium text-muted-foreground uppercase tracking-wide">Schedule</h4>
    {#if hasCalendar}
      <DropdownMenu.Root>
        <DropdownMenu.Trigger
          class="inline-flex items-center justify-center size-6 rounded-md text-muted-foreground hover:text-foreground hover:bg-muted transition-colors cursor-pointer"
        >
          <Calendar class="size-3.5" />
        </DropdownMenu.Trigger>
        <DropdownMenu.Portal>
          <DropdownMenu.Content
            class="z-50 bg-card text-card-foreground text-xs border border-border rounded-md shadow-md min-w-[160px] p-1"
            sideOffset={4}
          >
            <DropdownMenu.Item
              class="flex items-center gap-2 px-2.5 py-1.5 rounded-sm cursor-pointer data-[highlighted]:bg-muted transition-colors"
              onSelect={() => {
                const a = document.createElement("a");
                a.href = `/api/courses/${course.termSlug}/${course.crn}/calendar.ics`;
                a.download = "";
                a.click();
              }}
            >
              <Download class="size-3.5" />
              Download ICS
            </DropdownMenu.Item>
            <DropdownMenu.Item
              class="flex items-center gap-2 px-2.5 py-1.5 rounded-sm cursor-pointer data-[highlighted]:bg-muted transition-colors"
              onSelect={() => {
                window.open(
                  `/api/courses/${course.termSlug}/${course.crn}/gcal`,
                  "_blank",
                  "noopener"
                );
              }}
            >
              <ExternalLink class="size-3.5" />
              Google Calendar
            </DropdownMenu.Item>
          </DropdownMenu.Content>
        </DropdownMenu.Portal>
      </DropdownMenu.Root>
    {/if}
  </div>

  {#if course.meetingTimes.length > 0}
    <div class="flex flex-col gap-1">
      {#each course.meetingTimes as mt, i (i)}
        <div class="flex items-baseline flex-wrap gap-x-1.5 gap-y-0.5 text-sm">
          {#if mt.days.length === 0 && mt.timeRange === null}
            <span class="italic text-muted-foreground">TBA</span>
          {:else}
            {#if mt.days.length > 0}
              <span class="font-medium text-foreground">{formatMeetingDaysLong(mt)}</span>
            {/if}
            {#if mt.timeRange !== null}
              <span class="text-foreground/80">
                {formatISOTime(mt.timeRange.start)}&ndash;{formatISOTime(mt.timeRange.end)}
              </span>
            {:else}
              <span class="italic text-muted-foreground">Time TBA</span>
            {/if}
            {#if mt.location?.building ?? mt.location?.room}
              <span class="text-muted-foreground">&middot;</span>
              <span class="inline-flex items-center gap-1 text-xs text-muted-foreground">
                <MapPin class="size-3 shrink-0" />
                {mt.location.buildingDescription ?? mt.location.building}{mt.location.room
                  ? ` ${mt.location.room}`
                  : ""}
              </span>
            {/if}
          {/if}
          {#if !sharedDateRange}
            <span class="text-muted-foreground">&middot;</span>
            <span class="text-xs text-muted-foreground/70">
              {formatDate(mt.dateRange.start)}&ndash;{formatDate(mt.dateRange.end)}
            </span>
          {/if}
        </div>
      {/each}

      {#if sharedDateRange}
        <div class="text-xs text-muted-foreground/70 mt-0.5">
          {formatDate(sharedDateRange.start)} &ndash; {formatDate(sharedDateRange.end)}
        </div>
      {/if}
    </div>
  {:else}
    <span class="italic text-muted-foreground text-sm">TBA</span>
  {/if}
</div>
