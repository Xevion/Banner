import { describe, expect, test } from "vitest";
import { InstructorNames, unresolvedSlugs } from "./instructor-names";

describe("InstructorNames", () => {
  test("falls back to the slug when the name is unknown", () => {
    const names = new InstructorNames();
    expect(names.get("smith-abc")).toBe("smith-abc");
  });

  test("returns a seeded name", () => {
    const names = new InstructorNames({ "smith-abc": "Smith, John" });
    expect(names.get("smith-abc")).toBe("Smith, John");
  });

  test("seeding merges rather than replacing", () => {
    const names = new InstructorNames({ "a-1": "Adams, Riley" });
    names.seed({ "b-2": "Brooks, Jordan" });
    expect(names.get("a-1")).toBe("Adams, Riley");
    expect(names.get("b-2")).toBe("Brooks, Jordan");
  });

  test("a later seed wins for the same slug", () => {
    const names = new InstructorNames({ "dup-1": "Stale" });
    names.seed({ "dup-1": "Adams, Riley" });
    expect(names.get("dup-1")).toBe("Adams, Riley");
  });

  test("seeding also warms the store the loader consults", () => {
    new InstructorNames({ "warmed-1": "Chen, Morgan" });
    expect(unresolvedSlugs(["warmed-1"])).toEqual([]);
  });

  // Leaving the page and returning remounts the provider, and the load that
  // brought us back skipped the slugs this tab already knows. Starting from
  // what is known is what keeps those chips named rather than showing a slug.
  test("starts from the names this tab already learned", () => {
    new InstructorNames({ "prior-1": "Diaz, Avery" });
    expect(new InstructorNames().get("prior-1")).toBe("Diaz, Avery");
  });
});

// Each test uses slugs of its own: the warm store outlives a navigation by
// design, so it also outlives a test.
describe("unresolvedSlugs in the browser", () => {
  test("reports every slug when none are known", () => {
    expect(unresolvedSlugs(["fresh-1", "fresh-2"])).toEqual(["fresh-1", "fresh-2"]);
  });

  test("skips slugs already learned in this tab", () => {
    new InstructorNames({ "known-1": "Adams, Riley" });
    expect(unresolvedSlugs(["known-1", "other-1"])).toEqual(["other-1"]);
  });

  test("asks for a repeated slug only once", () => {
    expect(unresolvedSlugs(["twice-1", "twice-1"])).toEqual(["twice-1"]);
  });
});
