import { mockCourses } from "../src/lib/stories/fixtures/courses";
import { expect, test } from "./fixtures";

test("renders the course table from the production bundle", async ({ page }) => {
  const scripts: string[] = [];
  page.on("request", (request) => {
    if (request.resourceType() === "script") scripts.push(request.url());
  });

  await page.goto("/");

  const rows = page.locator("[data-search-results] tbody tr");
  await expect(rows).toHaveCount(mockCourses.length);
  await expect(page.getByText("No courses found")).toHaveCount(0);

  // The bundle under test has to be the built one. A dev server serves
  // /@vite/client plus unhashed module paths and no immutable chunks at all.
  expect(scripts.some((url) => url.includes("/_app/immutable/"))).toBe(true);
  expect(scripts.some((url) => url.includes("/@vite/") || url.includes("/@fs/"))).toBe(false);
});

test("opens autocomplete suggestions once the query is long enough", async ({ page }) => {
  await page.goto("/");

  // The filter bar mounts a mobile and a desktop copy; only one is on screen.
  const search = page.locator("input[placeholder^='Search courses']:visible");
  await search.click();
  await search.pressSequentially("comp", { delay: 40 });

  const suggestions = page.locator("#search-autocomplete-list");
  await expect(suggestions).toBeVisible();

  // The subject row can only come from the client-side fuzzy match over the
  // subject list; the course row proves the suggest request came back.
  await expect(suggestions.getByText("Computer Science")).toBeVisible();
  await expect(suggestions.getByText("Application Programming")).toBeVisible();
});
