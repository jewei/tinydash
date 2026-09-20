import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { privatePath } from "./policy";

const failures = new Set<string>();
for (const line of readFileSync(0, "utf8").trim().split("\n")) {
  if (!line) continue;
  const [ref, sha] = line.split(/\s+/);
  if (/^0+$/.test(sha)) continue; // A deleted ref introduces no history.
  const paths = execFileSync(
    "git",
    [
      "log",
      "--diff-merges=separate",
      sha,
      "--format=",
      "--name-only",
      "--no-renames",
      "-z",
    ],
    { encoding: "utf8", maxBuffer: 32 * 1024 * 1024 },
  );
  for (const path of paths.split("\0")) {
    if (privatePath(path)) failures.add(`${ref}: ${path}`);
  }
}
if (failures.size) {
  console.error(
    "Push rejected. These private paths remain in the proposed history:",
  );
  console.error([...failures].join("\n"));
  process.exitCode = 1;
}
