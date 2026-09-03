import { describe, expect, it } from "vitest";
import type { SortKey, SortKeyOption } from "$lib/bindings";
import { formatSort, headerSortStep, parseSort } from "./sort";

const labels = new Map<SortKey, SortKeyOption>([
  [
    "instructor_rating",
    { key: "instructor_rating", ascLabel: "Lowest rated", descLabel: "Highest rated" },
  ],
  [
    "instructor_name",
    { key: "instructor_name", ascLabel: "Name, A to Z", descLabel: "Name, Z to A" },
  ],
  ["start_time", { key: "start_time", ascLabel: "Earliest first", descLabel: "Latest first" }],
]);

const step = (columnId: string, raw: string | null) =>
  headerSortStep(columnId, parseSort(raw), labels);

describe("wire format", () => {
  it("round-trips an ordered sort", () => {
    const raw = "start_time,-instructor_rating,duration";
    expect(formatSort(parseSort(raw))).toBe(raw);
  });

  it("treats an absent sort as no terms", () => {
    expect(parseSort(null)).toEqual([]);
    expect(parseSort("")).toEqual([]);
  });

  it("reads the descending prefix", () => {
    expect(parseSort("-days")).toEqual([{ key: "days", desc: true }]);
  });

  it("drops a key the backend does not recognize", () => {
    expect(parseSort("bogus")).toEqual([]);
  });

  it("keeps only the valid keys out of a mixed list", () => {
    expect(parseSort("start_time,bogus,-days")).toEqual([
      { key: "start_time", desc: false },
      { key: "days", desc: true },
    ]);
  });

  it("falls back to catalog order when every key is invalid", () => {
    expect(parseSort("bogus,-also_bogus")).toEqual([]);
  });

  it("keeps only the first occurrence of a repeated key", () => {
    expect(parseSort("start_time,-start_time,duration")).toEqual([
      { key: "start_time", desc: false },
      { key: "duration", desc: false },
    ]);
  });
});

describe("header cycle", () => {
  it("walks a single-key column through asc, desc, off", () => {
    expect(step("time", null)?.next).toEqual({ key: "start_time", desc: false });
    expect(step("time", "start_time")?.next).toEqual({ key: "start_time", desc: true });
    expect(step("time", "-start_time")?.next).toBeNull();
  });

  // The instructor header's five states are this rule on a two-key column, not
  // a mechanism of its own.
  it("walks a two-key column through both keys before clearing", () => {
    expect(step("instructor", null)?.next).toEqual({ key: "instructor_rating", desc: false });
    expect(step("instructor", "instructor_rating")?.next).toEqual({
      key: "instructor_rating",
      desc: true,
    });
    expect(step("instructor", "-instructor_rating")?.next).toEqual({
      key: "instructor_name",
      desc: false,
    });
    expect(step("instructor", "instructor_name")?.next).toEqual({
      key: "instructor_name",
      desc: true,
    });
    expect(step("instructor", "-instructor_name")?.next).toBeNull();
  });

  it("restarts the cycle when the active sort belongs to another column", () => {
    expect(step("instructor", "start_time")?.active).toBeNull();
    expect(step("instructor", "start_time")?.next).toEqual({
      key: "instructor_rating",
      desc: false,
    });
  });

  it("names the key only when the column offers a choice", () => {
    expect(step("time", "start_time")?.suffix).toBeNull();
    expect(step("instructor", "instructor_rating")?.suffix).toBe("INSTRUCTOR RATING");
  });

  it("phrases the tooltip as the action the click performs", () => {
    expect(step("time", null)?.title).toBe("Click to sort: earliest first");
    expect(step("time", "start_time")?.title).toBe("Click to sort: latest first");
    expect(step("time", "-start_time")?.title).toBe("Click to clear sorting");
  });

  it("reports the indicator from the active term", () => {
    expect(step("time", "start_time")?.indicator).toBe("asc");
    expect(step("time", "-start_time")?.indicator).toBe("desc");
    expect(step("time", null)?.indicator).toBe("none");
  });

  it("offers nothing for a column with no keys", () => {
    expect(headerSortStep("unknown", [], labels)).toBeNull();
  });
});
