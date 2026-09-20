import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { basename, dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import {
  binaryIdentity,
  fileHash,
  readBuildRecord,
  type BuildRecord,
} from "./identity.ts";

// The platform installer extracts packageBinary from the verified package.
// Compare it again after replacement, then bind that executable to the package
// checksum and build record. Never stamp an arbitrary existing executable.
export async function recordInstalledBuild(
  artifacts: string,
  packagePath: string,
  packageBinary: string,
  installedBinary: string,
  output: string,
) {
  assert.equal(
    dirname(resolve(packagePath)),
    resolve(artifacts),
    "Package must be inside the artifact directory",
  );
  const checker = resolve(
    dirname(fileURLToPath(import.meta.url)),
    "../ci/artifacts.ts",
  );
  execFileSync("bun", [checker, "verify", artifacts], { stdio: "pipe" });
  const sums = await readFile(resolve(artifacts, "SHA256SUMS"), "utf8");
  const names = new Set(
    sums
      .trim()
      .split("\n")
      .map((line) => line.slice(66)),
  );
  assert(
    names.has("build.json") && names.has(basename(packagePath)),
    "Package and build record must both have verified checksums",
  );
  const build = await readBuildRecord(resolve(artifacts, "build.json"));
  const payload = await binaryIdentity(packageBinary);
  const installed = await binaryIdentity(installedBinary);
  assert.equal(
    installed.sha256,
    payload.sha256,
    "Installed executable differs from the extracted package",
  );
  const record: BuildRecord = {
    ...build,
    kind: "installed",
    binary: installed,
    package: {
      name: basename(packagePath),
      sha256: await fileHash(packagePath),
    },
  };
  await mkdir(dirname(resolve(output)), { recursive: true });
  await writeFile(output, JSON.stringify(record, null, 2) + "\n");
  return record;
}

if (
  process.argv[1] &&
  resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
  const [artifacts, packagePath, payload, binary, output, ...extra] =
    process.argv.slice(2);
  assert(
    output && extra.length === 0,
    "Usage: bun scripts/verify/installed.ts <artifacts> <package> <extracted-executable> <installed-executable> <record>",
  );
  await recordInstalledBuild(artifacts, packagePath, payload, binary, output);
}
