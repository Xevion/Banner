import {
  clearFilters,
  countActive,
  defaultFilters,
  expandCampusFromParams,
  formatCompactTime,
  instructorDisplayName,
  isFiltersEmpty,
  parseFilters,
  parseTimeInput,
  populateInstructorCache,
  searchKey,
  serializeFilters,
  toAPIParams,
  toggleDay,
  toggleValue,
} from "$lib/filters";
import { CAMPUS_GROUPS } from "$lib/labels";
import { describe, expect, it } from "vitest";

describe("parseTimeInput", () => {
  it("parses AM time", () => {
    expect(parseTimeInput("10:30 AM")).toBe("1030");
  });

  it("parses PM time", () => {
    expect(parseTimeInput("3:00 PM")).toBe("1500");
  });

  it("parses 12:00 PM as noon", () => {
    expect(parseTimeInput("12:00 PM")).toBe("1200");
  });

  it("parses 12:00 AM as midnight", () => {
    expect(parseTimeInput("12:00 AM")).toBe("0000");
  });

  it("parses case-insensitive AM/PM", () => {
    expect(parseTimeInput("9:15 am")).toBe("0915");
    expect(parseTimeInput("2:45 Pm")).toBe("1445");
  });

  it("parses military time", () => {
    expect(parseTimeInput("14:30")).toBe("1430");
    expect(parseTimeInput("9:05")).toBe("0905");
  });

  it("returns null for empty string", () => {
    expect(parseTimeInput("")).toBeNull();
    expect(parseTimeInput("   ")).toBeNull();
  });

  it("returns null for non-time strings", () => {
    expect(parseTimeInput("abc")).toBeNull();
    expect(parseTimeInput("hello world")).toBeNull();
  });

  it("parses out-of-range military time (no validation beyond format)", () => {
    // The regex matches but doesn't validate hour/minute ranges
    expect(parseTimeInput("25:00")).toBe("2500");
  });

  it("trims whitespace", () => {
    expect(parseTimeInput("  10:00 AM  ")).toBe("1000");
  });
});

describe("formatCompactTime", () => {
  it("formats morning time", () => {
    expect(formatCompactTime("0930")).toBe("9:30 AM");
  });

  it("formats afternoon time", () => {
    expect(formatCompactTime("1500")).toBe("3:00 PM");
  });

  it("formats noon", () => {
    expect(formatCompactTime("1200")).toBe("12:00 PM");
  });

  it("formats midnight", () => {
    expect(formatCompactTime("0000")).toBe("12:00 AM");
  });

  it("returns empty string for null", () => {
    expect(formatCompactTime(null)).toBe("");
  });

  it("returns empty string for invalid length", () => {
    expect(formatCompactTime("12")).toBe("");
    expect(formatCompactTime("123456")).toBe("");
  });
});

describe("toggleDay", () => {
  it("adds a day not in the list", () => {
    expect(toggleDay(["monday"], "wednesday")).toEqual(["monday", "wednesday"]);
  });

  it("removes a day already in the list", () => {
    expect(toggleDay(["monday", "wednesday"], "monday")).toEqual(["wednesday"]);
  });

  it("adds to empty list", () => {
    expect(toggleDay([], "friday")).toEqual(["friday"]);
  });

  it("removes last day", () => {
    expect(toggleDay(["monday"], "monday")).toEqual([]);
  });
});

describe("toggleValue", () => {
  it("adds a value not in the array", () => {
    expect(toggleValue(["OA"], "HB")).toEqual(["OA", "HB"]);
  });

  it("removes a value already in the array", () => {
    expect(toggleValue(["OA", "HB"], "OA")).toEqual(["HB"]);
  });

  it("adds to empty array", () => {
    expect(toggleValue([], "OA")).toEqual(["OA"]);
  });

  it("removes last value", () => {
    expect(toggleValue(["OA"], "OA")).toEqual([]);
  });
});

