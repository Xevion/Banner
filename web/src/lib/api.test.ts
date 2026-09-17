import { beforeEach, describe, expect, it, vi } from "vitest";
import { BannerApiClient } from "./api";

global.fetch = vi.fn();

describe("BannerApiClient", () => {
  let apiClient: BannerApiClient;

  beforeEach(() => {
    apiClient = new BannerApiClient();
    // Resets rather than clears: a queued mockResolvedValueOnce a test did not
    // consume would otherwise answer the next test's first request.
    vi.resetAllMocks();
  });

  it("should fetch status data", async () => {
    const mockStatus = {
      status: "active" as const,
      version: "0.3.4",
      commit: "abc1234",
      services: {
        web: { name: "web", status: "active" as const },
        database: { name: "database", status: "connected" as const },
      },
    };

    vi.mocked(fetch).mockResolvedValueOnce({
      ok: true,
      json: () => Promise.resolve(mockStatus),
    } as Response);

    const result = await apiClient.getStatus();

    expect(fetch).toHaveBeenCalledWith("/api/status", undefined);
    expect(result.isOk).toBe(true);
    if (result.isOk) {
      expect(result.value).toEqual(mockStatus);
    }
  });

  it("should handle API errors", async () => {
    vi.mocked(fetch).mockResolvedValueOnce({
      ok: false,
      status: 500,
      statusText: "Internal Server Error",
      json: () => Promise.reject(new Error("no json")),
    } as unknown as Response);

    const result = await apiClient.getStatus();

    expect(result.isErr).toBe(true);
    if (result.isErr) {
      expect(result.error.message).toBe("API request failed: 500 Internal Server Error");
    }
  });

  it("should search courses with all params", async () => {
    const mockResponse = {
      items: [],
      total: 0,
      page: 1,
      perPage: 25,
    };

    vi.mocked(fetch).mockResolvedValueOnce({
      ok: true,
      json: () => Promise.resolve(mockResponse),
    } as Response);

    const result = await apiClient.searchCourses({
      term: "202420",
      subject: ["CS"],
      query: "data",
      openOnly: true,
      perPage: 25,
      page: 3,
    });

    expect(fetch).toHaveBeenCalledWith(
      "/api/courses/search?term=202420&subject=CS&query=data&openOnly=true&perPage=25&page=3",
      undefined
    );
    expect(result.isOk).toBe(true);
    if (result.isOk) {
      expect(result.value).toEqual(mockResponse);
    }
  });

  it("should search courses with minimal params", async () => {
    const mockResponse = {
      items: [],
      total: 0,
      page: 1,
      perPage: 25,
    };

    vi.mocked(fetch).mockResolvedValueOnce({
      ok: true,
      json: () => Promise.resolve(mockResponse),
    } as Response);

    const result = await apiClient.searchCourses({ term: "202420" });

    expect(fetch).toHaveBeenCalledWith("/api/courses/search?term=202420", undefined);
    expect(result.isOk).toBe(true);
  });

  it("should fetch reference data", async () => {
    const mockRef = [
      { code: "F", description: "Face to Face" },
      { code: "OL", description: "Online" },
    ];

    vi.mocked(fetch).mockResolvedValueOnce({
      ok: true,
      json: () => Promise.resolve(mockRef),
    } as Response);

    const result = await apiClient.getReference("instructional_methods");

    expect(fetch).toHaveBeenCalledWith("/api/reference/instructional_methods", undefined);
    expect(result.isOk).toBe(true);
    if (result.isOk) {
      expect(result.value).toEqual(mockRef);
    }
  });
});

describe("concurrent GETs", () => {
  beforeEach(() => {
    vi.resetAllMocks();
  });

  it("shares one request between callers asking at the same time", async () => {
    const sections = [{ crn: "12345" }];
    vi.mocked(fetch).mockResolvedValue({
      ok: true,
      json: () => Promise.resolve(sections),
    } as Response);

    const client = new BannerApiClient();
    const [first, second] = await Promise.all([
      client.getRelatedSections("fall-2026", "CS", "3443"),
      client.getRelatedSections("fall-2026", "CS", "3443"),
    ]);

    expect(fetch).toHaveBeenCalledTimes(1);
    expect(first.isOk).toBe(true);
    expect(second.isOk).toBe(true);
  });

  it("shares across client instances, since they all reach the same tab", async () => {
    vi.mocked(fetch).mockResolvedValue({
      ok: true,
      json: () => Promise.resolve([]),
    } as Response);

    await Promise.all([
      new BannerApiClient().getRelatedSections("fall-2026", "CS", "3443"),
      new BannerApiClient().getRelatedSections("fall-2026", "CS", "3443"),
    ]);

    expect(fetch).toHaveBeenCalledTimes(1);
  });

  it("keeps different URLs apart", async () => {
    vi.mocked(fetch).mockResolvedValue({
      ok: true,
      json: () => Promise.resolve([]),
    } as Response);

    const client = new BannerApiClient();
    await Promise.all([
      client.getRelatedSections("fall-2026", "CS", "3443"),
      client.getRelatedSections("fall-2026", "CS", "3743"),
    ]);

    expect(fetch).toHaveBeenCalledTimes(2);
  });

  it("holds nothing once a request settles", async () => {
    vi.mocked(fetch).mockResolvedValue({
      ok: true,
      json: () => Promise.resolve([]),
    } as Response);

    const client = new BannerApiClient();
    await client.getRelatedSections("fall-2026", "CS", "3443");
    await client.getRelatedSections("fall-2026", "CS", "3443");

    expect(fetch).toHaveBeenCalledTimes(2);
  });

  it("does not share a request that carries a body", async () => {
    vi.mocked(fetch).mockResolvedValue({
      ok: true,
      json: () => Promise.resolve({ trends: {} }),
    } as Response);

    const client = new BannerApiClient();
    await Promise.all([
      client.getCourseTrends("fall-2026", ["12345"]),
      client.getCourseTrends("fall-2026", ["12345"]),
    ]);

    expect(fetch).toHaveBeenCalledTimes(2);
  });
});
