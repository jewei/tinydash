import assert from "node:assert/strict";

const [platform] = process.argv.slice(2);
assert(
  platform === "macos" || platform === "windows",
  "Usage: bun scripts/release/tauri-build-config.ts macos|windows",
);

const publicKey = process.env.TAURI_UPDATER_PUBLIC_KEY?.trim();
assert(publicKey, "Missing TAURI_UPDATER_PUBLIC_KEY");
const plugins = { updater: { pubkey: publicKey } };

let config: object;
if (platform === "macos") {
  const signingIdentity = process.env.APPLE_SIGNING_IDENTITY?.trim();
  assert(
    signingIdentity?.startsWith("Developer ID Application: "),
    "A Developer ID Application identity is required for macos",
  );
  config = {
    plugins,
    bundle: {
      createUpdaterArtifacts: true,
      macOS: { signingIdentity },
    },
  };
} else {
  config = {
    plugins,
    bundle: {
      createUpdaterArtifacts: true,
      windows: {
        certificateThumbprint: null,
        signCommand: null,
      },
    },
  };
}
process.stdout.write(JSON.stringify(config));
