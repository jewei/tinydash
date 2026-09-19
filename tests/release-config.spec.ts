import { execFileSync, spawnSync } from "node:child_process";
import { expect, test } from "@playwright/test";

for (const platform of ["macos", "windows"]) {
  test(`${platform} release configuration supplies the updater public key`, () => {
    const publicKey = "test-updater-public-key";
    const config = JSON.parse(
      execFileSync("bun", ["scripts/release/tauri-build-config.ts", platform], {
        encoding: "utf8",
        env: {
          ...process.env,
          APPLE_SIGNING_IDENTITY: "Developer ID Application: Test",
          WINDOWS_CERTIFICATE_THUMBPRINT: "TEST",
          TAURI_UPDATER_PUBLIC_KEY: ` ${publicKey}\n`,
        },
      }),
    );
    expect(config.bundle.createUpdaterArtifacts).toBe(true);
    expect(config.plugins.updater.pubkey).toBe(publicKey);
  });

  test(`${platform} release configuration rejects a missing updater key`, () => {
    const result = spawnSync(
      "bun",
      ["scripts/release/tauri-build-config.ts", platform],
      {
        encoding: "utf8",
        env: {
          ...process.env,
          APPLE_SIGNING_IDENTITY: "Developer ID Application: Test",
          WINDOWS_CERTIFICATE_THUMBPRINT: "TEST",
          TAURI_UPDATER_PUBLIC_KEY: " \n",
        },
      },
    );
    expect(result.status).not.toBe(0);
    expect(result.stderr).toContain("Missing TAURI_UPDATER_PUBLIC_KEY");
    expect(result.stdout).toBe("");
  });
}
