import { BannerApiClient } from "$lib/api";
import type { TrendSample } from "$lib/bindings";

/**
 * Enrollment trends for the sections currently on screen.
 *
 * Fetched separately from search so a decoration never delays the results themselves,
 * and cached per term so paging back to a page already seen costs nothing.
 */
class CourseTrends {
  private samples = $state<Record<string, TrendSample[]>>({});
  private requested = new Set<string>();

  private key(term: string, crn: string): string {
    return `${term}:${crn}`;
  }

  get(term: string, crn: string): TrendSample[] | undefined {
    return this.samples[this.key(term, crn)];
  }

  /** Fetch trends for any of these CRNs not already loaded or in flight. */
  async load(term: string, crns: string[]): Promise<void> {
    const missing = crns.filter((crn) => !this.requested.has(this.key(term, crn)));
    if (missing.length === 0) return;

    for (const crn of missing) this.requested.add(this.key(term, crn));

    const result = await new BannerApiClient().getCourseTrends(term, missing);
    result.match({
      Ok: (data) => {
        for (const [crn, points] of Object.entries(data.trends)) {
          if (points) this.samples[this.key(term, crn)] = points;
        }
      },
      Err: () => {
        // Let a failed batch be retried the next time these rows are shown.
        for (const crn of missing) this.requested.delete(this.key(term, crn));
      },
    });
  }
}

export const courseTrends = new CourseTrends();
