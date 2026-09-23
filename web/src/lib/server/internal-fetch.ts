const FORWARDED = ["x-internal-token", "x-forwarded-for"] as const;

/**
 * Copies the backend's internal token and resolved client address from the page request onto an SSR API fetch.
 *
 * SvelteKit's server `fetch` forwards only cookies and authorization, so without this every SSR API call
 * reaches the backend with no client address and no rate-limit bypass. Only same-origin `/api/` requests
 * receive the headers, and only when the page request carried a token, since the token is a secret.
 */
export function withInternalHeaders(page: Request, request: Request): Request {
  const url = new URL(request.url);
  if (url.origin !== new URL(page.url).origin || !url.pathname.startsWith("/api/")) {
    return request;
  }
  if (!page.headers.has("x-internal-token")) {
    return request;
  }

  for (const name of FORWARDED) {
    const value = page.headers.get(name);
    if (value !== null) request.headers.set(name, value);
  }
  return request;
}
