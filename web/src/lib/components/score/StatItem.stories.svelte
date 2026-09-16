<script module>
import { defineMeta } from "@storybook/addon-svelte-csf";
import StatItem from "./StatItem.svelte";

const { Story } = defineMeta({
  title: "Components/Score/StatItem",
  component: StatItem,
  tags: ["autodocs"],
});
</script>

<Story name="Default" args={{ value: "4.5", label: "Instructor" }} />

<!-- The style prop is how callers colour a value by its rating. -->
<Story name="Colored" args={{ value: "4.5", label: "Overall", style: "color: #16a34a" }} />

<!-- A formatted count, which is the other thing this renders. -->
<Story name="Count" args={{ value: "1,034", label: "Responses" }} />

<!-- The row shape it is always used in, to check the columns line up. -->
<Story name="Row">
  {#snippet template()}
    <div class="flex items-center gap-6">
      <StatItem value="4.5" label="Overall" style="color: #16a34a" />
      <StatItem value="2.8" label="Difficulty" />
      <StatItem value="87%" label="Take Again" />
      <StatItem value="1,034" label="Responses" />
    </div>
  {/snippet}
</Story>
