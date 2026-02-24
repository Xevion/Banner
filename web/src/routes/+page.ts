import { BannerApiClient } from "$lib/api";
import { parseFilters, toAPIParams } from "$lib/filters";
import type { PageLoad } from "./$types";

export const load: PageLoad = async ({ url, fetch }) => {
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

  const validSubjects = new Set(searchOptions.subjects.map((s) => s.code));
  const filters = parseFilters(url.searchParams, validSubjects);

  const offset = Number(url.searchParams.get("offset")) || 0;
  const sortBy = url.searchParams.get("sort_by");
  const sortDir = url.searchParams.get("sort_dir");
  const sorting = sortBy ? [{ id: sortBy, desc: sortDir === "desc" }] : [];

  const apiParams = toAPIParams(filters, {
    term: url.searchParams.get("term") ?? defaultTerm,
    limit: 25,
    offset,
    sorting,
  });

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
