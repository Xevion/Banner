import { expect, test } from "./fixtures";

/** Outlasts both a close animation and any deferred close a handler may queue. */
const SETTLE_MS = 400;

/** The filter bar mounts a mobile and a desktop copy; only one is on screen. */
const TERM_INPUT = "input[aria-label='Select term']:visible";

/** Any term that is not the current one, so its row carries no "current" tag. */
const OTHER_TERM = "Spring 2027";

test("the term selector stays open after a single click", async ({ page }) => {
  await page.goto("/");

  const trigger = page.locator(TERM_INPUT);
  await trigger.click();

  const option = page.getByText(OTHER_TERM, { exact: true });
  await expect(option).toBeVisible();

  // Asserting the moment the list appears proves nothing: it used to open and
  // then close on its own. Only a wait past the close tells the two apart.
  await page.waitForTimeout(SETTLE_MS);

  await expect(option).toBeVisible();
  await expect(trigger).toHaveAttribute("aria-expanded", "true");
});

test("the term list survives rapid clicking without contradicting aria-expanded", async ({
  page,
}) => {
  await page.goto("/");

  const trigger = page.locator(TERM_INPUT);
  for (let i = 0; i < 5; i++) await trigger.click({ delay: 30 });
  await page.waitForTimeout(SETTLE_MS);

  // Whichever way it settles, what the widget reports and what it shows have to
  // agree. A mismatch is the state machine racing itself.
  const expanded = (await trigger.getAttribute("aria-expanded")) === "true";
  const listed = await page.getByText(OTHER_TERM, { exact: true }).isVisible();
  expect(listed).toBe(expanded);
});

test("escape closes the term list", async ({ page }) => {
  await page.goto("/");

  const trigger = page.locator(TERM_INPUT);
  await trigger.click();
  await expect(page.getByText(OTHER_TERM, { exact: true })).toBeVisible();

  await page.keyboard.press("Escape");
  await page.waitForTimeout(SETTLE_MS);

  await expect(page.getByText(OTHER_TERM, { exact: true })).toBeHidden();
  await expect(trigger).toHaveAttribute("aria-expanded", "false");
});

test("clicking outside closes the term list", async ({ page }) => {
  await page.goto("/");

  const trigger = page.locator(TERM_INPUT);
  await trigger.click();
  await expect(page.getByText(OTHER_TERM, { exact: true })).toBeVisible();

  // Moving to another field is the ordinary way out of this list, and unlike a
  // bare coordinate it cannot land on whatever happens to be laid out there.
  await page.locator("input[placeholder^='Search courses']:visible").click();
  await page.waitForTimeout(SETTLE_MS);

  await expect(page.getByText(OTHER_TERM, { exact: true })).toBeHidden();
});

test("typing filters the term list and clearing restores it", async ({ page }) => {
  await page.goto("/");

  const trigger = page.locator(TERM_INPUT);
  await trigger.click();
  await trigger.fill("Spring");

  await expect(page.getByText(OTHER_TERM, { exact: true })).toBeVisible();
  await expect(page.getByText("Fall 2026", { exact: true })).toBeHidden();

  await trigger.fill("");
  await expect(page.getByText(OTHER_TERM, { exact: true })).toBeVisible();
});

test("the term box names the term already in effect instead of the placeholder", async ({
  page,
}) => {
  await page.goto("/");

  const input = page.locator(TERM_INPUT);
  // A term is already applied on load, so leaving the placeholder up says the
  // box is empty when it is not, and invites a re-pick that changes the URL.
  await expect(input).not.toHaveValue("");

  await input.click();
  const selected = await page.getByRole("option", { name: /current/ }).textContent();
  await expect(input).toHaveValue((selected ?? "").replace("current", "").trim());
});

test("the term box still names the selection after the list is dismissed", async ({ page }) => {
  await page.goto("/");

  const input = page.locator(TERM_INPUT);
  const onLoad = await input.inputValue();

  await input.click();
  await input.fill("Spr");
  await page.keyboard.press("Escape");
  await page.waitForTimeout(SETTLE_MS);

  // Abandoning a search must restore the label, not leave the typing behind.
  await expect(input).toHaveValue(onLoad);
});

test("the current term is pinned to the top of the list and marked", async ({ page }) => {
  await page.goto("/");

  await page.locator(TERM_INPUT).click();

  // Naming a term here would rot the moment the fixture rolls forward, so this
  // checks the reordering itself: exactly one term is marked current, and the
  // list leads with it.
  await expect(page.getByRole("option", { name: /current/ })).toHaveCount(1);
  await expect(page.getByRole("option").first()).toContainText("current");
});

test("selecting a term closes the list and shows the chosen label", async ({ page }) => {
  await page.goto("/");

  const trigger = page.locator(TERM_INPUT);
  await trigger.click();
  await page.getByText(OTHER_TERM, { exact: true }).click();
  await page.waitForTimeout(SETTLE_MS);

  await expect(page.getByText(OTHER_TERM, { exact: true })).toBeHidden();
  // The search text has to give way to the selection, not linger in the box.
  await expect(trigger).toHaveValue(OTHER_TERM);
});

test("re-picking the term already in effect leaves it in effect", async ({ page }) => {
  await page.goto("/");

  const input = page.locator(TERM_INPUT);
  const before = await input.inputValue();

  await input.click();
  await page.getByRole("option", { name: /current/ }).click();
  await page.waitForTimeout(SETTLE_MS);

  // Choosing what is already chosen must not round-trip through "nothing
  // selected", which empties the box and re-applies the term as a fresh pick.
  await expect(input).toHaveValue(before);
});

test("clicking the term already in effect cannot deselect it", async ({ page }) => {
  await page.goto("/");

  const input = page.locator(TERM_INPUT);
  await input.click();

  const current = page.getByRole("option", { name: /current/ });
  await expect(current).toHaveAttribute("aria-selected", "true");

  // Picking the active term is a no-op, never a toggle back to nothing chosen.
  // The selection has to survive in place, not just be restored on reopen.
  await current.click();
  await page.waitForTimeout(SETTLE_MS);
  await expect(current).toHaveAttribute("aria-selected", "true");
  await expect(input).toHaveValue(
    await current.textContent().then((t) => (t ?? "").replace("current", "").trim())
  );
});

test("the term list marks its selection without a toggle affordance", async ({ page }) => {
  await page.goto("/");

  await page.locator(TERM_INPUT).click();

  // A checkbox-like tick invites clicking it off again. Selection is shown by
  // the row itself, so there is nothing in the row suggesting it can be undone.
  await expect(page.getByRole("option", { name: /current/ }).locator("svg")).toHaveCount(0);
});
