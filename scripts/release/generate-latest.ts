import assert from "node:assert/strict";
import { readFile, readdir, writeFile } from "node:fs/promises";
import { join } from "node:path";

const [directory, tag, repository] = process.argv.slice(2);
assert(
  directory && tag && repository,
  "Usage: bun scripts/release/generate-latest.ts <release-dir> <tag> <owner/repo>",
);
assert(
  /^v\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$/.test(tag),
  `Invalid release tag: ${tag}`,
);
assert(/^[^/]+\/[^/]+$/.test(repository), `Invalid repository: ${repository}`);

const names = await readdir(directory);
const findOne = (suffix: string) => {
  const matches = names.filter((name) => name.endsWith(suffix));
  assert.equal(
    matches.length,
    1,
    `Expected one ${suffix}, found ${matches.length}`,
  );
  return matches[0];
};
const pair = (signatureName: string) => {
  const artifact = signatureName.slice(0, -4);
  assert(
    names.includes(artifact),
    `Missing updater artifact for ${signatureName}`,
  );
  return { artifact, signatureName };
};

const mac = pair(findOne(".app.tar.gz.sig"));
const windows = pair(findOne("-setup.exe.sig"));
const signature = async (name: string) =>
  readFile(join(directory, name), "utf8");
const assetUrl = (tagName: string, name: string) =>
  `https://github.com/${repository}/releases/download/${tagName}/${encodeURIComponent(name)}`;

const latest = {
  version: tag.slice(1),
  notes: `TinyDash ${tag}.`,
  platforms: {
    "darwin-aarch64": {
      signature: await signature(mac.signatureName),
      url: assetUrl(tag, mac.artifact),
    },
    "windows-x86_64": {
      signature: await signature(windows.signatureName),
      url: assetUrl(tag, windows.artifact),
    },
  },
};
await writeFile(
  join(directory, "latest.json"),
  `${JSON.stringify(latest, null, 2)}\n`,
);
console.log(
  "Generated latest.json from matching macOS and Windows updater signatures.",
);
