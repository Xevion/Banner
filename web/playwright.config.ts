import { defineConfig, devices } from "@playwright/test";

function randomPort(): number {
  return 30000 + Math.floor(Math.random() * 20000);
}

/**
 * Ports are drawn fresh per run, above the range a dev server or the backend
 * would sit on. Fixed ports meant one leaked server from an aborted run made
 * every later run fail to start, and two runs could never overlap. Two runs at
 * once can still collide, just rarely enough not to plan around.
 *
 * They go through the environment because the runner re-imports this file in
 * every worker process. Drawing on each import would hand each worker a
 * different port from the one the server actually started on. The pair is
 * drawn together for the same reason: setting only one leaves the other redrawn
 * per worker.
 */
if (!process.env.E2E_APP_PORT || !process.env.E2E_STUB_PORT) {
  const appDraw = randomPort();
  let stubDraw = randomPort();
  while (stubDraw === appDraw) stubDraw = randomPort();
  process.env.E2E_APP_PORT = String(appDraw);
  process.env.E2E_STUB_PORT = String(stubDraw);
}

const appPort = Number(process.env.E2E_APP_PORT);
const stubPort = Number(process.env.E2E_STUB_PORT);
const baseURL = `http://localhost:${appPort}`;

export default defineConfig({
  testDir: "e2e",
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  // A retry in CI separates a genuine break from a flake without hiding either:
  // a test that only passes on the second attempt is reported as flaky.
  retries: process.env.CI ? 2 : 0,
  workers: 1,
  reporter: process.env.CI ? [["github"], ["html", { open: "never" }], ["list"]] : [["list"]],
  use: {
    baseURL,
    // Keyed to failure rather than to a retry. Paired with retries: 0 locally,
    // "on-first-retry" captured nothing at all, which left every failure to be
    // re-run by hand to find out what happened.
    trace: "retain-on-failure",
    screenshot: "only-on-failure",
  },
  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"] } }],
  webServer: [
    {
      command: "bun e2e/stub-api.ts",
      port: stubPort,
      reuseExistingServer: false,
      env: { E2E_STUB_PORT: String(stubPort) },
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
