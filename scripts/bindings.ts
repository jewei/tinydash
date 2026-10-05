// Check that src/generated matches the Rust IPC types. Run with
// `bun scripts/bindings.ts`; `bun run verify` and CI run it too.
//
// Runs the Rust tests with ts-rs writing to a temporary folder, so a failed
// build never leaves src/generated half written. If the new bindings differ,
// they replace src/generated (including removed types) and the script fails,
// so the change is reviewed and committed.
import { createHash } from "node:crypto";
import { cp, mkdtemp, readdir, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const folder = resolve("src/generated");

async function fingerprint(path: string) {
  const hash = createHash("sha256");
  const names = (await readdir(path).catch(() => [])).sort((a, b) => a.localeCompare(b));
  for (const name of names) hash.update(name).update(await readFile(join(path, name)));
  return hash.digest("hex");
}

const fresh = await mkdtemp(join(tmpdir(), "tinydash-bindings-"));
const test = Bun.spawnSync(
  ["cargo", "test", "--manifest-path", "src-tauri/Cargo.toml", "--locked"],
  {
    env: { ...process.env, TS_RS_EXPORT_DIR: fresh },
    stdout: "inherit",
    stderr: "inherit",
  },
);
if (test.exitCode !== 0) {
  await rm(fresh, { recursive: true, force: true });
  process.exit(test.exitCode ?? 1);
}

const changed = (await fingerprint(fresh)) !== (await fingerprint(folder));
if (changed) {
  await rm(folder, { recursive: true, force: true });
  await cp(fresh, folder, { recursive: true });
}
await rm(fresh, { recursive: true, force: true });
if (changed) {
  console.error(`\n${folder} was out of date and is now updated. Review and commit it.`);
  process.exit(1);
}
