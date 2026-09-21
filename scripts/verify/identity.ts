import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { constants } from "node:fs";
import { access, lstat, readFile, readlink, stat } from "node:fs/promises";
import { resolve } from "node:path";

export type SourceIdentity = {
  commit: string;
  dirty: boolean;
  sha256: string;
};

export type BinaryIdentity = {
  path: string;
  sha256: string;
  bytes: number;
};

export type BuildRecord = {
  schema: 1;
  source: SourceIdentity;
  platform: string;
  arch: string;
  builtAt: string;
  command: string[];
  configuration?: { path: string; sha256: string }[];
  binary: BinaryIdentity;
  kind: "local" | "installed";
  package?: { name: string; sha256: string };
};

const digest = (bytes: string | Buffer) =>
  createHash("sha256").update(bytes).digest("hex");

export async function fileHash(path: string): Promise<string> {
  return digest(await readFile(path));
}

// Include tracked deletions and untracked inputs. Git's ignore rules exclude
// build products and private evidence. Hash file names as well as their bytes.
export async function sourceIdentity(
  root = process.cwd(),
): Promise<SourceIdentity> {
  const git = (args: string[]) =>
    execFileSync("git", args, {
      cwd: root,
      encoding: "utf8",
      maxBuffer: 16 * 1024 * 1024,
    });
  const paths = [
    ...new Set(
      git(["ls-files", "--cached", "--others", "--exclude-standard", "-z"])
        .split("\0")
        .filter(Boolean),
    ),
  ].sort();
  const hash = createHash("sha256");
  for (const name of paths) {
    const path = resolve(root, name);
    let info;
    try {
      info = await lstat(path);
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code !== "ENOENT")
        throw new Error(`Cannot fingerprint ${name}: ${String(error)}`);
      hash.update(JSON.stringify([name, "deleted"]) + "\n");
      continue;
    }
    try {
      assert(
        !info.isDirectory(),
        "Submodules and source directories are not supported",
      );
      if (info.isSymbolicLink())
        assert((await stat(path)).isFile(), "Link target must be a file");
      const entry = [
        name,
        info.mode & 0o111,
        info.isSymbolicLink() ? await readlink(path) : null,
        await fileHash(path),
      ];
      hash.update(JSON.stringify(entry) + "\n");
    } catch (error) {
      throw new Error(`Cannot fingerprint ${name}: ${String(error)}`);
    }
  }
  return {
    commit: git(["rev-parse", "HEAD"]).trim(),
    dirty:
      git(["status", "--porcelain", "--untracked-files=all"]).trim().length > 0,
    sha256: hash.digest("hex"),
  };
}

export function assertSameSource(
  before: SourceIdentity,
  after: SourceIdentity,
) {
  assert(
    before.commit === after.commit && before.sha256 === after.sha256,
    "Source changed during verification. Repeat the affected checks on the final source.",
  );
}

export async function binaryIdentity(path: string): Promise<BinaryIdentity> {
  const fullPath = resolve(path);
  const info = await stat(fullPath);
  assert(info.isFile(), `Not an executable file: ${fullPath}`);
  await access(fullPath, constants.X_OK);
  const bytes = await readFile(fullPath);
  const magic = bytes.subarray(0, 4).toString("hex");
  const native =
    process.platform === "win32"
      ? bytes.subarray(0, 2).toString() === "MZ"
      : process.platform === "linux"
        ? magic === "7f454c46"
        : [
            "cffaedfe",
            "cefaedfe",
            "feedfacf",
            "feedface",
            "cafebabe",
            "bebafeca",
            "cafebabf",
            "bfbafeca",
          ].includes(magic);
  assert(native, `Not a native ${process.platform} executable: ${fullPath}`);
  return { path: fullPath, sha256: digest(bytes), bytes: bytes.length };
}

export async function readBuildRecord(path: string): Promise<BuildRecord> {
  const record: BuildRecord = JSON.parse(await readFile(path, "utf8"));
  const sha = (value: unknown) =>
    typeof value === "string" && /^[a-f0-9]{64}$/.test(value);
  assert(
    record.schema === 1 && /^(local|installed)$/.test(record.kind),
    "Unsupported build record",
  );
  assert(
    record.source &&
      /^[a-f0-9]{40,64}$/.test(record.source.commit) &&
      sha(record.source.sha256) &&
      typeof record.source.dirty === "boolean",
    "Missing build source identity",
  );
  assert(
    record.binary &&
      sha(record.binary.sha256) &&
      typeof record.binary.path === "string",
    "Missing executable identity",
  );
  assert(
    record.platform === process.platform && record.arch === process.arch,
    "Build platform or architecture does not match this host",
  );
  if (record.kind === "installed")
    assert(
      record.package && sha(record.package.sha256),
      "Missing installed package identity",
    );
  return record;
}

export async function verifyBuild(record: BuildRecord, binary: string) {
  if (record.kind === "local") {
    for (const config of record.configuration ?? []) {
      assert.equal(
        await fileHash(config.path),
        config.sha256,
        "Build configuration changed. Rebuild before testing.",
      );
    }
  }
  const actual = await binaryIdentity(binary);
  assert.equal(
    actual.sha256,
    record.binary.sha256,
    "Executable differs from its build record. Rebuild or reinstall before testing.",
  );
  return actual;
}
