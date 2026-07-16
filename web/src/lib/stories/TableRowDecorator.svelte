<script lang="ts">
import type { Snippet } from "svelte";

let { children }: { children: Snippet } = $props();
</script>

<!-- Cell components render a bare <td>, which needs a table ancestor to lay out. -->
<table class="text-sm">
  <tbody>
    <tr>
      {@render children()}
    </tr>
  </tbody>
</table>
