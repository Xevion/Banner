import { BannerApiClient } from "$lib/api";
import type { PageLoad } from "./$types";

export const prerender = false;

export const load: PageLoad = async ({ params, url, fetch }) => {
  const client = new BannerApiClient(undefined, fetch);

  const termParam = url.searchParams.get("term") ?? undefined;

  const searchOptionsResult = await client.getSearchOptions(termParam);
  const searchOptions = searchOptionsResult.isOk ? searchOptionsResult.value : null;

  // Resolve subject description
  const subjectDescription =
    searchOptions?.subjects.find((s) => s.code === params.subject)?.description ?? null;

  // Use the first term (most recent) if no term specified
  const effectiveTerm = termParam ?? searchOptions?.terms[0]?.slug;

  let searchResult = null;
  if (effectiveTerm) {
    const result = await client.searchCourses({
      term: effectiveTerm,
      subject: [params.subject],
      limit: 100,
    });
    if (result.isOk) {
      searchResult = result.value;
    }
  }

  return {
    searchOptions,
    searchResult,
    subject: params.subject,
    subjectDescription,
    term: effectiveTerm ?? null,
  };
};
