import { describe, expect, it, vi } from "vitest";
import type { SortKey, SortKeyOption } from "$lib/bindings";
import { formatSort, parseSort, type SortTerm } from "$lib/sort";
import { SortController } from "./useSort.svelte";

const option = (key: SortKey, ascLabel: string, descLabel: string): SortKeyOption => ({
  key,
  ascLabel,
  descLabel,
});

const CATALOG: SortKeyOption[] = [
  option("start_time", "Earliest first", "Latest first"),
  option("duration", "Shortest first", "Longest first"),
  option("weekly_minutes", "Least time per week", "Most time per week"),
  option("days", "Earliest weekday first", "Latest weekday first"),
  option("seats_open", "Nearly full", "Most seats open"),
  option("fill_ratio", "Emptiest first", "Fullest first"),
  option("wait_count", "Shortest waitlist", "Longest waitlist"),
];

const asc = (key: SortKey): SortTerm => ({ key, desc: false });

function make(initial: SortTerm[] = [], onChange?: (terms: SortTerm[]) => void) {
  return new SortController({ catalog: () => CATALOG, initial, onChange });
}

describe("terms", () => {
  it("starts from the terms it was given", () => {
    expect(make([asc("days"), { key: "start_time", desc: true }]).terms).toEqual([
      { key: "days", desc: false },
      { key: "start_time", desc: true },
    ]);
  });

  it("reports whether a key is sorting and which way round", () => {
    const sort = make([{ key: "duration", desc: true }]);
    expect(sort.isActive("duration")).toBe(true);
    expect(sort.directionOf("duration")).toBe("desc");
    expect(sort.isActive("days")).toBe(false);
    expect(sort.directionOf("days")).toBeNull();
    expect(sort.indexOf("days")).toBe(-1);
  });
});

describe("append", () => {
  it("adds a key as the lowest-priority tiebreaker", () => {
    const sort = make([asc("days")]);
    expect(sort.append({ key: "start_time", desc: true })).toBe(true);
    expect(sort.terms).toEqual([
      { key: "days", desc: false },
      { key: "start_time", desc: true },
    ]);
  });

  it("refuses a key that is already sorting", () => {
    const sort = make([asc("days")]);
    expect(sort.append({ key: "days", desc: true })).toBe(false);
    expect(sort.terms).toEqual([{ key: "days", desc: false }]);
  });

  it("fills to the cap and then refuses instead of dropping the oldest term", () => {
    const sort = make();
    for (const key of ["days", "start_time", "duration", "seats_open"] as const) {
      expect(sort.append(asc(key))).toBe(true);
    }
    expect(sort.isFull).toBe(true);
    expect(sort.append(asc("fill_ratio"))).toBe(false);
    expect(sort.terms.map((term) => term.key)).toEqual([
      "days",
      "start_time",
      "duration",
      "seats_open",
    ]);
  });

  it("takes the cap from its options", () => {
    const sort = new SortController({ catalog: () => CATALOG, maxTerms: 1 });
    expect(sort.append(asc("days"))).toBe(true);
    expect(sort.append(asc("start_time"))).toBe(false);
  });
});

describe("remove", () => {
  it("drops one key and keeps the order of the rest", () => {
    const sort = make([asc("days"), asc("start_time"), asc("duration")]);
    expect(sort.remove("start_time")).toBe(true);
    expect(sort.terms.map((term) => term.key)).toEqual(["days", "duration"]);
  });

  it("reports nothing removed for an inactive key", () => {
    const sort = make([asc("days")]);
    expect(sort.remove("wait_count")).toBe(false);
  });
});

