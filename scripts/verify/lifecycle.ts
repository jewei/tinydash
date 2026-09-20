import { spawnSync, type ChildProcess } from "node:child_process";
import { setTimeout as delay } from "node:timers/promises";

// Keep handlers installed through cleanup. A second signal must not bypass it.
export function cancellation() {
  const controller = new AbortController();
  const cancel = (reason: string) => controller.abort(new Error(reason));
  const interrupt = () => cancel("Verification interrupted by SIGINT");
  const terminate = () => cancel("Verification interrupted by SIGTERM");
  const disconnect = () => cancel("Verification parent disconnected");
  const message = (value: unknown) => {
    if (
      typeof value === "object" &&
      value !== null &&
      "type" in value &&
      value.type === "cancel"
    )
      cancel("Verification cancelled by parent");
  };
  process.on("SIGINT", interrupt);
  process.on("SIGTERM", terminate);
  process.on("message", message);
  process.on("disconnect", disconnect);
  return {
    signal: controller.signal,
    dispose() {
      process.removeListener("SIGINT", interrupt);
      process.removeListener("SIGTERM", terminate);
      process.removeListener("message", message);
      process.removeListener("disconnect", disconnect);
      if (process.connected) process.disconnect?.();
    },
  };
}

export async function cleanupAll(actions: (() => void | Promise<void>)[]) {
  const errors: unknown[] = [];
  for (const action of actions) {
    try {
      await action();
    } catch (error) {
      errors.push(error);
    }
  }
  if (errors.length) throw new AggregateError(errors, "Cleanup incomplete");
}

export function errorDetails(error: unknown): string {
  if (error instanceof AggregateError)
    return `${error.message}\n${error.errors.map(errorDetails).join("\n")}`;
  return String(error instanceof Error ? error.stack : error);
}

export function waitForExit(
  child: ChildProcess,
  signal: AbortSignal,
  timeout: number,
) {
  return new Promise<void>((done, reject) => {
    const finish = (error?: Error) => {
      clearTimeout(timer);
      signal.removeEventListener("abort", abort);
      if (error) reject(error);
      else done();
    };
    const abort = () => finish(signal.reason);
    const exited = (code: number | null) =>
      finish(
        code === 0
          ? undefined
          : new Error(`${child.spawnfile} exited with code ${code}`),
      );
    const timer = setTimeout(
      () => finish(new Error(`${child.spawnfile} did not exit`)),
      timeout,
    );
    child.once("error", finish);
    child.once("exit", exited);
    signal.addEventListener("abort", abort, { once: true });
    if (signal.aborted) abort();
    else if (child.exitCode !== null || child.signalCode !== null)
      exited(child.exitCode);
  });
}

// The driver owns a separate POSIX process group. Windows taskkill owns the
// tree rooted at the still-running process. Never kill by application name.
export async function stopProcessTree(child: ChildProcess) {
  if (!child.pid) return;
  const exited = () => child.exitCode !== null || child.signalCode !== null;
  if (process.platform === "win32") {
    if (exited())
      throw new Error(
        `Cannot confirm descendants of exited process ${child.pid}`,
      );
    const result = spawnSync(
      "taskkill.exe",
      ["/PID", String(child.pid), "/T", "/F"],
      { encoding: "utf8", timeout: 10_000 },
    );
    if (result.error || result.status !== 0)
      throw new Error(
        `Cannot stop process tree ${child.pid}: ${result.error ?? result.stderr}`,
      );
  } else {
    const target = -child.pid;
    const alive = () => {
      try {
        process.kill(target, 0);
        return true;
      } catch (error) {
        if ((error as NodeJS.ErrnoException).code === "ESRCH") return false;
        // A group can still exist while its last process is being reaped.
        // EPERM is not proof of exit; keep waiting within the deadline.
        if ((error as NodeJS.ErrnoException).code === "EPERM") return true;
        throw error;
      }
    };
    const send = (signal: NodeJS.Signals) => {
      try {
        process.kill(target, signal);
      } catch (error) {
        if ((error as NodeJS.ErrnoException).code !== "ESRCH") throw error;
      }
    };
    if (!alive()) return;
    send("SIGTERM");
    const deadline = Date.now() + 3_000;
    while (alive() && Date.now() < deadline) await delay(50);
    if (alive()) send("SIGKILL");
    const forcedDeadline = Date.now() + 3_000;
    while (alive() && Date.now() < forcedDeadline) await delay(50);
    if (alive()) throw new Error(`Process ${target} did not stop`);
  }
  const deadline = Date.now() + 3_000;
  while (!exited() && Date.now() < deadline) await delay(50);
  if (!exited())
    throw new Error(`No exit confirmation for process ${child.pid}`);
}
