import { execFileSync, spawn } from "node:child_process";
import { createWriteStream, existsSync } from "node:fs";
import { mkdir, open, readFile, rm, writeFile } from "node:fs/promises";
import { createServer } from "node:net";
import { resolve } from "node:path";
import { nativeTestBinary } from "./native.ts";
import { cancellation } from "./lifecycle.ts";

if (!process.versions.bun) throw new Error("Run this check with Bun.");

const mode = process.argv[2] ?? "quick";
if (!["quick", "full", "native"].includes(mode) || process.argv.length > 3)
  throw new Error("Usage: bun scripts/verify/run.ts [quick|full|native]");
const cancelled = cancellation();
const output = resolve(
  "test-results/verification",
  `${new Date().toISOString().replace(/[:.]/g, "-")}-${process.pid}-${mode}`,
);
await mkdir(output, { recursive: true });
const steps: { command: string[]; status: number; seconds: number }[] = [];
const env: NodeJS.ProcessEnv = {
  ...process.env,
  TINYDASH_TEST_OUTPUT: resolve(output, "browser"),
  TINYDASH_NATIVE_OUTPUT: resolve(output, "native"),
};
let failure: string | undefined;
let lock: Awaited<ReturnType<typeof open>> | undefined;
const native: {
  cleanup: "not-started" | "unknown" | "complete" | "incomplete";
} = {
  cleanup: "not-started",
};
const lockPath = resolve("test-results/verification/native.lock");
const commit = execFileSync("git", ["rev-parse", "HEAD"], {
  encoding: "utf8",
}).trim();
const dirty =
  execFileSync("git", ["status", "--porcelain"], { encoding: "utf8" }).trim()
    .length > 0;

async function run(command: string[]) {
  cancelled.signal.throwIfAborted();
  const start = performance.now();
  const log = createWriteStream(
    resolve(output, `${String(steps.length + 1).padStart(2, "0")}.log`),
  );
  console.log(`\n> ${command.join(" ")}`);
  const child = spawn(
    command[0] === "bun" ? process.execPath : command[0],
    command.slice(1),
    {
      env,
      stdio:
        mode === "native"
          ? ["ignore", "pipe", "pipe", "ipc"]
          : ["ignore", "pipe", "pipe"],
      // Keep terminal signals out of the suite. It receives cancellation over IPC.
      detached: mode === "native",
      windowsHide: true,
    },
  );
  if (mode === "native") native.cleanup = "unknown";
  let shutdownTimer: ReturnType<typeof setTimeout> | undefined;
  const forward = () => {
    if (mode === "native") {
      // A signal sent through child.kill() can terminate Windows children
      // without running their handlers. Ask the suite to cancel over IPC.
      if (child.connected)
        child.send({ type: "cancel" }, (error: Error | null) => {
          if (error) console.error("Cannot send cancellation:", error);
        });
    } else child.kill("SIGINT");
    shutdownTimer ??= setTimeout(() => {
      console.error("Verification shutdown timed out. Cleanup is unconfirmed.");
      child.kill("SIGKILL");
    }, 90_000);
  };
  cancelled.signal.addEventListener("abort", forward, { once: true });
  child.once("spawn", () => {
    if (cancelled.signal.aborted) forward();
  });
  child.stdout!.on("data", (chunk) => {
    process.stdout.write(chunk);
    log.write(chunk);
  });
  child.stderr!.on("data", (chunk) => {
    process.stderr.write(chunk);
    log.write(chunk);
  });
  let spawnError: Error | undefined;
  child.once("error", (error) => {
    spawnError = error;
    if (mode === "native" && !child.pid) native.cleanup = "not-started";
    log.write(String(error));
  });
  const status = await new Promise<number>((done) =>
    child.once("close", (code) => done(code ?? 1)),
  );
  cancelled.signal.removeEventListener("abort", forward);
  clearTimeout(shutdownTimer);
  await new Promise<void>((done) => log.end(done));
  steps.push({
    command,
    status,
    seconds: Math.round((performance.now() - start) / 10) / 100,
  });
  if (spawnError) throw spawnError;
  cancelled.signal.throwIfAborted();
  if (status !== 0)
    throw new Error(`${command.join(" ")} failed with exit code ${status}`);
}

