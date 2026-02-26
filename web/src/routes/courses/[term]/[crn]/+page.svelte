<script lang="ts">
import type { CourseResponse, SearchOptionsResponse } from "$lib/bindings";
import Footer from "$lib/components/Footer.svelte";
import { buildAttributeMap, setCourseDetailContext } from "$lib/components/course-detail/context";
import CourseDetailTabs from "$lib/components/course-detail/CourseDetailTabs.svelte";
import { formatCreditHours } from "$lib/course";
import { getInstructionalMethodLabel } from "$lib/labels";
import Breadcrumb from "$lib/components/Breadcrumb.svelte";
import { Calendar, ExternalLink } from "@lucide/svelte";
import { untrack } from "svelte";

interface PageData {
  course: CourseResponse;
  searchOptions: SearchOptionsResponse | null;
  term: string;
}

let { data }: { data: PageData } = $props();

const course = untrack(() => data.course);

const attributes = $derived(data.searchOptions?.reference.attributes ?? []);
const attributeMap = $derived(buildAttributeMap(attributes));

setCourseDetailContext({
  get attributeMap() {
    return attributeMap;
  },
  navigateToSection: null,
});

const calendarIcsUrl = $derived(`/api/courses/${course.termSlug}/${course.crn}/calendar.ics`);
const calendarGcalUrl = $derived(`/api/courses/${course.termSlug}/${course.crn}/gcal`);
const coursePageUrl = $derived(`/courses/${data.term}/${course.subject}/${course.courseNumber}`);
</script>

<svelte:head>
  <title>{course.subject} {course.courseNumber} &mdash; CRN {course.crn} | Banner</title>
</svelte:head>

<div class="min-h-screen flex flex-col items-center px-3 md:px-5 pb-5 pt-20">
  <div class="w-full max-w-6xl flex flex-col pt-2">
    <Breadcrumb
      items={[
        { label: "Home", href: "/" },
        { label: "Subjects", href: "/subjects" },
        { label: course.subject, href: `/subjects/${course.subject}` },
        {
          label: course.courseNumber,
          href: `/courses/${data.term}/${course.subject}/${course.courseNumber}`,
        },
        { label: course.sequenceNumber ?? `CRN ${course.crn}` },
      ]}
    />

    <!-- Header -->
    <div class="mb-6">
      <h1 class="text-2xl font-bold">
        <a href={coursePageUrl} class="hover:underline hover:text-foreground transition-colors">
          {course.title}
        </a>
      </h1>
      <div class="flex flex-wrap items-center gap-2 mt-1.5">
        <a href={coursePageUrl} class="text-sm text-muted-foreground font-mono hover:underline hover:text-foreground transition-colors">
          {course.subject} {course.courseNumber}{course.sequenceNumber
            ? `-${course.sequenceNumber}`
            : ""}
        </a>
        <span class="text-border">|</span>
        <span
          class="inline-flex items-center px-2 py-0.5 text-xs font-medium rounded
                 bg-muted text-muted-foreground"
        >
          CRN {course.crn}
        </span>
        {#if course.instructionalMethod}
          <span class="text-border">|</span>
          <span class="text-sm text-muted-foreground">
            {getInstructionalMethodLabel(course.instructionalMethod, "detail")}
          </span>
        {/if}
        <span class="text-border">|</span>
        <span class="text-sm text-muted-foreground">
          {formatCreditHours(course)} credits
        </span>
      </div>

      <!-- Calendar export links -->
      <div class="flex items-center gap-3 mt-3">
        <a
          href={calendarIcsUrl}
          class="inline-flex items-center gap-1.5 text-xs text-muted-foreground hover:text-foreground transition-colors"
        >
          <Calendar class="size-3.5" />
          Download ICS
        </a>
        <a
          href={calendarGcalUrl}
          target="_blank"
          rel="noopener"
          class="inline-flex items-center gap-1.5 text-xs text-muted-foreground hover:text-foreground transition-colors"
        >
          <ExternalLink class="size-3.5" />
          Add to Google Calendar
        </a>
      </div>
    </div>

    <!-- Course detail tabs -->
    <div class="rounded-lg border border-border bg-card overflow-hidden mb-8">
      <CourseDetailTabs {course} />
    </div>

    <Footer />
  </div>
</div>
