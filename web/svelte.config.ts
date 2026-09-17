import { fileURLToPath } from "node:url";
import adapter from "@sveltejs/adapter-node";
import type { Config } from "@sveltejs/kit";
import { vitePreprocess } from "@sveltejs/vite-plugin-svelte";

const posthogHost = process.env.PUBLIC_POSTHOG_HOST ?? "https://us.posthog.com";

// Backend and frontend share one .env at the repository root. Resolved from this file rather
// than the working directory, because the dev server is launched from both root and web/.
const envDir = fileURLToPath(new URL("..", import.meta.url));

const config: Config = {
  preprocess: vitePreprocess(),
  kit: {
    // Rust serves /_app/* off disk and compress-assets.ts writes the encoded
    // variants it negotiates, so the adapter does neither.
    adapter: adapter({
      out: "build",
      precompress: false,
    }),
    env: { dir: envDir },
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
