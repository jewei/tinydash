import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

type Read = (path: string) => string;
const number = (value: string) =>
  Number(value.replaceAll("_", "").replaceAll(",", ""));
const capture = (text: string, pattern: RegExp, label: string) => {
  const match = pattern.exec(text);
  assert(
    match,
    `Cannot find ${label}; update the focused documentation contract`,
  );
  return match;
};

// Small, explicit contracts for duplicated public facts, not a Rust/Markdown
// generator. Formatting/shape changes fail visibly rather than skipping checks.
export function checkDocumentation(read: Read) {
  const settings = read("src-tauri/src/settings.rs");
  const reference = read("docs/reference/settings.md");
  const example = JSON.parse(
    capture(reference, /```json\n([\s\S]*?)\n```/, "settings example")[1],
  );
  const defaults = capture(
    settings,
    /impl Default for Settings \{([\s\S]*?)visible_categories:/,
    "settings defaults",
  )[1];
  for (const key of [
    "clearQueryOnOpen",
    "hideOnBlur",
    "clipboardHistoryLimit",
    "fileSearchLimit",
    "fileWatchEnabled",
    "currencyRatesEnabled",
  ]) {
    const field = key.replace(/[A-Z]/g, (letter) => `_${letter.toLowerCase()}`);
    const value = capture(
      defaults,
      new RegExp(`\\b${field}: (true|false|[\\d_]+),`),
      key,
    )[1];
    assert.equal(
      example[key],
      value === "true" ? true : value === "false" ? false : number(value),
      `Documented default: ${key}`,
    );
  }
  // clipboardHistoryEnabled=false is an intentional example, not the legacy
  // deserialization default. Fresh-install consent remains covered by Rust/UI tests.
  assert.equal(
    example.shortcut,
    capture(
      settings,
      /DEFAULT_SHORTCUT: &str = "([^"]+)"/,
      "default shortcut",
    )[1],
    "Documented shortcut",
  );
  for (const [field, key, phrase] of [
    ["clipboard_history_limit", "clipboardHistoryLimit", "a value between"],
    ["file_search_limit", "fileSearchLimit", "restricted to"],
  ]) {
    const range = capture(
      settings,
      new RegExp(
        `\\((\\d+)\\.\\.=([\\d_]+)\\)\\.contains\\(&self\\.${field}\\)`,
      ),
      `${key} range`,
    );
    const documented = capture(
      reference,
      new RegExp(
        `\`${key}\` (?:to|is) ${phrase} ([\\d,]+) (?:and|through) ([\\d,]+)`,
      ),
      `${key} documented range`,
    );
    assert.deepEqual(
      documented.slice(1).map(number),
      range.slice(1).map(number),
      `Documented range: ${key}`,
    );
  }
  const clipboard = read("docs/reference/features/clipboard.md");
  assert.equal(
    number(
      capture(
        clipboard,
        /default limit is ([\d,]+) unpinned/,
        "clipboard default",
      )[1],
    ),
    example.clipboardHistoryLimit,
  );

  const commandDoc = read("docs/reference/command-line.md");
  const startup = read("src-tauri/src/launcher/startup.rs");
  const options = [...startup.matchAll(/"(--[\w-]+)" =>/g)].map(
    (match) => match[1],
  );
  assert(options.length > 0, "Missing command parser arms");
  assert(
    read("src-tauri/src/lib.rs").includes('args.as_slice() == ["--help"]'),
    "Missing help command",
  );
  const documentedOptions = [
    ...commandDoc.matchAll(/\| `tinydash (--[\w-]+)(?: CATEGORY)?`/g),
  ].map((match) => match[1]);
  assert.deepEqual(
    documentedOptions.sort(),
    [...options, "--help"].sort(),
    "Documented commands",
  );
  const query = read("src-tauri/src/launcher/query.rs");
  const modes = [...query.matchAll(/Self::\w+ => "(\w+)"/g)].map(
    (match) => match[1],
  );
  assert(modes.length > 0, "Missing search mode strings");
  const categoryLine = capture(
    commandDoc,
    /`CATEGORY` accepts ([^.]+)\./,
    "command categories",
  )[1];
  assert.deepEqual(
    [...categoryLine.matchAll(/`(\w+)`/g)].map((match) => match[1]).sort(),
    modes.sort(),
    "Documented categories",
  );

  const limit = number(
    capture(
      read("src-tauri/src/launcher/search.rs"),
      /RESULT_LIMIT: usize = ([\d_]+);/,
      "result limit",
    )[1],
  );
  const architecture = read("docs/explanation/architecture.md");
  const launcher = read("docs/reference/features/launcher.md");
  const privacy = read("docs/explanation/data-and-privacy.md");
  for (const [text, pattern] of [
    [architecture, /top (\d+) results/g],
    [launcher, /(\d+)-result limit/g],
    [privacy, /(\d+)-result limit/g],
  ] as const) {
    const matches = [...text.matchAll(pattern)];
    assert(matches.length > 0, "Missing documented result limit");
    for (const match of matches)
      assert.equal(number(match[1]), limit, "Documented result limit");
  }
  const ranking = read("src-tauri/src/ranking/mod.rs");
  const frequency = capture(
    ranking,
    /usage.count.min\((\d+)\) \* (\d+)/,
    "frequency bonus",
  );
  const recency = number(
    capture(
      ranking,
      /let recency = (\d+) \/ \(days \+ 1\)/,
      "recency bonus",
    )[1],
  );
  const exact = number(
    capture(ranking, /EXACT_BONUS: u32 = ([\d_]+);/, "exact bonus")[1],
  );
  const prefix = number(
    capture(ranking, /PREFIX_BONUS: u32 = ([\d_]+);/, "prefix bonus")[1],
  );
  const bonuses = capture(
    privacy,
    /frequency bonus is ([\d,]+) points per use, up to ([\d,]+) points\. The recency bonus starts at ([\d,]+) points[\s\S]*?combined limit is ([\d,]+) points, compared with ([\d,]+) for an exact match and ([\d,]+) for a prefix match/,
    "documented bonuses",
  );
  const frequencyMax = number(frequency[1]) * number(frequency[2]);
  assert.deepEqual(
    bonuses.slice(1).map(number),
    [
      number(frequency[2]),
      frequencyMax,
      recency,
      frequencyMax + recency,
      exact,
      prefix,
    ],
    "Documented ranking bonuses",
  );
  const labels: Record<string, string> = {
    App: "Apps",
    File: "Files",
    SystemCommand: "System",
    Clipboard: "Clipboard",
    Emoji: "Emoji",
  };
  const categoryBlock = capture(
    ranking,
    /let category = match result.kind \{([\s\S]*?)_ => 0,/,
    "ranking categories",
  )[1];
  const order = [
    ...categoryBlock.matchAll(
      /ResultKind::(\w+)(?: \| ResultKind::\w+)? => (\d+),/g,
    ),
  ]
    .sort((a, b) => number(a[2]) - number(b[2]))
    .map((match) => labels[match[1]]);
  assert(order.length > 0 && order.every(Boolean), "Unknown ranking category");
  const proseOrder = capture(
    privacy,
    /in this order: ([^.]+)\./,
    "documented category order",
  )[1]
    .replace(", then ", ", ")
    .split(", ");
  assert.deepEqual(proseOrder, order, "Documented category order");
}

if (resolve(process.argv[1] ?? "") === fileURLToPath(import.meta.url)) {
  checkDocumentation((path) => readFileSync(path, "utf8"));
  console.log(
    "Focused documentation contracts passed (defaults, limits, commands, ranking).",
  );
}
