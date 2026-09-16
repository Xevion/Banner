import { BannerApiClient } from "$lib/api";
import type { CourseResponse } from "$lib/bindings";
import { error } from "@sveltejs/kit";
import type { PageLoad } from "./$types";

export const prerender = false;

export const load: PageLoad = async ({ params, fetch }) => {
  const client = new BannerApiClient(undefined, fetch);

  const [sectionsResult, searchOptionsResult] = await Promise.all([
    client.getRelatedSections(params.term, params.subject, params.number),
    client.getSearchOptions(params.term),
  ]);

  if (sectionsResult.isErr) {
    if (sectionsResult.error.isNotFound()) {
      error(404, "Course not found");
    }
    error(500, sectionsResult.error.message);
  }

  // Destructured so the non-empty invariant survives into the page's own type.
  const [first, ...rest] = sectionsResult.value;
  if (!first) {
    error(404, "Course not found");
  }
  const sections: [CourseResponse, ...CourseResponse[]] = [first, ...rest];

  const searchOptions = searchOptionsResult.isOk ? searchOptionsResult.value : null;

  return {
    sections,
    searchOptions,
    term: params.term,
    subject: params.subject,
    courseNumber: params.number,
  };
};
