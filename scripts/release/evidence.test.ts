import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { cpSync } from "node:fs";
import {
  mkdir,
  mkdtemp,
  readFile,
  readdir,
  rm,
  writeFile,
} from "node:fs/promises";
import { tmpdir } from "node:os";
import { resolve } from "node:path";
import { test, type TestContext } from "node:test";
import {
  assetSetHash,
  createEvidence,
  readAssets,
  validateEvidence,
  type ReleaseEvidence,
} from "./evidence.ts";
import { publish } from "./publish.ts";

const hash = (value: string | Buffer) =>
  createHash("sha256").update(value).digest("hex");
const commit = "a".repeat(40);
const tag = "v0.1.3";
async function fixture(t: TestContext) {
  const root = await mkdtemp(resolve(tmpdir(), "tinydash-evidence-test-"));
  t.after(() => rm(root, { recursive: true, force: true }));
  const assets = resolve(root, "assets");
  const proof = resolve(root, "proof");
  await mkdir(assets);
  await mkdir(proof);
  for (const name of [
    "TinyDash_0.1.3_aarch64.dmg",
    "TinyDash_0.1.3_x64-setup.exe",
    "TinyDash_0.1.3_amd64.deb",
    "TinyDash.app.tar.gz",
    "TinyDash.app.tar.gz.sig",
    "TinyDash_0.1.3_x64-setup.exe.sig",
  ]) {
    await writeFile(
      resolve(assets, name),
      `Synthetic validator fixture, NOT executable/native evidence: ${name}\n`,
    );
  }
  for (const [label, platform, arch] of [
    ["macos", "darwin", "arm64"],
    ["windows", "win32", "x64"],
    ["linux", "linux", "x64"],
  ]) {
    await writeFile(
      resolve(assets, `build-${label}.json`),
      JSON.stringify({
        schema: 1,
        platform,
        arch,
        source: { commit, dirty: false, sha256: hash("source") },
        binary: { sha256: hash(`${label} binary`) },
      }),
    );
  }
  await writeFile(
    resolve(assets, "latest.json"),
    JSON.stringify({ version: tag.slice(1) }),
  );
  const checksums = async () => {
    const names = (await readdir(assets))
      .filter((name) => name !== "SHA256SUMS")
      .sort();
    await writeFile(
      resolve(assets, "SHA256SUMS"),
      (
        await Promise.all(
          names.map(
            async (name) =>
              `${hash(await readFile(resolve(assets, name)))}  ${name}\n`,
          ),
        )
      ).join(""),
    );
  };
  await checksums();
  const record = await createEvidence(assets, tag, commit);
  const attach = async (name: string, content: string) => {
    const path = resolve(proof, name);
    await mkdir(resolve(path, ".."), { recursive: true });
    await writeFile(path, content);
    return { path: name, sha256: hash(content) };
  };
  for (const check of record.checks) {
    check.status = "passed";
    check.reviewer = "Synthetic regression fixture (not an actual release)";
    check.procedure = check.id;
    check.recordedAt = "2026-09-28T00:00:00Z";
    check.evidence = [
      await attach(`${check.id}/report.txt`, `Synthetic ${check.id}\n`),
    ];
    if (check.id.startsWith("automated/native-")) {
      const windows = check.id.endsWith("windows");
      const packageName = Object.keys(record.assets).find((name) =>
        name.endsWith(windows ? "-setup.exe" : ".deb"),
      )!;
      const build = JSON.parse(
        await readFile(
          resolve(assets, `build-${windows ? "windows" : "linux"}.json`),
          "utf8",
        ),
      );
      const result = {
        mode: "native",
        passed: true,
        nativeCleanup: "complete",
        platform: windows ? "win32" : "linux",
        source: { commit, sha256: hash("test source") },
        build: {
          ...build,
          kind: "installed",
          package: { name: packageName, sha256: record.assets[packageName] },
        },
        steps: [{ command: ["bun", "tests/native/smoke.ts"], status: 0 }],
      };
      check.evidence.push(
        await attach(`${check.id}/result.json`, JSON.stringify(result)),
      );
    }
  }
  return {
    root,
    assets,
    proof,
    record,
    attach,
    checksums,
    validate: (value: ReleaseEvidence = record) =>
      validateEvidence(value, assets, proof, tag, commit),
    save: async () => {
      const path = resolve(proof, "release-evidence.json");
      await writeFile(path, JSON.stringify(record));
      return path;
    },
  };
}

