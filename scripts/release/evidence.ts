import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import {
  lstat,
  readFile,
  readdir,
  realpath,
  writeFile,
} from "node:fs/promises";
import { basename, dirname, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

const hash = (bytes: string | Buffer) =>
  createHash("sha256").update(bytes).digest("hex");
const sha = /^[a-f0-9]{64}$/;
const platforms = ["macos", "windows", "linux-x11", "linux-wayland"];
const sections = [
  "installation",
  "launcher",
  "settings",
  "tools",
  "calculations-emoji",
  "ranking",
  "clipboard",
  "files",
  "system",
  "update-recovery-removal",
];
export const requiredChecks = [
  "automated/source-macos",
  "automated/source-windows",
  "automated/source-linux",
  "automated/native-windows",
  "automated/native-linux-x11",
  "automated/upgrade-windows",
  "automated/upgrade-linux",
  "signing/macos-developer-id-notarization-stapling",
  "signing/windows-unsigned-preview",
  "signing/updater-signatures",
  ...platforms.flatMap((platform) =>
    sections.map((section) => `manual/${platform}/${section}`),
  ),
];

type Attachment = { path: string; sha256: string };
export type Check = {
  id: string;
  status: "pending" | "passed" | "failed";
  assetSetSha256: string;
  reviewer: string;
  procedure: string;
  recordedAt: string;
  evidence: Attachment[];
};
export type ReleaseEvidence = {
  schema: 1;
  tag: string;
  commit: string;
  assets: Record<string, string>;
  checks: Check[];
};

// Exactly one checksum per actual file. An omitted package is not a valid set.
export async function readAssets(directory: string) {
  const entries = await readdir(directory, { withFileTypes: true });
  assert(
    entries.every((entry) => entry.isFile()),
    "Assets must be regular files",
  );
  const assets: Record<string, string> = {};
  for (const line of (await readFile(resolve(directory, "SHA256SUMS"), "utf8"))
    .trim()
    .split("\n")) {
    const match = /^([a-f0-9]{64})  ([^/\\\r\n]+)$/.exec(line);
    assert(match && match[2] !== "SHA256SUMS", "Invalid checksum entry");
    const [, digest, name] = match;
    assert(!Object.hasOwn(assets, name), `Duplicate checksum: ${name}`);
    Object.defineProperty(assets, name, {
      value: digest,
      enumerable: true,
      writable: true,
      configurable: true,
    });
    assert.equal(
      hash(await readFile(resolve(directory, name))),
      digest,
      `Changed asset: ${name}`,
    );
  }
  assert.deepEqual(
    Object.keys(assets).sort(),
    entries
      .map((entry) => entry.name)
      .filter((name) => name !== "SHA256SUMS")
      .sort(),
    "Missing asset hashes",
  );
  for (const suffix of [
    ".dmg",
    "-setup.exe",
    ".deb",
    ".app.tar.gz",
    ".app.tar.gz.sig",
    "-setup.exe.sig",
  ]) {
    assert.equal(
      Object.keys(assets).filter((name) => name.endsWith(suffix)).length,
      1,
      `Expected one ${suffix}`,
    );
  }
  for (const name of [
    "latest.json",
    "build-macos.json",
    "build-windows.json",
    "build-linux.json",
  ]) {
    assert(Object.hasOwn(assets, name), `Missing ${name}`);
  }
  return assets;
}

export function assetSetHash(assets: Record<string, string>) {
  return hash(
    JSON.stringify(
      Object.entries(assets).sort(([a], [b]) => a.localeCompare(b, "en")),
    ),
  );
}

export async function createEvidence(
  directory: string,
  tag: string,
  commit: string,
): Promise<ReleaseEvidence> {
  const assets = await readAssets(directory);
  return {
    schema: 1,
    tag,
    commit,
    assets,
    checks: requiredChecks.map((id) => ({
      id,
      status: "pending",
      assetSetSha256: assetSetHash(assets),
      reviewer: "",
      procedure: "",
      recordedAt: "",
      evidence: [],
    })),
  };
}

async function attachmentBytes(root: string, attachment: Attachment) {
  assert(
    attachment &&
      typeof attachment.path === "string" &&
      sha.test(attachment.sha256),
    "Missing evidence path/hash",
  );
  const realRoot = await realpath(root);
  const path = await realpath(resolve(root, attachment.path));
  assert(
    path.startsWith(realRoot + sep),
    "Evidence must stay inside its directory",
  );
  assert((await lstat(path)).isFile(), "Evidence must be a file");
  const bytes = await readFile(path);
  assert(bytes.length > 0, "Empty evidence");
  assert.equal(
    hash(bytes),
    attachment.sha256,
    `Changed evidence: ${attachment.path}`,
  );
  return bytes;
}

export async function validateEvidence(
  record: ReleaseEvidence,
  directory: string,
  evidenceRoot: string,
  tag: string,
  commit: string,
) {
  assert.equal(record.schema, 1, "Unknown evidence schema");
  assert(/^v\d+\.\d+\.\d+(?:-[\w.-]+)?$/.test(tag), "Invalid release tag");
  assert(/^[a-f0-9]{40}$/.test(commit), "Invalid source commit");
  assert.equal(record.tag, tag, "Wrong evidence tag");
  assert.equal(record.commit, commit, "Wrong evidence source");
  const assets = await readAssets(directory);
  assert.deepEqual(
    record.assets,
    assets,
    "Evidence does not cover the exact asset hashes",
  );
  const assetSet = assetSetHash(assets);
  const latest = JSON.parse(
    await readFile(resolve(directory, "latest.json"), "utf8"),
  );
  assert.equal(latest.version, tag.slice(1), "Wrong updater version");
  for (const [label, platform, arch] of [
    ["macos", "darwin", "arm64"],
    ["windows", "win32", "x64"],
    ["linux", "linux", "x64"],
  ]) {
    const build = JSON.parse(
      await readFile(resolve(directory, `build-${label}.json`), "utf8"),
    );
    assert.equal(build.source?.commit, commit, `Wrong ${label} build source`);
    assert.equal(build.source?.dirty, false, `Dirty ${label} build source`);
    assert(
      sha.test(build.source?.sha256),
      `Missing ${label} source fingerprint`,
    );
    assert.equal(build.schema, 1, `Unknown ${label} build schema`);
    assert.equal(build.platform, platform, `Wrong ${label} build platform`);
    assert.equal(build.arch, arch, `Wrong ${label} build architecture`);
    assert(sha.test(build.binary?.sha256), `Missing ${label} executable hash`);
  }
  assert(Array.isArray(record.checks), "Missing checks");
  assert.deepEqual(
    record.checks.map((check) => check.id).sort(),
    [...requiredChecks].sort(),
    "Missing, duplicate, or unexpected checks",
  );
  for (const check of record.checks) {
    assert.equal(check.status, "passed", `Incomplete check: ${check.id}`);
    assert.equal(check.assetSetSha256, assetSet, `Stale check: ${check.id}`);
    assert(
      typeof check.reviewer === "string" && check.reviewer.trim(),
      `Missing reviewer: ${check.id}`,
    );
    assert(
      typeof check.procedure === "string" && check.procedure.trim(),
      `Missing procedure: ${check.id}`,
    );
    assert(
      typeof check.recordedAt === "string" &&
        Number.isFinite(Date.parse(check.recordedAt)),
      `Missing timestamp: ${check.id}`,
    );
    assert(
      Array.isArray(check.evidence) && check.evidence.length > 0,
      `Missing evidence: ${check.id}`,
    );
    const attachments = await Promise.all(
      check.evidence.map((attachment) =>
        attachmentBytes(evidenceRoot, attachment),
      ),
    );
    if (check.id.startsWith("automated/native-")) {
      // A build log or browser pass cannot stand in for an installed native run.
      const platform = check.id.endsWith("windows") ? "win32" : "linux";
      const suffix = platform === "win32" ? "-setup.exe" : ".deb";
      const packageName = Object.keys(assets).find((name) =>
        name.endsWith(suffix),
      )!;
      const resultIndex = check.evidence.findIndex(
        (entry) => basename(entry.path) === "result.json",
      );
      assert(resultIndex >= 0, `Missing native result.json: ${check.id}`);
      const result = JSON.parse(attachments[resultIndex].toString("utf8"));
      assert.equal(result.mode, "native", "Not native evidence");
      assert.equal(result.passed, true, "Native check failed");
      assert.equal(
        result.nativeCleanup,
        "complete",
        "Native cleanup not confirmed",
      );
      assert.equal(result.platform, platform, "Wrong native platform");
      assert.equal(result.build?.source?.commit, commit, "Wrong tested source");
      assert.equal(
        result.build?.kind,
        "installed",
        "Native check must test the installed package",
      );
      assert.equal(
        result.build?.package?.name,
        packageName,
        "Wrong tested package",
      );
      assert.equal(
        result.build?.package?.sha256,
        assets[packageName],
        "Wrong tested package hash",
      );
      const build = JSON.parse(
        await readFile(
          resolve(
            directory,
            `build-${platform === "win32" ? "windows" : "linux"}.json`,
          ),
          "utf8",
        ),
      );
      assert.equal(
        result.build?.binary?.sha256,
        build.binary.sha256,
        "Wrong tested executable hash",
      );
      assert.equal(
        result.build?.source?.sha256,
        build.source.sha256,
        "Wrong tested source fingerprint",
      );
      assert(
        /^[a-f0-9]{40}$/.test(result.source?.commit) &&
          sha.test(result.source?.sha256),
        "Missing test-code identity",
      );
      assert(
        Array.isArray(result.steps) &&
          result.steps.some(
            (step: { command?: string[]; status?: number }) =>
              step.status === 0 &&
              JSON.stringify(step.command) ===
                JSON.stringify(["bun", "tests/native/smoke.ts"]),
          ),
        "Native suite did not run",
      );
      assert(
        result.steps.every((step: { status?: number }) => step.status === 0),
        "Native command failed",
      );
    }
  }
}

if (resolve(process.argv[1] ?? "") === fileURLToPath(import.meta.url)) {
  const [command, directory, evidence, tag, commit] = process.argv.slice(2);
  assert(
    directory && evidence && tag && commit,
    "Usage: bun scripts/release/evidence.ts init|validate ASSETS EVIDENCE_JSON TAG COMMIT",
  );
  if (command === "init") {
    await writeFile(
      evidence,
      JSON.stringify(await createEvidence(directory, tag, commit), null, 2) +
        "\n",
      { flag: "wx" },
    );
    console.log(
      "Created pending evidence; this does not authorize publication.",
    );
  } else if (command === "validate") {
    await validateEvidence(
      JSON.parse(await readFile(evidence, "utf8")),
      directory,
      dirname(resolve(evidence)),
      tag,
      commit,
    );
    console.log(
      "Release evidence passed. Reviewer attestations still require human review.",
    );
  } else throw new Error("Expected init or validate");
}
