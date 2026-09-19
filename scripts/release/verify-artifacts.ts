import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readdir, readFile } from "node:fs/promises";
import { join } from "node:path";

const [directory, expectedPlatform, expectedArch] = process.argv.slice(2);
assert(
  directory && expectedPlatform && expectedArch,
  "Usage: bun scripts/release/verify-artifacts.ts <directory> <platform> <architecture>",
);

const buildText = await readFile(join(directory, "build.txt"), "utf8");
const fields = Object.fromEntries(
  buildText
    .split("\n")
    .filter(Boolean)
    .map((line) => {
      const separator = line.indexOf(": ");
      assert(separator > 0, `Invalid build metadata line: ${line}`);
      return [line.slice(0, separator), line.slice(separator + 2)];
    }),
);
for (const name of [
  "Commit",
  "Source",
  "Version",
  "OS",
  "Architecture",
  "Distribution",
  "Publisher signing",
]) {
  assert(fields[name], `Missing build metadata field: ${name}`);
}
const supportedPairs = new Map([
  ["darwin", "ARM64"],
  ["win32", "X64"],
  ["linux", "X64"],
]);
assert.equal(
  supportedPairs.get(expectedPlatform),
  expectedArch,
  `Unsupported platform and architecture pair: ${expectedPlatform}/${expectedArch}`,
);
assert.equal(
  fields.OS,
  expectedPlatform,
  `Expected ${expectedPlatform}, got ${fields.OS}`,
);
assert.equal(
  fields.Architecture,
  expectedArch,
  `Expected ${expectedArch}, got ${fields.Architecture}`,
);
const packageJson = JSON.parse(await readFile("package.json", "utf8"));
assert.equal(
  fields.Version,
  packageJson.version,
  "Build version does not match package.json",
);
const tauriConfig = JSON.parse(
  await readFile("src-tauri/tauri.conf.json", "utf8"),
);
assert.equal(
  fields.Version,
  tauriConfig.version,
  "Build version does not match tauri.conf.json",
);
const commit = execFileSync("git", ["rev-parse", "HEAD"], {
  encoding: "utf8",
}).trim();
assert.equal(
  fields.Commit,
  commit,
  "Build was not made from the checked-out commit",
);
if (process.env.RELEASE_MODE === "true") {
  assert.equal(
    process.env.UPDATER_ARTIFACTS === "true",
    expectedPlatform !== "linux",
    "Release updates require signatures on Mac and Windows; Linux uses manual .deb updates",
  );
  assert.equal(
    fields.Source,
    "clean worktree",
    "Release source had local changes",
  );
  const expectedDistribution =
    process.env.UPDATER_ARTIFACTS === "true"
      ? "release candidate with updater signatures"
      : "release candidate";
  assert.equal(fields.Distribution, expectedDistribution);
  const expectedPublisher =
    expectedPlatform === "darwin"
      ? "Developer ID"
      : expectedPlatform === "win32"
        ? "none (unsigned preview)"
        : "none";
  assert.equal(
    fields["Publisher signing"],
    expectedPublisher,
    "Publisher signing does not match the release policy",
  );
} else {
  assert.equal(
    fields.Distribution,
    "unsigned test build",
    "Unexpected distribution type in test mode",
  );
  assert.equal(fields["Publisher signing"], "none");
}

const names = (await readdir(directory)).sort();
const sumNames = (await readFile(join(directory, "SHA256SUMS"), "utf8"))
  .trim()
  .split("\n")
  .map((line) => {
    const match = /^([a-f0-9]{64})  (.+)$/.exec(line);
    assert(match, `Invalid checksum entry: ${line}`);
    return match[2];
  })
  .sort();
assert.deepEqual(
  sumNames,
  names.filter((name) => name !== "SHA256SUMS"),
  "Every artifact file except SHA256SUMS must have one checksum",
);

const version = fields.Version;
let requiredSuffix: string;
if (expectedPlatform === "darwin") {
  requiredSuffix = `_${version}_aarch64.dmg`;
} else if (expectedPlatform === "win32") {
  requiredSuffix = `_${version}_x64-setup.exe`;
} else if (expectedPlatform === "linux") {
  requiredSuffix = `_${version}_amd64.deb`;
} else {
  throw new Error(`Unsupported release platform: ${expectedPlatform}`);
}
const packageNames = names.filter(
  (name) =>
    name.endsWith(".dmg") ||
    name.endsWith("-setup.exe") ||
    name.endsWith(".deb"),
);
assert.equal(packageNames.length, 1, "Expected one direct installer package");
assert.equal(
  packageNames.filter((name) => name.endsWith(requiredSuffix)).length,
  1,
  `Expected one artifact ending ${requiredSuffix}`,
);
if (process.env.UPDATER_ARTIFACTS === "true") {
  if (expectedPlatform === "darwin") {
    const updaterArchives = names.filter((name) =>
      name.endsWith(".app.tar.gz"),
    );
    const updaterSignatures = names.filter((name) =>
      name.endsWith(".app.tar.gz.sig"),
    );
    assert.equal(
      updaterArchives.length,
      1,
      "Expected one macOS updater archive",
    );
    assert.equal(
      updaterSignatures.length,
      1,
      "Expected one macOS updater signature",
    );
    assert(
      updaterSignatures[0] === `${updaterArchives[0]}.sig`,
      "macOS updater signature does not match its archive",
    );
    assert(
      (await readFile(join(directory, updaterSignatures[0]), "utf8")).trim(),
      "macOS updater signature is empty",
    );
  } else if (expectedPlatform === "win32") {
    const signature = `_${version}_x64-setup.exe.sig`;
    assert(
      names.filter((name) => name.endsWith("-setup.exe.sig")).length === 1 &&
        names.filter((name) => name.endsWith(signature)).length === 1 &&
        names.includes(`${packageNames[0]}.sig`),
      "Expected one Windows updater signature with the release architecture",
    );
    assert(
      (
        await readFile(join(directory, `${packageNames[0]}.sig`), "utf8")
      ).trim(),
      "Windows updater signature is empty",
    );
  } else {
    throw new Error("Updater artifacts are unsupported for Linux .deb builds");
  }
}
for (const name of ["build.txt", "install.md", "desktop-checks.md"]) {
  assert(names.includes(name), `Missing release guidance: ${name}`);
}
console.log(
  `Verified ${names.length} files for ${expectedPlatform}/${expectedArch}`,
);
