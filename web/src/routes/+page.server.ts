import { BannerApiClient } from "$lib/api";
import type { SearchParams } from "$lib/bindings";
import type { SortColumn, SortDirection } from "$lib/bindings";
import { expandCampusFromParams } from "$lib/stores/search-filters.svelte";
import type { PageServerLoad } from "./$types";

function parseIntOrNull(value: string | null): number | null {
  if (value === null || value === "") return null;
  const n = Number(value);
  return Number.isNaN(n) ? null : n;
}

function buildSearchParams(url: URL, defaultTerm: string): SearchParams {
  const p = url.searchParams;
  return {
    term: p.get("term") ?? defaultTerm,
    limit: 25,
    offset: Number(p.get("offset")) || 0,
    sortBy: (p.get("sort_by") as SortColumn) ?? null,
    sortDir: (p.get("sort_dir") as SortDirection) ?? null,
    subject: p.getAll("subject"),
    query: p.get("query") ?? p.get("q") ?? null,
    openOnly: p.get("open") === "true",
    waitCountMax: parseIntOrNull(p.get("wait_count_max")),
    days: p.getAll("days"),
    timeStart: p.get("time_start"),
    timeEnd: p.get("time_end"),
    instructionalMethod: p.getAll("instructional_method"),
    campus: expandCampusFromParams(p),
    partOfTerm: p.getAll("part_of_term"),
    attributes: p.getAll("attributes"),
    creditHourMin: parseIntOrNull(p.get("credit_hour_min")),
    creditHourMax: parseIntOrNull(p.get("credit_hour_max")),
    instructor: p.get("instructor") ?? null,
    courseNumberLow: parseIntOrNull(p.get("course_number_low")),
    courseNumberHigh: parseIntOrNull(p.get("course_number_high")),
  };
}

export const load: PageServerLoad = async ({ url, fetch }) => {
  const client = new BannerApiClient(undefined, fetch);
  const urlTerm = url.searchParams.get("term");

  const optionsResult = await client.getSearchOptions(urlTerm ?? undefined);
  if (optionsResult.isErr) {
    console.error("Failed to load search options:", optionsResult.error);
    return {
      searchOptions: null,
      searchResult: null,
      searchError: "Failed to load search options",
      searchMeta: null,
      urlSearch: url.search,
    };
  }

  const searchOptions = optionsResult.value;
  const defaultTerm = searchOptions.terms[0]?.slug ?? "";
  const apiParams = buildSearchParams(url, defaultTerm);

  // Validate subjects against this term's available subjects
  const validSubjects = new Set(searchOptions.subjects.map((s) => s.code));
  apiParams.subject = apiParams.subject.filter((s) => validSubjects.has(s));

  const t0 = performance.now();
  const searchResult = await client.searchCourses(apiParams);
  const durationMs = performance.now() - t0;

  if (searchResult.isErr) {
    return {
      searchOptions,
      searchResult: null,
      searchError: searchResult.error.message,
      searchMeta: null,
      urlSearch: url.search,
    };
  }

  return {
    searchOptions,
    searchResult: searchResult.value,
    searchError: null,
    searchMeta: {
      totalCount: searchResult.value.totalCount,
      durationMs,
      timestamp: new Date(),
    },
    urlSearch: url.search,
  };
};
