import {
  columnVisibilityFeature,
  createSortedRowModel,
  rowSortingFeature,
  sortFn_alphanumeric,
  sortFn_datetime,
  sortFn_text,
  tableFeatures,
} from "@tanstack/table-core";

/**
 * The feature set every table in the app is built from.
 *
 * TanStack v9 opts into features explicitly, and the enabled set is part of
 * each table's type. Sharing one declaration keeps every table assignable to
 * the same generic, so components like SortableHeader stay reusable.
 */
export const APP_TABLE_FEATURES = tableFeatures({
  columnVisibilityFeature,
  rowSortingFeature,
  sortedRowModel: createSortedRowModel(),
  // Only the three `sortFn: "auto"` can resolve to; every other column passes a
  // function directly, and unregistered auto falls back to the built-in basic sort.
  sortFns: {
    alphanumeric: sortFn_alphanumeric,
    datetime: sortFn_datetime,
    text: sortFn_text,
  },
});

export type AppTableFeatures = typeof APP_TABLE_FEATURES;
