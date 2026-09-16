import assert from "node:assert/strict";
import { spawn, spawnSync, type ChildProcess } from "node:child_process";
import { closeSync, openSync } from "node:fs";
import { access, mkdir, readFile, writeFile } from "node:fs/promises";
import { createServer } from "node:net";
import { resolve } from "node:path";
import { setTimeout as delay } from "node:timers/promises";
import { installFixtures } from "./fixtures";

if (process.platform !== "win32" && process.platform !== "linux") {
  throw new Error("Native WebDriver checks require Windows or Linux.");
}
if (!process.versions.bun) throw new Error("Run this check with Bun.");

const output = resolve("test-results/native");
await mkdir(output, { recursive: true });
const binary = resolve(
  process.env.TINYDASH_NATIVE_BINARY ??
    `src-tauri/target/release/tinydash${process.platform === "win32" ? ".exe" : ""}`,
);
await access(binary);
const fixtures = await installFixtures();
const elementKey = "element-6066-11e4-a52e-4f735466cecf";
let session: string | undefined;
let driver: ChildProcess | undefined;
let driverError: Error | undefined;
let application: ChildProcess | undefined;
let applicationError: Error | undefined;
let log: number | undefined;
let appLog: number | undefined;
const passed: string[] = [];

async function freePort(): Promise<number> {
  const server = createServer();
  return new Promise((resolvePort, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", () => {
      const address = server.address();
      assert(address && typeof address !== "string");
      server.close((error) =>
        error ? reject(error) : resolvePort(address.port),
      );
    });
  });
}

const port = await freePort();
let nativePort = await freePort();
while (nativePort === port) nativePort = await freePort();

async function request<T>(
  path: string,
  method = "GET",
  body?: unknown,
  timeout = 15_000,
): Promise<T> {
  const response = await fetch(`http://127.0.0.1:${port}${path}`, {
    method,
    headers: { "Content-Type": "application/json" },
    body: body === undefined ? undefined : JSON.stringify(body),
    signal: AbortSignal.timeout(timeout),
  }).catch((error: unknown) => {
    throw new Error(`${method} ${path} failed: ${String(error)}`, {
      cause: error,
    });
  });
  const result = (await response.json()) as {
    value: T & { error?: string; message?: string };
  };
  if (!response.ok || result.value?.error) {
    throw new Error(
      `${method} ${path}: ${result.value?.message ?? response.statusText}`,
    );
  }
  return result.value;
}

async function until(
  description: string,
  check: () => Promise<boolean>,
  timeout = 20_000,
) {
  const deadline = Date.now() + timeout;
  let lastError: unknown;
  while (Date.now() < deadline) {
    if (driverError) throw driverError;
    if (applicationError) throw applicationError;
    if (driver && driver.exitCode !== null) {
      throw new Error(`tauri-driver exited with code ${driver.exitCode}`);
    }
    try {
      if (await check()) return;
    } catch (error) {
      lastError = error;
    }
    await delay(100);
  }
  throw new Error(`Timed out: ${description}`, { cause: lastError });
}

const observe = <T>(script: string) =>
  request<T>(`/session/${session}/execute/sync`, "POST", { script, args: [] });

const keys = (element: string, text: string) =>
  request(`/session/${session}/element/${element}/value`, "POST", {
    text,
    value: [...text],
  });

function pass(description: string) {
  passed.push(description);
  console.log(`PASS: ${description}`);
}

async function saveScreen(name: string) {
  const screenshot = await request<string>(`/session/${session}/screenshot`);
  await writeFile(resolve(output, name), Buffer.from(screenshot, "base64"));
}

