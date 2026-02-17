<script lang="ts">
import type { CourseResponse } from "$lib/bindings";
import { formatCreditHours } from "$lib/course";
import { formatNumber } from "$lib/utils";
import { useClipboard } from "$lib/composables/useClipboard.svelte";
import { getInstructionalMethodLabel, getCampusLabel, getAttributeLabel } from "$lib/labels";
import { Check, ClipboardCopy, Info, Link } from "@lucide/svelte";
import { Tabs } from "bits-ui";
import CourseDetailInstructors from "./CourseDetailInstructors.svelte";
import CourseDetailSchedule from "./CourseDetailSchedule.svelte";
import RelatedSections from "./RelatedSections.svelte";
import SimpleTooltip from "../SimpleTooltip.svelte";

let { course }: { course: CourseResponse } = $props();

let activeTab = $state("overview");

const crnClipboard = useClipboard();
const linkClipboard = useClipboard();

function courseUrl(): string {
  return `${window.location.origin}/courses/${course.termCode}/${course.crn}`;
}

let leftColumn: HTMLDivElement;
let rightColumn: HTMLDivElement;
let sectionCount = $state(0);

$effect(() => {
  if (!leftColumn || !rightColumn) return;

  const mdQuery = window.matchMedia("(min-width: 768px)");

  function update() {
    if (mdQuery.matches) {
      rightColumn.style.maxHeight = `${leftColumn.offsetHeight}px`;
    } else {
      rightColumn.style.maxHeight = "";
    }
  }

  const observer = new ResizeObserver(update);
  observer.observe(leftColumn);
  mdQuery.addEventListener("change", update);

  return () => {
    observer.disconnect();
    mdQuery.removeEventListener("change", update);
  };
});
</script>

