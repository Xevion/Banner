import { vitePreprocess } from "@sveltejs/vite-plugin-svelte";
import adapter from "@sveltejs/adapter-node";

const posthogHost = process.env.PUBLIC_POSTHOG_HOST || "https://us.posthog.com";

/** @type {import('@sveltejs/kit').Config} */
const config = {
  preprocess: vitePreprocess(),
  kit: {
    // Rust serves /_app/* from the embedded binary and pre-compresses assets at
    // build time, so the adapter does neither.
    adapter: adapter({
      out: "build",
      precompress: false,
    }),
    csp: {
      mode: "auto",
      reportOnly: {
        "default-src": ["self"],
        "script-src": ["self", posthogHost],
        "script-src-attr": ["unsafe-inline"],
        "style-src": ["self", "unsafe-inline"],
        "img-src": ["self", "data:", "https://cdn.discordapp.com"],
        "connect-src": [
          "self",
          posthogHost,
          ...(process.env.NODE_ENV !== "production" ? ["ws://localhost:3001"] : []),
        ],
        "font-src": ["self", "data:"],
        "frame-ancestors": ["none"],
        "base-uri": ["self"],
        "form-action": ["self"],
        "object-src": ["none"],
        "report-uri": ["/api/csp-report"],
      },
    },
  },
};

export default config;
