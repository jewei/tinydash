import assert from "node:assert/strict";
import { createRequire } from "node:module";
import { readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";

interface DatabaseHandle {
  run(sql: string, ...parameters: unknown[]): unknown;
  query(sql: string): { all(): unknown[] };
  close(): void;
}

// Bun provides SQLite for the test process. The app still owns its database.
const { Database } = createRequire(import.meta.url)("bun:sqlite") as {
  Database: new (path: string) => DatabaseHandle;
};

export const upgradeText = "TinyDash upgrade fixture: spaces and emoji 🚀";
export const upgradeSettings = {
  currencyRatesEnabled: false,
  clearQueryOnOpen: false,
  hideOnBlur: false,
  clipboardHistoryEnabled: true,
  clipboardHistoryDecided: true,
  fileSearchRoots: [],
  fileWatchEnabled: false,
  startAtLogin: false,
};

export async function writeUpgradeSettings(config: string) {
  await writeFile(
    join(config, "settings.json"),
    JSON.stringify(upgradeSettings),
  );
}

export function seedUpgradeData(data: string) {
  const database = new Database(join(data, "tinydash.sqlite3"));
  try {
    // The older installed app must create its own schema before this setup.
    database.run(
      "INSERT INTO clipboard_history (id, content, created_at, last_used_at, pinned, sort_order) VALUES (987654, ?, 100, 101, 0, 1)",
      upgradeText,
    );
    database.run(
      "INSERT INTO pinned_items (category, result_id) VALUES ('all', 'clipboard:987654'), ('clipboard', 'clipboard:987654')",
    );
    database.run(
      "INSERT INTO usage_history (result_id, use_count, last_used_at) VALUES ('clipboard:987654', 7, 101)",
    );
  } finally {
    database.close();
  }
}

export async function readUpgradeData(config: string, data: string) {
  const settings = JSON.parse(
    await readFile(join(config, "settings.json"), "utf8"),
  ) as Record<string, unknown>;
  for (const [name, expected] of Object.entries(upgradeSettings)) {
    assert.deepEqual(
      settings[name],
      expected,
      `Saved setting changed: ${name}`,
    );
  }
  const database = new Database(join(data, "tinydash.sqlite3"));
  try {
    return {
      settings,
      clipboard: database
        .query("SELECT * FROM clipboard_history WHERE id = 987654")
        .all(),
      pins: database
        .query(
          "SELECT * FROM pinned_items WHERE result_id = 'clipboard:987654' ORDER BY category",
        )
        .all(),
      usage: database
        .query(
          "SELECT * FROM usage_history WHERE result_id = 'clipboard:987654'",
        )
        .all(),
    };
  } finally {
    database.close();
  }
}
