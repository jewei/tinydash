import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  copyFile,
  mkdir,
  readFile,
  readdir,
  writeFile,
} from "node:fs/promises";
import { basename, join, resolve } from "node:path";

const [command, directory] = process.argv.slice(2);
assert(
  directory,
  "Usage: bun scripts/ci/artifacts.ts prepare|checksums|verify <directory>",
);
const output = resolve(directory);
const checksum = async (name: string) =>
  createHash("sha256")
    .update(await readFile(join(output, name)))
    .digest("hex");

async function writeChecksums(files: string[]) {
  const sums = await Promise.all(
    files.sort().map(async (name) => `${await checksum(name)}  ${name}\n`),
  );
  await writeFile(join(output, "SHA256SUMS"), sums.join(""));
}

if (command === "verify") {
  const lines = (await readFile(join(output, "SHA256SUMS"), "utf8"))
    .trim()
    .split("\n");
  for (const line of lines) {
    const match = /^([a-f0-9]{64})  (.+)$/.exec(line);
    assert(match, "Invalid checksum entry");
    const [, expected, name] = match;
    assert.equal(basename(name), name, "Checksum paths must be filenames");
    assert.equal(await checksum(name), expected, `Checksum mismatch: ${name}`);
    console.log(`Verified ${name}`);
  }
} else if (command === "checksums") {
  const entries = await readdir(output, { withFileTypes: true });
  assert(
    entries.every((entry) => entry.isFile()),
    "Expected only artifact files",
  );
  await writeChecksums(
    entries.map((entry) => entry.name).filter((name) => name !== "SHA256SUMS"),
  );
} else if (command === "prepare") {
  await mkdir(output, { recursive: true });
  assert.equal(
    (await readdir(output)).length,
    0,
    "Use an empty output directory",
  );
  const config = JSON.parse(
    await readFile("src-tauri/tauri.conf.json", "utf8"),
  );
  const release = resolve("src-tauri/target/release");
  const arch = process.env.RUNNER_ARCH ?? process.arch.toUpperCase();
  const releaseMode = process.env.RELEASE_MODE === "true";
  const files: string[] = [];
  const copy = async (source: string, name = basename(source)) => {
    await copyFile(source, join(output, name));
    files.push(name);
  };
  const packageFile = async (folder: string, suffix: string) => {
    const path = join(release, "bundle", folder);
    const matches = (await readdir(path)).filter(
      (name) => name.includes(`_${config.version}_`) && name.endsWith(suffix),
    );
    assert.equal(
      matches.length,
      1,
      `Expected one ${suffix} for ${config.version}`,
    );
    await copy(join(path, matches[0]));
  };

  switch (process.platform) {
    case "darwin": {
      await packageFile("dmg", ".dmg");
      if (!releaseMode) {
        const name = `TinyDash-macos-${arch}.zip`;
        execFileSync("ditto", [
          "-c",
          "-k",
          "--keepParent",
          join(release, "bundle/macos/TinyDash.app"),
          join(output, name),
        ]);
        files.push(name);
      }
      break;
    }
    case "win32":
      await packageFile("nsis", "-setup.exe");
      if (!releaseMode)
        await copy(
          join(release, "tinydash.exe"),
          `TinyDash-windows-${arch}.exe`,
        );
      break;
    case "linux": {
      await packageFile("deb", ".deb");
      if (!releaseMode) {
        const name = `TinyDash-linux-${arch}.tar.gz`;
        execFileSync("tar", [
          "-C",
          release,
          "-czf",
          join(output, name),
          "tinydash",
        ]);
        files.push(name);
      }
      break;
    }
    default:
      throw new Error(`Unsupported build platform: ${process.platform}`);
  }
  await copy("docs/install.md");
  await copy("docs/desktop-checks.md");
  const commit = execFileSync("git", ["rev-parse", "HEAD"], {
    encoding: "utf8",
  }).trim();
  const modified = execFileSync("git", ["status", "--porcelain"], {
    encoding: "utf8",
  }).trim();
  const publisherSigning = releaseMode
    ? process.platform === "darwin"
      ? "Developer ID"
      : process.platform === "win32"
        ? "none (unsigned preview)"
        : "none"
    : "none";
  await writeFile(
    join(output, "build.txt"),
    `Commit: ${commit}\nSource: ${modified ? "modified worktree" : "clean worktree"}\nVersion: ${config.version}\nOS: ${process.platform}\nArchitecture: ${arch}\nDistribution: ${releaseMode ? (process.env.UPDATER_ARTIFACTS === "true" ? "release candidate with updater signatures" : "release candidate") : "unsigned test build"}\nPublisher signing: ${publisherSigning}\n`,
  );
  files.push("build.txt");
  await writeChecksums(files);
  console.log(`Prepared ${files.length} files in ${output}`);
} else {
  throw new Error("Expected prepare, checksums, or verify");
}
