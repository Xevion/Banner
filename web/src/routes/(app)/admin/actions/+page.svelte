<script lang="ts">
import { goto } from "$app/navigation";
import {
  ACTION_LABELS,
  ACTIONS,
  ENTITIES,
  ENTITY_LABELS,
  countActiveFilters,
  fromLocalInput,
  serializeActionLogParams,
  toLocalInput,
} from "$lib/action-log";
import type { ActionLogParams, AdminAction, AdminAuditEntry, AdminEntity } from "$lib/bindings";
import ErrorPanel from "$lib/components/ErrorPanel.svelte";
import Pagination from "$lib/components/Pagination.svelte";
import SimpleTooltip from "$lib/components/SimpleTooltip.svelte";
import StyledSelect, { type SelectItem } from "$lib/components/StyledSelect.svelte";
import { formatAbsoluteDate } from "$lib/date";
import { relativeTime } from "$lib/time";
import type { PageProps } from "./$types";

let { data }: PageProps = $props();

const draft = $derived(data.filters);
const entries = $derived(data.log?.entries ?? []);
const total = $derived(data.log?.total ?? 0);
const activeFilters = $derived(countActiveFilters(data.filters));
const now = new Date();

async function apply(next: Partial<ActionLogParams>) {
  const merged = { ...draft, ...next, page: 1 };
  const search = serializeActionLogParams(merged);
  await goto(`?${search}`, { keepFocus: true, noScroll: true });
}

async function goToPage(page: number) {
  const search = serializeActionLogParams({ ...data.filters, page });
  await goto(`?${search}`, { noScroll: true });
}

async function clearFilters() {
  await goto("?", { noScroll: true });
}

/** Narrow the log to one record, which is how a reviewer traces its history. */
async function focusEntity(entry: AdminAuditEntry, id: string) {
  await apply({ entityType: entry.entityType, entityId: id });
}

function detailPairs(entry: AdminAuditEntry): [string, string][] {
  const detail = entry.detail;
  if (!detail || typeof detail !== "object" || Array.isArray(detail)) return [];
  return Object.entries(detail).map(([key, value]) => [
    key,
    typeof value === "string" ? value : JSON.stringify(value),
  ]);
}

const actorItems = $derived<SelectItem[]>([
  { value: "", label: "Anyone" },
  ...data.actors.map((actor) => ({ value: actor.discordId, label: actor.discordUsername })),
]);

const actionItems: SelectItem[] = [
  { value: "", label: "Any action" },
  ...ACTIONS.map((action) => ({ value: action, label: ACTION_LABELS[action] })),
];

const entityItems: SelectItem[] = [
  { value: "", label: "Any entity" },
  ...ENTITIES.map((entity) => ({ value: entity, label: ENTITY_LABELS[entity] })),
];

const inputClass =
  "bg-background border-border h-[30px] rounded-md border px-2.5 text-xs text-foreground " +
  "placeholder:text-muted-foreground outline-none focus-visible:ring-2 focus-visible:ring-ring";
</script>

<svelte:head>
  <title>Action Log | Banner</title>
</svelte:head>

<div class="mb-4 flex items-baseline justify-between gap-3">
  <h1 class="text-lg font-semibold text-foreground">Action Log</h1>
  <p class="text-sm text-muted-foreground tabular-nums">
    {total}
    {total === 1 ? "action" : "actions"}
  </p>
</div>