test("complete synthetic record validates; generated template stays pending", async (t) => {
  const f = await fixture(t);
  await f.validate();
  const pending = await createEvidence(f.assets, tag, commit);
  assert(
    pending.checks.every(
      (check) => check.status === "pending" && check.evidence.length === 0,
    ),
  );
  await assert.rejects(f.validate(pending), /Incomplete check/);
});
test("changed bytes, removed hashes, extra assets, and duplicate checksums are rejected", async (t) => {
  const f = await fixture(t);
  const name = Object.keys(f.record.assets)[0];
  const path = resolve(f.assets, name);
  const original = await readFile(path);
  await writeFile(path, "tampered");
  await assert.rejects(f.validate(), /Changed asset/);
  await writeFile(path, original);
  const sumsPath = resolve(f.assets, "SHA256SUMS");
  const sums = await readFile(sumsPath, "utf8");
  await writeFile(
    sumsPath,
    sums
      .split("\n")
      .filter((line) => !line.endsWith(`  ${name}`))
      .join("\n"),
  );
  await assert.rejects(f.validate(), /Missing asset hashes/);
  await writeFile(sumsPath, sums + sums.split("\n")[0] + "\n");
  await assert.rejects(f.validate(), /Duplicate checksum/);
  await writeFile(sumsPath, sums);
  await writeFile(resolve(f.assets, "extra.bin"), "unlisted");
  await assert.rejects(f.validate(), /Missing asset hashes/);
});
test("new checksums cannot authorize a changed package using stale evidence", async (t) => {
  const f = await fixture(t);
  await writeFile(
    resolve(f.assets, "TinyDash_0.1.3_x64-setup.exe"),
    "replacement",
  );
  await f.checksums();
  await assert.rejects(f.validate(), /exact asset hashes/);
  f.record.assets = await readAssets(f.assets);
  await assert.rejects(f.validate(), /Stale check/);
  for (const check of f.record.checks)
    check.assetSetSha256 = assetSetHash(f.record.assets);
  await assert.rejects(f.validate(), /Wrong tested package hash/);
});
for (const [name, mutate] of [
  [
    "missing asset hash",
    (record: ReleaseEvidence) => {
      delete record.assets[Object.keys(record.assets)[0]];
    },
  ],
  [
    "missing automated check",
    (record: ReleaseEvidence) => {
      record.checks = record.checks.filter(
        (check) => check.id !== "automated/source-macos",
      );
    },
  ],
  [
    "missing manual check",
    (record: ReleaseEvidence) => {
      record.checks.pop();
    },
  ],
  [
    "missing signing check",
    (record: ReleaseEvidence) => {
      record.checks = record.checks.filter(
        (check) => check.id !== "signing/updater-signatures",
      );
    },
  ],
  [
    "duplicate check",
    (record: ReleaseEvidence) => {
      record.checks.push(record.checks[0]);
    },
  ],
  [
    "failed check",
    (record: ReleaseEvidence) => {
      record.checks[0].status = "failed";
    },
  ],
  [
    "pending manual check",
    (record: ReleaseEvidence) => {
      record.checks.at(-1)!.status = "pending";
    },
  ],
  [
    "empty evidence",
    (record: ReleaseEvidence) => {
      record.checks[0].evidence = [];
    },
  ],
  [
    "missing evidence hash",
    (record: ReleaseEvidence) => {
      record.checks[0].evidence[0].sha256 = "";
    },
  ],
  [
    "missing reviewer",
    (record: ReleaseEvidence) => {
      record.checks[0].reviewer = "";
    },
  ],
  [
    "wrong source",
    (record: ReleaseEvidence) => {
      record.commit = "b".repeat(40);
    },
  ],
  [
    "wrong tag",
    (record: ReleaseEvidence) => {
      record.tag = "v9.9.9";
    },
  ],
] as const) {
  test(`rejects ${name}`, async (t) => {
    const f = await fixture(t);
    mutate(f.record);
    await assert.rejects(f.validate());
  });
}
test("evidence tampering and escape paths are rejected", async (t) => {
  const f = await fixture(t);
  await writeFile(
    resolve(f.proof, f.record.checks[0].evidence[0].path),
    "changed",
  );
  await assert.rejects(f.validate(), /Changed evidence/);
  await writeFile(resolve(f.root, "outside.txt"), "outside");
  f.record.checks[0].evidence = [
    { path: "../outside.txt", sha256: hash("outside") },
  ];
  await assert.rejects(f.validate(), /inside its directory/);
});
for (const [name, key, value] of [
  ["browser pass", "mode", "browser"],
  ["failed run", "passed", false],
  ["unconfirmed cleanup", "nativeCleanup", "unknown"],
  ["wrong platform", "platform", "darwin"],
  [
    "build without suite",
    "steps",
    [{ command: ["bun", "scripts/verify/build.ts"], status: 0 }],
  ],
  [
    "failed suite command",
    "steps",
    [{ command: ["bun", "tests/native/smoke.ts"], status: 1 }],
  ],
] as const) {
  test(`native proof rejects ${name}`, async (t) => {
    const f = await fixture(t);
    const check = f.record.checks.find(
      (check) => check.id === "automated/native-windows",
    )!;
    const entry = check.evidence[1];
    const result = JSON.parse(
      await readFile(resolve(f.proof, entry.path), "utf8"),
    );
    result[key] = value;
    check.evidence[1] = await f.attach(entry.path, JSON.stringify(result));
    await assert.rejects(f.validate());
  });
}
test("native proof requires the installed package and matching executable", async (t) => {
  const f = await fixture(t);
  const check = f.record.checks.find(
    (check) => check.id === "automated/native-windows",
  )!;
  const entry = check.evidence[1];
  const result = JSON.parse(
    await readFile(resolve(f.proof, entry.path), "utf8"),
  );
  result.build.kind = "local";
  check.evidence[1] = await f.attach(entry.path, JSON.stringify(result));
  await assert.rejects(f.validate(), /installed package/);
  result.build.kind = "installed";
  result.build.binary.sha256 = hash("other executable");
  check.evidence[1] = await f.attach(entry.path, JSON.stringify(result));
  await assert.rejects(f.validate(), /executable hash/);
});

