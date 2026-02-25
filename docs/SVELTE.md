# Svelte Style Guide (Frontend)

General principles in [STYLE.md](STYLE.md).

## Architecture

### Route Organization

File-based routing with SvelteKit conventions:

```
src/routes/
+-- +layout.svelte        # Root layout (nav, theme, error boundary)
+-- +page.svelte           # Home page
+-- +error.svelte          # Global error page
+-- (app)/
|   +-- +layout.svelte    # App layout (sidebar, auth)
|   +-- search/
|   |   +-- +page.svelte  # Course search
|   |   +-- +page.ts      # Load function
|   +-- timeline/         # Schedule timeline
|   +-- course/[term]/[crn]/  # Course detail
|   +-- admin/            # Admin routes (dashboard, scraper, RMP)
+-- auth/                 # OAuth callback
```

- Load functions in `+page.ts` (universal, not server-side)
- No `+page.server.ts` -- all data fetching goes through the API client
- Load functions receive SvelteKit's `fetch` for SSR compatibility

### Data Fetching

```typescript
export const load: PageLoad = async ({ fetch }) => {
    const api = new BannerApiClient(undefined, fetch);
    const result = await api.searchCourses({ term });
    return result.match({
        Ok: (data) => ({ courses: data.courses }),
        Err: () => ({ courses: [] }),
    });
};
```

- Pass `fetch` from load context to `BannerApiClient` constructor for SSR
- Handle errors in load functions -- return fallback data, don't throw
- Component receives data via `let { data } = $props()`

## Error Handling

- **API layer**: `true-myth` Result pattern with `.match({ Ok, Err })` for exhaustive handling
- **Components**: `<svelte:boundary onerror={handler}>` for render-time errors
- **Load functions**: Two patterns depending on page type (see below)
- Never let API errors bubble unhandled -- always match or catch

### Load Function Error Patterns

**List pages** -- return fallback data + optional error message:
```typescript
return result.match({
    Ok: (data) => ({ courses: data.courses }),
    Err: (error) => ({ courses: [], error: error.message }),
});
```

**Detail pages** -- throw SvelteKit `error()` for the error page:
```typescript
return result.match({
    Ok: (course) => ({ course }),
    Err: (err) => {
        if (err.isNotFound()) error(404, { message: "Course not found" });
        error(500, { message: "Failed to load course" });
    },
});
```

### Error Boundaries

```svelte
<svelte:boundary onerror={(e) => console.error(e)}>
    <RiskyComponent />
    {#snippet failed(error, reset)}
        <ErrorBoundaryFallback {error} {reset} />
    {/snippet}
</svelte:boundary>
```

Use error boundaries around components that do complex rendering (canvas timelines, dynamic layouts, user-generated content). The app layout auto-resets boundaries on navigation -- no manual cleanup needed when the user navigates away from an errored page.

## State Management

**Escalation ladder** -- use the simplest pattern that works:

1. **Component-local `$state()`** -- default for most state
2. **Module-level runes** (`.svelte.ts` files) -- when 2+ unrelated components share state
3. **Svelte context** -- for deep component trees, avoiding prop drilling
4. **Global stores** -- only for truly global concerns (theme, auth)

```typescript
// 1. Component-local
let count = $state(0);
let doubled = $derived(count * 2);

// 2. Module-level store (auth.svelte.ts)
class AuthStore {
    state = $state<AuthState>({ kind: "loading" });
    get isAuthenticated() { return this.state.kind === "authenticated"; }
}
export const authStore = new AuthStore();

// 3. Context (in layout)
setContext(TABLE_CONTEXT_KEY, tableUtils);
// (in child)
const utils = getContext<TableUtils>(TABLE_CONTEXT_KEY);
```

Module-level stores use class-based patterns with `$state` runes -- no legacy `writable`/`readable` stores.

### Filter Registry Pattern

Course search filters use a **declarative registry** pattern: a single `FILTER_REGISTRY` object in `$lib/filters.ts` defines every filter once (URL key, serializer, default, grouping). All operations -- parse, serialize, count active, change detection, API conversion -- are pure functions derived from that registry.

**Key design decisions:**

- **`FilterState` is a plain object**, not a class with rune fields. Its type is inferred from the registry via mapped types. Reactivity is added externally by wrapping with `$state()` in `createFilterState()`.
- **Composable serializers** (`ParamSerializer<T>`) encapsulate encode/decode/default/isActive per value type. Built-ins: `stringParam()`, `boolParam()`, `intParam()`, `arrayParam()`, `campusParam()`.
- **Compile-time sync** with the backend -- a type assertion in `filters.ts` ensures `FilterState` stays structurally equal to the `SearchParams` binding (minus pagination/sort fields). If Rust adds a filter field and you don't add a registry entry, the build fails.
- **Groups** -- filters sharing a `group` name (e.g. `creditHourMin`/`creditHourMax` -> `"creditHour"`) count as one in `countActive()`.
- **Aliases** -- legacy URL param names (e.g. `q` -> `query`) are tried when the primary key is absent.

