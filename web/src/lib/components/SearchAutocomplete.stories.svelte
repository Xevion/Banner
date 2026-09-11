<script module>
import { client } from "$lib/api";
import FiltersDecorator from "$lib/stories/FiltersDecorator.svelte";
import { defineMeta } from "@storybook/addon-svelte-csf";
import { expect, mocked, userEvent, waitFor, within } from "storybook/test";
import { err, ok } from "true-myth/result";
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

const baseArgs = { subjects, selectedTerm: "fall-2026" };

const { Story } = defineMeta({
  title: "Components/SearchAutocomplete",
  component: SearchAutocomplete,
  tags: ["autodocs"],
  parameters: {
    layout: "padded",
    docs: {
      // Every state this component has lives in a list that only opens once
      // something has been typed. Without autoplay the docs page is six
      // identical empty boxes.
      story: { autoplay: true, height: "320px" },
    },
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

/** Types a query and hands back the portalled list the component opens. */
async function search(canvasElement, query = "comp") {
  const input = within(canvasElement).getByRole("combobox");
  await userEvent.click(input);
  await userEvent.type(input, query);
  const list = within(document.body);
  // The rows fly in over 150ms, so wait rather than sample the moment they mount.
  await waitFor(() => expect(document.getElementById("search-autocomplete-list")).not.toBeNull());
  return list;
}

/** Replaces the stubbed answer for the query this story is about to type. */
function answerWith(result) {
  mocked(client.suggest).mockReturnValue(Promise.resolve(result));
}

/** A request that never settles, so the loading state stays put to be read. */
function neverAnswer() {
  mocked(client.suggest).mockReturnValue(new Promise(() => {}));
}
</script>

<Story name="Default" args={baseArgs} parameters={{ docs: { story: { autoplay: false, height: "120px" } } }} />

<!-- Subjects match locally, courses and instructors come from the server. -->
<Story
  name="Suggestions"
  args={baseArgs}
  play={async ({ canvasElement }) => {
    const list = await search(canvasElement);
    await expect(await list.findByText("Computer Science")).toBeInTheDocument();
    await expect(await list.findByText("Application Programming")).toBeInTheDocument();
    await expect(await list.findByText("John Smith")).toBeInTheDocument();
  }}
/>

<!-- Ranked together by match quality rather than grouped by kind. -->
<Story
  name="Ranked Together"
  args={baseArgs}
  play={async ({ canvasElement }) => {
    const list = await search(canvasElement);
    await list.findByText("Application Programming");
    const rows = document.querySelectorAll("#search-autocomplete-list [data-command-item]");
    const values = [...rows].map((r) => r.getAttribute("data-value"));
    // The course scores 0.8, above the subject's fuzzy match on "comp".
    await expect(values[0]).toBe("course:CS:3443:Application Programming");
    await expect(values).toContain("subject:CS");
  }}
/>

<!-- Nothing to show yet: the query matches no subject and the server is slow. -->
<Story
  name="Searching"
  args={baseArgs}
  play={async ({ canvasElement }) => {
    neverAnswer();
    const list = await search(canvasElement, "zzzz");
    await expect(await list.findByText("Searching...")).toBeInTheDocument();
  }}
/>

<!--
  The in-between state, and the reason local matching exists: subjects match in
  the browser and show at once, while the server is still being waited on.
-->
<Story
  name="Updating"
  args={baseArgs}
  play={async ({ canvasElement }) => {
    neverAnswer();
    const list = await search(canvasElement);
    await expect(await list.findByText("Computer Science")).toBeInTheDocument();
    await expect(await list.findByText("Updating...")).toBeInTheDocument();
    await expect(list.queryByText("Searching...")).toBeNull();
  }}
/>

<Story
  name="No Results"
  args={baseArgs}
  play={async ({ canvasElement }) => {
    answerWith(ok({ courses: [], instructors: [] }));
    const list = await search(canvasElement, "zzzz");
    await expect(await list.findByText("No results found.")).toBeInTheDocument();
  }}
/>

<!-- A failed lookup is said out loud, not passed off as "no results found". -->
<Story
  name="Request Failed"
  args={baseArgs}
  play={async ({ canvasElement }) => {
    answerWith(err({ message: "Suggestions are unavailable" }));
    const list = await search(canvasElement);
    await expect(await list.findByRole("alert")).toHaveTextContent("Suggestions are unavailable");
    await expect(list.queryByText("No results found.")).toBeNull();
  }}
/>

<!-- Picking a course applies its subject and its title as filters at once. -->
<Story
  name="Applying a Course"
  args={baseArgs}
  play={async ({ canvasElement }) => {
    const list = await search(canvasElement);
    await userEvent.click(await list.findByText("Application Programming"));

    const input = within(canvasElement).getByRole("combobox");
    // The box empties on selection, or the applied filter and the text disagree.
    await waitFor(async () => await expect(input).toHaveValue(""));
    // The panel flies out over 150ms, so it outlives the click that dismissed it.
    await waitFor(async () =>
      await expect(document.getElementById("search-autocomplete-list")).toBeNull()
    );
  }}
/>

<!--
  The first row is highlighted as soon as there is one, so Enter always has a
  target. Moving that highlight with the arrow keys is covered end to end
  instead: synthetic key events do not drive it in this environment.
-->
<Story
  name="Highlighted Row"
  args={baseArgs}
  play={async ({ canvasElement }) => {
    const list = await search(canvasElement);
    await list.findByText("John Smith");

    const rows = document.querySelectorAll("#search-autocomplete-list [data-command-item]");
    const highlighted = document.querySelectorAll(
      "#search-autocomplete-list [data-command-item][data-selected]"
    );
    await expect(rows.length).toBe(3);
    await expect(highlighted.length).toBe(1);
  }}
/>

<!-- Long titles are held to one row each rather than reflowing the list. -->
<Story
  name="Long Titles"
  args={baseArgs}
  play={async ({ canvasElement }) => {
    answerWith(
      ok({
        courses: [
          {
            subject: "CS",
            courseNumber: "4633",
            title:
              "Special Topics in Computer Science: Distributed Systems, Consensus and Fault Tolerance",
            sectionCount: 1,
            score: 0.9,
          },
        ],
        instructors: [],
      })
    );
    const list = await search(canvasElement);
    const row = await list.findByText(/Special Topics in Computer Science/);
    await expect(row).toHaveClass(/truncate/);
  }}
/>
