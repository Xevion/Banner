import type { SearchParams } from "$lib/api";
import type { CodeDescription, SortColumn, SortDirection } from "$lib/bindings";
import { CAMPUS_GROUPS } from "$lib/labels";
import type { SortingState } from "@tanstack/table-core";

export { DAY_OPTIONS } from "$lib/days";

export function toggleDay(days: string[], day: string): string[] {
  return days.includes(day) ? days.filter((d) => d !== day) : [...days, day];
}

export function parseTimeInput(input: string): string | null {
  const trimmed = input.trim();
  if (trimmed === "") return null;

  const ampmMatch = /^(\d{1,2}):(\d{2})\s*(AM|PM)$/i.exec(trimmed);
  if (ampmMatch) {
    let hours = parseInt(ampmMatch[1], 10);
    const minutes = parseInt(ampmMatch[2], 10);
    const period = ampmMatch[3].toUpperCase();
    if (period === "PM" && hours !== 12) hours += 12;
    if (period === "AM" && hours === 12) hours = 0;
    return String(hours).padStart(2, "0") + String(minutes).padStart(2, "0");
  }

  const militaryMatch = /^(\d{1,2}):(\d{2})$/.exec(trimmed);
  if (militaryMatch) {
    const hours = parseInt(militaryMatch[1], 10);
    const minutes = parseInt(militaryMatch[2], 10);
    return String(hours).padStart(2, "0") + String(minutes).padStart(2, "0");
  }

  return null;
}

export function formatCompactTime(time: string | null): string {
  if (time?.length !== 4) return "";
  const hours = parseInt(time.slice(0, 2), 10);
  const minutes = time.slice(2);
  const period = hours >= 12 ? "PM" : "AM";
  const displayHours = hours === 0 ? 12 : hours > 12 ? hours - 12 : hours;
  return `${displayHours}:${minutes} ${period}`;
}

export function toggleValue(arr: string[], code: string): string[] {
  return arr.includes(code) ? arr.filter((v) => v !== code) : [...arr, code];
}

export interface GroupedAttributes {
  core: CodeDescription[];
  level: CodeDescription[];
  special: CodeDescription[];
}

export function groupAttributes(
  attributes: CodeDescription[],
  groups: { core: Set<string>; level: Set<string> }
): GroupedAttributes {
  const core: CodeDescription[] = [];
  const level: CodeDescription[] = [];
  const special: CodeDescription[] = [];

  for (const attr of attributes) {
    if (groups.core.has(attr.filterValue)) {
      core.push(attr);
    } else if (groups.level.has(attr.filterValue)) {
      level.push(attr);
    } else {
      special.push(attr);
    }
  }

  return { core, level, special };
}

/** Encodes/decodes a single typed value to/from URLSearchParams. */
export interface ParamSerializer<T> {
  encode(params: URLSearchParams, key: string, value: T): void;
  decode(params: URLSearchParams, key: string): T;
  defaultValue: T;
  isActive(value: T): boolean;
}

/** `string | null` -- present when non-null, omitted otherwise. */
export function stringParam(): ParamSerializer<string | null> {
  return {
    defaultValue: null,
    encode(params, key, value) {
      if (value !== null) params.set(key, value);
    },
    decode(params, key) {
      return params.get(key) ?? null;
    },
    isActive(value) {
      return value !== null && value !== "";
    },
  };
}

/** `boolean` -- serialized as `"true"` when active, omitted when `false`. */
export function boolParam(): ParamSerializer<boolean> {
  return {
    defaultValue: false,
    encode(params, key, value) {
      if (value) params.set(key, "true");
    },
    decode(params, key) {
      return params.get(key) === "true";
    },
    isActive(value) {
      return value;
    },
  };
}

/** `number | null` -- serialized as string, omitted when null. */
export function intParam(): ParamSerializer<number | null> {
  return {
    defaultValue: null,
    encode(params, key, value) {
      if (value !== null) params.set(key, String(value));
    },
    decode(params, key) {
      const raw = params.get(key);
      if (raw === null || raw === "") return null;
      const n = Number(raw);
      return Number.isNaN(n) ? null : n;
    },
    isActive(value) {
      return value !== null;
    },
  };
}