**Adding a new filter:**

1. Add the field to `SearchParams` in Rust (with `#[ts(export)]`)
2. Run `just bindings` -- the compile-time assertion will fail until you add the entry
3. Add one entry to `FILTER_REGISTRY` in `$lib/filters.ts`
4. Wire up the UI control in the relevant filter component

No other files need changes -- parsing, serialization, URL sync, active count, and API conversion all derive from the registry automatically.

```typescript
// $lib/filters.ts -- registry drives everything
export const FILTER_REGISTRY = {
  subject: { urlKey: "subject", serializer: arrayParam() },
  query:   { urlKey: "query",   serializer: stringParam(), aliases: ["q"], countAsActive: false },
  openOnly: { urlKey: "open",   serializer: boolParam() },
  // ... one entry per filter
} as const satisfies Record<string, FilterDef>;

// Type derived from registry -- never manually defined
export type FilterState = {
  -readonly [K in keyof typeof FILTER_REGISTRY]: InferValue<...>;
};

// Pure operations -- no Svelte dependency
defaultFilters(): FilterState
parseFilters(params: URLSearchParams): FilterState
serializeFilters(state: FilterState): URLSearchParams
countActive(state: FilterState): number
clearFilters(state: FilterState): void  // mutates in-place
toAPIParams(state: FilterState, meta): SearchParams
```

```typescript
// Thin reactive wrapper -- $lib/stores/search-filters.svelte.ts
export function createFilterState(params?: URLSearchParams): FilterState {
  const state: FilterState = $state(params ? parseFilters(params) : defaultFilters());
  return state;
}
```

This pattern keeps the registry importable in universal load functions (`+page.ts`) since it has no Svelte runtime dependency.

## Component Patterns

### Granularity

- Extract components when they're **reused**, **too large** (100+ lines of template), or **mature enough** to be a stable abstraction
- Don't extract prematurely -- inline code in `+page.svelte` is fine while iterating
- `bits-ui` components are the base layer for interactive primitives; compose from these

### Props

Svelte 5 `$props()` with TypeScript types:

```svelte
<script lang="ts">
    import type { CourseResponse } from "$lib/bindings";

    let {
        course,
        expanded = false,
        onToggle,
    }: {
        course: CourseResponse;
        expanded?: boolean;
        onToggle?: () => void;
    } = $props();
</script>
```

- Use function props for callbacks (`onToggle`, `onSelect`), not custom events
- Default optional props in the destructuring
- Use `$bindable()` for two-way binding when the parent needs to control state
- Use `Snippet` type for render-prop / slot patterns

### Reactivity

- `$state()` for mutable reactive values
- `$derived()` for computed values (replaces `$:` reactive statements)
- `$derived.by()` for complex computations that need a function body
- `$effect()` for side effects (DOM manipulation, event listeners, external subscriptions)
- Avoid `$effect` for data transformations -- use `$derived` instead

### Snippets

Use snippets for named content regions within a component:

```svelte
{#snippet seatsDisplay()}
    {@const openSeats = course.enrollment.max - course.enrollment.current}
    <span class={cn(openSeats > 0 ? "text-green-600" : "text-red-600")}>
        {openSeats} open
    </span>
{/snippet}
```

## bits-ui

Headless UI primitives with compound component pattern:

```svelte
<Tooltip.Root delayDuration={200}>
    <Tooltip.Trigger>
        {#snippet child({ props })}
            <span {...props}>{@render children()}</span>
        {/snippet}
    </Tooltip.Trigger>
    <Tooltip.Portal>
        <Tooltip.Content side="top" class={cn(tooltipContentClass)}>
            {@render content()}
        </Tooltip.Content>
    </Tooltip.Portal>
</Tooltip.Root>
```

- Use `data-[state=active]:` and similar data attribute selectors for state-based styling
- Compose bits-ui primitives with Tailwind classes via `cn()` -- no wrapper components unless heavily reused
- Available primitives: Tooltip, Tabs, ContextMenu, DropdownMenu, Combobox, and more

## TanStack Table

Tables use `createSvelteTable` -- a runes-based wrapper around TanStack Table core:

```typescript
const table = createSvelteTable({
    data: courses,
    columns: columnDefs,
    getCoreRowModel: getCoreRowModel(),
    getSortedRowModel: getSortedRowModel(),
    state: { sorting, columnVisibility },
});
```

- **Column definitions**: Declare in a separate file (`columns.ts`), map `cell` keys to Svelte components
- **Cell rendering**: Use `FlexRender` utility for dynamic cell component dispatch
- **State**: Sorting and column visibility managed as `$state` runes, passed into table options
- **Manual sorting**: Supported via `manualSorting` prop for server-side sort

## Styling

