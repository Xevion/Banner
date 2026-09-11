import {
  columnVisibilityFeature,
  createSortedRowModel,
  rowSortingFeature,
  sortFns,
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
  sortFns,
});

export type AppTableFeatures = typeof APP_TABLE_FEATURES;
