import { spawn } from "node:child_process";
import { mkdir, open, rm, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import {
  assertSameSource,
  binaryIdentity,
  fileHash,
  sourceIdentity,
  type BuildRecord,
} from "./identity.ts";
import { cancellation, stopProcessTree } from "./lifecycle.ts";

if (!process.versions.bun) throw new Error("Run this build with Bun.");

// This entry point builds before it writes the record. An old executable can
// never acquire a new source revision merely by running a metadata command.
const args = process.argv.slice(2);
const configurations: string[] = [];
for (let i = 0; i < args.length; i++) {
  if (args[i] === "--no-bundle") continue;
  if (args[i] === "--config" && args[i + 1]) {
    const config = args[++i];
    // Tauri accepts a file path or inline JSON. Inline settings remain in the
    // recorded command; file settings also need a hash to detect later edits.
    if (config.trimStart().startsWith("{")) JSON.parse(config);
    else configurations.push(resolve(config));
  } else
    throw new Error(
      "Usage: bun scripts/verify/build.ts [--no-bundle] [--config <file-or-json>]",
    );
}
if (process.env.CARGO_TARGET_DIR)
  throw new Error(
    "Verification builds use the default release target directory.",
  );
const recordPath = resolve("src-tauri/target/verification-build.json");
const lockPath = resolve("src-tauri/target/verification-build.lock");
const binaryPath = resolve(
  `src-tauri/target/release/tinydash${process.platform === "win32" ? ".exe" : ""}`,
);
let lock: Awaited<ReturnType<typeof open>> | undefined;
const cancelled = cancellation();
let child: ReturnType<typeof spawn> | undefined;
let stopping: Promise<void> | undefined;
try {
  await mkdir(resolve(recordPath, ".."), { recursive: true });
  lock = await open(lockPath, "wx");
  await lock.writeFile(String(process.pid));
  await rm(recordPath, { force: true });
  const source = await sourceIdentity();
  const configuration = await Promise.all(
    configurations.map(async (path) => ({
      path,
      sha256: await fileHash(path),
    })),
  );
  // If a custom Cargo target or runner writes elsewhere, fail instead of
  // associating a previous executable with the new source.
  await rm(binaryPath, { force: true });
  const command = ["bun", "run", "tauri", "build", ...args];
  child = spawn(process.execPath, command.slice(1), {
    stdio: "inherit",
    detached: process.platform !== "win32",
  });
  const stop = () => {
    stopping ??= stopProcessTree(child!);
    void stopping.catch(() => {});
  };
  cancelled.signal.addEventListener("abort", stop, { once: true });
  if (cancelled.signal.aborted) stop();
  const code = await new Promise<number | null>((done, reject) => {
    child!.once("error", reject);
    child!.once("close", done);
  });
  if (stopping) await stopping;
  cancelled.signal.throwIfAborted();
  if (code !== 0) throw new Error(`Tauri build exited with code ${code}`);
  assertSameSource(source, await sourceIdentity());
  for (const config of configuration) {
    if (config.sha256 !== (await fileHash(config.path)))
      throw new Error("Tauri configuration changed during the build.");
  }
  const record: BuildRecord = {
    schema: 1,
    source,
    platform: process.platform,
    arch: process.arch,
    builtAt: new Date().toISOString(),
    command,
    configuration,
    binary: await binaryIdentity(binaryPath),
    kind: "local",
  };
  await writeFile(recordPath, JSON.stringify(record, null, 2) + "\n");
  console.log(`Build record: ${recordPath}`);
} finally {
  try {
    if (stopping) await stopping;
    if (lock) {
      await lock.close();
      await rm(lockPath);
    }
  } finally {
    cancelled.dispose();
  }
}
