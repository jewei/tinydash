import assert from "node:assert/strict";
import {
  execFileSync,
  spawn,
  spawnSync,
  type ChildProcess,
} from "node:child_process";
import { closeSync, openSync } from "node:fs";
import {
  access,
  mkdir,
  readFile,
  readdir,
  rm,
  writeFile,
} from "node:fs/promises";
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

function clipboardText(): string {
  if (process.platform === "win32") {
    return execFileSync(
      "powershell.exe",
      [
        "-NoProfile",
        "-NonInteractive",
        "-Command",
        "[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new(); Get-Clipboard -Raw",
      ],
      { encoding: "utf8", timeout: 5_000 },
    ).trimEnd();
  }
  return execFileSync("xclip", ["-selection", "clipboard", "-o"], {
    encoding: "utf8",
    timeout: 5_000,
  });
}

function setClipboardText(text: string) {
  if (process.platform === "win32") {
    execFileSync(
      "powershell.exe",
      [
        "-NoProfile",
        "-NonInteractive",
        "-Command",
        "Set-Clipboard -Value $env:TINYDASH_TEST_CLIPBOARD",
      ],
      {
        env: { ...process.env, TINYDASH_TEST_CLIPBOARD: text },
        timeout: 5_000,
      },
    );
  } else {
    execFileSync("xclip", ["-selection", "clipboard", "-i"], {
      input: text,
      stdio: ["pipe", "ignore", "ignore"],
      timeout: 5_000,
    });
  }
}

async function click(selector: string) {
  const element = await request<Record<string, string>>(
    `/session/${session}/element`,
    "POST",
    { using: "css selector", value: selector },
  );
  await request(
    `/session/${session}/element/${element[elementKey]}/click`,
    "POST",
    {},
  );
}

async function selectMode(mode: "clipboard" | "files") {
  // All mode also matches paths. Windows Start Menu paths can match words
  // such as "second", so clipboard checks must select the intended provider.
  // WebKitWebDriver can change the option without delivering its change
  // event. Select as browser test drivers do, through the normal DOM events.
  // This still runs the frontend handler and real Rust IPC; no results are mocked.
  await request(`/session/${session}/execute/sync`, "POST", {
    script: `const select = document.querySelector('select');
      select.value = arguments[0];
      select.dispatchEvent(new Event('input', { bubbles: true }));
      select.dispatchEvent(new Event('change', { bubbles: true }));`,
    args: [mode],
  });
  const placeholder =
    mode === "clipboard"
      ? "Search clipboard history..."
      : "Search filenames and paths...";
  await until(`${mode} mode is ready`, () =>
    observe<boolean>(
      `return document.querySelector('input')?.placeholder === ${JSON.stringify(placeholder)} && document.querySelector('[role=listbox]')?.getAttribute('aria-busy') === 'false'`,
    ),
  );
}

