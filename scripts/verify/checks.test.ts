import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import {
  mkdtempSync,
  readFileSync,
  readdirSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { resolve } from "node:path";
import { test } from "node:test";
import { needsSourceChecks, requireChecks } from "./checks.ts";

const needs = (source: "true" | "false") => ({
  changes: { result: "success", outputs: { source } },
  repository: { result: "success" },
  ...Object.fromEntries(
    ["quick", "desktop", "native"].map((job) => [
      job,
      { result: source === "true" ? "success" : "skipped" },
    ]),
  ),
});

test("docs-only classification is narrow; unknown/mixed/renamed source paths are heavy", () => {
  for (const paths of [
    [],
    ["README.md"],
    ["docs/image.png", "CONTRIBUTING.markdown"],
  ])
    assert.equal(needsSourceChecks(paths), false);
  for (const path of [
    "src/main.ts",
    "scripts/verify/docs.ts",
    ".github/workflows/check.yml",
    "Cargo.lock",
    "README.MD",
    "AGENTS.md.sh",
  ])
    assert.equal(needsSourceChecks(["docs/guide.md", path]), true);
});
test("only explicitly intentional docs skips pass", () => {
  requireChecks(needs("false"));
  requireChecks(needs("true"));
  for (const source of ["true", "false"] as const) {
    for (const job of ["changes", "repository", "quick", "desktop", "native"]) {
      for (const result of ["failure", "cancelled", "unknown", "", "skipped"]) {
        if (
          source === "false" &&
          ["quick", "desktop", "native"].includes(job) &&
          result === "skipped"
        )
          continue;
        const record: Record<
          string,
          { result: string; outputs?: Record<string, string> }
        > = needs(source);
        record[job] = { ...record[job], result };
        assert.throws(
          () => requireChecks(record),
          `${source}/${job}/${result}`,
        );
      }
      const missing = needs(source) as Record<string, { result: string }>;
      delete missing[job];
      assert.throws(() => requireChecks(missing));
    }
  }
  for (const source of [undefined, "", "yes", "TRUE"]) {
    const record = needs("false");
    Object.assign(record.changes.outputs, { source });
    assert.throws(() => requireChecks(record));
  }
});

test("real git diff counts deleted/renamed source and manual runs; failed diffs emit no skip", () => {
  const root = mkdtempSync(resolve(tmpdir(), "tinydash-checks-"));
  const script = resolve("scripts/verify/checks.ts");
  try {
    const git = (...args: string[]) =>
      execFileSync("git", args, { cwd: root, encoding: "utf8" }).trim();
    git("init", "-q");
    git("config", "user.name", "Synthetic CI test");
    git("config", "user.email", "test@example.invalid");
    writeFileSync(resolve(root, "source.ts"), "source\n");
    git("add", ".");
    git("commit", "-qm", "source");
    const base = git("rev-parse", "HEAD");
    git("mv", "source.ts", "source.md");
    git("commit", "-qm", "rename");
    const head = git("rev-parse", "HEAD");
    const output = resolve(root, "output");
    const run = (env: Record<string, string>) => {
      writeFileSync(output, "");
      execFileSync(process.execPath, [script, "changes"], {
        cwd: root,
        env: {
          ...process.env,
          GITHUB_OUTPUT: output,
          FORCE_SOURCE: "false",
          ...env,
        },
        stdio: "pipe",
      });
      return readFileSync(output, "utf8");
    };
    assert.equal(run({ DIFF_BASE: base, DIFF_HEAD: head }), "source=true\n");
    assert.equal(run({ DIFF_BASE: head, DIFF_HEAD: head }), "source=false\n");
    assert.equal(
      run({ DIFF_BASE: head, DIFF_HEAD: head, FORCE_SOURCE: "true" }),
      "source=true\n",
    );
    assert.equal(
      run({ DIFF_BASE: "0".repeat(40), DIFF_HEAD: head }),
      "source=true\n",
    );
    assert.throws(() => run({ DIFF_BASE: "f".repeat(40), DIFF_HEAD: head }));
    assert.equal(readFileSync(output, "utf8"), "");
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

// Resolved from GitHub's tag-ref API. rust-cache is an annotated tag and uses
// its peeled commit, not the tag-object SHA. See verify.md for update procedure.
const pins: Record<string, [string, string]> = {
  "actions/checkout": ["3d3c42e5aac5ba805825da76410c181273ba90b1", "v7.0.1"],
  "oven-sh/setup-bun": ["0c5077e51419868618aeaa5fe8019c62421857d6", "v2.2.0"],
  "actions/upload-artifact": [
    "043fb46d1a93c77aae656e7c1c64a875d1fc6a0a",
    "v7.0.1",
  ],
  "actions/download-artifact": [
    "3e5f45b2cfb9172054b4087a40e8e0b5a5461e7c",
    "v8.0.1",
  ],
  "Swatinem/rust-cache": ["6323deb102c322ba6fcbdcafc7e3dddab59af2b6", "v2.9.2"],
};
function checkPins(content: string) {
  for (const match of content.matchAll(/^\s*(?:- )?uses: (.+)$/gm)) {
    if (match[1].startsWith("./")) continue;
    const action = /^([^@\s]+)@([a-f0-9]{40}) # (v[\d.]+)$/.exec(match[1]);
    assert(action, `Unpinned action: ${match[1]}`);
    assert.deepEqual(
      pins[action[1]],
      [action[2], action[3]],
      `Unverified action/version: ${match[1]}`,
    );
  }
}
test("every third-party workflow action uses a verified full commit pin", () => {
  for (const file of readdirSync(".github/workflows").filter((file) =>
    /\.ya?ml$/.test(file),
  ))
    checkPins(readFileSync(resolve(".github/workflows", file), "utf8"));
  assert.throws(() => checkPins("      - uses: actions/checkout@v7.0.1"));
  assert.throws(() =>
    checkPins(`      - uses: actions/checkout@${"0".repeat(40)} # v7.0.1`),
  );
});
test("stable final gate always runs, depends on all checks, and workflow has no docs path filter", () => {
  const workflow = readFileSync(".github/workflows/check.yml", "utf8");
  assert(!/paths(?:-ignore)?:/.test(workflow));
  assert.match(
    workflow,
    /name: Required checks\n\s+if: always\(\)\n\s+needs: \[changes, repository, quick, desktop, native\]/,
  );
  assert.match(workflow, /NEEDS_JSON: \$\{\{ toJSON\(needs\) \}\}/);
  assert.match(workflow, /run: bun scripts\/verify\/checks.ts required/);
  assert.match(workflow, /run: bun run check:repo/);
  assert.match(workflow, /run: bun scripts\/verify\/docs.ts/);
});
