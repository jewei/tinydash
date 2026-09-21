// Run the real wrapper and suite without touching a desktop or user settings.
import { mock } from "bun:test";
import * as childProcess from "node:child_process";
import { createServer } from "node:http";
import * as files from "node:fs/promises";
import { resolve } from "node:path";
import { setTimeout as delay } from "node:timers/promises";

const source = process.env.TINYDASH_CANCELLATION_SOURCE;
const directory = process.cwd();
const phase = process.env.TINYDASH_CANCELLATION_PHASE;
const helper = resolve(source, "tests/fixtures/native-cancellation.mjs");
const { execFileSync, spawn } = childProcess;
const { mkdir, readFile, writeFile } = files;

if (process.argv[2] === "worker" || process.argv[2] === "compiler") {
  await writeFile(`${process.argv[2]}.pid`, String(process.pid));
  if (process.argv[2] === "compiler") await writeFile("ready", "compiler");
  setInterval(() => {}, 1_000);
} else if (process.argv[2] === "driver") {
  const worker = spawn(process.execPath, [helper, "worker"], {
    stdio: "ignore",
  });
  const workerClosed = new Promise((done) => worker.once("close", done));
  process.on("SIGTERM", async () => {
    worker.kill("SIGTERM");
    await workerClosed;
    if (phase !== "stubborn") {
      server.closeAllConnections();
      server.close();
    }
  });
  await writeFile("driver.pid", String(process.pid));
  const server = createServer(async (request, response) => {
    if (phase === "session") {
      if (request.url === "/status") {
        response.end(JSON.stringify({ value: { ready: true } }));
        return;
      }
      if (request.url === "/session") {
        response.end(JSON.stringify({ value: { sessionId: "dummy" } }));
        return;
      }
      if (request.url.endsWith("/timeouts")) {
        await writeFile("ready", "session request");
        return;
      }
      if (request.method === "DELETE")
        await writeFile("session-deleted", "yes");
    }
    if (request.url === "/status") {
      await writeFile("ready", "request");
      // Leave a request pending. Cancellation must abort it and the retry loop.
      return;
    }
    response.end(JSON.stringify({ value: null }));
  }).listen(
    Number(process.argv[process.argv.indexOf("--port") + 1]),
    "127.0.0.1",
  );
} else {
  mock.module(resolve(source, "scripts/verify/native.ts"), () => ({
    nativeTestBinary: () => process.execPath,
  }));
  mock.module("node:child_process", () => ({
    ...childProcess,
    execFileSync(command, args, options) {
      if (
        (command === "sh" && args[1]?.startsWith("pgrep -x tinydash")) ||
        (command === "powershell.exe" && args.includes("-Command"))
      )
        return "";
      return execFileSync(command, args, options);
    },
    spawn(command, args, options) {
      if (command === "tauri-driver")
        return spawn(process.execPath, [helper, "driver", ...args], options);
      if (command === "rustc")
        return spawn(process.execPath, [helper, "compiler"], options);
      return spawn(command, args, options);
    },
  }));
  if (process.argv[2] === "wrapper") {
    // Lifecycle cases deliberately change files in their temporary checkout.
    // Identity and stale-build rejection have their own regression tests.
    const identityPath = resolve(source, "scripts/verify/identity.ts");
    const identity = await import(identityPath);
    const fixtureSource = await identity.sourceIdentity();
    const binary = { path: process.execPath, sha256: "0".repeat(64), bytes: 1 };
    mock.module(identityPath, () => ({
      ...identity,
      sourceIdentity: async () => fixtureSource,
      readBuildRecord: async () => ({
        schema: 1,
        source: fixtureSource,
        binary,
        kind: "local",
      }),
      verifyBuild: async () => binary,
    }));
    process.env.TINYDASH_NATIVE_MANIFEST = "synthetic-lifecycle-build.json";
    process.argv = [process.execPath, "scripts/verify/run.ts", "native"];
    await import(resolve(source, "scripts/verify/run.ts"));
  } else {
    if (phase === "lost-result")
      mock.module("node:fs/promises", () => ({
        ...files,
        writeFile(path, ...args) {
          if (String(path).endsWith("cleanup.json"))
            return Promise.reject(
              new Error("Synthetic cleanup record failure"),
            );
          return writeFile(path, ...args);
        },
      }));
    if (phase !== "compiler") {
      mock.module(resolve(source, "tests/native/fixtures.ts"), () => ({
        async installFixtures(signal, registerCleanup = () => {}) {
          const settingsPath = resolve(
            directory,
            "config/dev.tinydash.launcher/settings.json",
          );
          await mkdir(resolve(settingsPath, ".."), { recursive: true });
          const original = await readFile("settings.json");
          const cleanup = async () => {
            await writeFile("cleanup-started", "yes");
            if (phase === "cleanup") await delay(500);
            if (phase === "cleanup-failure")
              throw new Error("Synthetic cleanup failure");
            await writeFile("settings.json", original);
            await writeFile("cleanup-finished", "yes");
          };
          registerCleanup(cleanup, directory);
          await writeFile("settings.json", "temporary settings");
          await writeFile(settingsPath, "{}");
          if (phase === "setup") {
            await writeFile("ready", "setup");
            await delay(60_000, undefined, { signal });
          }
          if (phase === "setup-failure")
            throw new Error("Synthetic setup failure");
          return {
            directory,
            filePath: resolve(directory, "document.txt"),
            fileMarker: resolve(directory, "opened.txt"),
            fileRoot: directory,
            env: {
              ...process.env,
              XDG_CONFIG_HOME: resolve(directory, "config"),
            },
            cleanup,
          };
        },
      }));
    }
    await import(resolve(source, "tests/native/smoke.ts"));
  }
}
