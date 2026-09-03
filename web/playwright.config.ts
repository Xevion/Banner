import { defineConfig, devices } from "@playwright/test";

const appPort = 4173;
const stubPort = 8788;
const baseURL = `http://localhost:${appPort}`;

export default defineConfig({
  testDir: "e2e",
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: 0,
  workers: 1,
  reporter: process.env.CI ? [["github"], ["list"]] : [["list"]],
  use: { baseURL, trace: "on-first-retry" },
  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"] } }],
  webServer: [
    {
      command: "bun e2e/stub-api.ts",
      port: stubPort,
      reuseExistingServer: false,
    },
    {
      // adapter-node output, started the way the container starts it. Nothing
      // in this path can fall back to a dev server or to Vite's transform
      // pipeline, so the tests only ever see the rolldown bundle.
      command: "bun run build && node build/index.js",
      port: appPort,
      reuseExistingServer: false,
      timeout: 180_000,
      env: {
        PORT: String(appPort),
        ORIGIN: baseURL,
        BACKEND_URL: `http://127.0.0.1:${stubPort}`,
      },
    },
  ],
});
