import { vitePreprocess } from "@sveltejs/vite-plugin-svelte";
import adapter from "@xevion/svelte-adapter-bun";

const posthogHost = process.env.PUBLIC_POSTHOG_HOST || "https://us.posthog.com";

/** @type {import('@sveltejs/kit').Config} */
const config = {
  preprocess: vitePreprocess(),
  kit: {
    adapter: adapter({
      out: "build",
      precompress: false,
      serveAssets: false,
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
