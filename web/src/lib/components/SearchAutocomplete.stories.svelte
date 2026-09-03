<script module>
import { client } from "$lib/api";
import FiltersDecorator from "$lib/stories/FiltersDecorator.svelte";
import { defineMeta } from "@storybook/addon-svelte-csf";
import { expect, mocked, userEvent, within } from "storybook/test";
import { ok } from "true-myth/result";
import SearchAutocomplete from "./SearchAutocomplete.svelte";

const subjects = [
  { code: "CS", description: "Computer Science", filterValue: "CS" },
  { code: "MAT", description: "Mathematics", filterValue: "MAT" },
  { code: "ENG", description: "English", filterValue: "ENG" },
  { code: "PHY", description: "Physics", filterValue: "PHY" },
];

const suggestions = {
  courses: [
    {
      subject: "CS",
      courseNumber: "3443",
      title: "Application Programming",
      sectionCount: 4,
      score: 0.8,
    },
  ],
  instructors: [
    { id: 1001, slug: "john-smith-abc", displayName: "John Smith", sectionCount: 3, score: 0.6 },
  ],
};

const { Story } = defineMeta({
  title: "Components/SearchAutocomplete",
  component: SearchAutocomplete,
  tags: ["autodocs"],
  parameters: {
    layout: "padded",
  },
  beforeEach: () => {
    mocked(client.suggest).mockResolvedValue(ok(suggestions));
  },
  decorators: [
    (storyFn) => {
      storyFn();
      return { Component: FiltersDecorator };
    },
  ],
});
</script>

<Story name="Default" args={{ subjects, selectedTerm: "fall-2026" }} />

<Story
  name="Suggestions"
  args={{ subjects, selectedTerm: "fall-2026" }}
  play={async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const input = canvas.getByRole("combobox");

    await userEvent.click(input);
    await userEvent.type(input, "comp");

    // The popover is portalled, so it lands outside the story canvas. It flies
    // in over 150ms, so assert presence rather than racing the transition.
    const popover = within(document.body);
    await expect(await popover.findByText("Computer Science")).toBeInTheDocument();
    await expect(await popover.findByText("Application Programming")).toBeInTheDocument();
  }}
/>
