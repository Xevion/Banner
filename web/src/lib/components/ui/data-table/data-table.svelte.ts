import {
  type RowData,
  type TableOptions,
  type TableOptionsResolved,
  type TableState,
  type Updater,
  createTable,
} from "@tanstack/table-core";
import { SvelteSet } from "svelte/reactivity";

/**
 * Creates a reactive TanStack table for Svelte 5 using runes.
 *
 * Adapted from shadcn-svelte's data-table wrapper -- uses `$state` and
 * `$effect.pre` instead of Svelte stores for reactivity.
 */
export function createSvelteTable<TData extends RowData>(options: TableOptions<TData>) {
  const resolvedOptions: TableOptionsResolved<TData> = mergeObjects(
    {
      state: {},
      onStateChange: () => {
        /* default noop */
      },
      renderFallbackValue: null,
      mergeOptions: (
        defaultOptions: TableOptions<TData>,
        options: Partial<TableOptions<TData>>
      ) => {
        return mergeObjects(defaultOptions, options);
      },
    },
    options
  );

  const table = createTable(resolvedOptions);
  let state = $state<Partial<TableState>>(table.initialState);

  function updateOptions() {
    table.setOptions((prev) => {
      return mergeObjects(prev, options, {
        state: mergeObjects(state, options.state ?? {}),

        onStateChange: (updater: Updater<TableState>) => {
          if (updater instanceof Function) state = updater(state as TableState);
          else state = { ...state, ...updater };

          options.onStateChange?.(updater);
        },
      });
    });
  }

  updateOptions();

  $effect.pre(() => {
    updateOptions();
  });

  return table;
}

type MaybeThunk<T extends object> = T | (() => T | null | undefined);
type Intersection<T extends readonly unknown[]> = (T extends [infer H, ...infer R]
  ? H & Intersection<R>
  : unknown) & {};

/**
 * Lazily merges several objects (or thunks) while preserving
 * getter semantics from every source. Proxy-based.
 */
/* eslint-disable @typescript-eslint/no-explicit-any -- dynamic proxy-based merge intentionally operates on untyped values */
export function mergeObjects<Sources extends readonly MaybeThunk<any>[]>(
  ...sources: Sources
): Intersection<{ [K in keyof Sources]: Sources[K] }> {
  const resolve = <T extends object>(src: MaybeThunk<T>): T | undefined =>
    typeof src === "function" ? (src() ?? undefined) : src;

  const findSourceWithKey = (key: PropertyKey): object | undefined => {
    for (let i = sources.length - 1; i >= 0; i--) {
      const obj = resolve(sources[i] as MaybeThunk<object>);
      if (obj && key in obj) return obj;
    }
    return undefined;
  };

  return new Proxy(Object.create(null) as object, {
    get(_, key) {
      const src = findSourceWithKey(key) as Record<PropertyKey, unknown> | undefined;
      return src?.[key];
    },

    has(_, key) {
      return !!findSourceWithKey(key);
    },

    ownKeys(): (string | symbol)[] {
      const all = new SvelteSet<string | symbol>();
      for (const s of sources) {
        const obj = resolve(s as MaybeThunk<object>);
        if (obj) {
          for (const k of Reflect.ownKeys(obj)) {
            all.add(k);
          }
        }
      }
      return [...all];
    },

    getOwnPropertyDescriptor(_, key) {
      const src = findSourceWithKey(key) as Record<PropertyKey, unknown> | undefined;
      if (!src) return undefined;
      return {
        configurable: true,
        enumerable: true,

        value: src[key],
        writable: true,
      };
    },
  }) as Intersection<{ [K in keyof Sources]: Sources[K] }>;
  /* eslint-enable @typescript-eslint/no-explicit-any */
}
