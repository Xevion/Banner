import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { storybookTest } from "@storybook/addon-vitest/vitest-plugin";
import { sveltekit } from "@sveltejs/kit/vite";
import tailwindcss from "@tailwindcss/vite";
import devtoolsJson from "vite-plugin-devtools-json";
import { defineConfig } from "vitest/config";

const dirname =
  typeof __dirname !== "undefined" ? __dirname : path.dirname(fileURLToPath(import.meta.url));

function getVersion() {
  const filename = "Cargo.toml";
  const paths = [resolve(dirname, filename), resolve(dirname, "..", filename)];

  for (const path of paths) {
    try {
      if (!existsSync(path)) continue;

      const content = readFileSync(path, "utf8");
      const match = /^version\s*=\s*"([^"]+)"/m.exec(content);

      if (match) return match[1];
    } catch {
      // Continue to next path
    }
  }

  return "unknown";
}

const version = getVersion();

export default defineConfig({
  plugins: [tailwindcss(), sveltekit(), devtoolsJson()],
  resolve: process.env.VITEST ? { conditions: ["browser"] } : undefined,
  test: {
    projects: [
      {
        extends: true,
        test: {
          name: "unit",
          globals: true,
          environment: "jsdom",
          include: ["src/**/*.test.ts"],
        },
      },

      {
        extends: true,
        plugins: [
          storybookTest({
            configDir: path.join(dirname, ".storybook"),
            storybookScript: "bun run storybook --ci",
          }),
        ],
        resolve: { conditions: ["svelte", "browser"] },
        test: {
          name: "storybook",
          browser: {
            enabled: true,
            provider: "playwright",
            headless: true,
            instances: [{ browser: "chromium" }],
          },
          setupFiles: ["./.storybook/vitest.setup.ts"],
        },
      },
    ],
  },
  clearScreen: false,
  server: {
    port: 3001,
    watch: { ignored: ["**/.svelte-kit/generated/**"] },
    proxy: {
      "/api": {
        target: "http://localhost:8080",
        changeOrigin: true,
        secure: false,
        ws: true,
      },
    },
  },
  build: { sourcemap: true },
  define: { __APP_VERSION__: JSON.stringify(version) },
});
