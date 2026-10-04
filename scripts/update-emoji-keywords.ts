// Regenerate bundled search terms from pinned Unicode CLDR data. No runtime download.
import { createHash } from "node:crypto";
import { mkdir, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../", import.meta.url));
const revision = "4d06be52b51bb2f75688d0abe55c52a66afed790"; // CLDR JSON 48.0.0
const base = `https://raw.githubusercontent.com/unicode-org/cldr-json/${revision}`;
const sources = {
  zh: [
    "8d2dfe9bf41894bd17c9c1a1906a31bf0ccc06204bf4b77c2ab2b0d8f79a5fdf",
    "1ff96249af6fc69067f2dfabc26b450786a93d8048f2764ecfad53fd64953f92",
  ],
  ms: [
    "768da2c8147cd4adcd7034e618ecb3f9c8b3e65813fe386672ad78f38aa9b5d8",
    "8a387a0a093e07fef6dcbef57f1c7d7404b03b7bcee40289dcedd0cbea0c2418",
  ],
  es: [
    "339cdf9ae5fe6d3250d3c53b95ff6d8a4dae96c7c0d279756ee7a73c43c84b56",
    "f9f43c327c5b76dbd337496738c0f5c526ee1c803dd683d60100b5fce85ab79e",
  ],
};
async function download(path: string, sha256: string) {
  const response = await fetch(`${base}/${path}`);
  if (!response.ok) throw new Error(`${path}: HTTP ${response.status}`);
  const text = await response.text();
  if (createHash("sha256").update(text).digest("hex") !== sha256)
    throw new Error(`Source hash mismatch: ${path}`);
  return text;
}
const compare = (a: string, b: string) => (a < b ? -1 : a > b ? 1 : 0);
await mkdir(resolve(root, "src-tauri/data/emoji"), { recursive: true });
for (const [locale, hashes] of Object.entries(sources)) {
  const rows = new Map<string, { name: string; terms: Set<string> }>();
  for (const [index, category] of [
    "annotations",
    "annotationsDerived",
  ].entries()) {
    const pkg =
      index === 0 ? "cldr-annotations-full" : "cldr-annotations-derived-full";
    const text = await download(
      `cldr-json/${pkg}/${category}/${locale}/annotations.json`,
      hashes[index],
    );
    const records: Record<string, { default: string[]; tts: string[] }> =
      JSON.parse(text)[category].annotations;
    for (const [emoji, annotation] of Object.entries(records)) {
      // Search indexes base emoji once; the native emoji catalog supplies variants.
      if (/[\u{1f3fb}-\u{1f3ff}]/u.test(emoji)) continue;
      const row = rows.get(emoji) ?? { name: "", terms: new Set<string>() };
      const names = (annotation.tts ?? []).filter((term) => term !== "↑↑↑");
      if (names.length > 1)
        throw new Error(`Unexpected short names: ${locale}`);
      if (!row.name && names[0]) row.name = names[0].normalize("NFC");
      for (const term of [
        ...(annotation.tts ?? []),
        ...(annotation.default ?? []),
      ]) {
        if (term === "↑↑↑") continue;
        if (/[\t\r\n]/.test(term))
          throw new Error(`Unexpected term delimiter: ${locale}`);
        row.terms.add(term.normalize("NFC"));
      }
      if (row.terms.size) rows.set(emoji, row);
    }
  }
  const output =
    [...rows]
      .sort(([a], [b]) => compare(a, b))
      .map(([emoji, { name, terms }]) =>
        [
          emoji,
          name,
          ...[...terms].filter((term) => term !== name).sort(compare),
        ].join("\t"),
      )
      .join("\n") + "\n";
  await writeFile(resolve(root, `src-tauri/data/emoji/${locale}.tsv`), output);
  console.log(`${locale}: ${rows.size} keyword rows`);
}
await writeFile(
  resolve(root, "src-tauri/data/emoji/LICENSE"),
  await download(
    "LICENSE",
    "220ba0e1c43b99530d2d5bdb892a99dca0989414f51ab695ecd90163eaa1ec3b",
  ),
);
