import { describe, expect, test } from "vitest";
import { ownPayload } from "./route-data";

interface HomeData {
  searchOptions: string | null;
  resolvedInstructors: Record<string, string>;
}

/** What the router passes through from a route whose load returns nothing. */
const layoutOnly = { user: null } as unknown as HomeData;

describe("ownPayload", () => {
  test("returns the payload that carries the marker", () => {
    const data: HomeData = { searchOptions: "opts", resolvedInstructors: {} };
    expect(ownPayload(data, "searchOptions")).toBe(data);
  });

  test("returns null for another route's payload", () => {
    expect(ownPayload(layoutOnly, "searchOptions")).toBeNull();
  });

  test("keeps a payload whose marker is present but empty", () => {
    const data: HomeData = { searchOptions: null, resolvedInstructors: {} };
    expect(ownPayload(data, "searchOptions")).toBe(data);
  });
});
