import type { SearchResponse } from "$lib/api";
import { BannerApiClient } from "$lib/api";
import type { SearchOptionsResponse } from "$lib/bindings";
import { parseFilters, toAPIParams } from "$lib/filters";
import { parseSort } from "$lib/sort";
import { unresolvedSlugs } from "$lib/stores/instructor-names";
import type { PageLoad } from "./$types";

/** Rows per request. Returned below so the pager counts against what was asked. */
const PAGE_SIZE = 25;

interface SearchMeta {
  totalCount: number;
  durationMs: number;
  timestamp: Date;
}

/**
 * The page's data, with every field stated once.
 *
 * Each failure returns the same shape as success so the page reads one set of
 * fields, and an omitted field is the empty case rather than a missing key.
 */
function pageData(
  url: URL,
  parts: {
    searchOptions?: SearchOptionsResponse | null;
    resolvedInstructors?: Record<string, string>;
    searchResult?: SearchResponse | null;
    searchError?: string | null;
    searchMeta?: SearchMeta | null;
  }
) {
  return {
    searchOptions: parts.searchOptions ?? null,
    resolvedInstructors: parts.resolvedInstructors ?? {},
    searchResult: parts.searchResult ?? null,
    searchError: parts.searchError ?? null,
    searchMeta: parts.searchMeta ?? null,
    urlSearch: url.search,
    pageSize: PAGE_SIZE,
  };
}

export const load: PageLoad = async ({ url, fetch }) => {
  const client = new BannerApiClient(undefined, fetch);
  const urlTerm = url.searchParams.get("term");

  const optionsResult = await client.getSearchOptions(urlTerm ?? undefined);
  if (optionsResult.isErr) {
    const { code, message } = optionsResult.error;
    console.error(`Failed to load search options [${code}]: ${message}`);
    return pageData(url, { searchError: `Failed to load search options: ${message}` });
  }

  const searchOptions = optionsResult.value;
  const defaultTerm = searchOptions.terms[0]?.slug ?? "";
  const validSubjects = new Set(searchOptions.subjects.map((s) => s.code));

  // A server render knows no names and resolves every slug; a client navigation
  // after an autocomplete pick already holds them and asks for nothing.
  const pending = unresolvedSlugs(url.searchParams.getAll("instructor"));
  let resolvedInstructors: Record<string, string> = {};
  if (pending.length > 0) {
    const resolveResult = await client.resolveInstructors(pending);
    // A failure here costs the chips their names, not the results: the slug
    // reads poorly but still says which instructor is being filtered on.
    if (resolveResult.isOk) {
      resolvedInstructors = resolveResult.value;
    }
  }

  const filters = parseFilters(url.searchParams, validSubjects);

  const apiParams = toAPIParams(filters, {
    term: urlTerm ?? defaultTerm,
    limit: PAGE_SIZE,
    offset: Number(url.searchParams.get("offset")) || 0,
    sorting: parseSort(url.searchParams.get("sort")),
  });

  const t0 = performance.now();
  const searchResult = await client.searchCourses(apiParams);
  const durationMs = performance.now() - t0;

  if (searchResult.isErr) {
    return pageData(url, {
      searchOptions,
      resolvedInstructors,
      searchError: searchResult.error.message,
    });
  }

  return pageData(url, {
    searchOptions,
    resolvedInstructors,
    searchResult: searchResult.value,
    searchMeta: {
      totalCount: searchResult.value.totalCount,
      durationMs,
      timestamp: new Date(),
    },
  });
};
