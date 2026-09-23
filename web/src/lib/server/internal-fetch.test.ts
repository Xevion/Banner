import { describe, expect, it } from "vitest";
import { withInternalHeaders } from "./internal-fetch";

const ORIGIN = "https://banner.xevion.dev";

function pageRequest(headers: Record<string, string>): Request {
  return new Request(`${ORIGIN}/courses`, { headers });
}

const PROXIED_PAGE = pageRequest({
  "x-internal-token": "secret",
  "x-forwarded-for": "203.0.113.7",
});

describe("SSR fetches to the backend", () => {
  it("carry the page's internal token and client address to same-origin API calls", () => {
    const out = withInternalHeaders(PROXIED_PAGE, new Request(`${ORIGIN}/api/auth/me`));

    expect(out.headers.get("x-internal-token")).toBe("secret");
    expect(out.headers.get("x-forwarded-for")).toBe("203.0.113.7");
  });

  it("never send the token to another origin", () => {
    const out = withInternalHeaders(PROXIED_PAGE, new Request("https://example.com/api/auth/me"));

    expect(out.headers.has("x-internal-token")).toBe(false);
    expect(out.headers.has("x-forwarded-for")).toBe(false);
  });

  it("never send the token to a same-origin path outside the API", () => {
    const out = withInternalHeaders(PROXIED_PAGE, new Request(`${ORIGIN}/sitemap.xml`));

    expect(out.headers.has("x-internal-token")).toBe(false);
  });

  it("add nothing when the page request carried no token", () => {
    const out = withInternalHeaders(pageRequest({}), new Request(`${ORIGIN}/api/auth/me`));

    expect(out.headers.has("x-internal-token")).toBe(false);
    expect(out.headers.has("x-forwarded-for")).toBe(false);
  });
});
