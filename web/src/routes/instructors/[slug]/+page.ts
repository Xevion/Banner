import { BannerApiClient } from "$lib/api";
import { error, redirect } from "@sveltejs/kit";
import type { PageLoad } from "./$types";

export const prerender = false;

export const load: PageLoad = async ({ params, fetch }) => {
  const client = new BannerApiClient(undefined, fetch);

  const [profileResult, searchOptionsResult] = await Promise.all([
    client.getInstructor(params.slug),
    client.getSearchOptions(),
  ]);

  if (profileResult.isErr) {
    if (profileResult.error.isNotFound()) {
      error(404, "Instructor not found");
    }
    error(500, profileResult.error.message);
  }

  const profile = profileResult.value;

  // The API resolves numeric ids and absorbed slugs too, so land those on the canonical URL.
  if (profile.instructor.slug !== params.slug) {
    redirect(308, `/instructors/${profile.instructor.slug}`);
  }

  if (searchOptionsResult.isErr) {
    console.warn("Failed to load search options:", searchOptionsResult.error.message);
  }
  const searchOptions = searchOptionsResult.isOk ? searchOptionsResult.value : null;

  // Fetch sections for the instructor's most recent known term
  const allTerms = searchOptions?.terms ?? [];
  const instructorTermSlugs = new Set(profile.teachingHistory.map((h) => h.termSlug));
  const instructorTerms = allTerms.filter((t) => instructorTermSlugs.has(t.slug));
  const defaultTerm = instructorTerms[0]?.slug ?? null;
  let initialSections = null;
  if (defaultTerm) {
    const sectionsResult = await client.getInstructorSections(params.slug, defaultTerm);
    if (sectionsResult.isOk) {
      initialSections = sectionsResult.value;
    } else {
      console.warn("Failed to load initial sections:", sectionsResult.error.message);
    }
  }

  return {
    profile,
    searchOptions,
    initialSections,
    initialTerm: defaultTerm ?? null,
    slug: params.slug,
  };
};
