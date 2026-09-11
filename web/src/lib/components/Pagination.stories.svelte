<script module>
import { defineMeta } from "@storybook/addon-svelte-csf";
import { expect, fn, within } from "storybook/test";
import Pagination from "./Pagination.svelte";

const { Story } = defineMeta({
  title: "Components/Pagination",
  component: Pagination,
  tags: ["autodocs"],
  parameters: {
    // This is a full-width bar. Centred, it shrinks to its contents and the
    // result count wraps over three lines, which is not how it ever renders.
    layout: "padded",
  },
});

/** The pager names the page it is on, so the story can be held to its title. */
function expectPage(canvasElement, page, of) {
  const canvas = within(canvasElement);
  return expect(
    canvas.getByRole("button", { name: `Page ${page} of ${of}, click to select page` })
  ).toBeVisible();
}
</script>

<Story
  name="First Page"
  args={{ totalCount: 100, offset: 0, limit: 25, onPageChange: fn() }}
  play={async ({ canvasElement }) => await expectPage(canvasElement, 1, 4)}
/>

<Story
  name="Middle Page"
  args={{ totalCount: 100, offset: 50, limit: 25, onPageChange: fn() }}
  play={async ({ canvasElement }) => await expectPage(canvasElement, 3, 4)}
/>

<Story
  name="Last Page"
  args={{ totalCount: 100, offset: 75, limit: 25, onPageChange: fn() }}
  play={async ({ canvasElement }) => await expectPage(canvasElement, 4, 4)}
/>

<Story
  name="Many Pages"
  args={{ totalCount: 1000, offset: 250, limit: 25, onPageChange: fn() }}
  play={async ({ canvasElement }) => await expectPage(canvasElement, 11, 40)}
/>

<Story
  name="Loading"
  args={{ totalCount: 100, offset: 50, limit: 25, loading: true, onPageChange: fn() }}
/>

<!-- The bar hides itself rather than showing a pager with nowhere to go. -->
<Story
  name="Single Page"
  args={{ totalCount: 20, offset: 0, limit: 25, onPageChange: fn() }}
  play={async ({ canvasElement }) =>
    await expect(within(canvasElement).queryAllByRole("button")).toHaveLength(0)}
/>

<Story
  name="Empty"
  args={{ totalCount: 0, offset: 0, limit: 25, onPageChange: fn() }}
  play={async ({ canvasElement }) =>
    await expect(within(canvasElement).queryAllByRole("button")).toHaveLength(0)}
/>

<!-- The other half of the component: prev/next arrows instead of a page menu. -->
<Story
  name="Simple Variant"
  args={{ variant: 'simple', totalCount: 100, currentPage: 2, perPage: 25, onPageChange: fn() }}
  play={async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(canvas.getByRole("button", { name: "Previous page" })).toBeEnabled();
    await expect(canvas.getByRole("button", { name: "Next page" })).toBeEnabled();
  }}
/>

<!-- Both ends of the simple variant stop rather than wrapping around. -->
<Story
  name="Simple Variant, First Page"
  args={{ variant: 'simple', totalCount: 100, currentPage: 1, perPage: 25, onPageChange: fn() }}
  play={async ({ canvasElement }) =>
    await expect(
      within(canvasElement).getByRole("button", { name: "Previous page" })
    ).toBeDisabled()}
/>
