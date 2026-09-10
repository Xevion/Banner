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

export const test = base.extend<{
  /** Routes a test breaks on purpose, to check how the app reports the failure. */
  allowedFailures: RegExp[];
}>({
  allowedFailures: [[], { option: true }],

  page: async ({ page, allowedFailures }, use) => {
    const isIgnored = (url: string) =>
      [...IGNORED_URLS, ...allowedFailures].some((pattern) => pattern.test(url));

    const problems: string[] = [];

    page.on("console", (message) => {
      if (message.type() !== "error") return;
      // A route a test broke on purpose also makes the browser log the failed
      // load, which says nothing beyond what the test already arranged.
      if (isIgnored(message.location().url)) return;
      problems.push(`console error: ${message.text()}`);
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