describe("defaultFilters", () => {
  it("returns all fields with default values", () => {
    const state = defaultFilters();
    expect(state.subject).toEqual([]);
    expect(state.query).toBeNull();
    expect(state.openOnly).toBe(false);
    expect(state.waitCountMax).toBeNull();
    expect(state.days).toEqual([]);
    expect(state.timeStart).toBeNull();
    expect(state.timeEnd).toBeNull();
    expect(state.instructionalMethod).toEqual([]);
    expect(state.campus).toEqual([]);
    expect(state.partOfTerm).toEqual([]);
    expect(state.attributes).toEqual([]);
    expect(state.creditHourMin).toBeNull();
    expect(state.creditHourMax).toBeNull();
    expect(state.instructor).toEqual([]);
    expect(state.courseNumberLow).toBeNull();
    expect(state.courseNumberHigh).toBeNull();
  });

  it("returns independent array instances", () => {
    const a = defaultFilters();
    const b = defaultFilters();
    a.subject.push("MATH");
    expect(b.subject).toEqual([]);
  });
});

describe("parseFilters", () => {
  it("parses basic query params", () => {
    const params = new URLSearchParams({ query: "calculus", open: "true" });
    params.append("subject", "MATH");
    params.append("subject", "CS");

    const state = parseFilters(params);

    expect(state.query).toBe("calculus");
    expect(state.openOnly).toBe(true);
    expect(state.subject).toEqual(["MATH", "CS"]);
  });

  it("supports legacy 'q' param name", () => {
    const params = new URLSearchParams({ q: "calculus" });
    const state = parseFilters(params);
    expect(state.query).toBe("calculus");
  });

  it("prefers 'query' over 'q' when both present", () => {
    const params = new URLSearchParams({ query: "algebra", q: "calculus" });
    const state = parseFilters(params);
    expect(state.query).toBe("algebra");
  });

  it("parses numeric params", () => {
    const params = new URLSearchParams({
      wait_count_max: "10",
      credit_hour_min: "3",
      credit_hour_max: "4",
      course_number_low: "1000",
      course_number_high: "2000",
    });

    const state = parseFilters(params);

    expect(state.waitCountMax).toBe(10);
    expect(state.creditHourMin).toBe(3);
    expect(state.creditHourMax).toBe(4);
    expect(state.courseNumberLow).toBe(1000);
    expect(state.courseNumberHigh).toBe(2000);
  });

  it("handles array params", () => {
    const params = new URLSearchParams();
    params.append("days", "monday");
    params.append("days", "wednesday");
    params.append("campus", "main");
    params.append("instructional_method", "InPerson");

    const state = parseFilters(params);

    expect(state.days).toEqual(["monday", "wednesday"]);
    expect(state.campus).toEqual(["main"]);
    expect(state.instructionalMethod).toEqual(["InPerson"]);
  });

  it("expands availability=campus into campus codes", () => {
    const params = new URLSearchParams({ availability: "campus" });
    const state = parseFilters(params);
    expect(state.campus).toEqual(CAMPUS_GROUPS.campusStudents);
  });

  it("expands availability=online into campus codes", () => {
    const params = new URLSearchParams({ availability: "online" });
    const state = parseFilters(params);
    expect(state.campus).toEqual(CAMPUS_GROUPS.onlinePrograms);
  });

  it("falls back to individual campus params when no availability param", () => {
    const params = new URLSearchParams();
    params.append("campus", "Main");
    params.append("campus", "Downtown");
    const state = parseFilters(params);
    expect(state.campus).toEqual(["Main", "Downtown"]);
  });

  it("filters invalid subjects when validSubjects provided", () => {
    const params = new URLSearchParams();
    params.append("subject", "MATH");
    params.append("subject", "INVALID");
    params.append("subject", "CS");

    const state = parseFilters(params, new Set(["MATH", "CS"]));
    expect(state.subject).toEqual(["MATH", "CS"]);
  });

  it("returns defaults for empty params", () => {
    const state = parseFilters(new URLSearchParams());
    expect(state).toEqual(defaultFilters());
  });

  it("handles NaN numeric params gracefully", () => {
    const params = new URLSearchParams({ wait_count_max: "abc" });
    const state = parseFilters(params);
    expect(state.waitCountMax).toBeNull();
  });
});

