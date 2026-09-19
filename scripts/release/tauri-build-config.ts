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
  const signingIdentity = process.env.APPLE_SIGNING_IDENTITY;
  assert(signingIdentity, "Missing signing configuration for macos");
  config = {
    plugins,
    bundle: {
      createUpdaterArtifacts: true,
      macOS: { signingIdentity },
    },
  };
} else {
  const certificateThumbprint = process.env.WINDOWS_CERTIFICATE_THUMBPRINT;
  assert(certificateThumbprint, "Missing signing configuration for windows");
  config = {
    plugins,
    bundle: {
      createUpdaterArtifacts: true,
      windows: {
        certificateThumbprint,
        digestAlgorithm: "sha256",
        timestampUrl: "https://timestamp.digicert.com",
      },
    },
  };
}
process.stdout.write(JSON.stringify(config));
