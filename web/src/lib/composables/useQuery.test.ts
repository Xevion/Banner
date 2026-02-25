import type { ApiErrorClass } from "$lib/api";
import { err, ok } from "true-myth/result";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { QueryController } from "./useQuery.svelte";

/** Helper to create a mock ApiErrorClass */
function mockApiError(message: string, code = "INTERNAL_ERROR"): ApiErrorClass {
  return { message, code, name: "ApiError", details: null } as unknown as ApiErrorClass;
}

describe("QueryController", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  describe("initial state", () => {
    it("starts with null data and no error", () => {
      const fetcher = vi.fn().mockResolvedValue(ok({ count: 42 }));
      const controller = new QueryController({ fetcher });

      expect(controller.data).toBeNull();
      expect(controller.error).toBeNull();
      expect(controller.isLoading).toBe(false);
    });

    it("uses initial data when provided", () => {
      const fetcher = vi.fn().mockResolvedValue(ok({ count: 42 }));
      const controller = new QueryController({ fetcher, initial: { count: 0 } });

      expect(controller.data).toEqual({ count: 0 });
      expect(controller.error).toBeNull();
      expect(controller.isLoading).toBe(false);
    });

    it("does not fetch automatically on construction", () => {
      const fetcher = vi.fn().mockResolvedValue(ok({ count: 42 }));
      new QueryController({ fetcher });

      expect(fetcher).not.toHaveBeenCalled();
    });
  });

  describe("fetch", () => {
    it("sets loading state and updates data on success", async () => {
      const fetcher = vi.fn().mockResolvedValue(ok({ count: 42 }));
      const controller = new QueryController({ fetcher });

      const fetchPromise = controller.fetch();
      expect(controller.isLoading).toBe(true);

      await fetchPromise;

      expect(controller.data).toEqual({ count: 42 });
      expect(controller.error).toBeNull();
      expect(controller.isLoading).toBe(false);
      expect(fetcher).toHaveBeenCalledTimes(1);
    });

    it("updates error and preserves previous data on error", async () => {
      const fetcher = vi
        .fn()
        .mockResolvedValueOnce(ok({ count: 42 }))
        .mockResolvedValueOnce(err(mockApiError("Network error")));

      const controller = new QueryController({ fetcher });

      // First fetch succeeds
      await controller.fetch();
      expect(controller.data).toEqual({ count: 42 });

      // Second fetch fails -- data preserved
      await controller.fetch();
      expect(controller.data).toEqual({ count: 42 });
      expect(controller.error).toEqual(mockApiError("Network error"));
      expect(controller.isLoading).toBe(false);
    });

    it("sets error on first fetch failure with no previous data", async () => {
      const apiError = mockApiError("Not found", "NOT_FOUND");
      const fetcher = vi.fn().mockResolvedValue(err(apiError));
      const controller = new QueryController({ fetcher });

      await controller.fetch();

      expect(controller.data).toBeNull();
      expect(controller.error).toEqual(apiError);
      expect(controller.isLoading).toBe(false);
    });

    it("clears error on successful fetch after failure", async () => {
      const apiError = mockApiError("Server error");
      const fetcher = vi
        .fn()
        .mockResolvedValueOnce(err(apiError))
        .mockResolvedValueOnce(ok({ count: 1 }));

      const controller = new QueryController({ fetcher });

      await controller.fetch();
      expect(controller.error).toEqual(apiError);

      await controller.fetch();
      expect(controller.error).toBeNull();
      expect(controller.data).toEqual({ count: 1 });
    });
  });

  describe("stale response handling", () => {
    it("ignores stale responses when a new fetch starts", async () => {
      let resolveFirst!: (value: unknown) => void;
      const firstPromise = new Promise((resolve) => {
        resolveFirst = resolve;
      });

      const fetcher = vi
        .fn()
        .mockReturnValueOnce(firstPromise)
        .mockResolvedValueOnce(ok({ count: 2 }));

      const controller = new QueryController({ fetcher });

      // Start first fetch (will hang)
      const fetch1 = controller.fetch();

      // Start second fetch -- should mark first as stale
      const fetch2 = controller.fetch();
      expect(fetcher).toHaveBeenCalledTimes(2);

      // Second completes first
      await fetch2;
      expect(controller.data).toEqual({ count: 2 });

      // First completes late -- should be ignored
      resolveFirst(ok({ count: 1 }));
      await fetch1;

      expect(controller.data).toEqual({ count: 2 }); // Still 2, not 1
    });
  });

  describe("debounce", () => {
    it("debounces fetch calls", async () => {
      const fetcher = vi.fn().mockResolvedValue(ok({ count: 42 }));
      const controller = new QueryController({ fetcher, debounce: 300 });

      controller.debouncedFetch();
      controller.debouncedFetch();
      controller.debouncedFetch();

      expect(fetcher).not.toHaveBeenCalled();

      await vi.advanceTimersByTimeAsync(300);

      expect(fetcher).toHaveBeenCalledTimes(1);
      expect(controller.data).toEqual({ count: 42 });
    });

    it("resets debounce timer on subsequent calls", async () => {
      const fetcher = vi.fn().mockResolvedValue(ok({ count: 42 }));
      const controller = new QueryController({ fetcher, debounce: 300 });

      controller.debouncedFetch();

      await vi.advanceTimersByTimeAsync(200);
      expect(fetcher).not.toHaveBeenCalled();

      // Reset the timer
      controller.debouncedFetch();

      await vi.advanceTimersByTimeAsync(200);
      expect(fetcher).not.toHaveBeenCalled(); // Still waiting

      await vi.advanceTimersByTimeAsync(100);
      expect(fetcher).toHaveBeenCalledTimes(1);
    });

    it("does not set isLoading during debounce wait", async () => {
      const fetcher = vi.fn().mockResolvedValue(ok({ count: 42 }));
      const controller = new QueryController({ fetcher, debounce: 300 });

      controller.debouncedFetch();

      expect(controller.isLoading).toBe(false);

      await vi.advanceTimersByTimeAsync(300);
      // After debounce fires, fetch starts -- but since it's async and resolved,
      // isLoading may have flipped. Let's just check fetcher was called.
      expect(fetcher).toHaveBeenCalledTimes(1);
    });

    it("fetches immediately when debounce is 0", () => {
      const fetcher = vi.fn().mockResolvedValue(ok({ count: 42 }));
      const controller = new QueryController({ fetcher, debounce: 0 });

      controller.debouncedFetch();

      // With debounce 0, it should call fetch directly (no setTimeout)
      expect(fetcher).toHaveBeenCalledTimes(1);
    });
  });

  describe("callbacks", () => {
    it("calls onSuccess after successful fetch", async () => {
      const onSuccess = vi.fn();
      const fetcher = vi.fn().mockResolvedValue(ok({ count: 42 }));
      const controller = new QueryController({ fetcher, onSuccess });

      await controller.fetch();

      expect(onSuccess).toHaveBeenCalledTimes(1);
      expect(onSuccess).toHaveBeenCalledWith({ count: 42 });
    });

    it("calls onError after failed fetch", async () => {
      const onError = vi.fn();
      const apiError = mockApiError("Server error");
      const fetcher = vi.fn().mockResolvedValue(err(apiError));
      const controller = new QueryController({ fetcher, onError });

      await controller.fetch();

      expect(onError).toHaveBeenCalledTimes(1);
      expect(onError).toHaveBeenCalledWith(apiError);
    });

    it("does not call onSuccess on error", async () => {
      const onSuccess = vi.fn();
      const fetcher = vi.fn().mockResolvedValue(err(mockApiError("fail")));
      const controller = new QueryController({ fetcher, onSuccess });

      await controller.fetch();

      expect(onSuccess).not.toHaveBeenCalled();
    });

    it("does not call onError on success", async () => {
      const onError = vi.fn();
      const fetcher = vi.fn().mockResolvedValue(ok({ count: 1 }));
      const controller = new QueryController({ fetcher, onError });

      await controller.fetch();

      expect(onError).not.toHaveBeenCalled();
    });
  });

  describe("destroy", () => {
    it("cancels pending debounce timers", async () => {
      const fetcher = vi.fn().mockResolvedValue(ok({ count: 42 }));
      const controller = new QueryController({ fetcher, debounce: 300 });

      controller.debouncedFetch();
      controller.destroy();

      await vi.advanceTimersByTimeAsync(300);

      expect(fetcher).not.toHaveBeenCalled();
    });

    it("prevents future fetches", async () => {
      const fetcher = vi.fn().mockResolvedValue(ok({ count: 42 }));
      const controller = new QueryController({ fetcher });

      controller.destroy();
      await controller.fetch();

      expect(fetcher).not.toHaveBeenCalled();
    });

    it("ignores in-flight response after destroy", async () => {
      let resolveFetch!: (value: unknown) => void;
      const fetchPromise = new Promise((resolve) => {
        resolveFetch = resolve;
      });
      const fetcher = vi.fn().mockReturnValue(fetchPromise);
      const controller = new QueryController({ fetcher });

      const promise = controller.fetch();
      controller.destroy();

      resolveFetch(ok({ count: 42 }));
      await promise;

      expect(controller.data).toBeNull(); // Not updated -- destroyed
    });
  });
});
