import { cp, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { execFileSync } from "node:child_process";
import { validateRelease } from "../../site/downloads";
import { renderPage } from "../../site/render";

const root = fileURLToPath(new URL("../../", import.meta.url));
const output = resolve(root, "site-dist");
const manifest = validateRelease(
  JSON.parse(
    await readFile(
      process.env.TINYDASH_RELEASE_MANIFEST ??
        resolve(root, "site/release.json"),
      "utf8",
    ),
  ),
);
await rm(output, { recursive: true, force: true });
await mkdir(resolve(output, "assets/fonts"), { recursive: true });
execFileSync(
  "bun",
  [
    "build",
    resolve(root, "site/downloads.ts"),
    "--outdir",
    resolve(output, "assets"),
    "--target=browser",
    "--minify",
  ],
  { stdio: "inherit" },
);
await Promise.all([
  ...["styles.css", "tokens.css"].map((name) =>
    cp(resolve(root, "site", name), resolve(output, name)),
  ),
  writeFile(
    resolve(output, "index.html"),
    renderPage(
      await readFile(resolve(root, "site/index.html"), "utf8"),
      manifest,
    ),
  ),
  cp(resolve(root, "public/fonts"), resolve(output, "assets/fonts"), {
    recursive: true,
  }),
  cp(
    resolve(root, "docs/assets/launcher-macos-light.png"),
    resolve(output, "assets/launcher-macos-light.png"),
  ),
  cp(
    resolve(root, "src-tauri/icons/32x32.png"),
    resolve(output, "assets/icon.png"),
  ),
  writeFile(
    resolve(output, "release.json"),
    `${JSON.stringify(manifest, null, 2)}\n`,
  ),
  writeFile(resolve(output, ".nojekyll"), ""),
]);
console.log(
  `Built ${output}. ${manifest.version ? `Release ${manifest.version}.` : "No public installers configured."}`,
);
