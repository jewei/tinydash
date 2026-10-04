// Regenerate src/generated from the Rust IPC types and report what changed.
//
//   bun scripts/bindings.ts          fail if regenerating changed anything
//   bun scripts/bindings.ts --ci     also fail if src/generated differs from git
//
// The folder is emptied first, so a binding for a deleted Rust type cannot
// linger. `cargo test` writes the bindings (see .cargo/config.toml).
import { createHash } from "node:crypto";
import { readdir, readFile, rm } from "node:fs/promises";

const folder = "src/generated";

async function fingerprint() {
  const hash = createHash("sha256");
  const names = (await readdir(folder).catch(() => [])).sort();
  for (const name of names) hash.update(name).update(await readFile(`${folder}/${name}`));
  return hash.digest("hex");
}

function run(command: string[]) {
  const result = Bun.spawnSync(command, { stdout: "inherit", stderr: "inherit" });
  if (result.exitCode !== 0) process.exit(result.exitCode ?? 1);
}

const before = await fingerprint();
await rm(folder, { recursive: true, force: true });
run(["cargo", "test", "--manifest-path", "src-tauri/Cargo.toml", "--locked"]);

if ((await fingerprint()) !== before) {
  console.error(`\n${folder} changed. Review and commit the new bindings, then run this again.`);
  process.exit(1);
}
if (process.argv.includes("--ci")) {
  const status = Bun.spawnSync(["git", "status", "--porcelain", "--", folder]);
  const changes = status.stdout.toString().trim();
  if (changes) {
    console.error(`\n${folder} is not committed:\n${changes}`);
    process.exit(1);
  }
}
