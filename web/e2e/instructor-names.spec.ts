import { expect, test } from "./fixtures";

/** The slug the stub knows a name for, as it appears in a shared link. */
const SLUG = "john-smith-abc";
const NAME = "John Smith";

test("a filtered instructor is named rather than shown as a slug", async ({ page }) => {
  await page.goto(`/?instructor=${SLUG}`);

  await expect(page.getByText(NAME).first()).toBeVisible();
});

test("the name survives leaving the page and coming back", async ({ page }) => {
  await page.goto(`/?instructor=${SLUG}`);
  await expect(page.getByText(NAME).first()).toBeVisible();

  // Both moves stay inside the one tab, so what the page learned on arrival is
  // still around. A reload would rebuild it from nothing and pass either way.
  await page.getByRole("link", { name: /Timeline/i }).click();
  await expect(page).toHaveURL(/\/timeline$/);
  await page.goBack();

  // The page is built fresh here, while its load asked for no name it had
  // already resolved. Knowing only that load's answer leaves the bare slug.
  await expect(page).toHaveURL(new RegExp(`instructor=${SLUG}`));
  await expect(page.getByText(NAME).first()).toBeVisible();
  await expect(page.getByText(SLUG)).toHaveCount(0);
});
