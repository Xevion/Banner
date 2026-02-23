import { formatCompactTime, parseTimeInput, toggleDay, toggleValue } from "$lib/filters";
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
