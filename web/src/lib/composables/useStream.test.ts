import type { ScrapeJobDto, ScrapeJobEvent } from "$lib/bindings";
/**
 * Unit tests for stream reducer utilities.
 */
import { describe, expect, it } from "vitest";
import { addItem, removeById, updateById } from "./reducers";

// Helper to create mock ScrapeJobDto
function mockJob(overrides: Partial<ScrapeJobDto> = {}): ScrapeJobDto {
  return {
    id: 1,
    targetType: "subject",
    targetPayload: { subject: "CS", term: "202620" },
    priority: "low",
    retryCount: 0,
    maxRetries: 3,
    queuedAt: "2024-01-01T00:00:00Z",
    executeAt: "2024-01-01T00:00:00Z",
    lockedAt: null,
    createdAt: "2024-01-01T00:00:00Z",
    status: "pending",
    ...overrides,
  };
}

describe("updateById", () => {
  it("updates the matching item by id", () => {
    const jobs = [mockJob({ id: 1 }), mockJob({ id: 2 }), mockJob({ id: 3 })];

    const result = updateById(jobs, 2, { lockedAt: "2024-01-01T12:00:00Z" });

    expect(result).toHaveLength(3);
    expect(result[0].lockedAt).toBeNull();
    expect(result[1].lockedAt).toBe("2024-01-01T12:00:00Z");
    expect(result[2].lockedAt).toBeNull();
  });

  it("preserves other properties when updating", () => {
    const jobs = [mockJob({ id: 1, priority: "high", retryCount: 2 })];

    const result = updateById(jobs, 1, { retryCount: 3 });

    expect(result[0].priority).toBe("high");
    expect(result[0].retryCount).toBe(3);
  });

  it("returns a new array (immutable)", () => {
    const jobs = [mockJob({ id: 1 })];

    const result = updateById(jobs, 1, { lockedAt: "2024-01-01T12:00:00Z" });

    expect(result).not.toBe(jobs);
    expect(result[0]).not.toBe(jobs[0]);
  });

  it("handles item not found gracefully", () => {
    const jobs = [mockJob({ id: 1 })];

    const result = updateById(jobs, 999, { lockedAt: "2024-01-01T12:00:00Z" });

    expect(result).toHaveLength(1);
    expect(result[0].lockedAt).toBeNull();
  });

  it("handles empty array", () => {
    const result = updateById([] as ScrapeJobDto[], 1, { lockedAt: "2024-01-01T12:00:00Z" });

    expect(result).toHaveLength(0);
  });
});

describe("removeById", () => {
  it("removes the item with matching id", () => {
    const jobs = [mockJob({ id: 1 }), mockJob({ id: 2 }), mockJob({ id: 3 })];

    const result = removeById(jobs, 2);

    expect(result).toHaveLength(2);
    expect(result.map((j) => j.id)).toEqual([1, 3]);
  });

  it("returns a new array (immutable)", () => {
    const jobs = [mockJob({ id: 1 }), mockJob({ id: 2 })];

    const result = removeById(jobs, 1);

    expect(result).not.toBe(jobs);
  });

  it("handles item not found gracefully", () => {
    const jobs = [mockJob({ id: 1 })];

    const result = removeById(jobs, 999);

    expect(result).toHaveLength(1);
  });

  it("handles empty array", () => {
    const result = removeById([] as ScrapeJobDto[], 1);

    expect(result).toHaveLength(0);
  });
});

describe("addItem", () => {
  it("adds item to the end of array without sort", () => {
    const jobs = [mockJob({ id: 1 }), mockJob({ id: 2 })];
    const newJob = mockJob({ id: 3 });

    const result = addItem(jobs, newJob);

    expect(result).toHaveLength(3);
    expect(result[2].id).toBe(3);
  });

  it("sorts items when sort function provided", () => {
    const jobs = [mockJob({ id: 2 }), mockJob({ id: 1 })];
    const newJob = mockJob({ id: 3 });

    const result = addItem(jobs, newJob, (a, b) => a.id - b.id);

    expect(result.map((j) => j.id)).toEqual([1, 2, 3]);
  });

  it("returns a new array (immutable)", () => {
    const jobs = [mockJob({ id: 1 })];
    const newJob = mockJob({ id: 2 });

    const result = addItem(jobs, newJob);

    expect(result).not.toBe(jobs);
  });

  it("handles empty array", () => {
    const newJob = mockJob({ id: 1 });

    const result = addItem([] as ScrapeJobDto[], newJob);

    expect(result).toHaveLength(1);
    expect(result[0].id).toBe(1);
  });
});

describe("reducer patterns for scrape jobs", () => {
  describe("created event", () => {
    it("adds the new job to the list", () => {
      const jobs = [mockJob({ id: 1 })];
      const event: ScrapeJobEvent = {
        type: "created",
        job: mockJob({ id: 2 }),
      };

      const result = addItem(jobs, event.job);

      expect(result).toHaveLength(2);
      expect(result.find((j) => j.id === 2)).toBeDefined();
    });
  });

  describe("locked event", () => {
    it("updates the job with locked status", () => {
      const jobs = [mockJob({ id: 1, lockedAt: null }), mockJob({ id: 2, lockedAt: null })];
      const event: ScrapeJobEvent = {
        type: "locked",
        id: 1,
        lockedAt: "2024-01-01T12:00:00Z",
        status: "pending",
      };

      const result = updateById(jobs, event.id, {
        lockedAt: event.lockedAt,
      });

      expect(result[0].lockedAt).toBe("2024-01-01T12:00:00Z");
      expect(result[1].lockedAt).toBeNull();
    });
  });

  describe("completed event", () => {
    it("removes the job from the list", () => {
      const jobs = [mockJob({ id: 1 }), mockJob({ id: 2 })];
      const event: ScrapeJobEvent = {
        type: "completed",
        id: 1,
        subject: "CS",
      };

      const result = removeById(jobs, event.id);

      expect(result).toHaveLength(1);
      expect(result[0].id).toBe(2);
    });
  });

  describe("retried event", () => {
    it("updates retry count and clears lock", () => {
      const jobs = [mockJob({ id: 1, retryCount: 0, lockedAt: "2024-01-01T10:00:00Z" })];
      const event: ScrapeJobEvent = {
        type: "retried",
        id: 1,
        retryCount: 1,
        queuedAt: "2024-01-01T12:00:00Z",
        status: "pending",
      };

      const result = updateById(jobs, event.id, {
        retryCount: event.retryCount,
        queuedAt: event.queuedAt,
        lockedAt: null,
      });

      expect(result[0].retryCount).toBe(1);
      expect(result[0].queuedAt).toBe("2024-01-01T12:00:00Z");
      expect(result[0].lockedAt).toBeNull();
    });
  });

  describe("exhausted event", () => {
    it("updates the job status to exhausted", () => {
      const jobs = [mockJob({ id: 1 })];
      const event: ScrapeJobEvent = {
        type: "exhausted",
        id: 1,
      };

      const result = updateById(jobs, event.id, { status: "exhausted" });

      expect(result).toHaveLength(1);
      expect(result[0].status).toBe("exhausted");
    });
  });

  describe("deleted event", () => {
    it("removes the job from the list", () => {
      const jobs = [mockJob({ id: 1 }), mockJob({ id: 2 }), mockJob({ id: 3 })];
      const event: ScrapeJobEvent = {
        type: "deleted",
        id: 2,
      };

      const result = removeById(jobs, event.id);

      expect(result).toHaveLength(2);
      expect(result.map((j) => j.id)).toEqual([1, 3]);
    });
  });
});
