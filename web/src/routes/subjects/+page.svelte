<script lang="ts">
import type { SearchOptionsResponse } from "$lib/bindings";
import Breadcrumb from "$lib/components/Breadcrumb.svelte";
import Footer from "$lib/components/Footer.svelte";
import { Search } from "@lucide/svelte";

interface PageData {
  searchOptions: SearchOptionsResponse | null;
}

let { data }: { data: PageData } = $props();

let search = $state("");

const subjects = $derived(data.searchOptions?.subjects ?? []);

const filteredSubjects = $derived.by(() => {
  if (!search) return subjects;
  const q = search.toLowerCase();
  return subjects.filter(
    (s) => s.code.toLowerCase().includes(q) || s.description.toLowerCase().includes(q)
  );
});
</script>

<svelte:head>
  <title>Subjects | Banner</title>
</svelte:head>

<div class="min-h-screen flex flex-col items-center px-3 md:px-5 pb-5 pt-20">
  <div class="w-full max-w-6xl flex flex-col pt-2">
    <Breadcrumb items={[{ label: "Home", href: "/" }, { label: "Subjects" }]} />
    <h1 class="text-2xl font-bold mb-4">Subjects</h1>

    <!-- Search -->
    <div class="relative max-w-sm mb-4">
      <Search class="absolute left-3 top-1/2 -translate-y-1/2 size-4 text-muted-foreground" />
      <input
        type="text"
        placeholder="Search subjects..."
        bind:value={search}
        class="w-full h-9 pl-9 pr-3 text-sm rounded-md border border-border bg-card
               focus:outline-none focus:ring-2 focus:ring-ring"
      />
    </div>

    <!-- Results count -->
    <p class="text-xs text-muted-foreground mb-3">
      {filteredSubjects.length} subject{filteredSubjects.length !== 1 ? "s" : ""}
    </p>

    <!-- Subject grid -->
    {#if filteredSubjects.length > 0}
      <div class="grid grid-cols-1 sm:grid-cols-2 md:grid-cols-3 lg:grid-cols-4 gap-3">
        {#each filteredSubjects as subject (subject.code)}
          <a
            href="/subjects/{subject.code}"
            class="block rounded-lg border border-border bg-card px-4 py-3
                   hover:border-foreground/20 hover:shadow-sm transition-all"
          >
            <div class="font-semibold text-sm">{subject.code}</div>
            <div class="text-xs text-muted-foreground mt-0.5 line-clamp-2">
              {subject.description}
            </div>
          </a>
        {/each}
      </div>
    {:else}
      <div class="text-center py-16 text-muted-foreground">
        <p class="text-sm">No subjects found matching "{search}".</p>
      </div>
    {/if}

    <Footer />
  </div>
</div>