async function reopen() {
  // Start the executable as a desktop shortcut would. Its single-instance
  // handler must show the resident window and reset the search field.
  const child = spawn(binary, [], { env: fixtures.env, stdio: "ignore" });
  try {
    await new Promise<void>((resolveExit, reject) => {
      const timeout = setTimeout(
        () => reject(new Error("The second instance did not exit")),
        10_000,
      );
      child.once("error", (error) => {
        clearTimeout(timeout);
        reject(error);
      });
      child.once("exit", (code) => {
        clearTimeout(timeout);
        if (code === 0) resolveExit();
        else reject(new Error(`The second instance exited with code ${code}`));
      });
    });
  } finally {
    if (child.exitCode === null) child.kill();
  }
  await until("the existing window reopens with an empty query", () =>
    observe<boolean>(
      `return document.querySelector('input[role=combobox]')?.value === ''
      && document.activeElement?.getAttribute('role') === 'combobox'
      && document.querySelector('[role=listbox]')?.getAttribute('aria-busy') === 'false'
      && document.querySelectorAll('[role=option]').length > 0`,
    ),
  );
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
  await keys(inputId, "12 * 8");
  await until(
    "arithmetic returns 96",
    async () => (await titles())[0] === "96",
  );
  await keys(inputId, "\uE009a\uE000");
  await keys(inputId, "5 ft to cm");
  await until(
    "units convert to 152.4 cm",
    async () =>
      (await titles())[0] === "152.4 cm" &&
      (await observe<boolean>(
        "return document.querySelector('[role=listbox]')?.getAttribute('aria-busy') === 'false'",
      )),
  );
  await saveScreen("calculator.png");
  await keys(inputId, "\uE007");
  await until(
    "the calculation reaches the OS clipboard",
    async () => clipboardText() === "152.4 cm",
  );
  pass(
    "Arithmetic and unit conversion run in Rust, and Enter copies the result to the OS clipboard",
  );

  await reopen();
  await keys(inputId, ":rocket");
  await until(
    "the local emoji provider finds rocket",
    async () =>
      (await titles())[0] === "rocket" &&
      (await observe<boolean>(
        "return document.querySelector('[role=listbox]')?.getAttribute('aria-busy') === 'false'",
      )),
  );
  await saveScreen("emoji.png");
  await keys(inputId, "\uE007");
  await until(
    "the emoji reaches the OS clipboard",
    async () => clipboardText() === "🚀",
  );
  pass(
    "Emoji search uses local Rust data, and Enter copies the complete emoji to the OS clipboard",
  );
  await reopen();
  pass("Starting TinyDash again reopens its existing window after copying");

  await keys(inputId, ":");
  await until(
    "the copied emoji ranks first without a search term",
    async () => (await titles())[0] === "rocket",
  );
  pass("Copying an emoji moves it to the top of the emoji list");
  await keys(inputId, "\uE009a\uE000\uE003");

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

  await reopen();
  await until(
    "the launched app ranks first in the complete index",
    async () => (await titles())[0] === orderedNames[1],
  );
  await saveScreen("usage-ranking.png");
  pass(
    "A successful launch moves the app to the top before the result limit is applied",
  );

  const firstClip = `TinyDash clipboard ${fixtures.nonce}\n  Preserve spaces and emoji 🚀`;
  const secondClip = `TinyDash second ${fixtures.nonce}`;
  await selectMode("clipboard");
  setClipboardText(firstClip);
  await keys(inputId, "\uE009a\uE000");
  await keys(inputId, `clipboard ${fixtures.nonce}`);
  await until("text copied by another process enters the history", async () =>
    (await titles()).some((title) => title === firstClip.split("\n")[0]),
  );
  await until("the clipboard preview preserves the full text", () =>
    observe<boolean>(
      `return document.querySelector('.clipboard-preview pre')?.textContent === ${JSON.stringify(firstClip)}`,
    ),
  );
  setClipboardText(firstClip);
  await delay(1_200);
  assert.equal(
    (await titles()).filter((title) => title === firstClip.split("\n")[0])
      .length,
    1,
  );
  pass(
    "External clipboard text is captured once and its full Unicode value appears in the preview",
  );
  setClipboardText(secondClip);
  // Wait for the next capture so Enter must copy the older historical value.
  await keys(inputId, "\uE009a\uE000");
  await keys(inputId, `second ${fixtures.nonce}`);
  await until(
    "the second external value enters history",
    async () => (await titles())[0] === secondClip,
  );
  await keys(inputId, "\uE009a\uE000");
  await keys(inputId, `clipboard ${fixtures.nonce}`);
  await until("the older entry remains searchable", () =>
    observe<boolean>(
      "return document.querySelector('.result-title')?.textContent.startsWith('TinyDash clipboard') && document.querySelector('[role=listbox]')?.getAttribute('aria-busy') === 'false'",
    ),
  );
  await saveScreen("clipboard.png");
  await keys(inputId, "\uE007");
  await until(
    "Enter restores the historical text to the OS clipboard",
    async () => clipboardText() === firstClip,
  );
  pass(
    "Enter copies a stored clipboard entry through the native clipboard plugin",
  );
  await reopen();
  await selectMode("clipboard");
  await keys(inputId, `clipboard ${fixtures.nonce}`);
  await until(
    "the copied history entry remains available",
    async () => (await titles())[0] === firstClip.split("\n")[0],
  );
  await keys(inputId, "\uE009\uE003\uE000");
  await until(
    "the delete shortcut removes the entry",
    async () => (await titles()).length === 0,
  );
  await reopen();
  await selectMode("clipboard");
  await keys(inputId, `clipboard ${fixtures.nonce}`);
  await delay(1_200);
  assert.deepEqual(await titles(), []);
  pass(
    "Deleting an entry keeps the window open and does not recapture unchanged clipboard text",
  );
  await keys(inputId, "\uE009a\uE000");
  await keys(inputId, `second ${fixtures.nonce}`);
  await until(
    "the other entry is still available",
    async () => (await titles())[0] === secondClip,
  );
  await click(".clear-history");
  await until("clear asks for confirmation with Cancel selected", () =>
    observe<boolean>(
      "return !!document.querySelector('dialog[open]') && document.activeElement?.textContent === 'Cancel'",
    ),
  );
  await click("dialog .cancel-button");
  assert.equal((await titles())[0], secondClip);
  await click(".clear-history");
  await click("dialog .clear-button");
  await until(
    "confirmed clear removes remaining history",
    async () => (await titles()).length === 0,
  );
  assert.equal(clipboardText(), firstClip);
  pass("Clear history requires confirmation and preserves the OS clipboard");

  await reopen();
  await selectMode("files");
  await until(
    "the file scan finds only the visible document",
    async () =>
      (await titles()).length === 1 &&
      (await titles())[0] === fixtures.fileName &&
      (await observe<boolean>(
        "return document.querySelector('.list-count')?.textContent === '1 file indexed'",
      )),
  );
  pass(
    "File scanning uses configured roots and excludes hidden files and excluded folders",
  );
  await keys(inputId, "bdg nts");
  await until(
    "filename fuzzy search finds the document",
    async () => (await titles())[0] === fixtures.fileName,
  );
  await keys(inputId, "\uE009a\uE000");
  await keys(inputId, "files/Budget");
  await until(
    "path search finds the document",
    async () => (await titles())[0] === fixtures.fileName,
  );
  await keys(inputId, "\uE009a\uE000");
  await keys(inputId, `Content-only-${fixtures.nonce}`);
  await until(
    "file contents are not searchable",
    async () => (await titles()).length === 0,
  );
  pass(
    "File search matches filenames and paths without indexing file contents",
  );
  await keys(inputId, "\uE009a\uE000");
  await keys(inputId, fixtures.fileName);
  await until("file search is ready to open", () =>
    observe<boolean>(
      `return document.querySelector('.result-title')?.textContent === ${JSON.stringify(fixtures.fileName)} && document.querySelector('[role=listbox]')?.getAttribute('aria-busy') === 'false'`,
    ),
  );
  await saveScreen("files.png");
  await keys(inputId, "\uE007");
  await until(
    "the OS file association opens the selected document",
    async () =>
      (await readFile(fixtures.fileMarker, "utf8")) === fixtures.filePath,
  );
  pass("Enter opens the selected file through its OS document association");
  await reopen();
  await selectMode("files");
  await keys(inputId, fixtures.fileName);
  await until("the indexed file remains searchable", () =>
    observe<boolean>(
      `return document.querySelector('.result-title')?.textContent === ${JSON.stringify(fixtures.fileName)} && document.querySelector('[role=listbox]')?.getAttribute('aria-busy') === 'false'`,
    ),
  );
  await rm(fixtures.filePath);
  await rm(fixtures.fileMarker);
  await keys(inputId, "\uE007");
  await until("a deleted file produces an action error", () =>
    observe<boolean>(
      "return document.querySelector('[role=alert]')?.textContent.includes('This file is no longer available')",
    ),
  );
  pass(
    "Opening a deleted file reports an error and keeps the launcher available",
  );
  const replacement = `Replacement ${fixtures.nonce}.txt`;
  await writeFile(
    resolve(fixtures.fileRoot, replacement),
    "New file after the initial scan\n",
  );
  await keys(inputId, "\uE009r\uE000");
  await until(
    "refresh removes the deleted path",
    async () => (await titles()).length === 0,
  );
  await keys(inputId, "\uE009a\uE000");
  await keys(inputId, replacement);
  await until(
    "refresh finds the new document",
    async () => (await titles())[0] === replacement,
  );
  pass("The Files refresh shortcut replaces the index after file changes");
  await writeFile(
    resolve(output, "result.json"),
    JSON.stringify({ passed }, null, 2),
  );
} catch (error) {
  console.error(error);
  await writeFile(
    resolve(output, "file-fixture.json"),
    JSON.stringify(
      {
        expected: fixtures.filePath,
        marker: await readFile(fixtures.fileMarker, "utf8").catch(() => null),
        files: await readdir(fixtures.fileRoot).catch(() => []),
      },
      null,
      2,
    ),
  );
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