describe("reorder", () => {
  it("shifts a key one place up", () => {
    const sort = make([asc("days"), asc("start_time"), asc("duration")]);
    expect(sort.move("duration", "up")).toBe(true);
    expect(sort.terms.map((term) => term.key)).toEqual(["days", "duration", "start_time"]);
  });

  it("shifts a key one place down", () => {
    const sort = make([asc("days"), asc("start_time"), asc("duration")]);
    expect(sort.move("days", "down")).toBe(true);
    expect(sort.terms.map((term) => term.key)).toEqual(["start_time", "days", "duration"]);
  });

  it("refuses to move past either end", () => {
    const sort = make([asc("days"), asc("start_time")]);
    expect(sort.move("days", "up")).toBe(false);
    expect(sort.move("start_time", "down")).toBe(false);
    expect(sort.move("wait_count", "up")).toBe(false);
    expect(sort.terms.map((term) => term.key)).toEqual(["days", "start_time"]);
  });
});

describe("direction", () => {
  it("flips a key in place", () => {
    const sort = make([asc("days"), asc("start_time")]);
    expect(sort.toggleDirection("start_time")).toBe(true);
    expect(sort.terms).toEqual([
      { key: "days", desc: false },
      { key: "start_time", desc: true },
    ]);
    expect(sort.toggleDirection("start_time")).toBe(true);
    expect(sort.directionOf("start_time")).toBe("asc");
  });

  it("reports no change when the key already points that way", () => {
    const sort = make([asc("days")]);
    expect(sort.setDirection("days", false)).toBe(false);
    expect(sort.setDirection("wait_count", true)).toBe(false);
  });
});

describe("replace and clear", () => {
  it("replaces every term with the one given", () => {
    const sort = make([asc("days"), asc("start_time"), asc("duration")]);
    sort.replace({ key: "fill_ratio", desc: true });
    expect(sort.terms).toEqual([{ key: "fill_ratio", desc: true }]);
  });

  it("clears every term", () => {
    const sort = make([asc("days"), asc("start_time")]);
    sort.clear();
    expect(sort.terms).toEqual([]);
    expect(sort.isEmpty).toBe(true);
  });

  it("routes a header click through the replace policy", () => {
    const sort = make([asc("days"), asc("start_time")]);
    sort.applyHeaderClick({ key: "duration", desc: true });
    expect(sort.terms).toEqual([{ key: "duration", desc: true }]);
    sort.applyHeaderClick(null);
    expect(sort.terms).toEqual([]);
  });
});

describe("catalogue", () => {
  it("offers only the keys with no term yet", () => {
    const sort = make([asc("days"), asc("fill_ratio")]);
    expect(sort.available.map((entry) => entry.key)).toEqual([
      "start_time",
      "duration",
      "weekly_minutes",
      "seats_open",
      "wait_count",
    ]);
  });

  it("names a key from the catalogue, each way round", () => {
    const sort = make();
    expect(sort.label("seats_open", false)).toBe("Nearly full");
    expect(sort.label("seats_open", true)).toBe("Most seats open");
    expect(sort.labelOf({ key: "fill_ratio", desc: true })).toBe("Fullest first");
  });

  it("falls back to the key when the catalogue has not arrived", () => {
    const sort = new SortController({ catalog: () => [] });
    expect(sort.label("seats_open", false)).toBe("seats_open");
  });
});

describe("change notification", () => {
  it("reports every change made through the controller", () => {
    const onChange = vi.fn();
    const sort = make([], onChange);
    sort.append(asc("days"));
    sort.toggleDirection("days");
    expect(onChange).toHaveBeenCalledTimes(2);
    expect(onChange).toHaveBeenLastCalledWith([{ key: "days", desc: true }]);
  });

  it("stays quiet when adopting terms decided elsewhere", () => {
    const onChange = vi.fn();
    const sort = make([], onChange);
    sort.sync([asc("days")]);
    expect(sort.terms).toEqual([{ key: "days", desc: false }]);
    expect(onChange).not.toHaveBeenCalled();
  });
});

describe("wire format", () => {
  it("round-trips a multi-term sort built through the controller", () => {
    const sort = make();
    sort.append(asc("days"));
    sort.append({ key: "start_time", desc: true });
    sort.append(asc("seats_open"));
    const raw = formatSort(sort.terms);
    expect(raw).toBe("days,-start_time,seats_open");
    expect(parseSort(raw)).toEqual([...sort.terms]);
  });
});
