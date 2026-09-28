import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { checkDocumentation } from "./docs.ts";

const read = (path: string) => readFileSync(path, "utf8");
test("documented facts agree with the checked-out source", () =>
  checkDocumentation(read));

for (const [file, before, after] of [
  [
    "docs/reference/settings.md",
    '"fileSearchLimit": 50000',
    '"fileSearchLimit": 50001',
  ],
  ["docs/reference/settings.md", '"hideOnBlur": true', '"hideOnBlur": false'],
  ["docs/reference/settings.md", "between 1 and 500", "between 1 and 501"],
  ["docs/reference/settings.md", "through 100,000", "through 200,000"],
  [
    "docs/reference/settings.md",
    '"shortcut": "Control+Shift+Space"',
    '"shortcut": "Alt+Space"',
  ],
  [
    "docs/reference/features/clipboard.md",
    "default limit is 100",
    "default limit is 90",
  ],
  [
    "docs/reference/command-line.md",
    "`tinydash --settings`",
    "`tinydash --setting`",
  ],
  ["docs/reference/command-line.md", "`timezone`", "`datetime`"],
  ["docs/explanation/architecture.md", "top 30 results", "top 40 results"],
  ["docs/reference/features/launcher.md", "30-result limit", "40-result limit"],
  [
    "docs/explanation/data-and-privacy.md",
    "Apps, Files, System",
    "Files, Apps, System",
  ],
  [
    "docs/explanation/data-and-privacy.md",
    "25 points per use",
    "26 points per use",
  ],
  [
    "src-tauri/src/ranking/mod.rs",
    "EXACT_BONUS: u32 = 10_000",
    "EXACT_BONUS: u32 = 11_000",
  ],
  [
    "src-tauri/src/launcher/query.rs",
    'Self::All => "all"',
    'Self::All => "everything"',
  ],
]) {
  test(`rejects drift in ${file}: ${before}`, () => {
    assert(read(file).includes(before), "Mutation must affect this checkout");
    assert.throws(() =>
      checkDocumentation((path) =>
        path === file ? read(path).replace(before, after) : read(path),
      ),
    );
  });
}
