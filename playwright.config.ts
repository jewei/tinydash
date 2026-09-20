import { defineConfig } from "@playwright/test";

const port = Number(process.env.TINYDASH_TEST_PORT ?? "1421");
if (!Number.isInteger(port) || port < 1 || port > 65535)
  throw new Error("TINYDASH_TEST_PORT must be a valid port number");
const baseURL = `http://127.0.0.1:${port}`;

export default defineConfig({
  testDir: "./tests",
  outputDir: process.env.TINYDASH_TEST_OUTPUT ?? "test-results/browser",
  fullyParallel: true,
  use: {
    baseURL,
    viewport: { width: 720, height: 550 },
    trace: process.env.TINYDASH_VERIFY_TRACE ? "on" : "retain-on-failure",
    screenshot: process.env.TINYDASH_VERIFY_TRACE ? "on" : "only-on-failure",
  },
  webServer: {
    command: `bun run dev --port ${port}`,
    url: baseURL,
    reuseExistingServer: false,
  },
});