try {
  log = openSync(resolve(output, "driver.log"), "w");
  const driverArgs = [
    "--port",
    String(port),
    "--native-port",
    String(nativePort),
  ];
  driver = spawn("tauri-driver", driverArgs, {
    env: fixtures.env,
    stdio: ["ignore", log, log],
    detached: process.platform !== "win32",
  });
  driver.once("error", (error) => (driverError = error));
  await until("WebDriver startup", async () => {
    const status = await request<{ ready: boolean }>(
      "/status",
      "GET",
      undefined,
      1_000,
    );
    return status.ready;
  });
  let capabilities: Record<string, unknown> = {
    "tauri:options": { application: binary },
  };
  if (process.platform === "win32") {
    // WebView2's launch mode relies on finding a DevToolsActivePort file.
    // Attach to an explicit loopback port so app startup remains observable.
    const debugPort = process.env.TINYDASH_NATIVE_DEBUG_PORT
      ? Number(process.env.TINYDASH_NATIVE_DEBUG_PORT)
      : await freePort();
    assert(Number.isInteger(debugPort) && debugPort > 0 && debugPort <= 65535);
    appLog = openSync(resolve(output, "application.log"), "w");
    application = spawn(binary, [], {
      env: {
        ...fixtures.env,
        WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${debugPort}`,
        WEBVIEW2_USER_DATA_FOLDER: resolve(
          fixtures.directory,
          "webview-profile",
        ),
      },
      stdio: ["ignore", appLog, appLog],
    });
    application.once("error", (error) => (applicationError = error));
    application.once("exit", (code, signal) => {
      applicationError = new Error(
        `TinyDash exited: code=${code}, signal=${signal}`,
      );
    });
    await until("TinyDash starts its WebView2 instance", async () => {
      const response = await fetch(
        `http://127.0.0.1:${debugPort}/json/version`,
        {
          signal: AbortSignal.timeout(1_000),
        },
      );
      return response.ok;
    });
    capabilities = {
      browserName: "webview2",
      "ms:edgeChromium": true,
      "ms:edgeOptions": { debuggerAddress: `127.0.0.1:${debugPort}` },
    };
  }
  const created = await request<{ sessionId: string }>(
    "/session",
    "POST",
    {
      capabilities: { alwaysMatch: capabilities },
    },
    60_000,
  );
  session = created.sessionId;
  assert(session, "WebDriver returned a session ID");
  await request(`/session/${session}/timeouts`, "POST", {
    implicit: 0,
    script: 5_000,
    pageLoad: 30_000,
  });
  // The WebView exists before launcher_ready shows the native window. Wait
  // for the initial search too, so typing cannot precede the first open event.
  await until("application startup and search input focus", () =>
    observe<boolean>(
      `return document.activeElement?.getAttribute('role') === 'combobox'
        && document.querySelector('[role=listbox]')?.getAttribute('aria-busy') === 'false'
        && /\\d+ installed/.test(document.querySelector('.list-count')?.textContent ?? '')
        && document.querySelectorAll('[role=option]').length > 0`,
    ),
  );
  pass(
    "The application loads its index and selects the search field for input",
  );

  const input = await request<Record<string, string>>(
    `/session/${session}/element`,
    "POST",
    { using: "css selector", value: '[role="combobox"]' },
  );
  const inputId = input[elementKey];
  assert(inputId, "The search field has a WebDriver element ID");
  const expectedNames = [`${fixtures.prefix} Alpha`, `${fixtures.prefix} Beta`];
  const titles = () =>
    observe<string[]>(
      "return [...document.querySelectorAll('[role=option] .result-title')].map(el => el.textContent)",
    );
  await keys(inputId, fixtures.prefix);
  await until("OS discovery finds both fixture applications", async () => {
    const names = await titles();
    return (
      names.length === 2 &&
      expectedNames.every((name) => names.includes(name)) &&
      (await observe<boolean>(
        "return document.querySelector('[role=listbox]')?.getAttribute('aria-busy') === 'false'",
      ))
    );
  });
  pass(
    "Search finds both real OS application entries through the Rust backend",
  );

  await keys(inputId, "\uE009a\uE000");
  await keys(inputId, `tdsm ${fixtures.nonce}`);
  await until("fuzzy search returns both fixture applications", async () => {
    const names = await titles();
    return (
      names.length === 2 &&
      expectedNames.every((name) => names.includes(name)) &&
      (await observe<boolean>(
        "return document.querySelector('[role=listbox]')?.getAttribute('aria-busy') === 'false'",
      ))
    );
  });
  pass("An abbreviation matches applications in the native index");

  const orderedNames = await titles();
  const selectedTitle = () =>
    observe<string>(
      "return document.querySelector('[role=option][aria-selected=true] .result-title')?.textContent",
    );
  assert.equal(await selectedTitle(), orderedNames[0]);
  await keys(inputId, "\uE015");
  assert.equal(await selectedTitle(), orderedNames[1]);
  await keys(inputId, "\uE013");
  assert.equal(await selectedTitle(), orderedNames[0]);
  await keys(inputId, "\uE013");
  assert.equal(await selectedTitle(), orderedNames[1]);
  pass("Arrow keys move selection and wrap at the start of the list");
  await saveScreen("search.png");

  const expectedLabel = orderedNames[1] === expectedNames[0] ? "Alpha" : "Beta";
  await keys(inputId, "\uE007");
  await until("Enter starts the selected native executable", async () => {
    try {
      return (await readFile(fixtures.marker, "utf8")) === expectedLabel;
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code === "ENOENT") return false;
      throw error;
    }
  });
  pass("Enter launches the selected fixture through the OS");
  await writeFile(
    resolve(output, "result.json"),
    JSON.stringify({ passed }, null, 2),
  );
} catch (error) {
  console.error(error);
  if (process.platform === "win32") {
    const diagnostic = spawnSync(
      "powershell.exe",
      [
        "-NoProfile",
        "-NonInteractive",
        "-File",
        resolve("scripts/ci/windows-processes.ps1"),
      ],
      { encoding: "utf8", timeout: 15_000 },
    );
    await writeFile(
      resolve(output, "windows-processes.txt"),
      `${diagnostic.stdout ?? ""}\n${diagnostic.stderr ?? ""}`,
    );
  }
  await writeFile(
    resolve(output, "failure.txt"),
    String(error instanceof Error ? error.stack : error),
  );
  if (session) {
    await saveScreen("failure.png").catch(() => {});
    await request<string>(`/session/${session}/source`)
      .then((source) => writeFile(resolve(output, "failure.html"), source))
      .catch(() => {});
  }
  process.exitCode = 1;
} finally {
  if (session) {
    await request(`/session/${session}`, "DELETE", undefined, 5_000).catch(
      () => {},
    );
  }
  if (driver?.pid) {
    if (process.platform === "win32") {
      spawnSync("taskkill.exe", ["/PID", String(driver.pid), "/T", "/F"], {
        stdio: "ignore",
      });
    } else {
      try {
        process.kill(-driver.pid, "SIGTERM");
      } catch (error) {
        if ((error as NodeJS.ErrnoException).code !== "ESRCH")
          console.error(error);
      }
    }
  }
  if (application?.pid) {
    spawnSync("taskkill.exe", ["/PID", String(application.pid), "/T", "/F"], {
      stdio: "ignore",
    });
  }
  if (log !== undefined) closeSync(log);
  if (appLog !== undefined) closeSync(appLog);
  await fixtures.cleanup().catch(async (error: unknown) => {
    console.error("Failed to remove the temporary application fixtures", error);
    await writeFile(
      resolve(output, "cleanup-failure.txt"),
      String(error instanceof Error ? error.stack : error),
    );
    process.exitCode = 1;
  });
}
