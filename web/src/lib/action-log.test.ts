import { describe, expect, it } from "vitest";
import {
  ACTION_LOG_PER_PAGE,
  countActiveFilters,
  fromLocalInput,
  parseActionLogParams,
  serializeActionLogParams,
  toLocalInput,
} from "./action-log";

function parse(query: string) {
  return parseActionLogParams(new URLSearchParams(query));
}

describe("parseActionLogParams", () => {
  it("keeps values the backend understands", () => {
    const params = parse("actor=42&action=instructor_merge&entityType=instructor&entityId=7");

    expect(params).toMatchObject({
      actor: "42",
      action: "instructor_merge",
      entityType: "instructor",
      entityId: "7",
      page: 1,
      perPage: ACTION_LOG_PER_PAGE,
    });
  });

  it("drops an action or entity outside the known vocabulary", () => {
    const params = parse("action=drop_database&entityType=planet");

    expect(params.action).toBeNull();
    expect(params.entityType).toBeNull();
  });

  it("treats blank and non-numeric pages as the first page", () => {
    expect(parse("entityId=%20%20").entityId).toBeNull();
    expect(parse("page=first").page).toBe(1);
    expect(parse("page=0").page).toBe(1);
    expect(parse("page=3").page).toBe(3);
  });
});

describe("serializeActionLogParams", () => {
  it("leaves defaults out so a shared link reads cleanly", () => {
    const search = serializeActionLogParams(parse("entityType=instructor&entityId=55"));

    expect(search.toString()).toBe("entityType=instructor&entityId=55");
  });

  it("round-trips every filter through the URL", () => {
    const original = parse(
      "actor=42&action=term_enable&entityType=term&entityId=202610&since=2026-01-01T00:00:00Z&page=2"
    );

    expect(parseActionLogParams(serializeActionLogParams(original))).toEqual(original);
  });
});

describe("countActiveFilters", () => {
  it("ignores pagination", () => {
    expect(countActiveFilters(parse("page=4"))).toBe(0);
    expect(countActiveFilters(parse("actor=42&entityId=7"))).toBe(2);
  });
});

describe("datetime-local conversion", () => {
  it("round-trips an instant through the local input format", () => {
    const iso = "2026-09-16T08:30:00.000Z";

    expect(fromLocalInput(toLocalInput(iso))).toBe(iso);
  });

  it("returns nothing for absent or unparseable values", () => {
    expect(toLocalInput(null)).toBe("");
    expect(toLocalInput("not a date")).toBe("");
    expect(fromLocalInput("")).toBeNull();
  });
});