<Tabs.Root bind:value={activeTab}>
    <Tabs.List class="flex border-b border-border bg-muted/20">
        <Tabs.Trigger
            value="overview"
            class="px-4 py-2 text-xs font-medium text-muted-foreground transition-colors hover:text-foreground data-[state=active]:text-foreground data-[state=active]:border-b-2 data-[state=active]:border-foreground data-[state=active]:-mb-px cursor-pointer"
        >
            Overview
        </Tabs.Trigger>
        <Tabs.Trigger
            value="history"
            class="px-4 py-2 text-xs font-medium text-muted-foreground transition-colors hover:text-foreground data-[state=active]:text-foreground data-[state=active]:border-b-2 data-[state=active]:border-foreground data-[state=active]:-mb-px cursor-pointer"
        >
            History
        </Tabs.Trigger>
    </Tabs.List>

    <Tabs.Content value="overview">
        <div class="grid grid-cols-1 md:grid-cols-[3fr_2fr] gap-x-6">
            <div
                bind:this={leftColumn}
                class="flex flex-col gap-4 min-w-0 pt-4 px-4 md:pb-4"
            >
                <div class="flex flex-wrap items-start gap-x-6 gap-y-3">
                    <div class="min-w-0">
                        <div class="flex items-center gap-1.5">
                            <h3
                                class="text-sm font-semibold text-foreground truncate"
                            >
                                {course.title}
                            </h3>
                            <button
                                onclick={(e) =>
                                    linkClipboard.copy(courseUrl(), e)}
                                class="inline-flex items-center text-muted-foreground hover:text-foreground transition-colors cursor-pointer shrink-0"
                                title="Copy link to this section"
                            >
                                {#if linkClipboard.copiedValue}
                                    <Check class="size-3" />
                                {:else}
                                    <Link class="size-3" />
                                {/if}
                            </button>
                        </div>
                        <div class="flex items-center gap-2 mt-1">
                            <span
                                class="text-xs text-muted-foreground font-mono"
                            >
                                {course.subject}
                                {course.courseNumber}{course.sequenceNumber
                                    ? `-${course.sequenceNumber}`
                                    : ""}
                            </span>
                            <span class="text-border">|</span>
                            <span
                                class="text-xs text-muted-foreground font-mono"
                                >CRN {course.crn}</span
                            >
                            <button
                                onclick={(e) =>
                                    crnClipboard.copy(course.crn, e)}
                                class="inline-flex items-center text-muted-foreground hover:text-foreground transition-colors cursor-pointer"
                                title="Copy CRN"
                            >
                                {#if crnClipboard.copiedValue === course.crn}
                                    <Check class="size-3" />
                                {:else}
                                    <ClipboardCopy class="size-3" />
                                {/if}
                            </button>
                        </div>
                    </div>

                    <div class="flex flex-col gap-1.5">
                        <div
                            class="flex flex-wrap items-center gap-x-3 gap-y-1 text-sm"
                        >
                            <span class="inline-flex items-center gap-1.5">
                                <span class="text-muted-foreground text-xs"
                                    >Delivery</span
                                >
                                <span class="text-foreground">
                                    {#if course.instructionalMethod}
                                        {getInstructionalMethodLabel(course.instructionalMethod, "detail")}
                                    {:else}
                                        &mdash;
                                    {/if}
                                    {#if course.campus}
                                        <span class="text-muted-foreground">
                                            &middot; {getCampusLabel(course.campus, "detail")}
                                        </span>
                                    {/if}
                                </span>
                            </span>

                            <span class="text-border">|</span>

                            <span class="inline-flex items-center gap-1.5">
                                <span class="text-muted-foreground text-xs"
                                    >Credits</span
                                >
                                <span class="text-foreground"
                                    >{formatCreditHours(course)}</span
                                >
                            </span>

                            {#if course.enrollment.waitCapacity > 0}
                                <span class="text-border">|</span>
                                <span class="inline-flex items-center gap-1.5">
                                    <span class="text-muted-foreground text-xs"
                                        >Waitlist</span
                                    >
                                    <span class="text-foreground">
                                        {formatNumber(
                                            course.enrollment.waitCount,
                                        )}/{formatNumber(
                                            course.enrollment.waitCapacity,
                                        )}
                                    </span>
                                </span>
                            {/if}

                            {#if course.crossList}
                                <span class="text-border">|</span>
                                <span class="inline-flex items-center gap-1.5">
                                    <SimpleTooltip
                                        text="Cross-listed sections share enrollment across multiple course numbers."
                                        delay={150}
                                        passthrough
                                    >
                                        <span
                                            class="text-muted-foreground text-xs inline-flex items-center gap-0.5"
                                        >
                                            Cross-list
                                            <Info
                                                class="size-2.5 text-muted-foreground/50"
                                            />
                                        </span>
                                    </SimpleTooltip>
                                    <span
                                        class="font-mono text-xs font-medium bg-muted border border-border rounded px-1.5 py-0.5"
                                    >
                                        {course.crossList.identifier}
                                    </span>
                                    {#if course.crossList.count != null && course.crossList.capacity != null}
                                        <span
                                            class="text-muted-foreground text-xs"
                                        >
                                            {formatNumber(
                                                course.crossList.count,
                                            )}/{formatNumber(
                                                course.crossList.capacity,
                                            )}
                                        </span>
                                    {/if}
                                </span>
                            {/if}
                        </div>

                        {#if course.attributes.length > 0}
                            <div class="flex flex-wrap items-center gap-1.5">
                                <span class="text-muted-foreground text-xs mr-1"
                                    >Attributes</span
                                >
                                {#each course.attributes as attr (attr)}
                                    <SimpleTooltip
                                        text={getAttributeLabel(attr, "tooltip")}
                                        delay={100}
                                        passthrough
                                    >
                                        <span
                                            class="inline-flex text-xs font-medium bg-muted border border-border rounded px-1.5 py-0.5 text-muted-foreground hover:text-foreground hover:border-foreground/20 transition-colors cursor-default"
                                        >
                                            {getAttributeLabel(attr, "filter")}
                                        </span>
                                    </SimpleTooltip>
                                {/each}
                            </div>
                        {/if}
                    </div>
                </div>

                <div class="flex flex-wrap items-start gap-x-6 gap-y-4">
                    <CourseDetailSchedule {course} />
                    <CourseDetailInstructors {course} />
                </div>
            </div>

            <div
                bind:this={rightColumn}
                class="min-w-0 px-4 py-4 md:border-l md:border-border md:pb-4 flex flex-col"
            >
                <RelatedSections {course} bind:sectionCount />
            </div>
        </div>
    </Tabs.Content>

    <Tabs.Content value="history">
        <div class="flex flex-col items-center justify-center py-8 text-center">
            <p class="text-sm text-muted-foreground">
                Historical enrollment and grade data coming soon.
            </p>
            <p class="text-xs text-muted-foreground/60 mt-1">
                Enrollment trends, fill rates, and grade distributions across
                past semesters.
            </p>
        </div>
    </Tabs.Content>
</Tabs.Root>
