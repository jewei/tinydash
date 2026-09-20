import { execFileSync, spawnSync } from "node:child_process";
import { chmodSync } from "node:fs";
import { resolve } from "node:path";

const hooks = resolve(".githooks");
const current = spawnSync("git", ["config", "--get", "core.hooksPath"], {
  encoding: "utf8",
});
if (current.status !== 0 && current.status !== 1)
  throw new Error(current.stderr || "Cannot read Git hook configuration");
if (current.stdout.trim() && resolve(current.stdout.trim()) !== hooks)
  throw new Error(
    "An existing core.hooksPath is configured. Add the repository checks to that hook before replacing it.",
  );
for (const name of ["pre-commit", "pre-push"])
  chmodSync(resolve(hooks, name), 0o755);
execFileSync("git", ["config", "--local", "core.hooksPath", hooks]);
console.log("Installed repository checks for commits and pushes.");