describe("serializeFilters", () => {
  it("serializes non-default values", () => {
    const state = defaultFilters();
    state.query = "calculus";
    state.openOnly = true;
    state.subject = ["MATH", "CS"];
    state.waitCountMax = 10;

    const params = serializeFilters(state);

    expect(params.get("query")).toBe("calculus");
    expect(params.get("open")).toBe("true");
    expect(params.getAll("subject")).toEqual(["MATH", "CS"]);
    expect(params.get("wait_count_max")).toBe("10");
  });

  it("omits default/null values", () => {
    const state = defaultFilters();
    const params = serializeFilters(state);

    expect(params.has("query")).toBe(false);
    expect(params.has("open")).toBe(false);
    expect(params.has("wait_count_max")).toBe(false);
    expect(params.has("subject")).toBe(false);
  });

  it("serializes array params correctly", () => {
    const state = defaultFilters();
    state.days = ["monday", "wednesday"];
    state.campus = ["main"];

    const params = serializeFilters(state);

    expect(params.getAll("days")).toEqual(["monday", "wednesday"]);
    expect(params.getAll("campus")).toEqual(["main"]);
  });

  it("compresses campus-students group into availability=campus", () => {
    const state = defaultFilters();
    state.campus = [...CAMPUS_GROUPS.campusStudents];
    const params = serializeFilters(state);
    expect(params.get("availability")).toBe("campus");
    expect(params.has("campus")).toBe(false);
  });

  it("compresses online-programs group into availability=online", () => {
    const state = defaultFilters();
    state.campus = [...CAMPUS_GROUPS.onlinePrograms];
    const params = serializeFilters(state);
    expect(params.get("availability")).toBe("online");
    expect(params.has("campus")).toBe(false);
  });

  it("serializes individual campus codes when not matching a group", () => {
    const state = defaultFilters();
    state.campus = ["Main", "Downtown"];
    const params = serializeFilters(state);
    expect(params.has("availability")).toBe(false);
    expect(params.getAll("campus")).toEqual(["Main", "Downtown"]);
  });
});

describe("parseFilters / serializeFilters roundtrip", () => {
  it("roundtrips all filter types", () => {
    populateInstructorCache({ "smith-abc": "Smith, John" });

    const original = defaultFilters();
    original.subject = ["MATH", "CS"];
    original.query = "calculus";
    original.openOnly = true;
    original.waitCountMax = 5;
    original.days = ["monday", "wednesday"];
    original.timeStart = "0800";
    original.timeEnd = "1700";
    original.instructionalMethod = ["InPerson"];
    original.campus = ["Main", "Downtown"];
    original.partOfTerm = ["Full"];
    original.attributes = ["CoreMath"];
    original.creditHourMin = 3;
    original.creditHourMax = 4;
    original.instructor = ["smith-abc"];
    original.courseNumberLow = 1000;
    original.courseNumberHigh = 4000;

    const params = serializeFilters(original);
    const restored = parseFilters(params);

    expect(restored).toEqual(original);

    // Display name resolves from cache
    expect(instructorDisplayName("smith-abc")).toBe("Smith, John");
  });

  it("roundtrips defaults (empty params)", () => {
    const original = defaultFilters();
    const params = serializeFilters(original);
    expect(params.toString()).toBe("");
    const restored = parseFilters(params);
    expect(restored).toEqual(original);
  });
});

describe("toAPIParams", () => {
  it("combines filters with pagination and sorting", () => {
    const state = defaultFilters();
    state.query = "calculus";
    state.subject = ["MATH"];
    state.openOnly = true;

    const sorting = [{ id: "course_code", desc: false }];
    const apiParams = toAPIParams(state, { term: "202501", limit: 25, offset: 0, sorting });

    expect(apiParams.term).toBe("202501");
    expect(apiParams.limit).toBe(25);
    expect(apiParams.offset).toBe(0);
    expect(apiParams.sortBy).toBe("course_code");
    expect(apiParams.sortDir).toBe("asc");
    expect(apiParams.query).toBe("calculus");
    expect(apiParams.subject).toEqual(["MATH"]);
    expect(apiParams.openOnly).toBe(true);
  });

  it("handles descending sort", () => {
    const sorting = [{ id: "seats", desc: true }];
    const apiParams = toAPIParams(defaultFilters(), {
      term: "202501",
      limit: 25,
      offset: 0,
      sorting,
    });
    expect(apiParams.sortBy).toBe("seats");
    expect(apiParams.sortDir).toBe("desc");
  });

  it("handles empty sorting", () => {
    const apiParams = toAPIParams(defaultFilters(), {
      term: "202501",
      limit: 25,
      offset: 0,
      sorting: [],
    });
    expect(apiParams.sortBy).toBeNull();
    expect(apiParams.sortDir).toBeNull();
  });
});

