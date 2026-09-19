import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readFile } from "node:fs/promises";

const tag = process.argv[2];
assert(tag, "Usage: bun scripts/release/verify-tag.ts v<version>");
assert(
  /^v[0-9]+\.[0-9]+\.[0-9]+(?:[-+][0-9A-Za-z.-]+)?$/.test(tag),
  `Release tag must be a version tag: ${tag}`,
);

const packageJson = JSON.parse(await readFile("package.json", "utf8"));
const tauri = JSON.parse(await readFile("src-tauri/tauri.conf.json", "utf8"));
const cargo = await readFile("src-tauri/Cargo.toml", "utf8");
const cargoVersion = /^version\s*=\s*"([^"]+)"/m.exec(cargo)?.[1];
assert(cargoVersion, "src-tauri/Cargo.toml has no package version");

const versions = [packageJson.version, tauri.version, cargoVersion];
assert(
  versions.every((version) => version === versions[0]),
  `Version files differ: ${versions.join(", ")}`,
);
assert.equal(
  tag,
  `v${versions[0]}`,
  `Tag ${tag} does not match version ${versions[0]}`,
);

const commit = execFileSync("git", ["rev-parse", "HEAD"], {
  encoding: "utf8",
}).trim();
const taggedCommit = execFileSync("git", ["rev-list", "-n", "1", tag], {
  encoding: "utf8",
}).trim();
assert.equal(
  commit,
  taggedCommit,
  `Checkout ${commit} is not the tagged commit ${taggedCommit}`,
);
console.log(`Verified ${tag} at ${commit}`);
