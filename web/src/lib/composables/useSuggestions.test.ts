import type { ApiErrorClass } from "$lib/api";
import { err, ok } from "true-myth/result";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { SuggestionQuery } from "./useSuggestions.svelte";

function apiError(message: string): ApiErrorClass {
  return { message, code: "INTERNAL_ERROR", name: "ApiError", details: null } as ApiErrorClass;
}

/** Mirrors the shape both callers use: an empty value distinct from "no answer yet". */
function makeQuery(fetcher: (q: string) => Promise<ReturnType<typeof ok<string[]>>>) {
  return new SuggestionQuery<string[]>({ fetcher, empty: [], debounce: 250 });
}

describe("SuggestionQuery", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  describe("the short-query threshold", () => {
    it("never asks the server about a query too short to be meaningful", async () => {
      const fetcher = vi.fn().mockResolvedValue(ok(["a"]));
      const query = makeQuery(fetcher);

      query.setQuery("c");
      await vi.advanceTimersByTimeAsync(1000);

      expect(fetcher).not.toHaveBeenCalled();
      expect(query.open).toBe(false);
      expect(query.loading).toBe(false);
    });

    it("abandons a pending fetch when the query is cut back below the threshold", async () => {
      const fetcher = vi.fn().mockResolvedValue(ok(["a"]));
      const query = makeQuery(fetcher);

      query.setQuery("comp");
      query.setQuery("c");
      await vi.advanceTimersByTimeAsync(1000);

      expect(fetcher).not.toHaveBeenCalled();
    });

    it("forgets earlier results when the query is cut back below the threshold", async () => {
      const fetcher = vi.fn().mockResolvedValue(ok(["Computer Science"]));
      const query = makeQuery(fetcher);

      query.setQuery("comp");
      await vi.advanceTimersByTimeAsync(300);
      expect(query.data).toEqual(["Computer Science"]);

      query.setQuery("c");
      // Leaving them up would offer results that no longer match what is typed.
      expect(query.data).toEqual([]);
      expect(query.settled).toBe(false);
    });
  });

  describe("debouncing", () => {
    it("asks once for a burst of typing rather than once per keystroke", async () => {
      const fetcher = vi.fn().mockResolvedValue(ok(["a"]));
      const query = makeQuery(fetcher);

      for (const q of ["co", "com", "comp", "compu"]) {
        query.setQuery(q);
        await vi.advanceTimersByTimeAsync(50);
      }
      await vi.advanceTimersByTimeAsync(300);

      expect(fetcher).toHaveBeenCalledTimes(1);
      expect(fetcher).toHaveBeenCalledWith("compu");
    });

    it("reports itself busy from the keystroke, not from when the request leaves", () => {
      const fetcher = vi.fn().mockResolvedValue(ok(["a"]));
      const query = makeQuery(fetcher);

      query.setQuery("comp");

      // The spinner has to cover the debounce window too, or it flickers on
      // every pause in typing.
      expect(query.loading).toBe(true);
      expect(query.open).toBe(true);
    });
  });

  describe("stale responses", () => {
    it("keeps the answer to the newest query when an older one lands late", async () => {
      const fetcher = vi
        .fn()
        .mockImplementationOnce(
          () => new Promise((resolve) => setTimeout(() => resolve(ok(["slow"])), 500))
        )
        .mockResolvedValueOnce(ok(["fast"]));
      const query = makeQuery(fetcher);

      query.setQuery("slow");
      await vi.advanceTimersByTimeAsync(250);
      query.setQuery("fast");
      await vi.advanceTimersByTimeAsync(1000);

      expect(query.data).toEqual(["fast"]);
      expect(query.loading).toBe(false);
    });
  });

  describe("failure", () => {
    it("surfaces a server failure instead of passing it off as an empty result", async () => {
      const fetcher = vi.fn().mockResolvedValue(err(apiError("upstream is down")));
      const query = makeQuery(fetcher);

      query.setQuery("comp");
      await vi.advanceTimersByTimeAsync(300);

      // Reporting "no results found" for a broken request tells the user their
      // search matched nothing, which is a different and wrong answer.
      expect(query.error).toBe("upstream is down");
      expect(query.data).toEqual([]);
      expect(query.loading).toBe(false);
    });

    it("clears a previous failure once a later query succeeds", async () => {
      const fetcher = vi
        .fn()
        .mockResolvedValueOnce(err(apiError("upstream is down")))
        .mockResolvedValueOnce(ok(["Computer Science"]));
      const query = makeQuery(fetcher);

      query.setQuery("comp");
      await vi.advanceTimersByTimeAsync(300);
      expect(query.error).not.toBeNull();

      query.setQuery("math");
      await vi.advanceTimersByTimeAsync(300);
      expect(query.error).toBeNull();
    });
  });

  describe("settled", () => {
    it("separates having no answer yet from having an answer of nothing", async () => {
      const fetcher = vi.fn().mockResolvedValue(ok([]));
      const query = makeQuery(fetcher);

      query.setQuery("zzzz");
      expect(query.settled).toBe(false);

      await vi.advanceTimersByTimeAsync(300);
      expect(query.settled).toBe(true);
      expect(query.data).toEqual([]);
    });
  });

  describe("lifecycle", () => {
    it("drops everything when a selection is taken", async () => {
      const fetcher = vi.fn().mockResolvedValue(ok(["Computer Science"]));
      const query = makeQuery(fetcher);

      query.setQuery("comp");
      await vi.advanceTimersByTimeAsync(300);

      query.reset();

      expect(query.text).toBe("");
      expect(query.data).toEqual([]);
      expect(query.open).toBe(false);
      expect(query.settled).toBe(false);
    });

    it("does not fetch after being destroyed", async () => {
      const fetcher = vi.fn().mockResolvedValue(ok(["a"]));
      const query = makeQuery(fetcher);

      query.setQuery("comp");
      query.destroy();
      await vi.advanceTimersByTimeAsync(1000);

      expect(fetcher).not.toHaveBeenCalled();
    });
  });
});
