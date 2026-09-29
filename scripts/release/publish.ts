import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { validateEvidence } from "./evidence.ts";

type Run = (command: string, args: string[]) => string;
type Release = {
  id: number;
  draft: boolean;
  assets: {
    id: number;
    name: string;
    size: number;
    updated_at: string;
    digest?: string;
  }[];
};
const assetIdentity = (release: Release) =>
  release.assets
    .map(({ id, name, size, updated_at, digest }) => ({
      id,
      name,
      size,
      updated_at,
      digest,
    }))
    .sort((a, b) => a.name.localeCompare(b.name, "en"));

export async function publish(
  tag: string,
  evidencePath: string,
  run: Run = (command, args) =>
    execFileSync(command, args, { encoding: "utf8" }),
) {
  assert(
    tag && evidencePath && /^v\d+\.\d+\.\d+(?:-[\w.-]+)?$/.test(tag),
    "Usage: bun scripts/release/publish.ts TAG EVIDENCE_JSON",
  );
  const gh = (...args: string[]) => run("gh", args);
  const repository = gh(
    "repo",
    "view",
    "--json",
    "nameWithOwner",
    "--jq",
    ".nameWithOwner",
  ).trim();
  const release = (): Release =>
    JSON.parse(gh("api", `repos/${repository}/releases/tags/${tag}`));
  const before = release();
  assert.equal(
    before.draft,
    true,
    "Publication gate requires an existing draft",
  );
  const ref = () => run("git", ["rev-parse", `${tag}^{commit}`]).trim();
  const commit = ref();
  const remoteCommit = () => {
    let remote = JSON.parse(
      gh("api", `repos/${repository}/git/ref/tags/${tag}`),
    ).object;
    for (let depth = 0; remote.type === "tag"; depth++) {
      assert(depth < 8, "Excessively nested release tag");
      remote = JSON.parse(
        gh("api", `repos/${repository}/git/tags/${remote.sha}`),
      ).object;
    }
    assert.equal(remote.type, "commit", "Tag must identify a commit");
    return remote.sha;
  };
  assert.equal(remoteCommit(), commit, "Local and remote release tags differ");
  const output = await mkdtemp(resolve(tmpdir(), "tinydash-publication-"));
  try {
    gh("release", "download", tag, "--dir", output);
    const evidence = JSON.parse(await readFile(evidencePath, "utf8"));
    await validateEvidence(
      evidence,
      output,
      dirname(resolve(evidencePath)),
      tag,
      commit,
    );
    // GitHub has no atomic compare-and-publish API: maintainers must freeze
    // draft/tag edits for this command. Download counters are not asset identity.
    const after = release();
    assert.equal(after.draft, true, "Draft changed during validation");
    assert.equal(after.id, before.id, "Release replaced during validation");
    assert.deepEqual(
      assetIdentity(after),
      assetIdentity(before),
      "Assets changed during validation",
    );
    assert.equal(ref(), commit, "Local tag changed during validation");
    assert.equal(
      remoteCommit(),
      commit,
      "Remote tag changed during validation",
    );
    gh("release", "edit", tag, "--draft=false");
    console.log(
      `Published ${tag} after checking exact downloaded assets and release evidence.`,
    );
  } finally {
    await rm(output, { recursive: true, force: true });
  }
}

if (resolve(process.argv[1] ?? "") === fileURLToPath(import.meta.url)) {
  await publish(process.argv[2], process.argv[3]);
}
