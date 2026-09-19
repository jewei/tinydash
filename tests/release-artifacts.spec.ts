import { execFileSync, spawnSync } from "node:child_process";
import { mkdtemp, readFile, rename, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { expect, test } from "@playwright/test";

const targets = [
  {
    os: "darwin",
    arch: "ARM64",
    suffix: "aarch64.dmg",
    publisher: "Developer ID",
  },
  {
    os: "win32",
    arch: "X64",
    suffix: "x64-setup.exe",
    publisher: "none (unsigned preview)",
  },
  { os: "linux", arch: "X64", suffix: "amd64.deb", publisher: "none" },
] as const;
type Target = (typeof targets)[number];

async function withArtifacts(
  target: Target,
  check: (
    directory: string,
    installer: string,
    signature: string | null,
  ) => Promise<void>,
) {
  const directory = await mkdtemp(join(tmpdir(), "tinydash-release-test-"));
  try {
    const { version } = JSON.parse(await readFile("package.json", "utf8"));
    const commit = execFileSync("git", ["rev-parse", "HEAD"], {
      encoding: "utf8",
    }).trim();
    const installer = `TinyDash_${version}_${target.suffix}`;
    const files: Record<string, string> = {
      [installer]: "Test fixture only, not an installer.",
      "install.md": "Test fixture instructions.",
      "desktop-checks.md": "Test fixture instructions.",
      "build.txt": `Commit: ${commit}\nSource: clean worktree\nVersion: ${version}\nOS: ${target.os}\nArchitecture: ${target.arch}\nDistribution: release candidate${target.os === "linux" ? "" : " with updater signatures"}\nPublisher signing: ${target.publisher}\n`,
    };
    const signature =
      target.os === "linux"
        ? null
        : target.os === "darwin"
          ? "TinyDash.app.tar.gz.sig"
          : `${installer}.sig`;
    if (target.os === "darwin")
      files["TinyDash.app.tar.gz"] = "Test fixture archive.";
    if (signature)
      files[signature] = "Test fixture signature. Not cryptographic evidence.";
    await Promise.all(
      Object.entries(files).map(([name, content]) =>
        writeFile(join(directory, name), content),
      ),
    );
    await check(directory, installer, signature);
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
}

function verify(
  directory: string,
  target: Target,
  updater = target.os !== "linux",
) {
  execFileSync("bun", ["scripts/ci/artifacts.ts", "checksums", directory]);
  execFileSync("bun", ["scripts/ci/artifacts.ts", "verify", directory]);
  return spawnSync(
    "bun",
    ["scripts/release/verify-artifacts.ts", directory, target.os, target.arch],
    {
      encoding: "utf8",
      env: {
        ...process.env,
        RELEASE_MODE: "true",
        UPDATER_ARTIFACTS: String(updater),
      },
    },
  );
}

for (const target of targets) {
  test(`${target.os} release metadata must report the expected publisher signing`, async () => {
    await withArtifacts(target, async (directory) => {
      const valid = verify(directory, target);
      expect(valid.stderr).toBe("");
      expect(valid.status).toBe(0);
      const path = join(directory, "build.txt");
      await writeFile(
        path,
        (await readFile(path, "utf8")).replace(
          `Publisher signing: ${target.publisher}`,
          "Publisher signing: trusted Windows publisher",
        ),
      );
      const invalid = verify(directory, target);
      expect(invalid.status).not.toBe(0);
      expect(invalid.stderr).toContain(
        "Publisher signing does not match the release policy",
      );
    });
  });

  if (target.os !== "linux") {
    test(`${target.os} release artifacts require updater signing`, async () => {
      await withArtifacts(target, async (directory) => {
        const result = verify(directory, target, false);
        expect(result.status).not.toBe(0);
        expect(result.stderr).toContain("Release updates require signatures");
      });
    });

    test(`${target.os} release artifacts reject an empty updater signature`, async () => {
      await withArtifacts(target, async (directory, _installer, signature) => {
        await writeFile(join(directory, signature!), " \n");
        const result = verify(directory, target);
        expect(result.status).not.toBe(0);
        expect(result.stderr).toContain("updater signature is empty");
      });
    });
  }
}

test("Windows updater signature must have the installer's exact filename", async () => {
  const target = targets[1];
  await withArtifacts(target, async (directory, _installer, signature) => {
    await rename(
      join(directory, signature!),
      join(directory, `Other${signature}`),
    );
    const result = verify(directory, target);
    expect(result.status).not.toBe(0);
    expect(result.stderr).toContain("Expected one Windows updater signature");
  });
});
