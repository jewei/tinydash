import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readdir, readFile, mkdir, writeFile, rename } from "node:fs/promises";
import { basename, dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import {
  platforms,
  validateRelease,
  type Download,
  type Platform,
  type Release,
} from "../../site/downloads";

interface PublicAsset {
  name: string;
  browser_download_url: string;
  size: number;
}
interface PublicRelease {
  id: number;
  draft: boolean;
  prerelease: boolean;
  tag_name: string;
  html_url: string;
  published_at: string;
  assets: PublicAsset[];
}
const repository = "https://api.github.com/repos/jewei/tinydash/releases";
const hash = (bytes: Uint8Array) =>
  createHash("sha256").update(bytes).digest("hex");

async function getJson(
  url: string,
  request: typeof fetch,
): Promise<PublicRelease> {
  // Deliberately unauthenticated. The public page cannot depend on a maintainer's token.
  const response = await request(url, {
    headers: { Accept: "application/vnd.github+json" },
    signal: AbortSignal.timeout(30_000),
  });
  assert(
    response.ok,
    `Anonymous release lookup failed: HTTP ${response.status}`,
  );
  return response.json() as Promise<PublicRelease>;
}

async function files(directory: string): Promise<string[]> {
  const entries = await readdir(directory, { withFileTypes: true });
  return (
    await Promise.all(
      entries.map((entry) =>
        entry.isDirectory()
          ? files(join(directory, entry.name))
          : entry.isFile()
            ? [join(directory, entry.name)]
            : [],
      ),
    )
  ).flat();
}

export async function prepareRelease(
  tag: string,
  candidateDirectory: string,
  macRequirement: string,
  destination: string,
  request: typeof fetch = fetch,
): Promise<void> {
  assert(
    tag && candidateDirectory && macRequirement && destination,
    "Usage: bun scripts/site/release.ts v<version> <reviewed-candidate-directory> <tested-macOS-requirement> <output.json>",
  );
  assert(
    /^v\d+\.\d+\.\d+$/.test(tag),
    "Use a stable numeric tag, such as v0.1.0.",
  );
  assert(
    /^macOS [\d.]+(?: or later)?$/.test(macRequirement),
    "Name the macOS version tested for this candidate, such as 'macOS 26'.",
  );
  const [release, latest, candidates] = await Promise.all([
    getJson(`${repository}/tags/${tag}`, request),
    getJson(`${repository}/latest`, request),
    files(resolve(candidateDirectory)),
  ]);
  assert(
    !release.draft && !release.prerelease,
    "The website only links a published stable release.",
  );
  assert.equal(release.tag_name, tag, "GitHub returned a different tag.");
  assert(
    Number.isSafeInteger(release.id) && release.id > 0,
    "Missing release ID.",
  );
  assert.equal(
    release.id,
    latest.id,
    "The selected release is not GitHub's latest stable release.",
  );
  const version = tag.slice(1);
  const suffixes: Record<Platform, string> = {
    macos: `_${version}_aarch64.dmg`,
    windows: `_${version}_x64-setup.exe`,
    linux: `_${version}_amd64.deb`,
  };
  const requirements = {
    macos: macRequirement,
    windows: "Windows 11",
    linux: "Ubuntu 24.04",
  };
  const downloads: Partial<Record<Platform, Download>> = {};
  for (const platform of platforms) {
    const local = candidates.filter((path) =>
      path.endsWith(suffixes[platform]),
    );
    assert.equal(
      local.length,
      1,
      `Expected one reviewed ${platform} installer, found ${local.length}.`,
    );
    const assets = release.assets.filter(
      (asset) => asset.name === basename(local[0]),
    );
    assert.equal(
      assets.length,
      1,
      `Expected one public asset named ${basename(local[0])}.`,
    );
    const asset = assets[0];
    const prefix = `https://github.com/jewei/tinydash/releases/download/${tag}/`;
    assert.equal(
      asset.browser_download_url,
      `${prefix}${asset.name}`,
      "Unexpected public download URL.",
    );
    const original = await readFile(local[0]);
    assert.equal(
      asset.size,
      original.length,
      `The public ${platform} file has a different size.`,
    );
    const response = await request(asset.browser_download_url, {
      signal: AbortSignal.timeout(120_000),
    });
    assert(
      response.ok,
      `Anonymous ${platform} download failed: HTTP ${response.status}`,
    );
    const bytes = new Uint8Array(await response.arrayBuffer());
    assert.equal(
      bytes.length,
      original.length,
      `Incomplete ${platform} download.`,
    );
    const checksum = hash(original);
    assert.equal(
      hash(bytes),
      checksum,
      `The public ${platform} file differs from the reviewed candidate.`,
    );
    downloads[platform] = {
      url: asset.browser_download_url,
      bytes: bytes.length,
      sha256: checksum,
      requirement: requirements[platform],
    };
  }
  const manifest: Release = validateRelease({
    version,
    releaseUrl: release.html_url,
    publishedAt: release.published_at,
    downloads,
  });
  const output = resolve(destination);
  await mkdir(dirname(output), { recursive: true });
  await writeFile(`${output}.tmp`, `${JSON.stringify(manifest, null, 2)}\n`);
  await rename(`${output}.tmp`, output);
  await writeFile(
    `${output}.evidence.json`,
    `${JSON.stringify({ checkedAt: new Date().toISOString(), releaseId: release.id, tag, anonymous: true, sameAsReviewedCandidate: true, downloads }, null, 2)}\n`,
  );
  console.log(
    `All three public installers match the reviewed candidate. Wrote ${output} and its evidence record.`,
  );
}

if (
  process.argv[1] &&
  resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
  const [tag, candidateDirectory, macRequirement, destination] =
    process.argv.slice(2);
  await prepareRelease(tag, candidateDirectory, macRequirement, destination);
}
