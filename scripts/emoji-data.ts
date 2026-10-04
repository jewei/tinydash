// Regenerate src-tauri/data/emoji/*.tsv from pinned Unicode CLDR data.
// Run with `bun run emoji-data`. To update CLDR, change the revision, run the
// script once to see the new hashes, review the diff, and pin the hashes.
import { createHash } from "node:crypto";
import { writeFile } from "node:fs/promises";

const revision = "4d06be52b51bb2f75688d0abe55c52a66afed790"; // CLDR JSON 48.0.0
const base = `https://raw.githubusercontent.com/unicode-org/cldr-json/${revision}/cldr-json`;

// SHA-256 of each source: [annotations, derived annotations].
const languages: Record<string, [string, string]> = {
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

type Annotations = Record<string, { default?: string[]; tts?: string[] }>;

async function annotations(path: string, sha256: string, key: string): Promise<Annotations> {
  const response = await fetch(`${base}/${path}`);
  if (!response.ok) throw new Error(`${path}: HTTP ${response.status}`);
  const text = await response.text();
  const actual = createHash("sha256").update(text).digest("hex");
  if (actual !== sha256) throw new Error(`${path}: expected ${sha256}, got ${actual}`);
  return JSON.parse(text)[key].annotations;
}

const byCodePoint = (a: string, b: string) => (a < b ? -1 : a > b ? 1 : 0);
const skinTone = /[\u{1f3fb}-\u{1f3ff}]/u;

for (const [language, [plainHash, derivedHash]] of Object.entries(languages)) {
  const sources = [
    await annotations(
      `cldr-annotations-full/annotations/${language}/annotations.json`,
      plainHash,
      "annotations",
    ),
    await annotations(
      `cldr-annotations-derived-full/annotationsDerived/${language}/annotations.json`,
      derivedHash,
      "annotationsDerived",
    ),
  ];
  const rows = new Map<string, { name: string; terms: Set<string> }>();
  for (const source of sources) {
    for (const [emoji, { tts = [], default: keywords = [] }] of Object.entries(source)) {
      // The app derives skin-tone variants itself; index base emoji only.
      if (skinTone.test(emoji)) continue;
      const row = rows.get(emoji) ?? { name: "", terms: new Set<string>() };
      for (const term of [...tts, ...keywords]) {
        if (term === "↑↑↑") continue; // CLDR's "inherit" marker
        if (/[\t\r\n]/.test(term)) throw new Error(`${language}: delimiter in "${term}"`);
        row.terms.add(term.normalize("NFC"));
      }
      row.name ||= (tts.find((term) => term !== "↑↑↑") ?? "").normalize("NFC");
      if (row.terms.size > 0) rows.set(emoji, row);
    }
  }
  const lines = [...rows]
    .sort(([a], [b]) => byCodePoint(a, b))
    .map(([emoji, { name, terms }]) =>
      [emoji, name, ...[...terms].filter((term) => term !== name).sort(byCodePoint)].join("\t"),
    );
  await writeFile(`src-tauri/data/emoji/${language}.tsv`, `${lines.join("\n")}\n`);
  console.log(`${language}: ${rows.size} rows`);
}
