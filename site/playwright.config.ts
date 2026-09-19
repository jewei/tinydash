import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./tests",
  fullyParallel: true,
  outputDir: "../artifacts/site-tests",
  use: {
    baseURL: "http://127.0.0.1:4174/tinydash/",
    trace: "retain-on-failure",
  },
  webServer: {
    command: "bun scripts/site/build.ts && bun scripts/site/serve.ts",
    cwd: "..",
    url: "http://127.0.0.1:4174/tinydash/",
    reuseExistingServer: !process.env.CI,
  },
});
