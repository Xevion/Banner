import { type Result, err, ok } from "true-myth/result";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { TimeRange, TimelineResponse } from "$lib/bindings";

type GetTimeline = (ranges: TimeRange[]) => Promise<Result<TimelineResponse, Error>>;

const { getTimeline } = vi.hoisted(() => ({ getTimeline: vi.fn<GetTimeline>() }));
vi.mock("$lib/api", () => ({ client: { getTimeline } }));

const { createTimelineStore } = await import("./store.svelte");

const SLOT = 15 * 60 * 1000;
const THROTTLE_MS = 500;

/** Milliseconds `n` slots after the epoch. */
const ms = (n: number) => n * SLOT;

/** ISO timestamp `n` slots after the epoch. */
const iso = (n: number) => new Date(ms(n)).toISOString();

/** Build a response from `[slotIndex, subjects]` pairs. */
function response(slots: [number, Record<string, number>][]): TimelineResponse {
  return {
    slots: slots.map(([n, subjects]) => ({ time: iso(n), subjects })),
    subjects: [...new Set(slots.flatMap(([, subjects]) => Object.keys(subjects)))],
  };
}

/** The ranges passed to the API on the nth call. */
const rangesOf = (call: number): TimeRange[] => getTimeline.mock.calls[call][0];

/**
 * A 20-slot viewport starting at slot `n`. The store's 15 % buffer is exactly
 * three slots at this span, so the requested range is [n - 3, n + 23] with no
 * rounding -- which keeps the range assertions below readable.
 */
const view = (store: { requestRange: (a: number, b: number) => void }, n: number) =>
  store.requestRange(ms(n), ms(n + 20));

/** Advance throttle timers and flush the promises they start. */
const tick = (advanceMs = 0) => vi.advanceTimersByTimeAsync(advanceMs);

describe("timeline store", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    getTimeline.mockReset();
    getTimeline.mockResolvedValue(ok(response([])));
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  describe("data", () => {
    it("surfaces fetched slots and subjects", async () => {
      getTimeline.mockResolvedValue(
        ok(
          response([
            [100, { CS: 5, MAT: 3 }],
            [101, { CS: 7 }],
          ])
        )
      );
      const store = createTimelineStore();

      // Read before the fetch resolves, the way a component's first render
      // does. Reading only afterwards would evaluate the derivation against
      // the already-populated map and hide a broken subscription.
      expect(store.data).toEqual([]);
      expect(store.subjects).toEqual([]);

      view(store, 100);
      await tick();

      expect(store.data.map((slot) => slot.time.getTime())).toEqual([ms(100), ms(101)]);
      expect(store.data[0].subjects).toEqual({ CS: 5, MAT: 3 });
      expect(store.subjects).toEqual(["CS", "MAT"]);
    });

    it("keeps reflecting later fetches", async () => {
      getTimeline.mockResolvedValueOnce(ok(response([[100, { CS: 5 }]])));
      const store = createTimelineStore();
      expect(store.data).toEqual([]);

      view(store, 100);
      await tick();
      expect(store.data).toHaveLength(1);

      getTimeline.mockResolvedValueOnce(ok(response([[124, { BIO: 2 }]])));
      view(store, 110);
      await tick(THROTTLE_MS);

      expect(store.data.map((slot) => slot.time.getTime())).toEqual([ms(100), ms(124)]);
      expect(store.subjects).toEqual(["BIO", "CS"]);
    });

    it("sorts slots by time regardless of arrival order", async () => {
      getTimeline.mockResolvedValue(
        ok(
          response([
            [105, { CS: 1 }],
            [100, { CS: 2 }],
            [103, { CS: 3 }],
          ])
        )
      );
      const store = createTimelineStore();
      expect(store.data).toEqual([]);

      view(store, 100);
      await tick();

      expect(store.data.map((slot) => slot.time.getTime())).toEqual([ms(100), ms(103), ms(105)]);
    });
  });

  describe("gap tracking", () => {
    it("requests the buffered viewport on the first call", async () => {
      const store = createTimelineStore();
      view(store, 100);
      await tick();

      expect(getTimeline).toHaveBeenCalledTimes(1);
      expect(rangesOf(0)).toEqual([{ start: iso(97), end: iso(123) }]);
    });

    it("does not refetch a range it already has", async () => {
      const store = createTimelineStore();
      view(store, 100);
      await tick();

      view(store, 100);
      await tick(THROTTLE_MS);

      expect(getTimeline).toHaveBeenCalledTimes(1);
    });

    it("fetches only the tail when the viewport moves forward", async () => {
      const store = createTimelineStore();
      view(store, 100);
      await tick();

      view(store, 110);
      await tick(THROTTLE_MS);

      expect(getTimeline).toHaveBeenCalledTimes(2);
      expect(rangesOf(1)).toEqual([{ start: iso(123), end: iso(133) }]);
    });

    it("splits one request around a range already loaded in the middle", async () => {
      const store = createTimelineStore();
      view(store, 100);
      await tick();

      // A 40-slot window buffers to six slots a side: [84, 136].
      store.requestRange(ms(90), ms(130));
      await tick(THROTTLE_MS);

      expect(rangesOf(1)).toEqual([
        { start: iso(84), end: iso(97) },
        { start: iso(123), end: iso(136) },
      ]);
    });

    it("aligns unaligned viewport bounds outward to slot boundaries", async () => {
      const store = createTimelineStore();
      store.requestRange(ms(100) + 1, ms(120) - 1);
      await tick();

      const [range] = rangesOf(0);
      expect(new Date(range.start).getTime()).toBeLessThanOrEqual(ms(100));
      expect(new Date(range.end).getTime()).toBeGreaterThanOrEqual(ms(120));
      expect(new Date(range.start).getTime() % SLOT).toBe(0);
      expect(new Date(range.end).getTime() % SLOT).toBe(0);
    });
  });

  describe("throttling", () => {
    it("collapses a burst of viewport changes into one request", async () => {
      const store = createTimelineStore();
      view(store, 100);
      await tick();

      for (let n = 101; n <= 110; n++) view(store, n);
      await tick(THROTTLE_MS);

      expect(getTimeline).toHaveBeenCalledTimes(2);
      // The burst's last viewport wins, not the one that started the timer.
      expect(rangesOf(1)).toEqual([{ start: iso(123), end: iso(133) }]);
    });

    it("drops a pending request on dispose", async () => {
      const store = createTimelineStore();
      view(store, 100);
      await tick();

      view(store, 200);
      store.dispose();
      await tick(THROTTLE_MS);

      expect(getTimeline).toHaveBeenCalledTimes(1);
    });
  });

  describe("failures", () => {
    it("keeps existing data and retries the range later", async () => {
      const consoleError = vi.spyOn(console, "error").mockImplementation(() => undefined);
      getTimeline.mockResolvedValueOnce(ok(response([[100, { CS: 5 }]])));
      const store = createTimelineStore();
      expect(store.data).toEqual([]);

      view(store, 100);
      await tick();

      getTimeline.mockResolvedValueOnce(err(new Error("boom")));
      view(store, 110);
      await tick(THROTTLE_MS);

      expect(store.data).toHaveLength(1);
      expect(consoleError).toHaveBeenCalled();

      // The failed gap was never marked loaded, so it is asked for again.
      getTimeline.mockResolvedValueOnce(ok(response([[124, { BIO: 2 }]])));
      view(store, 110);
      await tick(THROTTLE_MS);

      expect(rangesOf(2)).toEqual([{ start: iso(123), end: iso(133) }]);
      expect(store.data).toHaveLength(2);

      consoleError.mockRestore();
    });
  });
});
