import { describe, expect, test, vi } from "vitest";

// One module instance serves every request on the server, so the warm store
// must stay empty there. This file exists separately because the mock is
// hoisted over the whole module graph.
vi.mock("$app/environment", () => ({ browser: false }));

const { InstructorNames, unresolvedSlugs } = await import("./instructor-names");

describe("instructor names on the server", () => {
  test("learning a name does not warm the store", () => {
    new InstructorNames({ "leaked-1": "Adams, Riley" });
    expect(unresolvedSlugs(["leaked-1"])).toEqual(["leaked-1"]);
  });

  test("a seeded instance keeps its names for the render it belongs to", () => {
    const names = new InstructorNames({ "req-1": "Brooks, Jordan" });
    expect(names.get("req-1")).toBe("Brooks, Jordan");
  });

  test("one request's names never answer another's lookups", () => {
    new InstructorNames({ "req-a": "Adams, Riley" });
    const other = new InstructorNames();
    expect(other.get("req-a")).toBe("req-a");
  });
});
