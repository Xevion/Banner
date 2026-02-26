import { env } from "$env/dynamic/private";
import type { Handle, HandleServerError } from "@sveltejs/kit";
import { PostHog } from "posthog-node";

const backendUrl = env.BACKEND_URL ?? "http://localhost:8080";

const posthog =
  env.POSTHOG_KEY && env.POSTHOG_HOST
    ? new PostHog(env.POSTHOG_KEY, { host: env.POSTHOG_HOST })
    : null;

export const handle: Handle = async ({ event, resolve }) => {
  const { method } = event.request;
  const { pathname } = event.url;

  if (pathname.startsWith("/api/")) {
    const targetUrl = `${backendUrl}${pathname}${event.url.search}`;
    const headers = new Headers(event.request.headers);
    headers.delete("host");

    let response: Response;
    try {
      response = await fetch(targetUrl, {
        method,
        headers,
        body: event.request.body,
        redirect: "manual",
        // @ts-expect-error Bun supports duplex streaming
        duplex: "half",
      });

      // Follow backend-to-backend redirects that stay within /api/.
      // External redirects (OAuth, frontend routes) pass through to the
      // browser so Set-Cookie headers and redirect destinations are preserved.
      // 307/308 preserve the original method; 301/302/303 convert to GET.
      if (response.status >= 300 && response.status < 400) {
        const location = response.headers.get("location");
        if (location?.startsWith("/api/")) {
          const redirectMethod =
            response.status === 307 || response.status === 308 ? method : "GET";
          response = await fetch(`${backendUrl}${location}`, {
            method: redirectMethod,
            headers,
            body: redirectMethod === method ? event.request.body : undefined,
            redirect: "manual",
            // @ts-expect-error Bun supports duplex streaming
            duplex: redirectMethod !== "GET" ? "half" : undefined,
          });
        }
      }
    } catch {
      return new Response(JSON.stringify({ error: "Backend unavailable" }), {
        status: 502,
        headers: { "content-type": "application/json" },
      });
    }

    return new Response(response.body, {
      status: response.status,
      statusText: response.statusText,
      headers: response.headers,
    });
  }

  return resolve(event);
};

export const handleError: HandleServerError = ({ error, event, status }) => {
  const errorId = crypto.randomUUID();
  const timestamp = new Date().toISOString();
  const errorMessage = error instanceof Error ? error.message : String(error);
  const stack = error instanceof Error ? error.stack : undefined;

  if (posthog && status !== 404) {
    posthog.captureException(error, event.request.headers.get("x-request-id") ?? errorId, {
      method: event.request.method,
      path: event.url.pathname,
      status,
      errorId,
    });
  }

  return {
    message: status === 404 ? "Not Found" : errorMessage,
    errorId,
    timestamp,
    stack,
  };
};
