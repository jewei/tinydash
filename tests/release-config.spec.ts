import { execFileSync, spawnSync } from "node:child_process";
import { expect, test } from "@playwright/test";

function releaseEnvironment(values: NodeJS.ProcessEnv = {}): NodeJS.ProcessEnv {
  const env = { ...process.env };
  for (const key of Object.keys(env)) {
    if (
      /^(APPLE_|WINDOWS_|TAURI_|TINYDASH_UPDATE_ENDPOINT$|RUNNER_OS$|RELEASE_MODE$)/.test(
        key,
      )
    )
      delete env[key];
  }
  return { ...env, RELEASE_MODE: "true", ...values };
}

const updater = {
  TAURI_SIGNING_PRIVATE_KEY: "test-private-key",
  TAURI_UPDATER_PUBLIC_KEY: "test-public-key",
  TINYDASH_UPDATE_ENDPOINT: "https://example.com/latest.json",
};
const apple = {
  APPLE_CERTIFICATE: "test-certificate",
  APPLE_CERTIFICATE_PASSWORD: "test-password",
  APPLE_SIGNING_IDENTITY: "Developer ID Application: Test",
  APPLE_ID: "test@example.com",
  APPLE_PASSWORD: "test-app-password",
  APPLE_TEAM_ID: "TEST",
};

for (const platform of ["macos", "windows"]) {
  test(`${platform} release configuration supplies the updater public key`, () => {
    const publicKey = "test-updater-public-key";
    const config = JSON.parse(
      execFileSync("bun", ["scripts/release/tauri-build-config.ts", platform], {
        encoding: "utf8",
        env: releaseEnvironment({
          APPLE_SIGNING_IDENTITY: "Developer ID Application: Test",
          TAURI_UPDATER_PUBLIC_KEY: ` ${publicKey}\n`,
        }),
      }),
    );
    expect(config.bundle.createUpdaterArtifacts).toBe(true);
    expect(config.plugins.updater.pubkey).toBe(publicKey);
    if (platform === "windows") {
      expect(config.bundle.windows.certificateThumbprint).toBeNull();
      expect(config.bundle.windows.signCommand).toBeNull();
    }
  });

  test(`${platform} release configuration rejects a missing updater key`, () => {
    const result = spawnSync(
      "bun",
      ["scripts/release/tauri-build-config.ts", platform],
      {
        encoding: "utf8",
        env: releaseEnvironment({
          APPLE_SIGNING_IDENTITY: "Developer ID Application: Test",
          TAURI_UPDATER_PUBLIC_KEY: " \n",
        }),
      },
    );
    expect(result.status).not.toBe(0);
    expect(result.stderr).toContain("Missing TAURI_UPDATER_PUBLIC_KEY");
    expect(result.stdout).toBe("");
  });
}

test("Mac release configuration rejects an ad-hoc identity", () => {
  const result = spawnSync(
    "bun",
    ["scripts/release/tauri-build-config.ts", "macos"],
    {
      encoding: "utf8",
      env: releaseEnvironment({ ...updater, APPLE_SIGNING_IDENTITY: "-" }),
    },
  );
  expect(result.status).not.toBe(0);
  expect(result.stderr).toContain(
    "Developer ID Application identity is required",
  );
  expect(result.stdout).toBe("");
});

for (const [platform, values] of [
  ["macOS", { ...updater, ...apple }],
  ["Windows", updater],
  ["Linux", {}],
] as const) {
  test(`${platform} preflight requires only its own release credentials`, () => {
    const result = spawnSync("bash", ["scripts/release/require-config.sh"], {
      encoding: "utf8",
      env: releaseEnvironment({ RUNNER_OS: platform, ...values }),
    });
    expect(result.stderr).toBe("");
    expect(result.status).toBe(0);
  });
}

for (const platform of ["macOS", "Windows"]) {
  for (const key of Object.keys(updater)) {
    test(`${platform} preflight rejects missing ${key}`, () => {
      const result = spawnSync("bash", ["scripts/release/require-config.sh"], {
        encoding: "utf8",
        env: releaseEnvironment({
          RUNNER_OS: platform,
          ...apple,
          ...updater,
          [key]: " \n",
        }),
      });
      expect(result.status).not.toBe(0);
      expect(result.stderr).toContain(key);
      expect(result.stderr).not.toContain(updater.TAURI_SIGNING_PRIVATE_KEY);
      expect(result.stderr).not.toContain(apple.APPLE_PASSWORD);
    });
  }
  test(`${platform} preflight rejects an HTTP update feed`, () => {
    const result = spawnSync("bash", ["scripts/release/require-config.sh"], {
      encoding: "utf8",
      env: releaseEnvironment({
        RUNNER_OS: platform,
        ...apple,
        ...updater,
        TINYDASH_UPDATE_ENDPOINT: "http://example.com/latest.json",
      }),
    });
    expect(result.status).not.toBe(0);
    expect(result.stderr).toContain("HTTPS URL required");
  });
}

test("Mac preflight rejects an Apple Development identity", () => {
  const result = spawnSync("bash", ["scripts/release/require-config.sh"], {
    encoding: "utf8",
    env: releaseEnvironment({
      RUNNER_OS: "macOS",
      ...apple,
      ...updater,
      APPLE_SIGNING_IDENTITY: "Apple Development: Test",
    }),
  });
  expect(result.status).not.toBe(0);
  expect(result.stderr).toContain("Developer ID Application identity required");
});

test("release preflight rejects an unknown runner", () => {
  const result = spawnSync("bash", ["scripts/release/require-config.sh"], {
    encoding: "utf8",
    env: releaseEnvironment({ RUNNER_OS: "unknown" }),
  });
  expect(result.status).not.toBe(0);
  expect(result.stderr).toContain("RUNNER_OS");
});
