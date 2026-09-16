<script module>
import { defineMeta } from "@storybook/addon-svelte-csf";
import { expect, fn, mocked, userEvent, within } from "storybook/test";
import ErrorPanel from "./ErrorPanel.svelte";

const { Story } = defineMeta({
  title: "Components/ErrorPanel",
  component: ErrorPanel,
  tags: ["autodocs"],
});
</script>

<Story
  name="With Retry"
  args={{
    title: "Couldn't load instructors",
    message: "list instructors failed",
    onRetry: fn(),
  }}
  play={async ({ args, canvasElement }) => {
    const canvas = within(canvasElement);

    await expect(canvas.getByRole("alert")).toBeVisible();
    await expect(canvas.getByText("Couldn't load instructors")).toBeVisible();

    const retry = canvas.getByRole("button", { name: /Retry/i });
    await userEvent.click(retry);
    await expect(mocked(args.onRetry)).toHaveBeenCalled();
  }}
/>

<Story
  name="Without Retry"
  args={{ title: "Couldn't load details", message: "instructor detail failed" }}
  play={async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(canvas.queryByRole("button")).toBeNull();
  }}
/>

<Story
  name="Retrying"
  args={{ title: "Couldn't load instructors", message: "list instructors failed", onRetry: fn(), retrying: true }}
  play={async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(canvas.getByRole("button", { name: /Retry/i })).toBeDisabled();
  }}
/>

<Story name="Title Only" args={{ title: "Couldn't reach the server" }} />
