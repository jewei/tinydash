import { existsSync } from "node:fs";
import { createRequire } from "node:module";
import { join } from "node:path";

interface DatabaseHandle {
  run(sql: string, ...parameters: unknown[]): unknown;
  query(sql: string): { all(...parameters: unknown[]): unknown[] };
  close(): void;
}

const { Database } = createRequire(import.meta.url)("bun:sqlite") as {
  Database: new (path: string) => DatabaseHandle;
};

function open(data: string) {
  const path = join(data, "tinydash.sqlite3");
  if (!existsSync(path))
    throw new Error("The native app must create its database first.");
  return new Database(path);
}

// The disposable native test process owns this separate connection, not the
// app's connection. Always release it, including assertion/cancellation failure.
export async function withStorageLock<T>(
  data: string,
  action: () => Promise<T>,
) {
  const database = open(data);
  try {
    database.run("PRAGMA busy_timeout = 1000");
    database.run("BEGIN IMMEDIATE");
    try {
      return await action();
    } finally {
      database.run("ROLLBACK");
    }
  } finally {
    database.close();
  }
}

export function storedClipboard(data: string, text: string) {
  const database = open(data);
  try {
    return database
      .query("SELECT id FROM clipboard_history WHERE content = ?")
      .all(text) as { id: number }[];
  } finally {
    database.close();
  }
}

export function storedUsage(data: string, id: string) {
  const database = open(data);
  try {
    const rows = database
      .query("SELECT use_count FROM usage_history WHERE result_id = ?")
      .all(id) as { use_count: number }[];
    return rows[0]?.use_count ?? 0;
  } finally {
    database.close();
  }
}
