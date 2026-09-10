import { expect, test } from "./fixtures";

/** Outlasts both the blur timeout and the popover's fly transition. */
const SETTLE_MS = 400;

/** The filter bar mounts a mobile and a desktop copy; only one is on screen. */
const SEARCH = "input[placeholder^='Search courses']:visible";

test("suggestions survive clicking back into a search box that already has a query", async ({
  page,
}) => {
  await page.goto("/");

  const search = page.locator(SEARCH);
  await search.click();
  await search.pressSequentially("comp", { delay: 40 });

  const suggestions = page.locator("#search-autocomplete-list");
  await expect(suggestions).toBeVisible();

  // Clicking the input the user is already typing in must not dismiss its own
  // suggestions. This is the whole complaint: type, click, everything vanishes.
  await search.click();
  await page.waitForTimeout(SETTLE_MS);

  await expect(suggestions).toBeVisible();
  await expect(search).toHaveAttribute("aria-expanded", "true");
});

test("suggestions survive leaving a search box and clicking back in", async ({ page }) => {
  await page.goto("/");

  const search = page.locator(SEARCH);
  await search.click();
  await search.pressSequentially("comp", { delay: 40 });
  await expect(page.locator("#search-autocomplete-list")).toBeVisible();

  await page.locator("body").click({ position: { x: 5, y: 5 } });
  await page.waitForTimeout(SETTLE_MS);

  await search.click();
  await page.waitForTimeout(SETTLE_MS);

  await expect(page.locator("#search-autocomplete-list")).toBeVisible();
  await expect(search).toHaveAttribute("aria-expanded", "true");
});

test("escape closes the suggestions but keeps the typed query", async ({ page }) => {
  await page.goto("/");

  const search = page.locator(SEARCH);
  await search.click();
  await search.pressSequentially("comp", { delay: 40 });
  await expect(page.locator("#search-autocomplete-list")).toBeVisible();

  await search.press("Escape");
  await page.waitForTimeout(SETTLE_MS);

  await expect(page.locator("#search-autocomplete-list")).toBeHidden();
  await expect(search).toHaveValue("comp");
});

test("clicking outside closes the suggestions", async ({ page }) => {
  await page.goto("/");

  const search = page.locator(SEARCH);
  await search.click();
  await search.pressSequentially("comp", { delay: 40 });
  await expect(page.locator("#search-autocomplete-list")).toBeVisible();

  // Nothing closes on blur any more, so this is the dismissal path that has to
  // carry the weight.
  await page.locator("input[aria-label='Select term']:visible").click();
  await page.waitForTimeout(SETTLE_MS);

  await expect(page.locator("#search-autocomplete-list")).toBeHidden();
});

test("choosing a suggestion applies it as a filter and closes the list", async ({ page }) => {
  await page.goto("/");

  const search = page.locator(SEARCH);
  await search.click();
  await search.pressSequentially("comp", { delay: 40 });

  const suggestions = page.locator("#search-autocomplete-list");
  await expect(suggestions).toBeVisible();
  await suggestions.getByText("Computer Science").click();
  await page.waitForTimeout(SETTLE_MS);

  await expect(suggestions).toBeHidden();
  // Selecting has to clear the box, or the applied filter and the text disagree.
  await expect(search).toHaveValue("");
});
