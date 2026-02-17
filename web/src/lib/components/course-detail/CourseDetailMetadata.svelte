<script lang="ts">
import type { CourseResponse } from "$lib/bindings";
import { formatCreditHours } from "$lib/course";
import { formatNumber } from "$lib/utils";
import { getInstructionalMethodLabel, getCampusLabel, getAttributeLabel } from "$lib/labels";
import { Info } from "@lucide/svelte";
import SimpleTooltip from "../SimpleTooltip.svelte";

let { course }: { course: CourseResponse } = $props();
</script>

<div class="flex flex-col gap-2">
  <div class="flex flex-wrap items-center gap-x-4 gap-y-1 text-sm">
    <span class="inline-flex items-center gap-1.5">
      <span class="text-muted-foreground text-xs">Delivery</span>
      <span class="text-foreground">
        {#if course.instructionalMethod}
          {getInstructionalMethodLabel(course.instructionalMethod, "detail")}
        {:else}
          &mdash;
        {/if}
        {#if course.campus}
          <span class="text-muted-foreground"> &middot; {getCampusLabel(course.campus, "detail")}</span>
        {/if}
      </span>
    </span>

    <span class="text-border">|</span>

    <span class="inline-flex items-center gap-1.5">
      <span class="text-muted-foreground text-xs">Credits</span>
      <span class="text-foreground">{formatCreditHours(course)}</span>
    </span>

    {#if course.enrollment.waitCapacity > 0}
      <span class="text-border">|</span>
      <span class="inline-flex items-center gap-1.5">
        <span class="text-muted-foreground text-xs">Waitlist</span>
        <span class="text-foreground">
          {formatNumber(course.enrollment.waitCount)}/{formatNumber(
            course.enrollment.waitCapacity
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
          <span class="text-muted-foreground text-xs inline-flex items-center gap-0.5">
            Cross-list
            <Info class="size-2.5 text-muted-foreground/50" />
          </span>
        </SimpleTooltip>
        <span
          class="font-mono text-xs font-medium bg-muted border border-border rounded px-1.5 py-0.5"
        >
          {course.crossList.identifier}
        </span>
        {#if course.crossList.count != null && course.crossList.capacity != null}
          <span class="text-muted-foreground text-xs">
            {formatNumber(course.crossList.count)}/{formatNumber(course.crossList.capacity)}
          </span>
        {/if}
      </span>
    {/if}
    </div>

  {#if course.attributes.length > 0}
    <div class="flex flex-wrap items-center gap-1.5">
      <span class="text-muted-foreground text-xs mr-1">Attributes</span>
      {#each course.attributes as attr (attr)}
        <SimpleTooltip text={getAttributeLabel(attr, "tooltip")} delay={100} passthrough>
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