/** `string[]` -- repeated URL params (`?key=a&key=b`), omitted when empty. */
export function arrayParam(): ParamSerializer<string[]> {
  return {
    defaultValue: [],
    encode(params, key, value) {
      for (const v of value) params.append(key, v);
    },
    decode(params, key) {
      return params.getAll(key);
    },
    isActive(value) {
      return value.length > 0;
    },
  };
}

/** Known availability group names for compact URL serialization. */
const AVAILABILITY_GROUPS: Record<string, readonly string[]> = {
  campus: CAMPUS_GROUPS.campusStudents,
  online: CAMPUS_GROUPS.onlinePrograms,
};

function arraysEqualAsSet(a: string[], b: readonly string[]): boolean {
  if (a.length !== b.length) return false;
  const set = new Set(b);
  return a.every((v) => set.has(v));
}

/**
 * Campus array param with availability-group compression.
 *
 * Encode: if selected codes exactly match a known group, emit
 * `availability=<name>` instead of multiple `campus=X` params.
 *
 * Decode: if `availability` param matches a known group, expand it;
 * otherwise fall back to `campus` params.
 */
export function campusParam(): ParamSerializer<string[]> {
  return {
    defaultValue: [],
    encode(params, _key, value) {
      const matchedGroup = Object.entries(AVAILABILITY_GROUPS).find(([, codes]) =>
        arraysEqualAsSet(value, codes)
      );
      if (matchedGroup) {
        params.set("availability", matchedGroup[0]);
      } else {
        for (const v of value) params.append("campus", v);
      }
    },
    decode(params, _key) {
      const availability = params.get("availability");
      if (availability && availability in AVAILABILITY_GROUPS) {
        return [...AVAILABILITY_GROUPS[availability]];
      }
      return params.getAll("campus");
    },
    isActive(value) {
      return value.length > 0;
    },
  };
}

export interface FilterDef<T = unknown> {
  urlKey: string;
  serializer: ParamSerializer<T>;
  /** Legacy URL param aliases -- checked when primary key is absent. */
  aliases?: string[];
  /**
   * Filters sharing a group name count as one active filter in `countActive`.
   * Ungrouped filters each count as one.
   */
  group?: string;
  /** If false, this filter is excluded from `countActive`. Default: true. */
  countAsActive?: boolean;
}

/**
 * Central registry: each key is the API-compatible camelCase field name.
 * Adding a filter = adding one entry here. Everything else is derived.
 */
export const FILTER_REGISTRY = {
  subject: { urlKey: "subject", serializer: arrayParam() },
  query: { urlKey: "query", serializer: stringParam(), aliases: ["q"], countAsActive: false },
  openOnly: { urlKey: "open", serializer: boolParam() },
  waitCountMax: { urlKey: "wait_count_max", serializer: intParam() },
  days: { urlKey: "days", serializer: arrayParam() },
  timeStart: { urlKey: "time_start", serializer: stringParam(), group: "time" },
  timeEnd: { urlKey: "time_end", serializer: stringParam(), group: "time" },
  instructionalMethod: { urlKey: "instructional_method", serializer: arrayParam() },
  campus: { urlKey: "campus", serializer: campusParam() },
  partOfTerm: { urlKey: "part_of_term", serializer: arrayParam() },
  attributes: { urlKey: "attributes", serializer: arrayParam() },
  creditHourMin: { urlKey: "credit_hour_min", serializer: intParam(), group: "creditHour" },
  creditHourMax: { urlKey: "credit_hour_max", serializer: intParam(), group: "creditHour" },
  instructor: { urlKey: "instructor", serializer: stringParam() },
  courseNumberLow: { urlKey: "course_number_low", serializer: intParam(), group: "courseNumber" },
  courseNumberHigh: { urlKey: "course_number_high", serializer: intParam(), group: "courseNumber" },
} as const satisfies Record<string, FilterDef>;

type InferValue<S> = S extends ParamSerializer<infer T> ? T : never;

/** Plain object whose shape is derived from FILTER_REGISTRY. Mutable for reactive use. */
export type FilterState = {
  -readonly [K in keyof typeof FILTER_REGISTRY]: InferValue<
    (typeof FILTER_REGISTRY)[K]["serializer"]
  >;
};

