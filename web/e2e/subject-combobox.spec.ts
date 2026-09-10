import { expect, test } from "./fixtures";

/** Outlasts both a close animation and any deferred close a handler may queue. */
const SETTLE_MS = 400;

/** The filter bar mounts a mobile and a desktop copy; only one is on screen. */
const SUBJECT_INPUT = "input[aria-label='Search subjects']:visible";

test("the subject list stays open after a single click", async ({ page }) => {
  await page.goto("/");

  const trigger = page.locator(SUBJECT_INPUT);
  await trigger.click();

  const option = page.getByRole("option", { name: /Computer Science/ });
  await expect(option).toBeVisible();

  await page.waitForTimeout(SETTLE_MS);
  await expect(option).toBeVisible();
});

test("the subject box stays empty as selections accumulate", async ({ page }) => {
  await page.goto("/");

  const input = page.locator(SUBJECT_INPUT);
  await input.click();

  // Left alone, bits-ui writes the label of whatever was last picked into the
  // input, which reads as a search still in progress and filters the next pick.
  await page.getByRole("option", { name: /Computer Science/ }).click();
  await expect(input).toHaveValue("");

  await page.getByRole("option", { name: /Mathematics/ }).click();
  await expect(input).toHaveValue("");
});

test("a subject search is abandoned when the list closes", async ({ page }) => {
  await page.goto("/");

  const input = page.locator(SUBJECT_INPUT);
  await input.click();
  await input.fill("Comp");

  await page.locator("input[placeholder^='Search courses']:visible").click();
  await page.waitForTimeout(SETTLE_MS);

  // Reopening on leftover text would show a filtered list with no sign of why.
  await expect(input).toHaveValue("");
});

test("clicking outside closes the subject list", async ({ page }) => {
  await page.goto("/");

  const trigger = page.locator(SUBJECT_INPUT);
  await trigger.click();
  await expect(page.getByRole("option", { name: /Computer Science/ })).toBeVisible();

  // Moving to another field is the ordinary way out of this list, and unlike a
  // bare coordinate it cannot land on whatever happens to be laid out there.
  await page.locator("input[placeholder^='Search courses']:visible").click();
  await page.waitForTimeout(SETTLE_MS);

  await expect(page.getByRole("option", { name: /Computer Science/ })).toBeHidden();
});
