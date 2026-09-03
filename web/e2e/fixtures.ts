/**
 * Shared end-to-end fixtures.
 *
 * Every test built on this `test` export fails when the browser logged an error
 * or a request did not come back, which is what catches bundling faults that
 * only appear in the production build.
 */
import { test as base, expect } from "@playwright/test";

/** Requests whose failure says nothing about the app. */
const IGNORED_URLS = [/\/api\/csp-report$/];

/** Aborts are routine: hovering a link starts a preload the router then drops. */
const IGNORED_FAILURES = ["net::ERR_ABORTED"];

function isIgnored(url: string): boolean {
  return IGNORED_URLS.some((pattern) => pattern.test(url));
}

export const test = base.extend({
  page: async ({ page }, use) => {
    const problems: string[] = [];

    page.on("console", (message) => {
      if (message.type() === "error") problems.push(`console error: ${message.text()}`);
    });

    page.on("pageerror", (error) => {
      problems.push(`uncaught exception: ${error.message}`);
    });

    page.on("requestfailed", (request) => {
      const reason = request.failure()?.errorText ?? "unknown";
      if (IGNORED_FAILURES.includes(reason) || isIgnored(request.url())) return;
      problems.push(`request failed: ${request.url()} (${reason})`);
    });

    page.on("response", (response) => {
      if (response.status() < 400 || isIgnored(response.url())) return;
      problems.push(`request returned ${response.status()}: ${response.url()}`);
    });

    await use(page);

    expect(problems, "the browser reported no errors").toEqual([]);
  },
});

export { expect } from "@playwright/test";