// Compile-time assertion: FilterState must be assignable to the filter-relevant
// subset of SearchParams (the ts-rs binding). A mismatch here means the registry
// and the Rust backend have diverged.
type ApiFilterFields = Omit<SearchParams, "term" | "limit" | "offset" | "sortBy" | "sortDir">;
const _filterStateCheck: ApiFilterFields = {} as FilterState;
const _reverseCheck: FilterState = {} as ApiFilterFields;
void _filterStateCheck;
void _reverseCheck;

const registryEntries = Object.entries(FILTER_REGISTRY) as [
  keyof typeof FILTER_REGISTRY,
  FilterDef,
][];

/** Create a FilterState with all default values. */
export function defaultFilters(): FilterState {
  const state = {} as Record<string, unknown>;
  for (const [key, def] of registryEntries) {
    const dv = def.serializer.defaultValue;
    // Clone arrays so each state gets independent references
    state[key] = Array.isArray(dv) ? (dv as string[]).slice() : dv;
  }
  return state as FilterState;
}

/** Parse URL search params into a FilterState. */
export function parseFilters(params: URLSearchParams, validSubjects?: Set<string>): FilterState {
  const state = {} as Record<string, unknown>;
  for (const [key, def] of registryEntries) {
    let value = def.serializer.decode(params, def.urlKey);

    // If primary key produced a default and aliases exist, try aliases
    if (def.aliases && !def.serializer.isActive(value)) {
      for (const alias of def.aliases) {
        const aliased = def.serializer.decode(params, alias);
        if (def.serializer.isActive(aliased)) {
          value = aliased;
          break;
        }
      }
    }

    state[key] = value;
  }

  if (validSubjects) {
    (state as { subject: string[] }).subject = (state as { subject: string[] }).subject.filter(
      (s) => validSubjects.has(s)
    );
  }

  return state as FilterState;
}

/** Serialize a FilterState to URLSearchParams. Only includes non-default values. */
export function serializeFilters(state: FilterState): URLSearchParams {
  const params = new URLSearchParams();
  for (const [key, def] of registryEntries) {
    const value = state[key];
    if (def.serializer.isActive(value)) {
      def.serializer.encode(params, def.urlKey, value);
    }
  }
  return params;
}

/** Count active (non-default) filters, respecting groups. */
export function countActive(state: FilterState): number {
  const seen = new Set<string>();
  let count = 0;
  for (const [key, def] of registryEntries) {
    if (def.countAsActive === false) continue;
    if (!def.serializer.isActive(state[key])) continue;
    const group = def.group ?? key;
    if (!seen.has(group)) {
      seen.add(group);
      count++;
    }
  }
  return count;
}

/** Whether all filters are at their default values. */
export function isFiltersEmpty(state: FilterState): boolean {
  return countActive(state) === 0;
}

/**
 * Build a deterministic change-detection key.
 * Used to reset pagination when filters change.
 */
export function searchKey(state: FilterState): string {
  const params = serializeFilters(state);
  params.sort();
  return params.toString();
}

/**
 * Reset all fields of an existing FilterState to defaults (mutating).
 * Useful for clearing reactive state in-place.
 */
export function clearFilters(state: FilterState): void {
  const defaults = defaultFilters();
  for (const key of Object.keys(FILTER_REGISTRY) as (keyof FilterState)[]) {
    (state as Record<string, unknown>)[key] = defaults[key];
  }
}

/** Convert filter state + metadata to a full SearchParams for the API. */
export function toAPIParams(
  state: FilterState,
  meta: { term: string; limit: number; offset: number; sorting: SortingState }
): SearchParams {
  const sortBy: SortColumn | null =
    meta.sorting.length > 0 ? (meta.sorting[0].id as SortColumn) : null;
  const sortDir: SortDirection | null =
    meta.sorting.length > 0 ? (meta.sorting[0].desc ? "desc" : "asc") : null;

  return {
    ...state,
    term: meta.term,
    limit: meta.limit,
    offset: meta.offset,
    sortBy,
    sortDir,
  };
}

/**
 * Expand an `availability` param into campus codes.
 * Re-exported for backward compatibility; prefer using parseFilters() instead.
 */
export function expandCampusFromParams(params: URLSearchParams): string[] {
  const availability = params.get("availability");
  if (availability && availability in AVAILABILITY_GROUPS) {
    return [...AVAILABILITY_GROUPS[availability]];
  }
  return params.getAll("campus");
}
