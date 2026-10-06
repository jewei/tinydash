// Release TinyDash: `bun run release 0.2.1`, or `bun run release 0.2.1 --draft`
// to try the installers before you publish. It starts the Release workflow on
// main and follows it to the end; the workflow checks, builds, signs, and
// publishes (see docs/development.md).
import { $ } from "bun";

const args = process.argv.slice(2);
const draft = args.includes("--draft");
const version = args.find((arg) => arg !== "--draft") ?? "";
if (!/^\d+\.\d+\.\d+$/.test(version) || args.length > (draft ? 2 : 1)) {
  console.error("Usage: bun run release <version> [--draft], for example: bun run release 0.2.1");
  process.exit(1);
}

// A little early, as GitHub's clock may differ from this one.
const started = new Date(Date.now() - 10_000);
await $`gh workflow run release.yml --ref main -f version=${version} -f draft=${draft}`;

// `gh workflow run` does not say which run it started, so take the first one
// that GitHub lists after it.
let run: string | undefined;
for (let attempt = 0; attempt < 30 && !run; attempt += 1) {
  await Bun.sleep(2_000);
  const runs: { databaseId: number; createdAt: string }[] =
    await $`gh run list --workflow release.yml --event workflow_dispatch --limit 5 --json databaseId,createdAt`.json();
  run = runs
    .filter((entry) => new Date(entry.createdAt) >= started)
    .map((entry) => String(entry.databaseId))
    .at(-1);
}
if (!run) {
  console.error("The Release workflow did not start. See the Actions tab on GitHub.");
  process.exit(1);
}

const watch = await $`gh run watch ${run} --exit-status`.nothrow();
if (watch.exitCode !== 0) {
  console.error(`The release failed. Read the log: gh run view ${run} --log-failed`);
  process.exit(1);
}
const url = (await $`gh release view v${version} --json url -q .url`.text()).trim();
console.log(draft ? `Draft ready: ${url}` : `Released: ${url}`);
