/**
 * Keyboard behaviour shared by every filter-bar text box.
 *
 * These boxes each wrap an `<input>` in popover machinery that can take focus
 * away from it. When that happens the box still looks focused while the
 * keystrokes go somewhere else, so the checks here are about the plain editing
 * a text field owes its user: type, select all, replace, erase, tab out.
 */
import type { Locator } from "@playwright/test";
import { expect, test } from "./fixtures";

const SETTLE_MS = 400;

/** The filter bar mounts a mobile and a desktop copy; only one is on screen. */
const TERM = "input[aria-label='Select term']:visible";
const SUBJECT = "input[aria-label='Search subjects']:visible";
const SEARCH = "input[placeholder^='Search courses']:visible";

const BOXES = [
  {
    name: "term selector",
    selector: TERM,
    other: SEARCH,
    typed: "Spring",
    replacement: "Fall",
  },
  {
    name: "subject selector",
    selector: SUBJECT,
    other: SEARCH,
    typed: "Comp",
    replacement: "Math",
  },
  {
    name: "course search",
    selector: SEARCH,
    other: TERM,
    typed: "compu",
    replacement: "history",
  },
];

/** Whether the browser's focus is actually inside the given input. */
function isFocused(box: Locator): Promise<boolean> {
  return box.evaluate((el) => el === document.activeElement);
}

for (const box of BOXES) {
  test(`${box.name} accepts typed text and keeps focus`, async ({ page }) => {
    await page.goto("/");

    const input = page.locator(box.selector);
    await input.click();
    await input.pressSequentially(box.typed, { delay: 30 });

    await expect(input).toHaveValue(box.typed);
    // The popover must not pull focus off the field it is attached to.
    expect(await isFocused(input)).toBe(true);
  });

  test(`${box.name} replaces its contents after ctrl+a`, async ({ page }) => {
    await page.goto("/");

    const input = page.locator(box.selector);
    await input.click();
    await input.pressSequentially(box.typed, { delay: 30 });
    await expect(input).toHaveValue(box.typed);

    await input.press("ControlOrMeta+a");
    await input.pressSequentially(box.replacement, { delay: 30 });

    // Appending instead of replacing means the select-all never reached the
    // input, which is what happens when the popover holds focus.
    await expect(input).toHaveValue(box.replacement);
  });

  test(`${box.name} erases characters with backspace`, async ({ page }) => {
    await page.goto("/");

    const input = page.locator(box.selector);
    await input.click();
    await input.pressSequentially(box.typed, { delay: 30 });

    await input.press("Backspace");
    await expect(input).toHaveValue(box.typed.slice(0, -1));
  });

  test(`${box.name} is still editable after leaving it and coming back`, async ({ page }) => {
    await page.goto("/");

    const input = page.locator(box.selector);
    await input.click();
    await input.pressSequentially(box.typed, { delay: 30 });

    await page.locator(box.other).click();
    await page.waitForTimeout(SETTLE_MS);
    await input.click();
    await page.waitForTimeout(SETTLE_MS);

    // Returning to a box that already holds text reopens its list, and the list
    // must not take the focus with it. Losing it here leaves a box that looks
    // active, shows suggestions, and swallows every keystroke.
    expect(await isFocused(input)).toBe(true);

    await input.press("ControlOrMeta+a");
    await input.pressSequentially(box.replacement, { delay: 30 });
    await expect(input).toHaveValue(box.replacement);

    await input.press("Backspace");
    await expect(input).toHaveValue(box.replacement.slice(0, -1));
  });

  test(`${box.name} releases focus on tab`, async ({ page }) => {
    await page.goto("/");

    const input = page.locator(box.selector);
    await input.click();
    await input.pressSequentially(box.typed, { delay: 30 });

    await input.press("Tab");
    await page.waitForTimeout(SETTLE_MS);

    expect(await isFocused(input)).toBe(false);
  });
}
