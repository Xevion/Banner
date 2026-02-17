import { BannerApiClient } from "$lib/api";
import type { PageLoad } from "./$types";

export const load: PageLoad = async ({ fetch }) => {
  const client = new BannerApiClient(undefined, fetch);

  const [instructorsResult, subjectsResult] = await Promise.all([
    client.getAdminInstructors({ page: 1, perPage: 25 }),
    client.getReference("subject"),
  ]);

  return {
    instructors: instructorsResult.isOk ? instructorsResult.value : null,
    subjects: subjectsResult.isOk ? subjectsResult.value : [],
    error: instructorsResult.isErr ? instructorsResult.error.message : null,
  };
};
