import type { SortColumn, SortDirection } from "$lib/bindings";
import type { SearchParams } from "$lib/api";
import type { SortingState } from "@tanstack/table-core";
import { createContext } from "svelte";
import { SvelteURLSearchParams } from "svelte/reactivity";

/**
 * Extract just the filter-related fields from SearchParams.
 * Excludes pagination (limit, offset) and sorting (sortBy, sortDir) which are managed separately.
 */
type FilterFields = Omit<SearchParams, "term" | "limit" | "offset" | "sortBy" | "sortDir">;

/**
 * Helper to parse a URL param as a number or null
 */
function parseIntOrNull(value: string | null): number | null {
  if (value === null || value === "") return null;
  const n = Number(value);
  return Number.isNaN(n) ? null : n;
}

/**
 * Centralized search filter state management using Svelte 5 runes.
 * Consolidates all filter state and provides methods for URL/API serialization.
 */
export class SearchFilters implements FilterFields {
  // Selection filters
  subject = $state<string[]>([]);
  query = $state<string | null>(null);

  // Status filters
  openOnly = $state(false);
  waitCountMax = $state<number | null>(null);

  // Schedule filters
  days = $state<string[]>([]);
  timeStart = $state<string | null>(null);
  timeEnd = $state<string | null>(null);

  // Format & location
  instructionalMethod = $state<string[]>([]);
  campus = $state<string[]>([]);
  partOfTerm = $state<string[]>([]);

  // Catalog filters
  attributes = $state<string[]>([]);
  creditHourMin = $state<number | null>(null);
  creditHourMax = $state<number | null>(null);
  instructor = $state("");
  courseNumberLow = $state<number | null>(null);
  courseNumberHigh = $state<number | null>(null);

  /**
   * Count of active (non-default) filters
   */
  get activeCount(): number {
    return [
      this.subject.length > 0,
      this.openOnly,
      this.waitCountMax !== null,
      this.days.length > 0,
      this.timeStart !== null || this.timeEnd !== null,
      this.instructionalMethod.length > 0,
      this.campus.length > 0,
      this.partOfTerm.length > 0,
      this.attributes.length > 0,
      this.creditHourMin !== null || this.creditHourMax !== null,
      this.instructor !== "",
      this.courseNumberLow !== null || this.courseNumberHigh !== null,
    ].filter(Boolean).length;
  }

  /**
   * Whether all filters are at their default values
   */
  get isEmpty(): boolean {
    return this.activeCount === 0;
  }

  /**
   * Hydrates filter state from URL search params.
   * Validates subjects against the provided set if given.
   */
  fromURLParams(params: URLSearchParams, validSubjects?: Set<string>): void {
    const subjects = params.getAll("subject");
    this.subject = validSubjects ? subjects.filter((s) => validSubjects.has(s)) : subjects;

    // Support both old 'q' and new 'query' param names
    this.query = params.get("query") ?? params.get("q") ?? null;
    this.openOnly = params.get("open") === "true";
    this.waitCountMax = parseIntOrNull(params.get("wait_count_max"));
    this.days = params.getAll("days");
    this.timeStart = params.get("time_start");
    this.timeEnd = params.get("time_end");
    this.instructionalMethod = params.getAll("instructional_method");
    this.campus = params.getAll("campus");
    this.partOfTerm = params.getAll("part_of_term");
    this.attributes = params.getAll("attributes");
    this.creditHourMin = parseIntOrNull(params.get("credit_hour_min"));
    this.creditHourMax = parseIntOrNull(params.get("credit_hour_max"));
    this.instructor = params.get("instructor") ?? "";
    this.courseNumberLow = parseIntOrNull(params.get("course_number_low"));
    this.courseNumberHigh = parseIntOrNull(params.get("course_number_high"));
  }

  /**
   * Serializes filter state to URL search params.
   * Only includes non-default values to keep URLs clean.
   */
  toURLParams(): SvelteURLSearchParams {
    const params = new SvelteURLSearchParams();

    this.subject.forEach((s) => params.append("subject", s));
    if (this.query) params.set("query", this.query);
    if (this.openOnly) params.set("open", "true");
    if (this.waitCountMax !== null) params.set("wait_count_max", String(this.waitCountMax));
    this.days.forEach((d) => params.append("days", d));
    if (this.timeStart) params.set("time_start", this.timeStart);
    if (this.timeEnd) params.set("time_end", this.timeEnd);
    this.instructionalMethod.forEach((m) => params.append("instructional_method", m));
    this.campus.forEach((c) => params.append("campus", c));
    this.partOfTerm.forEach((p) => params.append("part_of_term", p));
    this.attributes.forEach((a) => params.append("attributes", a));
    if (this.creditHourMin !== null) params.set("credit_hour_min", String(this.creditHourMin));
    if (this.creditHourMax !== null) params.set("credit_hour_max", String(this.creditHourMax));
    if (this.instructor) params.set("instructor", this.instructor);
    if (this.courseNumberLow !== null)
      params.set("course_number_low", String(this.courseNumberLow));
    if (this.courseNumberHigh !== null)
      params.set("course_number_high", String(this.courseNumberHigh));

    return params;
  }

  /**
   * Converts filter state to API search params.
   * Combines filters with pagination and sorting parameters.
   */
  toAPIParams(term: string, limit: number, offset: number, sorting: SortingState): SearchParams {
    const sortBy: SortColumn | null = sorting.length > 0 ? (sorting[0].id as SortColumn) : null;
    const sortDir: SortDirection | null =
      sorting.length > 0 ? (sorting[0].desc ? "desc" : "asc") : null;

    return {
      term,
      limit,
      offset,
      sortBy,
      sortDir,
      subject: this.subject,
      query: this.query,
      openOnly: this.openOnly,
      waitCountMax: this.waitCountMax,
      days: this.days,
      timeStart: this.timeStart,
      timeEnd: this.timeEnd,
      instructionalMethod: this.instructionalMethod,
      campus: this.campus,
      partOfTerm: this.partOfTerm,
      attributes: this.attributes,
      creditHourMin: this.creditHourMin,
      creditHourMax: this.creditHourMax,
      instructor: this.instructor || null,
      courseNumberLow: this.courseNumberLow,
      courseNumberHigh: this.courseNumberHigh,
    };
  }

  /**
   * Builds a search key string for detecting filter changes.
   * Used to determine when to trigger a new search.
   */
  toSearchKey(): string {
    return [
      this.subject.join(","),
      this.query,
      this.openOnly,
      this.waitCountMax,
      this.days.join(","),
      this.timeStart,
      this.timeEnd,
      this.instructionalMethod.join(","),
      this.campus.join(","),
      this.partOfTerm.join(","),
      this.attributes.join(","),
      this.creditHourMin,
      this.creditHourMax,
      this.instructor,
      this.courseNumberLow,
      this.courseNumberHigh,
    ].join("|");
  }

  /**
   * Resets all filters to their default values
   */
  clear(): void {
    this.subject = [];
    this.query = null;
    this.openOnly = false;
    this.waitCountMax = null;
    this.days = [];
    this.timeStart = null;
    this.timeEnd = null;
    this.instructionalMethod = [];
    this.campus = [];
    this.partOfTerm = [];
    this.attributes = [];
    this.creditHourMin = null;
    this.creditHourMax = null;
    this.instructor = "";
    this.courseNumberLow = null;
    this.courseNumberHigh = null;
  }
}

export const [getFiltersContext, setFiltersContext] = createContext<SearchFilters>();