describe("countActive", () => {
  it("returns 0 for default state", () => {
    expect(countActive(defaultFilters())).toBe(0);
  });

  it("does not count query as active", () => {
    const state = defaultFilters();
    state.query = "calculus";
    expect(countActive(state)).toBe(0);
  });

  it("counts each active filter", () => {
    const state = defaultFilters();
    state.subject = ["MATH"];
    expect(countActive(state)).toBe(1);

    state.openOnly = true;
    expect(countActive(state)).toBe(2);

    state.days = ["monday"];
    expect(countActive(state)).toBe(3);
  });

  it("counts range filters as one when both set (group)", () => {
    const state = defaultFilters();
    state.creditHourMin = 3;
    state.creditHourMax = 4;
    expect(countActive(state)).toBe(1);
  });

  it("counts time range as one group", () => {
    const state = defaultFilters();
    state.timeStart = "0800";
    state.timeEnd = "1700";
    expect(countActive(state)).toBe(1);
  });

  it("counts course number range as one group", () => {
    const state = defaultFilters();
    state.courseNumberLow = 1000;
    state.courseNumberHigh = 4000;
    expect(countActive(state)).toBe(1);
  });

  it("counts instructor when non-null", () => {
    const state = defaultFilters();
    expect(countActive(state)).toBe(0);

    state.instructor = ["smith-abc"];
    expect(countActive(state)).toBe(1);
  });
});

describe("isFiltersEmpty", () => {
  it("is true for default state", () => {
    expect(isFiltersEmpty(defaultFilters())).toBe(true);
  });

  it("is false when any filter is active", () => {
    const state = defaultFilters();
    state.openOnly = true;
    expect(isFiltersEmpty(state)).toBe(false);
  });

  it("is true when only query is set (query does not count)", () => {
    const state = defaultFilters();
    state.query = "calculus";
    expect(isFiltersEmpty(state)).toBe(true);
  });
});

describe("clearFilters", () => {
  it("resets all fields to defaults", () => {
    const state = defaultFilters();
    state.query = "calculus";
    state.subject = ["MATH"];
    state.openOnly = true;
    state.waitCountMax = 10;
    state.days = ["monday"];
    state.creditHourMin = 3;

    clearFilters(state);

    expect(state.query).toBeNull();
    expect(state.subject).toEqual([]);
    expect(state.openOnly).toBe(false);
    expect(state.waitCountMax).toBeNull();
    expect(state.days).toEqual([]);
    expect(state.creditHourMin).toBeNull();
    expect(isFiltersEmpty(state)).toBe(true);
  });
});

describe("searchKey", () => {
  it("generates consistent keys for same state", () => {
    const state = defaultFilters();
    state.subject = ["MATH"];
    state.openOnly = true;

    expect(searchKey(state)).toBe(searchKey(state));
  });

  it("generates different keys for different states", () => {
    const a = defaultFilters();
    a.subject = ["MATH"];

    const b = defaultFilters();
    b.subject = ["MATH"];
    b.openOnly = true;

    expect(searchKey(a)).not.toBe(searchKey(b));
  });

  it("is empty string for default state", () => {
    expect(searchKey(defaultFilters())).toBe("");
  });
});

describe("expandCampusFromParams", () => {
  it("expands availability=campus to campus codes", () => {
    const params = new URLSearchParams({ availability: "campus" });
    expect(expandCampusFromParams(params)).toEqual(CAMPUS_GROUPS.campusStudents);
  });

  it("expands availability=online to campus codes", () => {
    const params = new URLSearchParams({ availability: "online" });
    expect(expandCampusFromParams(params)).toEqual(CAMPUS_GROUPS.onlinePrograms);
  });

  it("falls back to individual campus params", () => {
    const params = new URLSearchParams();
    params.append("campus", "Main");
    params.append("campus", "Downtown");
    expect(expandCampusFromParams(params)).toEqual(["Main", "Downtown"]);
  });

  it("returns empty array when no campus or availability params", () => {
    expect(expandCampusFromParams(new URLSearchParams())).toEqual([]);
  });

  it("ignores unknown availability values", () => {
    const params = new URLSearchParams({ availability: "unknown" });
    expect(expandCampusFromParams(params)).toEqual([]);
  });
});
