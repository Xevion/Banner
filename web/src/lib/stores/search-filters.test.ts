import { beforeEach, describe, expect, it } from "vitest";
import { SearchFilters } from "./search-filters.svelte";

describe("SearchFilters", () => {
  let filters: SearchFilters;

  beforeEach(() => {
    filters = new SearchFilters();
  });

  describe("fromURLParams", () => {
    it("should parse basic query params", () => {
      const params = new URLSearchParams({
        query: "calculus",
        open: "true",
      });
      params.append("subject", "MATH");
      params.append("subject", "CS");

      filters.fromURLParams(params);

      expect(filters.query).toBe("calculus");
      expect(filters.openOnly).toBe(true);
      expect(filters.subject).toEqual(["MATH", "CS"]);
    });

    it("should support legacy 'q' param name", () => {
      const params = new URLSearchParams({ q: "calculus" });

      filters.fromURLParams(params);

      expect(filters.query).toBe("calculus");
    });

    it("should parse numeric params correctly", () => {
      const params = new URLSearchParams({
        wait_count_max: "10",
        credit_hour_min: "3",
        credit_hour_max: "4",
        course_number_low: "1000",
        course_number_high: "2000",
      });

      filters.fromURLParams(params);

      expect(filters.waitCountMax).toBe(10);
      expect(filters.creditHourMin).toBe(3);
      expect(filters.creditHourMax).toBe(4);
      expect(filters.courseNumberLow).toBe(1000);
      expect(filters.courseNumberHigh).toBe(2000);
    });

    it("should handle array params", () => {
      const params = new URLSearchParams();
      params.append("days", "monday");
      params.append("days", "wednesday");
      params.append("campus", "main");
      params.append("instructional_method", "InPerson");

      filters.fromURLParams(params);

      expect(filters.days).toEqual(["monday", "wednesday"]);
      expect(filters.campus).toEqual(["main"]);
      expect(filters.instructionalMethod).toEqual(["InPerson"]);
    });

    it("should filter invalid subjects when validSubjects provided", () => {
      const params = new URLSearchParams();
      params.append("subject", "MATH");
      params.append("subject", "INVALID");
      params.append("subject", "CS");

      const validSubjects = new Set(["MATH", "CS"]);
      filters.fromURLParams(params, validSubjects);

      expect(filters.subject).toEqual(["MATH", "CS"]);
    });
  });

  describe("toURLParams", () => {
    it("should serialize all non-default values", () => {
      filters.query = "calculus";
      filters.openOnly = true;
      filters.subject = ["MATH", "CS"];
      filters.waitCountMax = 10;

      const params = filters.toURLParams();

      expect(params.get("query")).toBe("calculus");
      expect(params.get("open")).toBe("true");
      expect(params.getAll("subject")).toEqual(["MATH", "CS"]);
      expect(params.get("wait_count_max")).toBe("10");
    });

    it("should omit default/null values", () => {
      filters.query = null;
      filters.openOnly = false;
      filters.waitCountMax = null;

      const params = filters.toURLParams();

      expect(params.has("query")).toBe(false);
      expect(params.has("open")).toBe(false);
      expect(params.has("wait_count_max")).toBe(false);
    });

    it("should serialize array params correctly", () => {
      filters.days = ["monday", "wednesday"];
      filters.campus = ["main"];

      const params = filters.toURLParams();

      expect(params.getAll("days")).toEqual(["monday", "wednesday"]);
      expect(params.getAll("campus")).toEqual(["main"]);
    });
  });

  describe("toAPIParams", () => {
    it("should combine filters with pagination and sorting", () => {
      filters.query = "calculus";
      filters.subject = ["MATH"];
      filters.openOnly = true;

      const sorting = [{ id: "course_code", desc: false }];
      const apiParams = filters.toAPIParams("202501", 25, 0, sorting);

      expect(apiParams.term).toBe("202501");
      expect(apiParams.limit).toBe(25);
      expect(apiParams.offset).toBe(0);
      expect(apiParams.sortBy).toBe("course_code");
      expect(apiParams.sortDir).toBe("asc");
      expect(apiParams.query).toBe("calculus");
      expect(apiParams.subject).toEqual(["MATH"]);
      expect(apiParams.openOnly).toBe(true);
    });

    it("should handle descending sort", () => {
      const sorting = [{ id: "seats", desc: true }];
      const apiParams = filters.toAPIParams("202501", 25, 0, sorting);

      expect(apiParams.sortBy).toBe("seats");
      expect(apiParams.sortDir).toBe("desc");
    });

    it("should handle empty sorting", () => {
      const apiParams = filters.toAPIParams("202501", 25, 0, []);

      expect(apiParams.sortBy).toBe(null);
      expect(apiParams.sortDir).toBe(null);
    });
  });

  describe("activeCount", () => {
    it("should be 0 for default state", () => {
      expect(filters.activeCount).toBe(0);
    });

    it("should count each active filter", () => {
      filters.query = "calculus";
      expect(filters.activeCount).toBe(0); // query alone doesn't count as active

      filters.subject = ["MATH"];
      expect(filters.activeCount).toBe(1);

      filters.openOnly = true;
      expect(filters.activeCount).toBe(2);

      filters.days = ["monday"];
      expect(filters.activeCount).toBe(3);
    });

    it("should count range filters as one when both set", () => {
      filters.creditHourMin = 3;
      filters.creditHourMax = 4;
      expect(filters.activeCount).toBe(1);
    });

    it("should count instructor filter when non-empty", () => {
      filters.instructor = "";
      expect(filters.activeCount).toBe(0);

      filters.instructor = "Smith";
      expect(filters.activeCount).toBe(1);
    });
  });

  describe("isEmpty", () => {
    it("should be true for default state", () => {
      expect(filters.isEmpty).toBe(true);
    });

    it("should be false when any filter is active", () => {
      filters.openOnly = true;
      expect(filters.isEmpty).toBe(false);
    });
  });

  describe("clear", () => {
    it("should reset all filters to defaults", () => {
      filters.query = "calculus";
      filters.subject = ["MATH"];
      filters.openOnly = true;
      filters.waitCountMax = 10;
      filters.days = ["monday"];
      filters.creditHourMin = 3;

      filters.clear();

      expect(filters.query).toBe(null);
      expect(filters.subject).toEqual([]);
      expect(filters.openOnly).toBe(false);
      expect(filters.waitCountMax).toBe(null);
      expect(filters.days).toEqual([]);
      expect(filters.creditHourMin).toBe(null);
      expect(filters.isEmpty).toBe(true);
    });
  });

  describe("toSearchKey", () => {
    it("should generate consistent keys for same state", () => {
      filters.subject = ["MATH"];
      filters.openOnly = true;
      const key1 = filters.toSearchKey();
      const key2 = filters.toSearchKey();

      expect(key1).toBe(key2);
    });

    it("should generate different keys for different states", () => {
      filters.subject = ["MATH"];
      const key1 = filters.toSearchKey();

      filters.openOnly = true;
      const key2 = filters.toSearchKey();

      expect(key1).not.toBe(key2);
    });
  });
});
