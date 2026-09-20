import { execFileSync, spawnSync } from "node:child_process";
import { mkdtemp, mkdir, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { expect, test } from "@playwright/test";

const checker = resolve("scripts/verify/repository.ts");
const pushChecker = resolve("scripts/verify/push.ts");

async function fixture(run: (root: string) => Promise<void>) {
  const root = await mkdtemp(join(tmpdir(), "tinydash-repository-"));
  try {
    execFileSync("git", ["init", "--quiet"], { cwd: root });
    execFileSync("git", ["config", "core.hooksPath", join(root, "no-hooks")], {
      cwd: root,
    });
    await run(root);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
}

async function file(root: string, name: string, text: string) {
  await mkdir(dirname(join(root, name)), { recursive: true });
  await writeFile(join(root, name), text);
}

function check(root: string) {
  return spawnSync("bun", [checker, "--staged"], {
    cwd: root,
    encoding: "utf8",
  });
}

test.describe("repository privacy", { tag: "@smoke" }, () => {
  test("rejects a private file added with force", async () => {
    await fixture(async (root) => {
      await file(root, ".gitignore", ".local/\n");
      await file(root, ".local/research/private.md", "Private work\n");
      execFileSync("git", ["add", "--force", ".local/research/private.md"], {
        cwd: root,
      });
      const result = check(root);
      expect(result.status).toBe(1);
      expect(result.stderr).toContain("Private or generated path");
    });
  });

  test("rejects legacy work documents and permits the public feature catalog", async () => {
    await fixture(async (root) => {
      await file(root, "docs/release-checkpoint.md", "Work record\n");
      await file(root, "docs/reference/features/files.md", "# Files\n");
      execFileSync("git", ["add", "docs"], { cwd: root });
      expect(check(root).status).toBe(1);
      execFileSync("git", ["rm", "--cached", "docs/release-checkpoint.md"], {
        cwd: root,
      });
      expect(check(root).status).toBe(0);
    });
  });

  test("rejects links to a file that exists only in the working tree", async () => {
    await fixture(async (root) => {
      await file(
        root,
        "README.md",
        "# Example\n[Guide](docs/how-to/guide.md)\n",
      );
      await file(root, "docs/how-to/guide.md", "# Guide\n");
      execFileSync("git", ["add", "README.md"], { cwd: root });
      expect(check(root).status).toBe(1);
      execFileSync("git", ["add", "docs/how-to/guide.md"], { cwd: root });
      expect(check(root).status).toBe(0);
    });
  });

  test("checks staged anchors even when the working copy has a repair", async () => {
    await fixture(async (root) => {
      await file(
        root,
        "README.md",
        "# Example\n[Guide](docs/how-to/guide.md#start)\n",
      );
      await file(root, "docs/how-to/guide.md", "# Guide\n");
      execFileSync("git", ["add", "."], { cwd: root });
      await file(root, "docs/how-to/guide.md", "# Guide\n## Start\n");
      expect(check(root).stderr).toContain("missing anchor");
      execFileSync("git", ["add", "docs/how-to/guide.md"], { cwd: root });
      expect(check(root).status).toBe(0);
    });
  });

  test("rejects private history after the current tree removes the file", async () => {
    await fixture(async (root) => {
      const name =
        process.platform === "win32" ? "private name.md" : "private\nname.md";
      await file(root, `designs/${name}`, "Private design\n");
      execFileSync("git", ["add", "."], { cwd: root });
      const commit = (message: string) =>
        execFileSync(
          "git",
          [
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.invalid",
            "commit",
            "--quiet",
            "-m",
            message,
          ],
          { cwd: root },
        );
      commit("Add design");
      execFileSync("git", ["rm", "-r", "designs"], { cwd: root });
      commit("Remove design");
      expect(check(root).status).toBe(0);
      const sha = execFileSync("git", ["rev-parse", "HEAD"], {
        cwd: root,
        encoding: "utf8",
      }).trim();
      const result = spawnSync("bun", [pushChecker], {
        cwd: root,
        encoding: "utf8",
        input: `refs/heads/main ${sha} refs/heads/main ${"0".repeat(40)}\n`,
      });
      expect(result.status).toBe(1);
      expect(result.stderr).toContain("private paths remain");
    });
  });

  test("permits public installation guides in history after they move", async () => {
    await fixture(async (root) => {
      await file(root, "docs/install.md", "# Install\nPublic instructions.\n");
      execFileSync("git", ["add", "."], { cwd: root });
      const commit = (message: string) =>
        execFileSync(
          "git",
          [
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.invalid",
            "commit",
            "--quiet",
            "-m",
            message,
          ],
          { cwd: root },
        );
      commit("Add installation guide");
      expect(check(root).status).toBe(1);
      await mkdir(join(root, "docs/how-to"), { recursive: true });
      execFileSync("git", ["mv", "docs/install.md", "docs/how-to/install.md"], {
        cwd: root,
      });
      commit("Move public guide");
      expect(check(root).status).toBe(0);
      const sha = execFileSync("git", ["rev-parse", "HEAD"], {
        cwd: root,
        encoding: "utf8",
      }).trim();
      const result = spawnSync("bun", [pushChecker], {
        cwd: root,
        encoding: "utf8",
        input: `refs/heads/main ${sha} refs/heads/main ${"0".repeat(40)}\n`,
      });
      expect(result.status).toBe(0);
    });
  });
});
