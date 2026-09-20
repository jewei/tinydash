import { execFileSync, spawn } from "node:child_process";
import { createWriteStream, existsSync } from "node:fs";
import { mkdir, open, rm, writeFile } from "node:fs/promises";
import { createServer } from "node:net";
import { resolve } from "node:path";

const mode = process.argv[2] ?? "quick";
if (!["quick", "full", "native"].includes(mode) || process.argv.length > 3)
  throw new Error("Usage: bun scripts/verify/run.ts [quick|full|native]");
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
const lockPath = resolve("test-results/verification/native.lock");
const commit = execFileSync("git", ["rev-parse", "HEAD"], {
  encoding: "utf8",
}).trim();
const dirty =
  execFileSync("git", ["status", "--porcelain"], { encoding: "utf8" }).trim()
    .length > 0;

async function run(command: string[]) {
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
      stdio: ["ignore", "pipe", "pipe"],
    },
  );
  const forward = () => child.kill("SIGINT");
  process.once("SIGINT", forward);
  child.stdout.on("data", (chunk) => {
    process.stdout.write(chunk);
    log.write(chunk);
  });
  child.stderr.on("data", (chunk) => {
    process.stderr.write(chunk);
    log.write(chunk);
  });
  let spawnError: Error | undefined;
  child.once("error", (error) => {
    spawnError = error;
    log.write(String(error));
  });
  const status = await new Promise<number>((done) =>
    child.once("close", (code) => done(code ?? 1)),
  );
  process.removeListener("SIGINT", forward);
  await new Promise<void>((done) => log.end(done));
  steps.push({
    command,
    status,
    seconds: Math.round((performance.now() - start) / 10) / 100,
  });
  if (spawnError) throw spawnError;
  if (status !== 0)
    throw new Error(`${command.join(" ")} failed with exit code ${status}`);
}

async function testPort(): Promise<string> {
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
  if (mode === "native") {
    if (process.platform !== "win32" && process.platform !== "linux")
      throw new Error(
        "Native WebDriver checks require Windows or Linux. Use docs/how-to/desktop-checks.md on macOS.",
      );
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
    const binary = resolve(
      process.env.TINYDASH_NATIVE_BINARY ??
        `src-tauri/target/release/tinydash${process.platform === "win32" ? ".exe" : ""}`,
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
    env.TINYDASH_TEST_PORT = await testPort();
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
  if (lock) {
    await lock.close();
    await rm(lockPath);
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
        nativeEvidence: mode === "native" ? "native" : undefined,
      },
      null,
      2,
    ) + "\n",
  );
  console.log(`Evidence: ${output}`);
}
