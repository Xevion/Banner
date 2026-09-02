import type { SortingState } from "@tanstack/table-core";
import { describe, expect, it } from "vitest";
import {
  INSTRUCTOR_SORT_CYCLE,
  instructorSortLabel,
  instructorSortStep,
  nextInstructorSorting,
} from "./instructorSort";

describe("instructorSortStep", () => {
  it("reads an empty sort as neutral", () => {
    expect(instructorSortStep([]).key).toBeNull();
  });

  it("distinguishes the two rating directions", () => {
    expect(instructorSortStep([{ id: "rating", desc: true }]).indicator).toBe("desc");
    expect(instructorSortStep([{ id: "rating", desc: false }]).indicator).toBe("asc");
  });

  it("treats an unrelated column as neutral so a click takes over the sort", () => {
    expect(instructorSortStep([{ id: "seats", desc: true }]).key).toBeNull();
  });
});

describe("nextInstructorSorting", () => {
  it("walks the full cycle and wraps back to neutral", () => {
    let sorting: SortingState = [];
    const seen = INSTRUCTOR_SORT_CYCLE.map(() => {
      sorting = nextInstructorSorting(sorting);
      return sorting;
    });

    expect(seen).toEqual([
      [{ id: "rating", desc: true }],
      [{ id: "rating", desc: false }],
      [{ id: "instructor", desc: false }],
      [{ id: "instructor", desc: true }],
      [],
    ]);
  });

  it("enters the cycle at rating-descending from any unrelated sort", () => {
    expect(nextInstructorSorting([{ id: "seats", desc: false }])).toEqual([
      { id: "rating", desc: true },
    ]);
  });
});

describe("instructorSortLabel", () => {
  it("names the active key and stays silent when neutral", () => {
    expect(instructorSortLabel([])).toBeNull();
    expect(instructorSortLabel([{ id: "rating", desc: true }])).toBe("BY RATING");
    expect(instructorSortLabel([{ id: "instructor", desc: false }])).toBe("BY NAME");
  });
});
