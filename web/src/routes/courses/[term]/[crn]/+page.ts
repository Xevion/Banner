import { BannerApiClient } from "$lib/api";
import { error } from "@sveltejs/kit";
import type { PageLoad } from "./$types";

export const prerender = false;

export const load: PageLoad = async ({ params, fetch }) => {
  const client = new BannerApiClient(undefined, fetch);

  const courseResult = await client.getCourse(params.term, params.crn);

  if (courseResult.isErr) {
    if (courseResult.error.isNotFound()) {
      error(404, "Section not found");
    }
    error(500, courseResult.error.message);
  }

  return {
    course: courseResult.value,
    term: params.term,
  };
};
