import { execFileSync, spawnSync } from "node:child_process";
import {
  chmod,
  copyFile,
  mkdir,
  mkdtemp,
  readFile,
  readdir,
  rm,
  symlink,
  writeFile,
} from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { expect, test } from "@playwright/test";
import {
  assertSameSource,
  binaryIdentity,
  fileHash,
  readBuildRecord,
  sourceIdentity,
  verifyBuild,
  type BuildRecord,
} from "../scripts/verify/identity.ts";
import { recordInstalledBuild } from "../scripts/verify/installed.ts";

async function temporary(body: (directory: string) => Promise<void>) {
  const directory = await mkdtemp(join(tmpdir(), "tinydash-verification-"));
  try {
    await body(directory);
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
}

async function repository(directory: string) {
  const git = (args: string[]) =>
    execFileSync(
      "git",
      ["-c", "core.hooksPath=/dev/null", "-c", "commit.gpgsign=false", ...args],
      { cwd: directory, stdio: "pipe" },
    );
  git(["init", "--quiet"]);
  await mkdir(join(directory, "src"));
  await writeFile(join(directory, "src/main.ts"), "export const value = 1;\n");
  await writeFile(join(directory, ".gitignore"), "test-results/\n.local/\n");
  git(["add", "."]);
  git([
    "-c",
    "user.name=Verification test",
    "-c",
    "user.email=verification@example.invalid",
    "commit",
    "--quiet",
    "-m",
    "Fixture",
  ]);
}

async function buildRecord(directory: string): Promise<BuildRecord> {
  await repository(directory);
  return {
    schema: 1,
    source: await sourceIdentity(directory),
    platform: process.platform,
    arch: process.arch,
    builtAt: new Date().toISOString(),
    command: ["fixture"],
    binary: await binaryIdentity(process.execPath),
    kind: "local",
  };
}

test("verification fingerprints tracked edits, deletions, and untracked source", async () => {
  await temporary(async (directory) => {
    await repository(directory);
    const before = await sourceIdentity(directory);
    expect(before.dirty).toBe(false);
    const path = join(directory, "src/main.ts");
    await writeFile(path, "export const value = 2;\n");
    expect(() =>
      assertSameSource(before, { ...before, sha256: "changed" }),
    ).toThrow("Source changed");
    const changed = await sourceIdentity(directory);
    expect(changed.sha256).not.toBe(before.sha256);
    expect(changed.dirty).toBe(true);
    expect(() => assertSameSource(before, changed)).toThrow("Source changed");
    await writeFile(path, "export const value = 1;\n");
    assertSameSource(before, await sourceIdentity(directory));
    await writeFile(
      join(directory, "src/new.ts"),
      "export const extra = true;\n",
    );
    expect((await sourceIdentity(directory)).sha256).not.toBe(before.sha256);
    await rm(join(directory, "src/new.ts"));
    await rm(path);
    expect((await sourceIdentity(directory)).sha256).not.toBe(before.sha256);
  });
});

test("retained evidence does not invalidate the source identity", async () => {
  await temporary(async (directory) => {
    await repository(directory);
    const before = await sourceIdentity(directory);
    await mkdir(join(directory, "test-results"));
    await writeFile(
      join(directory, "test-results/result.json"),
      '{"passed":true}',
    );
    assertSameSource(before, await sourceIdentity(directory));
  });
});

test("source fingerprint errors identify a directory link", async () => {
  await temporary(async (directory) => {
    await repository(directory);
    const target = join(directory, "test-results");
    await mkdir(target);
    await symlink(target, join(directory, "dependencies"), "junction");
    await expect(sourceIdentity(directory)).rejects.toThrow(
      "Cannot fingerprint dependencies:",
    );
  });
});

test("native identity rejects text files even with executable permissions", async () => {
  await temporary(async (directory) => {
    const path = join(directory, "tinydash");
    await writeFile(path, "not a native executable\n");
    await chmod(path, 0o755);
    await expect(binaryIdentity(path)).rejects.toThrow("Not a native");
    await expect(binaryIdentity(directory)).rejects.toThrow(
      "Not an executable file",
    );
  });
});

test("build records reject changed executables and another platform", async () => {
  await temporary(async (directory) => {
    const record = await buildRecord(directory);
    await verifyBuild(record, process.execPath);
    await expect(
      verifyBuild(
        { ...record, binary: { ...record.binary, sha256: "0".repeat(64) } },
        process.execPath,
      ),
    ).rejects.toThrow("Executable differs");
    const path = join(directory, "build.json");
    await writeFile(
      path,
      JSON.stringify({ ...record, platform: "another-platform" }),
    );
    await expect(readBuildRecord(path)).rejects.toThrow(
      "platform or architecture",
    );
  });
});

test("a build record cannot verify source edited after its build", async () => {
  await temporary(async (directory) => {
    const record = await buildRecord(directory);
    await writeFile(
      join(directory, "src/main.ts"),
      "export const value = 99;\n",
    );
    const current = await sourceIdentity(directory);
    expect(record.source.commit).toBe(current.commit);
    expect(() => assertSameSource(record.source, current)).toThrow(
      "Source changed",
    );
  });
});

test("build helper rejects options that could stamp an old release executable", async () => {
  for (const flag of [
    "--help",
    "--debug",
    "-d",
    "--target=aarch64-apple-darwin",
    "--profile",
    "--config",
  ]) {
    const result = spawnSync("bun", ["scripts/verify/build.ts", flag], {
      encoding: "utf8",
      timeout: 10_000,
    });
    expect(result.status, result.stdout + result.stderr).not.toBe(0);
    expect(result.stderr).toContain("Usage:");
  }
});

test("local proof rejects a changed external Tauri configuration", async () => {
  await temporary(async (directory) => {
    const record = await buildRecord(directory);
    const config = join(directory, "test-config.json");
    await writeFile(config, '{"identifier":"dev.tinydash.test"}');
    record.configuration = [{ path: config, sha256: await fileHash(config) }];
    await verifyBuild(record, process.execPath);
    await writeFile(config, '{"identifier":"dev.tinydash.changed"}');
    await expect(verifyBuild(record, process.execPath)).rejects.toThrow(
      "Build configuration changed",
    );
  });
});

test("build records require a new executable and unchanged source", async () => {
  await temporary(async (directory) => {
    await repository(directory);
    await writeFile(
      join(directory, ".gitignore"),
      "test-results/\n.local/\nsrc-tauri/target/\n",
    );
    await writeFile(
      join(directory, "package.json"),
      JSON.stringify({ scripts: { tauri: "bun fixture-build.mjs" } }),
    );
    await writeFile(
      join(directory, "fixture-build.mjs"),
      `
      import { copyFile, mkdir, writeFile } from "node:fs/promises";
      if (process.env.VERIFICATION_BUILD_MODE === "no-output") process.exit(0);
      await mkdir("src-tauri/target/release", { recursive: true });
      await copyFile(process.execPath, "src-tauri/target/release/tinydash" + (process.platform === "win32" ? ".exe" : ""));
      if (process.env.VERIFICATION_BUILD_MODE === "edit-source") await writeFile("src/main.ts", "changed during build");
    `,
    );
    const inline = '{"bundle":{"createUpdaterArtifacts":true}}';
    const run = (mode: string) =>
      spawnSync(
        "bun",
        [resolve("scripts/verify/build.ts"), "--no-bundle", "--config", inline],
        {
          cwd: directory,
          env: {
            ...process.env,
            CARGO_TARGET_DIR: "",
            VERIFICATION_BUILD_MODE: mode,
          },
          encoding: "utf8",
          timeout: 15_000,
        },
      );
    const built = run("success");
    expect(built.status, built.stdout + built.stderr).toBe(0);
    const target = join(directory, "src-tauri/target");
    const record = await readBuildRecord(
      join(target, "verification-build.json"),
    );
    expect(record.command.at(-1)).toBe(inline);
    assertSameSource(record.source, await sourceIdentity(directory));
    await verifyBuild(record, record.binary.path);
    for (const mode of ["no-output", "edit-source"]) {
      const failed = run(mode);
      expect(failed.status, failed.stdout + failed.stderr).not.toBe(0);
      const files = await readdir(target);
      expect(files).not.toContain("verification-build.json");
      expect(files).not.toContain("verification-build.lock");
    }
  });
});

test("installed build records bind the selected package, payload, and source", async () => {
  await temporary(async (directory) => {
    const record = await buildRecord(directory);
    const artifacts = join(directory, "test-results/packages");
    await mkdir(artifacts, { recursive: true });
    const packagePath = join(artifacts, "test-package.bin");
    await writeFile(
      packagePath,
      "synthetic package; payload supplied by installer fixture",
    );
    await writeFile(join(artifacts, "build.json"), JSON.stringify(record));
    execFileSync(
      "bun",
      [resolve("scripts/ci/artifacts.ts"), "checksums", artifacts],
      { stdio: "pipe" },
    );
    const output = join(directory, "test-results/installed-build.json");
    const installed = await recordInstalledBuild(
      artifacts,
      packagePath,
      process.execPath,
      process.execPath,
      output,
    );
    expect(installed.source).toEqual(record.source);
    expect(installed.kind).toBe("installed");
    expect(installed.package?.sha256).toBe(await fileHash(packagePath));
    expect(await readBuildRecord(output)).toEqual(installed);
    const wrongBinary = join(directory, "test-results/wrong-binary");
    await copyFile(process.execPath, wrongBinary);
    await chmod(wrongBinary, 0o755);
    const bytes = await readFile(wrongBinary);
    bytes[bytes.length - 1] ^= 1;
    await writeFile(wrongBinary, bytes);
    await expect(
      recordInstalledBuild(
        artifacts,
        packagePath,
        process.execPath,
        wrongBinary,
        output,
      ),
    ).rejects.toThrow("differs from the extracted package");
    await writeFile(packagePath, "changed after checksum creation");
    await expect(
      recordInstalledBuild(
        artifacts,
        packagePath,
        process.execPath,
        process.execPath,
        output,
      ),
    ).rejects.toThrow();
  });
});

test("the wrapper retains separate runs and rejects source changes during a check", async () => {
  await temporary(async (directory) => {
    await repository(directory);
    await writeFile(
      join(directory, "package.json"),
      JSON.stringify({ scripts: { "test:ui": "bun fixture-check.mjs" } }),
    );
    await writeFile(
      join(directory, "fixture-check.mjs"),
      `
      import { mkdir, writeFile } from "node:fs/promises";
      import { join } from "node:path";
      await mkdir(process.env.TINYDASH_TEST_OUTPUT, { recursive: true });
      await writeFile(join(process.env.TINYDASH_TEST_OUTPUT, "fixture-evidence.txt"), "completed");
      if (process.env.VERIFICATION_EDIT_SOURCE === "1") await writeFile("src/main.ts", "changed during check");
    `,
    );
    const runner = resolve("scripts/verify/run.ts");
    const run = (edit = false) =>
      spawnSync("bun", [runner, "browser", "tests/example.spec.ts"], {
        cwd: directory,
        env: { ...process.env, VERIFICATION_EDIT_SOURCE: edit ? "1" : "0" },
        encoding: "utf8",
        timeout: 15_000,
      });
    for (let attempt = 0; attempt < 2; attempt++) {
      const result = run();
      expect(result.status, result.stdout + result.stderr).toBe(0);
    }
    const failed = run(true);
    expect(failed.status, failed.stdout + failed.stderr).not.toBe(0);
    const output = join(directory, "test-results/verification");
    const runs = (await readdir(output)).sort();
    expect(runs).toHaveLength(3);
    const results = [];
    for (const name of runs) {
      results.push(
        JSON.parse(await readFile(join(output, name, "result.json"), "utf8")),
      );
      expect(
        await readFile(
          join(output, name, "browser/fixture-evidence.txt"),
          "utf8",
        ),
      ).toBe("completed");
    }
    expect(results.map((result) => result.passed)).toEqual([true, true, false]);
    expect(results[2].failure).toContain("Source changed during verification");
    expect(results[2].source.sha256).not.toBe(results[2].finalSource.sha256);
  });
});
