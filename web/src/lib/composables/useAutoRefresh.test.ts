import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { AutoRefreshController } from "./useAutoRefresh.svelte";

describe("AutoRefreshController", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  describe("initial state", () => {
    it("starts with null data and no error", () => {
      const fetcher = vi.fn().mockResolvedValue({ count: 42 });
      const controller = new AutoRefreshController({ fetcher, interval: 5000 });

      expect(controller.data).toBeNull();
      expect(controller.error).toBeNull();
      expect(controller.isLoading).toBe(false);
      expect(controller.hasError).toBe(false);
    });

    it("does not fetch automatically on construction", () => {
      const fetcher = vi.fn().mockResolvedValue({ count: 42 });
      new AutoRefreshController({ fetcher, interval: 5000 });

      expect(fetcher).not.toHaveBeenCalled();
    });
  });

  describe("fetch", () => {
    it("sets loading state and fetches data", async () => {
      const fetcher = vi.fn().mockResolvedValue({ count: 42 });
      const controller = new AutoRefreshController({ fetcher, interval: 5000 });

      const fetchPromise = controller.fetch();
      expect(controller.isLoading).toBe(true);

      await fetchPromise;

      expect(controller.data).toEqual({ count: 42 });
      expect(controller.isLoading).toBe(false);
      expect(controller.error).toBeNull();
      expect(fetcher).toHaveBeenCalledTimes(1);

      controller.destroy(); // Prevent scheduled refresh
    });

    it("sets error state on fetch failure", async () => {
      const fetcher = vi.fn().mockRejectedValue(new Error("Network error"));
      const controller = new AutoRefreshController({ fetcher, interval: 5000 });

      await controller.fetch();

      expect(controller.data).toBeNull();
      expect(controller.error).toBe("Network error");
      expect(controller.hasError).toBe(true);
      expect(controller.isLoading).toBe(false);

      controller.destroy();
    });

    it("handles non-Error exceptions", async () => {
      const fetcher = vi.fn().mockRejectedValue("string error");
      const controller = new AutoRefreshController({ fetcher, interval: 5000 });

      await controller.fetch();

      expect(controller.error).toBe("Fetch failed");
      controller.destroy();
    });

    it("prevents concurrent fetches", async () => {
      let resolveFirst: (value: unknown) => void;
      const firstPromise = new Promise((resolve) => {
        resolveFirst = resolve;
      });
      const fetcher = vi.fn().mockReturnValueOnce(firstPromise).mockResolvedValue({ count: 2 });

      const controller = new AutoRefreshController({ fetcher, interval: 5000 });

      // Start first fetch
      const fetch1 = controller.fetch();
      expect(fetcher).toHaveBeenCalledTimes(1);

      // Try second fetch while first is in progress
      void controller.fetch();
      expect(fetcher).toHaveBeenCalledTimes(1); // Still 1 - blocked

      // Complete first fetch
      resolveFirst!({ count: 1 });
      await fetch1;

      expect(controller.data).toEqual({ count: 1 });
      controller.destroy();
    });
  });

  describe("auto-refresh", () => {
    it("schedules refresh after successful fetch", async () => {
      const fetcher = vi.fn().mockResolvedValue({ count: 1 });
      const controller = new AutoRefreshController({ fetcher, interval: 5000 });

      await controller.fetch();
      expect(fetcher).toHaveBeenCalledTimes(1);

      // Advance time to trigger refresh
      fetcher.mockResolvedValue({ count: 2 });
      await vi.advanceTimersByTimeAsync(5000);

      expect(fetcher).toHaveBeenCalledTimes(2);
      expect(controller.data).toEqual({ count: 2 });

      controller.destroy();
    });

    it("disables auto-refresh when interval is 0", async () => {
      const fetcher = vi.fn().mockResolvedValue({ count: 1 });
      const controller = new AutoRefreshController({ fetcher, interval: 0 });

      await controller.fetch();
      expect(fetcher).toHaveBeenCalledTimes(1);

      // Advance time - should not trigger another fetch
      await vi.advanceTimersByTimeAsync(60000);

      expect(fetcher).toHaveBeenCalledTimes(1);
      controller.destroy();
    });
  });

  describe("exponential backoff", () => {
    it("doubles interval on error", async () => {
      const fetcher = vi.fn().mockRejectedValue(new Error("fail"));
      const controller = new AutoRefreshController({ fetcher, interval: 5000, maxInterval: 60000 });

      // Initial fetch fails
      await controller.fetch();
      expect(fetcher).toHaveBeenCalledTimes(1);

      // Should wait 10000ms (doubled) for next attempt
      await vi.advanceTimersByTimeAsync(5000);
      expect(fetcher).toHaveBeenCalledTimes(1); // Not yet

      await vi.advanceTimersByTimeAsync(5000);
      expect(fetcher).toHaveBeenCalledTimes(2);

      controller.destroy();
    });

    it("caps backoff at maxInterval", async () => {
      const fetcher = vi.fn().mockRejectedValue(new Error("fail"));
      const controller = new AutoRefreshController({
        fetcher,
        interval: 30000,
        maxInterval: 60000,
      });

      // First failure: next interval = 60000
      await controller.fetch();

      // Second failure: would be 120000, but capped at 60000
      await vi.advanceTimersByTimeAsync(60000);
      expect(fetcher).toHaveBeenCalledTimes(2);

      // Third attempt at 60000 (capped)
      await vi.advanceTimersByTimeAsync(60000);
      expect(fetcher).toHaveBeenCalledTimes(3);

      controller.destroy();
    });

    it("resets interval on success after failures", async () => {
      const fetcher = vi
        .fn()
        .mockRejectedValueOnce(new Error("fail"))
        .mockResolvedValue({ ok: true });

      const controller = new AutoRefreshController({ fetcher, interval: 5000 });

      // First fetch fails
      await controller.fetch();

      // Second fetch succeeds after backoff (10000ms)
      await vi.advanceTimersByTimeAsync(10000);
      expect(fetcher).toHaveBeenCalledTimes(2);

      // Next refresh should be at base interval (5000ms)
      await vi.advanceTimersByTimeAsync(5000);
      expect(fetcher).toHaveBeenCalledTimes(3);

      controller.destroy();
    });
  });

  describe("pause and resume", () => {
    it("does not fetch when started paused", async () => {
      const fetcher = vi.fn().mockResolvedValue({ count: 1 });
      const controller = new AutoRefreshController({ fetcher, interval: 5000, paused: true });

      await controller.fetch();

      expect(fetcher).not.toHaveBeenCalled();
      expect(controller.data).toBeNull();
      controller.destroy();
    });

    it("resumes fetching when resume() is called", async () => {
      const fetcher = vi.fn().mockResolvedValue({ count: 1 });
      const controller = new AutoRefreshController({ fetcher, interval: 5000, paused: true });

      await controller.fetch();
      expect(fetcher).not.toHaveBeenCalled();

      controller.resume();
      // Wait for the fetch triggered by resume
      await vi.waitFor(() => expect(controller.data).toEqual({ count: 1 }));

      expect(fetcher).toHaveBeenCalledTimes(1);
      controller.destroy();
    });

    it("stops auto-refresh when pause() is called", async () => {
      const fetcher = vi.fn().mockResolvedValue({ count: 1 });
      const controller = new AutoRefreshController({ fetcher, interval: 5000 });

      await controller.fetch();
      expect(fetcher).toHaveBeenCalledTimes(1);

      controller.pause();

      // Advance time - should not trigger refresh
      await vi.advanceTimersByTimeAsync(10000);

      expect(fetcher).toHaveBeenCalledTimes(1);
      controller.destroy();
    });

    it("resume() is a no-op when not paused", async () => {
      const fetcher = vi.fn().mockResolvedValue({ count: 1 });
      const controller = new AutoRefreshController({ fetcher, interval: 5000 });

      await controller.fetch();
      expect(fetcher).toHaveBeenCalledTimes(1);

      // Resume when not paused - should not trigger extra fetch
      controller.resume();
      // Just verify no immediate second fetch
      expect(fetcher).toHaveBeenCalledTimes(1);

      controller.destroy();
    });
  });

  describe("trigger", () => {
    it("clears pending refresh and fetches immediately", async () => {
      const fetcher = vi.fn().mockResolvedValue({ count: 1 });
      const controller = new AutoRefreshController({ fetcher, interval: 5000 });

      await controller.fetch();
      expect(fetcher).toHaveBeenCalledTimes(1);

      // Advance part way to next refresh
      await vi.advanceTimersByTimeAsync(2000);

      // Trigger should fetch immediately and reset timer
      fetcher.mockResolvedValue({ count: 2 });
      controller.trigger();
      // Wait for fetch to complete
      await vi.waitFor(() => expect(controller.data).toEqual({ count: 2 }));

      expect(fetcher).toHaveBeenCalledTimes(2);

      // Next refresh should be 5000ms from now, not 3000ms
      fetcher.mockResolvedValue({ count: 3 });
      await vi.advanceTimersByTimeAsync(3000);
      expect(fetcher).toHaveBeenCalledTimes(2); // Not yet

      await vi.advanceTimersByTimeAsync(2000);
      expect(fetcher).toHaveBeenCalledTimes(3);

      controller.destroy();
    });
  });

  describe("minLoadingMs", () => {
    it("holds loading state for minimum duration", async () => {
      const fetcher = vi.fn().mockResolvedValue({ count: 1 });
      const controller = new AutoRefreshController({ fetcher, interval: 5000, minLoadingMs: 500 });

      const fetchPromise = controller.fetch();

      // Fetch completes immediately (mocked)
      await fetchPromise;

      // Loading should still be true (minLoadingMs not elapsed)
      expect(controller.isLoading).toBe(true);
      expect(controller.data).toEqual({ count: 1 });

      // After minLoadingMs, loading should be false
      await vi.advanceTimersByTimeAsync(500);

      expect(controller.isLoading).toBe(false);
      controller.destroy();
    });

    it("does not extend loading if fetch took longer than minLoadingMs", async () => {
      const fetcher = vi.fn().mockImplementation(async () => {
        // Simulate slow fetch by advancing timers during the fetch
        await new Promise((resolve) => setTimeout(resolve, 1000));
        return { count: 1 };
      });
      const controller = new AutoRefreshController({ fetcher, interval: 5000, minLoadingMs: 500 });

      const fetchPromise = controller.fetch();

      // Advance time past the fetch duration
      await vi.advanceTimersByTimeAsync(1000);
      await fetchPromise;

      // Loading should be false immediately (fetch took longer than minLoadingMs)
      expect(controller.isLoading).toBe(false);
      controller.destroy();
    });
  });

  describe("destroy", () => {
    it("cancels pending refresh", async () => {
      const fetcher = vi.fn().mockResolvedValue({ count: 1 });
      const controller = new AutoRefreshController({ fetcher, interval: 5000 });

      await controller.fetch();
      expect(fetcher).toHaveBeenCalledTimes(1);

      controller.destroy();

      // Advance time - should not trigger refresh
      await vi.advanceTimersByTimeAsync(10000);

      expect(fetcher).toHaveBeenCalledTimes(1);
    });

    it("prevents future fetches", async () => {
      const fetcher = vi.fn().mockResolvedValue({ count: 1 });
      const controller = new AutoRefreshController({ fetcher, interval: 5000 });

      controller.destroy();

      await controller.fetch();

      expect(fetcher).not.toHaveBeenCalled();
    });
  });
});
