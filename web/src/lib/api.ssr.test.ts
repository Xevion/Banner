import { beforeEach, describe, expect, it, vi } from "vitest";

// The client is a module singleton, so its caches are shared by every request
// the server handles. This file checks they stay inert there; the mock is
// hoisted over the whole module graph, so it needs a file of its own.
vi.mock("$app/environment", () => ({ browser: false }));

const { BannerApiClient } = await import("./api");

const OPTIONS = { terms: [], subjects: [], sorts: [] };

function respondWith(body: unknown) {
  return vi.fn().mockResolvedValue({ ok: true, json: () => Promise.resolve(body) });
}

describe("search options on the server", () => {
  let fetchFn: ReturnType<typeof respondWith>;

  beforeEach(() => {
    fetchFn = respondWith(OPTIONS);
  });

  it("asks the backend again rather than serving a cached copy", async () => {
    const client = new BannerApiClient(undefined, fetchFn);

    await client.getSearchOptions("202620");
    await client.getSearchOptions("202620");

    expect(fetchFn).toHaveBeenCalledTimes(2);
  });

  it("does not answer one client's request from another's", async () => {
    await new BannerApiClient(undefined, respondWith(OPTIONS)).getSearchOptions("202710");

    const second = respondWith(OPTIONS);
    await new BannerApiClient(undefined, second).getSearchOptions("202710");

    expect(second).toHaveBeenCalledTimes(1);
  });
});
