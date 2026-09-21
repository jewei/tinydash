import { execFileSync, spawn, spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import {
  mkdtemp,
  mkdir,
  readFile,
  readdir,
  rm,
  writeFile,
} from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { expect, test } from "@playwright/test";

const source = resolve(".");
const helper = resolve("tests/fixtures/native-cancellation.mjs");

for (const [phase, interrupt] of [
  ["setup", "ipc"],
  ["compiler", "ipc"],
  ["request", "SIGINT"],
  ["request", "SIGTERM"],
  ["request", "ipc"],
  ["session", "ipc"],
  ["stubborn", "ipc"],
  ["cleanup", "ipc"],
  ["cleanup-failure", "ipc"],
  ["lost-result", "ipc"],
  ["setup-failure", "none"],
] as const) {
  test(`native cleanup during ${phase} with ${interrupt}`, async () => {
    test.skip(
      process.platform === "win32" && interrupt.startsWith("SIG"),
      "Windows cancellation uses IPC because child.kill terminates processes abruptly.",
    );
    test.skip(
      phase === "session" && process.platform === "win32",
      "Windows stops process trees directly; session creation also needs a real WebView2 app.",
    );
    const directory = await mkdtemp(join(tmpdir(), "tinydash-cancel-"));
    let child: ReturnType<typeof spawn> | undefined;
    let log = "";
    try {
      execFileSync("git", ["init", "--quiet"], { cwd: directory });
      execFileSync(
        "git",
        [
          "-c",
          "user.name=Test",
          "-c",
          "user.email=test@example.invalid",
          "-c",
          "commit.gpgsign=false",
          "-c",
          `core.hooksPath=${join(directory, "no-hooks")}`,
          "commit",
          "--allow-empty",
          "-m",
          "Test",
        ],
        { cwd: directory },
      );
      await mkdir(join(directory, "tests/native"), { recursive: true });
      await writeFile(
        join(directory, "tests/native/smoke.ts"),
        `await import(${JSON.stringify(helper)});\n`,
      );
      await writeFile(join(directory, "settings.json"), "original settings");
      await mkdir(join(directory, "temporary"));
      child = spawn("bun", [helper, "wrapper"], {
        cwd: directory,
        env: {
          ...process.env,
          TINYDASH_CANCELLATION_SOURCE: source,
          TINYDASH_CANCELLATION_PHASE: phase,
          TINYDASH_NATIVE_TEST_PROFILE: "1",
          APPDATA: join(directory, "config"),
          TMPDIR: join(directory, "temporary"),
          TEMP: join(directory, "temporary"),
          TMP: join(directory, "temporary"),
        },
        stdio: ["ignore", "pipe", "pipe", "ipc"],
      });
      child.stdout!.on("data", (chunk) => (log += chunk));
      child.stderr!.on("data", (chunk) => (log += chunk));
      const closed = new Promise<number | null>((done) =>
        child!.once("close", done),
      );
      if (interrupt !== "none") {
        await expect
          .poll(() => existsSync(join(directory, "ready")), {
            message: "Dummy setup or driver becomes ready",
            timeout: 10_000,
          })
          .toBe(true);
        expect(
          existsSync(join(directory, "test-results/verification/native.lock")),
        ).toBe(true);
        if (interrupt === "ipc") child.send({ type: "cancel" });
        else child.kill(interrupt);
        if (phase === "cleanup") {
          await expect
            .poll(() => existsSync(join(directory, "cleanup-started")))
            .toBe(true);
          expect(
            existsSync(
              join(directory, "test-results/verification/native.lock"),
            ),
          ).toBe(true);
          child.send({ type: "cancel" });
        }
      }
      await expect
        .poll(() => child!.exitCode ?? child!.signalCode, {
          timeout: 15_000,
          message: log,
        })
        .not.toBeNull();
      expect(await closed).not.toBe(0);
      const complete = phase !== "cleanup-failure" && phase !== "lost-result";
      expect(
        await readFile(join(directory, "settings.json"), "utf8"),
        log,
      ).toBe(
        phase !== "cleanup-failure"
          ? "original settings"
          : "temporary settings",
      );
      expect(
        existsSync(join(directory, "test-results/verification/native.lock")),
        log,
      ).toBe(!complete);
      for (const name of ["driver", "worker", "compiler"]) {
        if (existsSync(join(directory, `${name}.pid`))) {
          const pid = Number(
            await readFile(join(directory, `${name}.pid`), "utf8"),
          );
          expect(() => process.kill(pid, 0), log).toThrow();
        }
      }
      expect(await readdir(join(directory, "temporary"))).toEqual([]);
      const results = await readdir(
        join(directory, "test-results/verification"),
      );
      const run = results.find((name) => name.endsWith("-native"))!;
      const result = JSON.parse(
        await readFile(
          join(directory, "test-results/verification", run, "result.json"),
          "utf8",
        ),
      );
      expect(result.passed).toBe(false);
      if (phase === "session")
        expect(existsSync(join(directory, "session-deleted"))).toBe(true);
      expect(result.nativeCleanup).toBe(
        complete
          ? "complete"
          : phase === "lost-result"
            ? "unknown"
            : "incomplete",
      );
    } finally {
      if (child?.connected) child.disconnect();
      child?.kill("SIGKILL");
      for (const name of ["driver", "worker", "compiler"]) {
        if (existsSync(join(directory, `${name}.pid`))) {
          const pid = Number(
            await readFile(join(directory, `${name}.pid`), "utf8"),
          );
          try {
            if (process.platform === "win32")
              spawnSync("taskkill.exe", ["/PID", String(pid), "/T", "/F"], {
                stdio: "ignore",
                timeout: 5_000,
              });
            else process.kill(name === "worker" ? pid : -pid, "SIGKILL");
          } catch {}
        }
      }
      await rm(directory, { recursive: true, force: true });
    }
  });
}
