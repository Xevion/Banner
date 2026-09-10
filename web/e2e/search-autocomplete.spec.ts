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

test("a course suggestion applies both its subject and its title", async ({ page }) => {
  await page.goto("/");

  const search = page.locator(SEARCH);
  await search.click();
  await search.pressSequentially("appl", { delay: 40 });

  const suggestions = page.locator("#search-autocomplete-list");
  await expect(suggestions).toBeVisible();
  await suggestions.getByText("Application Programming").click();

  // The pieces of a course used to be packed into one string and taken apart
  // again on the way out, which quietly dropped whatever it failed to parse.
  await expect(page).toHaveURL(/query=Application\+Programming/);
  await expect(page).toHaveURL(/subject=CS/);
});

test("enter searches for what was typed when no suggestions are showing", async ({ page }) => {
  await page.goto("/");

  const search = page.locator(SEARCH);
  await search.click();
  await search.pressSequentially("compilers", { delay: 40 });

  // Escape leaves the text but takes the list away, so there is nothing to
  // choose and Enter has to fall back to searching for the words themselves.
  await search.press("Escape");
  await expect(page.locator("#search-autocomplete-list")).toBeHidden();

  await search.press("Enter");
  await expect(page).toHaveURL(/query=compilers/);
});

test("enter takes the highlighted suggestion rather than the raw text", async ({ page }) => {
  await page.goto("/");

  const search = page.locator(SEARCH);
  await search.click();
  await search.pressSequentially("comp", { delay: 40 });
  await expect(page.locator("#search-autocomplete-list")).toBeVisible();

  await search.press("Enter");
  await page.waitForTimeout(SETTLE_MS);

  // With a list up, Enter belongs to the list. Searching for "comp" instead
  // would throw away the suggestion the user had already arrowed onto.
  await expect(search).toHaveValue("");
  await expect(page).not.toHaveURL(/query=comp(&|$)/);
});

test("the arrow keys move through the suggestions", async ({ page }) => {
  await page.goto("/");

  const search = page.locator(SEARCH);
  await search.click();
  await search.pressSequentially("comp", { delay: 40 });

  // The local subject matches show first and the server's arrive a debounce
  // later, so arrowing before then walks a list that is still growing.
  const items = page.locator("#search-autocomplete-list [data-command-item]");
  await expect(items).toHaveCount(3);

  const highlighted = page.locator("#search-autocomplete-list [data-command-item][data-selected]");
  const first = await highlighted.getAttribute("data-value");

  await search.press("ArrowDown");
  const second = await highlighted.getAttribute("data-value");
  expect(second).not.toBe(first);

  await search.press("ArrowUp");
  await expect(highlighted).toHaveAttribute("data-value", first ?? "");
});

test("an instructor already filtered on is not offered again", async ({ page }) => {
  // The stub answers every query with the same instructor, so once that one is
  // applied the search must stop offering it rather than re-adding it.
  await page.goto("/?instructor=john-smith-abc");

  const search = page.locator(SEARCH);
  await search.click();
  await search.pressSequentially("smith", { delay: 40 });

  const suggestions = page.locator("#search-autocomplete-list");
  await expect(suggestions).toBeVisible();
  await expect(suggestions.getByText("John Smith")).toHaveCount(0);
});

test.describe("when the suggestion request fails", () => {
  test.use({ allowedFailures: [/\/api\/suggest/] });

  test("the failure is reported instead of being shown as no results", async ({ page }) => {
    await page.route("**/api/suggest*", (route) =>
      route.fulfill({ status: 500, contentType: "application/json", body: '{"message":"boom"}' })
    );
    await page.goto("/");

    const search = page.locator(SEARCH);
    await search.click();
    await search.pressSequentially("comp", { delay: 40 });

    const suggestions = page.locator("#search-autocomplete-list");
    await expect(suggestions).toBeVisible();
    // "No results found." for a broken request answers a question the user
    // never asked, and hides that anything is wrong at all.
    await expect(suggestions.getByRole("alert")).toBeVisible();
    await expect(suggestions).not.toContainText("No results found.");
  });
});
