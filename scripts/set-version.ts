// Write a release version into src-tauri/Cargo.toml and Cargo.lock, which
// keep 0.0.0 between releases. The Release workflow runs it before it builds:
// `bun scripts/set-version.ts 0.2.1`. Tauri reads the app version from Cargo.
import { readFile, writeFile } from "node:fs/promises";

const version = process.argv[2] ?? "";
if (!/^\d+\.\d+\.\d+$/.test(version)) {
  console.error(`"${version}" is not a version such as 0.2.1.`);
  process.exit(1);
}

async function replace(path: string, pattern: RegExp, value: string) {
  const text = await readFile(path, "utf8");
  if (!pattern.test(text)) {
    console.error(`${path} has no 0.0.0 version for TinyDash to replace.`);
    process.exit(1);
  }
  await writeFile(path, text.replace(pattern, value));
}

// A Windows checkout may have CRLF line endings.
await replace("src-tauri/Cargo.toml", /^version = "0\.0\.0"/m, `version = "${version}"`);
await replace(
  "src-tauri/Cargo.lock",
  /(name = "tinydash"\r?\nversion = )"0\.0\.0"/,
  `$1"${version}"`,
);
console.log(`TinyDash version: ${version}`);