async function freePort(): Promise<string> {
  const server = createServer();
  return new Promise((done, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", () => {
      const address = server.address();
      if (!address || typeof address === "string")
        return reject(new Error("No test port"));
      server.close((error) =>
        error ? reject(error) : done(String(address.port)),
      );
    });
  });
}

try {
  cancelled.signal.throwIfAborted();
  if (mode === "native") {
    const binary = nativeTestBinary();
    lock = await open(lockPath, "wx");
    await lock.writeFile(String(process.pid));
    if (
      process.platform === "win32" &&
      process.env.RUNNER_ENVIRONMENT !== "github-hosted" &&
      process.env.TINYDASH_NATIVE_TEST_PROFILE !== "1"
    )
      throw new Error(
        "Use a separate Windows test user, then set TINYDASH_NATIVE_TEST_PROFILE=1. This suite clears clipboard history.",
      );
    if (!existsSync(binary))
      throw new Error(
        `Build or install the native application first: ${binary}`,
      );
    const probe =
      process.platform === "win32"
        ? [
            "powershell.exe",
            "-NoProfile",
            "-Command",
            "Get-Process -Name tinydash -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Id; exit 0",
          ]
        : ["sh", "-c", 'pgrep -x tinydash; result=$?; [ "$result" -le 1 ]'];
    const active = execFileSync(probe[0], probe.slice(1), {
      encoding: "utf8",
    }).trim();
    if (active)
      throw new Error(
        "Quit the existing TinyDash process before running native checks.",
      );
    console.log(
      `Doctor passed. Binary: ${binary}. No existing TinyDash process was found.`,
    );
    await run(["bun", "tests/native/smoke.ts"]);
  } else {
    env.TINYDASH_TEST_PORT = await freePort();
    await run(["bun", "scripts/verify/repository.ts"]);
    await run(["bun", "run", "format:check"]);
    await run([
      "cargo",
      "fmt",
      "--manifest-path",
      "src-tauri/Cargo.toml",
      "--check",
    ]);
    await run([
      "rustfmt",
      "--check",
      "--edition",
      "2024",
      "tests/native/fixture.rs",
    ]);
    if (mode === "quick") {
      env.TINYDASH_VERIFY_TRACE = "1";
      await run(["bun", "run", "typecheck"]);
      await run([
        "bun",
        "x",
        "--bun",
        "--no-install",
        "playwright",
        "test",
        "--grep",
        "@smoke",
      ]);
    } else {
      await run(["bun", "run", "build"]);
      await run([
        "cargo",
        "clippy",
        "--manifest-path",
        "src-tauri/Cargo.toml",
        "--all-targets",
        "--locked",
        "--",
        "-D",
        "warnings",
      ]);
      await run([
        "cargo",
        "test",
        "--manifest-path",
        "src-tauri/Cargo.toml",
        "--locked",
      ]);
      await run(["bun", "run", "test:ui"]);
    }
  }
} catch (error) {
  failure = String(error);
  console.error(failure);
  process.exitCode = 1;
} finally {
  if (native.cleanup === "unknown") {
    try {
      const result = JSON.parse(
        await readFile(
          resolve(env.TINYDASH_NATIVE_OUTPUT!, "cleanup.json"),
          "utf8",
        ),
      );
      native.cleanup = result.complete === true ? "complete" : "incomplete";
    } catch {
      // A missing or damaged result cannot establish that cleanup finished.
    }
    if (native.cleanup !== "complete") {
      failure ??= "Native cleanup is incomplete or unconfirmed.";
      console.error(`${failure} Retaining lock: ${lockPath}`);
      process.exitCode = 1;
    }
  }
  if (lock) {
    await lock.close();
    if (native.cleanup === "not-started" || native.cleanup === "complete")
      await rm(lockPath);
  }
  if (cancelled.signal.aborted) {
    failure ??= String(cancelled.signal.reason);
    process.exitCode = 1;
  }
  await writeFile(
    resolve(output, "result.json"),
    JSON.stringify(
      {
        mode,
        commit,
        dirty,
        platform: process.platform,
        passed: !failure,
        failure,
        steps,
        ...(mode === "native" ? { nativeCleanup: native.cleanup } : {}),
      },
      null,
      2,
    ) + "\n",
  );
  console.log(`Evidence: ${output}`);
  cancelled.dispose();
}