<div class="bg-card border-border mb-4 rounded-lg border p-3">
  <div class="flex flex-wrap items-end gap-3">
    <div class="flex flex-col gap-1 text-xs text-muted-foreground">
      <span>Actor</span>
      <StyledSelect
        items={actorItems}
        value={draft.actor ?? ""}
        onValueChange={(v) => apply({ actor: v || null })}
        placeholder="Anyone"
      />
    </div>

    <div class="flex flex-col gap-1 text-xs text-muted-foreground">
      <span>Action</span>
      <StyledSelect
        items={actionItems}
        value={draft.action ?? ""}
        onValueChange={(v) => apply({ action: (v || null) as AdminAction | null })}
        placeholder="Any action"
        triggerClass="min-w-[170px]"
      />
    </div>

    <div class="flex flex-col gap-1 text-xs text-muted-foreground">
      <span>Entity</span>
      <StyledSelect
        items={entityItems}
        value={draft.entityType ?? ""}
        onValueChange={(v) => apply({ entityType: (v || null) as AdminEntity | null })}
        placeholder="Any entity"
      />
    </div>

    <label class="flex flex-col gap-1 text-xs text-muted-foreground">
      Entity ID
      <input
        type="text"
        placeholder="e.g. 1042"
        class="{inputClass} w-28"
        value={draft.entityId ?? ""}
        onchange={(e) => apply({ entityId: e.currentTarget.value || null })}
      />
    </label>

    <label class="flex flex-col gap-1 text-xs text-muted-foreground">
      From
      <input
        type="datetime-local"
        class={inputClass}
        value={toLocalInput(draft.since)}
        onchange={(e) => apply({ since: fromLocalInput(e.currentTarget.value) })}
      />
    </label>

    <label class="flex flex-col gap-1 text-xs text-muted-foreground">
      Until
      <input
        type="datetime-local"
        class={inputClass}
        value={toLocalInput(draft.until)}
        onchange={(e) => apply({ until: fromLocalInput(e.currentTarget.value) })}
      />
    </label>

    {#if activeFilters > 0}
      <button
        onclick={clearFilters}
        class="h-[30px] rounded-md bg-muted px-2.5 text-xs font-medium text-foreground
          transition-colors hover:bg-muted/80 cursor-pointer"
      >
        Clear {activeFilters} filter{activeFilters === 1 ? "" : "s"}
      </button>
    {/if}
  </div>
</div>

{#if data.error}
  <div class="mb-4">
    <ErrorPanel title="Couldn't load the action log" message={data.error} />
  </div>
{/if}

<div class="bg-card border-border overflow-hidden rounded-lg border">
  <table class="w-full text-sm">
    <thead>
      <tr class="border-border border-b text-left text-muted-foreground">
        <th class="px-4 py-3 font-medium">Time</th>
        <th class="px-4 py-3 font-medium">Actor</th>
        <th class="px-4 py-3 font-medium">Action</th>
        <th class="px-4 py-3 font-medium">Entity</th>
        <th class="px-4 py-3 font-medium">Detail</th>
      </tr>
    </thead>
    <tbody>
      {#if entries.length === 0}
        <tr>
          <td colspan="5" class="px-4 py-12 text-center text-muted-foreground">
            {activeFilters > 0 ? "No actions match these filters." : "No admin actions recorded."}
          </td>
        </tr>
      {:else}
        {#each entries as entry (entry.id)}
          {@const rel = relativeTime(new Date(entry.createdAt), now)}
          <tr class="border-border border-b last:border-b-0 align-top">
            <td class="px-4 py-3 whitespace-nowrap">
              <SimpleTooltip text={formatAbsoluteDate(entry.createdAt)} side="right" passthrough>
                <span class="font-mono text-xs text-muted-foreground">
                  {rel.text === "now" ? "just now" : `${rel.text} ago`}
                </span>
              </SimpleTooltip>
            </td>
            <td class="px-4 py-3 whitespace-nowrap">
              <span class="text-foreground">{entry.actorUsername}</span>
            </td>
            <td class="px-4 py-3 whitespace-nowrap">
              <span class="rounded-full bg-muted px-2 py-0.5 text-xs font-medium text-foreground">
                {ACTION_LABELS[entry.action]}
              </span>
            </td>
            <td class="px-4 py-3 whitespace-nowrap">
              <span class="text-xs text-muted-foreground">
                {ENTITY_LABELS[entry.entityType]}
              </span>
              {#if entry.entityId}
                <button
                  onclick={() => focusEntity(entry, entry.entityId ?? "")}
                  class="ml-1 font-mono text-xs text-primary hover:underline cursor-pointer"
                >
                  {entry.entityId}
                </button>
              {:else}
                <span class="ml-1 text-xs text-muted-foreground/60">all</span>
              {/if}
              {#each entry.relatedIds as related (related)}
                <button
                  onclick={() => focusEntity(entry, related)}
                  class="ml-1 font-mono text-xs text-muted-foreground hover:underline cursor-pointer"
                >
                  +{related}
                </button>
              {/each}
            </td>
            <td class="px-4 py-3">
              <div class="flex flex-wrap gap-x-3 gap-y-1">
                {#each detailPairs(entry) as [key, value] (key)}
                  <span class="font-mono text-xs">
                    <span class="text-muted-foreground">{key}:</span>
                    <span class="text-foreground">{value}</span>
                  </span>
                {:else}
                  <span class="text-xs text-muted-foreground/60">&mdash;</span>
                {/each}
              </div>
            </td>
          </tr>
        {/each}
      {/if}
    </tbody>
  </table>
</div>

{#if total > entries.length || (data.filters.page ?? 1) > 1}
  <Pagination
    variant="simple"
    currentPage={data.filters.page ?? 1}
    totalCount={total}
    perPage={data.log?.perPage ?? 50}
    onPageChange={goToPage}
  />
{/if}
