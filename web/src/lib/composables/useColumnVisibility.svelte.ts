import type { VisibilityState } from "@tanstack/table-core";
import { untrack } from "svelte";
import { SvelteSet } from "svelte/reactivity";

export interface ColumnDef {
  id: string;
  label: string;
}

export interface UseColumnVisibilityOptions {
  /** Column IDs that auto-hide/show based on responsive breakpoints */
  autoHideColumns?: string[];
  /** Media query string for compact mode (default: "(min-width: 640px) and (max-width: 767px)") */
  compactQuery?: string;
  /** All available column definitions */
  columns: ColumnDef[];
}

/**
 * Manages column visibility state with responsive auto-hiding.
 *
 * The `visibility` field is reactive ($state) and can be used with `bind:` in Svelte templates.
 * Columns listed in `autoHideColumns` are hidden/shown based on a media query,
 * but only if the user hasn't manually toggled them.
 */
export class ColumnVisibilityController {
  /** Current column visibility state for TanStack Table */
  visibility: VisibilityState = $state({});

  /** Column definitions */
  readonly columns: ColumnDef[];

  /** Track columns the user has explicitly toggled */
  #userToggledColumns = new SvelteSet<string>();

  #isCompact = $state(false);
  readonly #autoHideColumns: string[];

  /** Whether any columns have been hidden */
  readonly hasCustomVisibility: boolean = $derived(
    Object.values(this.visibility).some((v) => v === false)
  );

  constructor(options: UseColumnVisibilityOptions) {
    this.columns = options.columns;
    this.#autoHideColumns = options.autoHideColumns ?? [];
    const compactQuery = options.compactQuery ?? "(min-width: 640px) and (max-width: 767px)";

    // Media query listener for responsive column hiding
    $effect(() => {
      if (typeof window === "undefined") return;
      const mql = window.matchMedia(compactQuery);
      this.#isCompact = mql.matches;
      const handler = (e: MediaQueryListEvent) => {
        this.#isCompact = e.matches;
      };
      mql.addEventListener("change", handler);
      return () => mql.removeEventListener("change", handler);
    });

    // Auto-hide/show columns based on compact mode (only for columns the user hasn't manually toggled)
    $effect(() => {
      const compact = this.#isCompact;
      const toggled = this.#userToggledColumns;
      const current = untrack(() => this.visibility);
      let changed = false;
      const next = { ...current };
      for (const col of this.#autoHideColumns) {
        if (toggled.has(col)) continue;
        if (compact && next[col] !== false) {
          next[col] = false;
          changed = true;
        } else if (!compact && next[col] === false) {
          delete next[col];
          changed = true;
        }
      }
      if (changed) this.visibility = next;
    });
  }

  /** Toggle a column's visibility (marks it as user-toggled) */
  toggle = (columnId: string, visible: boolean): void => {
    this.#userToggledColumns.add(columnId);
    this.visibility = { ...this.visibility, [columnId]: visible };
  };

  /** Reset all columns to default visibility */
  reset = (): void => {
    this.visibility = {};
    this.#userToggledColumns = new SvelteSet();
  };
}