test("publication downloads exact draft assets; no publish on missing proof or concurrent asset changes", async (t) => {
  const f = await fixture(t);
  const evidence = await f.save();
  let published = 0;
  let releases = 0;
  let replace = false;
  let publishedDraft = false;
  const run = (command: string, args: string[]) => {
    if (command === "git") return commit;
    assert.equal(command, "gh");
    if (args[0] === "repo") return "example/tinydash";
    if (args[0] === "api" && args[1].includes("/git/ref/"))
      return JSON.stringify({ object: { type: "commit", sha: commit } });
    if (args[0] === "api" && args[1].includes("/releases/tags/"))
      return JSON.stringify({
        id: 1,
        draft: !publishedDraft,
        assets: [
          {
            id: replace && releases++ > 0 ? 2 : 1,
            name: "package",
            size: 1,
            updated_at: "2026-09-28T00:00:00Z",
            download_count: releases++,
          },
        ],
      });
    if (args[0] === "release" && args[1] === "download") {
      cpSync(f.assets, args[4], { recursive: true });
      return "";
    }
    if (args[0] === "release" && args[1] === "edit") {
      assert.deepEqual(args, ["release", "edit", tag, "--draft=false"]);
      published++;
      return "";
    }
    throw new Error(`Unexpected command: ${command} ${args.join(" ")}`);
  };
  await publish(tag, evidence, run);
  assert.equal(published, 1);
  f.record.checks.pop();
  await f.save();
  await assert.rejects(publish(tag, evidence, run), /Missing, duplicate/);
  assert.equal(published, 1);
  const fresh = await fixture(t);
  const freshEvidence = await fresh.save();
  // Both synthetic fixtures use identical bytes, so they share the same hashes.
  replace = true;
  releases = 0;
  await assert.rejects(publish(tag, freshEvidence, run), /Assets changed/);
  assert.equal(published, 1);
  publishedDraft = true;
  await assert.rejects(publish(tag, freshEvidence, run), /existing draft/);
  assert.equal(published, 1);
});
