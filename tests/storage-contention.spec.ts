import { expect, test } from "@playwright/test";
import { execFileSync } from "node:child_process";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

test("native storage fixture releases its external transaction after failure", async () => {
  const directory = await mkdtemp(join(tmpdir(), "tinydash-contention-"));
  try {
    const output = execFileSync(
      "bun",
      [
        "-e",
        `import assert from 'node:assert/strict';
        import { Database } from 'bun:sqlite';
        import { withStorageLock, storedClipboard, storedUsage } from ${JSON.stringify(resolve("tests/native/storage.ts"))};
        const data = ${JSON.stringify(directory)};
        await assert.rejects(withStorageLock(data, async () => {}), /must create/);
        const db = new Database(data + '/tinydash.sqlite3');
        try {
          db.run('CREATE TABLE clipboard_history (id INTEGER, content TEXT)');
          db.run('CREATE TABLE usage_history (result_id TEXT, use_count INTEGER)');
          db.run("INSERT INTO clipboard_history VALUES (1, 'synthetic')");
          db.run("INSERT INTO usage_history VALUES ('clipboard:1', 2)");
          db.run('PRAGMA busy_timeout = 1');
          await assert.rejects(withStorageLock(data, async () => {
            assert.throws(() => db.run('DELETE FROM clipboard_history'), /locked/);
            assert.equal(storedClipboard(data, 'synthetic').length, 1);
            assert.equal(storedUsage(data, 'clipboard:1'), 2);
            throw new Error('synthetic cancellation');
          }), /synthetic cancellation/);
          db.run('DELETE FROM clipboard_history');
          assert.deepEqual(storedClipboard(data, 'synthetic'), []);
          assert.equal(storedUsage(data, 'missing'), 0);
          await withStorageLock(data, async () => {});
          console.log('released');
        } finally { db.close(); }`,
      ],
      { encoding: "utf8", timeout: 15_000 },
    );
    expect(output.trim()).toBe("released");
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});