- **Tailwind utility classes** directly on elements
- **`cn()` helper** (clsx + tailwind-merge) for conditional class composition
- **`tailwind-variants`** for component variants with structured APIs (buttons, badges, cards)
- **Dark mode** via `dark:` prefix with class-based strategy -- theme store manages the `.dark` class on `<html>`
- No CSS modules, no scoped `<style>` blocks except where Tailwind can't reach
- **Shared class constants** for repeated patterns (e.g., `tooltipContentClass`)

```typescript
// tailwind-variants for structured component styling
import { tv } from "tailwind-variants";

const button = tv({
    base: "inline-flex items-center justify-center rounded-md text-sm font-medium",
    variants: {
        variant: {
            primary: "bg-primary text-primary-foreground hover:bg-primary/90",
            outline: "border border-input bg-background hover:bg-accent",
        },
        size: {
            sm: "h-8 px-3 text-xs",
            md: "h-9 px-4",
            lg: "h-10 px-6",
        },
    },
    defaultVariants: { variant: "primary", size: "md" },
});
```

## Type Safety

- Import backend types: `import type { CourseResponse } from "$lib/bindings"`
- `type` imports enforced -- `import type { ... }` not `import { ... }` for types
- Generated bindings are the source of truth for API shapes -- never duplicate them manually
- Use `$types` imports for SvelteKit page data: `import type { PageData } from "./$types"`
- Type aliases are acceptable for convenience: `export type Term = TermResponse;`

## API Client

`BannerApiClient` class with typed methods per resource:

```typescript
const api = new BannerApiClient(undefined, fetch);
const result = await api.searchCourses({ term, subject });  // Result<SearchResponse, ApiError>
const course = await api.getCourse(term, crn);               // Result<CourseResponse, ApiError>
```

- All methods return `Result<T, ApiError>` -- never throw
- SSR-compatible via `fetch` parameter injection in the constructor
- Search options cached in-memory with TTL (10 minutes)
- 401 responses trigger `authStore.handleUnauthorized()` automatically

## WebSocket Client

`StreamClient` manages typed real-time subscriptions:

```typescript
const { modify, unsubscribe } = streamClient.subscribe("scrapeJobs", filter, {
    onSnapshot: (data) => { /* full state */ },
    onDelta: (delta) => { /* incremental update */ },
});
```

- Subscriptions are type-safe -- stream name determines filter and payload types via `StreamKind` discriminated union
- `modify()` updates the subscription filter without resubscribing
- Auto-reconnects with exponential backoff (max 30s, max 10 attempts)
- Auto-resubscribes on reconnect with current filters

### useStream Composable

For reactive stream data in components:

```typescript
const stream = useStream("scrapeJobs", filter, {
    reduce: (state, delta) => applyDelta(state, delta),
    initial: [],
});
// stream.data is reactive ($state)
```

### useAutoRefresh Composable

For polling-based data (non-WebSocket endpoints):

```typescript
const stats = useAutoRefresh({
    fetcher: () => client.getScraperStats(period, term),
    deps: () => [period, term],
    interval: 5000,
});
```

- Exponential backoff on errors (doubles up to 60s max)
- Resets interval on success
- Dependency tracking re-triggers on reactive value changes

Use `useStream` for real-time data with WebSocket support. Use `useAutoRefresh` for endpoints that only support polling.

## Charting

D3 and Layerchart for data visualization:

- **Timeline**: D3 scales + canvas rendering for interactive course schedule visualization (pan/zoom)
- **Analytics**: Layerchart components for scraper metrics, timeseries data
- Keep chart logic in dedicated components -- don't inline D3 code in page components
- Use `$effect` for D3 bindings that need DOM access

## Storybook

Component stories for visual development and testing:

- **File naming**: `ComponentName.stories.svelte` alongside the component
- **When to write stories**: Reusable components, components with multiple visual states, interactive primitives
- **a11y addon**: Enabled -- stories are automatically tested for accessibility violations
- **Visual testing**: Storybook tests run via Playwright (`just storybook-test`)
- Don't write stories for page-level components or one-off layouts -- focus on the component library

## Logging

- Use `console.warn` / `console.error` for issues that need developer attention
- No logging framework -- browser devtools are sufficient for a SvelteKit app
- Avoid `console.log` in committed code (use only for temporary debugging)

## Testing

- **Unit tests**: Vitest with jsdom environment (`just test web`)
- **Component tests**: Storybook stories with Playwright browser testing (`just storybook-test`)
- **E2E tests**: Playwright for multi-page flows
- Test user-visible behavior, not component internals
- Use Vitest for utility functions, store logic, API client behavior, and composables
- Use Storybook for visual states, interactive behavior, and accessibility

### Formatting

Biome handles all frontend formatting. ESLint handles TypeScript and Svelte linting rules. They coexist -- Biome for style, ESLint for correctness.

- Line width: 100 characters
- Indent: 2 spaces
- Double quotes, always semicolons, ES5 trailing commas
- `$lib/bindings/` excluded from Biome (generated code)
