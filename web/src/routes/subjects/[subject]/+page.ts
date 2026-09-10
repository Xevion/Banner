import { BannerApiClient } from "$lib/api";
import { error } from "@sveltejs/kit";
import type { PageLoad } from "./$types";

export const prerender = false;

export const load: PageLoad = async ({ params, url, fetch }) => {
  const client = new BannerApiClient(undefined, fetch);

  const termParam = url.searchParams.get("term") ?? undefined;

  // Unscoped on purpose: the term-scoped subject list omits subjects with no
  // sections that term, so a name looked up there vanishes for the terms a
  // subject sat out.
  const searchOptionsResult = await client.getSearchOptions();
  if (!searchOptionsResult.isOk) {
    error(503, "Course data is unavailable right now. Please try again shortly.");
  }
  const searchOptions = searchOptionsResult.value;

  const subject = searchOptions.subjects.find((s) => s.code === params.subject);
  if (!subject) {
    error(404, `There is no subject with the code "${params.subject}".`);
  }

  // Falling back to the newest term keeps a bare /subjects/ISC useful.
  const effectiveTerm = termParam ?? searchOptions.terms[0]?.slug;

  let searchResult = null;
  let searchError: string | null = null;
  if (effectiveTerm) {
    const result = await client.searchCourses({
      term: effectiveTerm,
      subject: [params.subject],
      limit: 100,
    });
    if (result.isOk) {
      searchResult = result.value;
    } else {
      // Kept out of `error()` on purpose: the subject and term picker are still
      // usable, and a failed lookup must not read as a term with no sections.
      searchError = result.error.message ?? "Could not load sections for this term.";
    }
  }

  return {
    searchOptions,
    searchResult,
    searchError,
    subject: params.subject,
    subjectDescription: subject.description,
    term: effectiveTerm ?? null,
  };
};
