/**
 * The instructor filter, which lives behind the "More" popover and so had no
 * coverage at all. Its list is inline rather than portalled, so it dismisses on
 * focus leaving the widget rather than through a popover layer.
 */
import type { Page } from "@playwright/test";
import { expect, test } from "./fixtures";

const SETTLE_MS = 400;
const INSTRUCTOR_INPUT = "input[aria-label='Search instructors']:visible";

async function openMore(page: Page) {
  await page.goto("/");
  await page.getByRole("button", { name: "More" }).click();
  await expect(page.locator(INSTRUCTOR_INPUT)).toBeVisible();
}

test("instructor suggestions stay open while typing", async ({ page }) => {
  await openMore(page);

  const input = page.locator(INSTRUCTOR_INPUT);
  await input.click();
  await input.pressSequentially("smi", { delay: 40 });

  const list = page.locator("#instructor-autocomplete-list");
  await expect(list).toBeVisible();

  // The list used to close on a 150ms timer started by any blur at all.
  await page.waitForTimeout(SETTLE_MS);
  await expect(list).toBeVisible();
  await expect(input).toHaveAttribute("aria-expanded", "true");
});

test("the instructor box stays editable after typing", async ({ page }) => {
  await openMore(page);

  const input = page.locator(INSTRUCTOR_INPUT);
  await input.click();
  await input.pressSequentially("smi", { delay: 40 });
  await page.waitForTimeout(SETTLE_MS);

  await input.press("ControlOrMeta+a");
  await input.pressSequentially("jones", { delay: 40 });
  await expect(input).toHaveValue("jones");
});

test.describe("when the instructor request fails", () => {
  test.use({ allowedFailures: [/\/api\/instructors\/suggest/] });

  test("the failure is reported instead of being shown as no results", async ({ page }) => {
    await page.route("**/api/instructors/suggest*", (route) =>
      route.fulfill({ status: 500, contentType: "application/json", body: '{"message":"boom"}' })
    );
    await openMore(page);

    const input = page.locator(INSTRUCTOR_INPUT);
    await input.click();
    await input.pressSequentially("smi", { delay: 40 });

    const list = page.locator("#instructor-autocomplete-list");
    await expect(list).toBeVisible();
    // This box used to drop the error on the floor and claim the instructor did
    // not exist, which is a different answer from "we could not look".
    await expect(list.getByRole("alert")).toBeVisible();
    await expect(list).not.toContainText("No results found.");
  });
});

test("escape closes the instructor suggestions but keeps the query", async ({ page }) => {
  await openMore(page);

  const input = page.locator(INSTRUCTOR_INPUT);
  await input.click();
  await input.pressSequentially("smi", { delay: 40 });
  await expect(page.locator("#instructor-autocomplete-list")).toBeVisible();

  await input.press("Escape");
  await page.waitForTimeout(SETTLE_MS);

  await expect(page.locator("#instructor-autocomplete-list")).toBeHidden();
  await expect(input).toHaveValue("smi");
});
