import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { appendFileSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

// Unknown paths are source changes. Deletions and both sides of renames count.
export function needsSourceChecks(paths: string[]): boolean {
  return paths.some(
    (path) => !path.startsWith("docs/") && !/\.(md|markdown)$/.test(path),
  );
}

type Need = { result: string; outputs?: Record<string, string> };
export function requireChecks(needs: Record<string, Need>) {
  assert.equal(
    needs.changes?.result,
    "success",
    "Change detection did not pass",
  );
  assert.equal(
    needs.repository?.result,
    "success",
    "Repository checks did not pass",
  );
  const source = needs.changes.outputs?.source;
  assert(
    source === "true" || source === "false",
    "Missing source classification",
  );
  for (const job of ["quick", "desktop", "native"]) {
    assert.equal(
      needs[job]?.result,
      source === "true" ? "success" : "skipped",
      `${job}: unexpected result (failure, cancellation, and unknown skips fail closed)`,
    );
  }
}

if (resolve(process.argv[1] ?? "") === fileURLToPath(import.meta.url)) {
  if (process.argv[2] === "required") {
    requireChecks(JSON.parse(process.env.NEEDS_JSON ?? "{}"));
    console.log("Required checks passed");
  } else if (process.argv[2] === "changes") {
    const base = process.env.DIFF_BASE;
    const head = process.env.DIFF_HEAD;
    // Manual/tag/new-branch runs deliberately exercise the complete pipeline.
    let source = true;
    if (
      base &&
      head &&
      !/^0+$/.test(base) &&
      process.env.FORCE_SOURCE !== "true"
    ) {
      for (const ref of [base, head])
        assert(/^[a-f0-9]{40}$/.test(ref), "Invalid diff SHA");
      const paths = execFileSync(
        "git",
        ["diff", "--no-renames", "--name-only", "-z", base, head],
        { encoding: "utf8" },
      )
        .split("\0")
        .filter(Boolean);
      source = needsSourceChecks(paths);
    }
    assert(process.env.GITHUB_OUTPUT, "Missing Actions output file");
    appendFileSync(process.env.GITHUB_OUTPUT, `source=${source}\n`);
  } else {
    throw new Error("Usage: bun scripts/verify/checks.ts changes|required");
  }
}
