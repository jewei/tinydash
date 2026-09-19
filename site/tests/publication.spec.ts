import { test, expect } from "@playwright/test";
import { mkdtemp, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { prepareRelease } from "../../scripts/site/release";

for (const failure of [
  "none",
  "changed bytes",
  "draft",
  "prerelease",
  "not latest",
  "missing platform",
  "anonymous download denied",
]) {
  test(`public download preparation: ${failure}`, async () => {
    const directory = await mkdtemp(join(tmpdir(), "tinydash-site-"));
    try {
      const candidate = join(directory, "candidate");
      await mkdir(candidate);
      const names = [
        "TinyDash_0.1.0_aarch64.dmg",
        "TinyDash_0.1.0_x64-setup.exe",
        "tinydash_0.1.0_amd64.deb",
      ];
      const bytes = new TextEncoder().encode("test candidate bytes");
      for (const name of names) await writeFile(join(candidate, name), bytes);
      if (failure === "missing platform") await rm(join(candidate, names[2]));
      const output = join(directory, "release.json");
      await writeFile(output, "previous verified release");
      const release = {
        id: 100,
        tag_name: "v0.1.0",
        draft: failure === "draft",
        prerelease: failure === "prerelease",
        html_url: "https://github.com/jewei/tinydash/releases/tag/v0.1.0",
        published_at: "2026-09-19T00:00:00Z",
        assets: names.map((name) => ({
          name,
          size: bytes.length,
          browser_download_url: `https://github.com/jewei/tinydash/releases/download/v0.1.0/${name}`,
        })),
      };
      const requests: string[] = [];
      const request: typeof fetch = async (input, init) => {
        const url = input.toString();
        requests.push(url);
        expect(new Headers(init?.headers).has("Authorization")).toBe(false);
        if (url.endsWith("/latest") && failure === "not latest")
          return Response.json({ ...release, id: 101 });
        if (url.startsWith("https://api.github.com/"))
          return Response.json(release);
        if (failure === "anonymous download denied")
          return new Response("Sign in", { status: 403 });
        return new Response(
          failure === "changed bytes" ? new Uint8Array(bytes.length) : bytes,
        );
      };
      const operation = prepareRelease(
        "v0.1.0",
        candidate,
        "macOS 26",
        output,
        request,
      );
      if (failure === "none") {
        await operation;
        const manifest = JSON.parse(await readFile(output, "utf8"));
        expect(Object.keys(manifest.downloads)).toEqual([
          "macos",
          "windows",
          "linux",
        ]);
        expect(manifest.downloads.macos.bytes).toBe(bytes.length);
        expect(manifest.downloads.macos.sha256).toMatch(/^[a-f0-9]{64}$/);
        expect(
          requests.filter((url) => url.includes("/download/")),
        ).toHaveLength(3);
        expect(
          JSON.parse(await readFile(`${output}.evidence.json`, "utf8"))
            .sameAsReviewedCandidate,
        ).toBe(true);
      } else {
        await expect(operation).rejects.toThrow();
        expect(await readFile(output, "utf8")).toBe(
          "previous verified release",
        );
      }
    } finally {
      await rm(directory, { recursive: true, force: true });
    }
  });
}
