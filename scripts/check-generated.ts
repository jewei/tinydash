// Fail when `cargo test` changed or added files in src/generated, which means
// the Rust IPC types changed without their TypeScript bindings.
const status = Bun.spawnSync(["git", "status", "--porcelain", "--", "src/generated"]);
const changes = status.stdout.toString().trim();
if (status.exitCode !== 0 || changes) {
  console.error(`src/generated is out of date. Commit these files:\n${changes}`);
  process.exit(1);
}
