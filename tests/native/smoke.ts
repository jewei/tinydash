import assert from "node:assert/strict";
import {
  execFileSync,
  spawn,
  spawnSync,
  type ChildProcess,
} from "node:child_process";
import { closeSync, openSync, writeFileSync } from "node:fs";
import {
  access,
  mkdir,
  readFile,
  readdir,
  rename,
  rm,
  writeFile,
} from "node:fs/promises";
import { createServer } from "node:net";
import { resolve } from "node:path";
import { setTimeout as sleep } from "node:timers/promises";
import { nativeTestBinary } from "../../scripts/verify/native.ts";
import {
  cancellation,
  cleanupAll,
  errorDetails,
  stopProcessTree,
  waitForExit,
} from "../../scripts/verify/lifecycle.ts";
import { installFixtures } from "./fixtures";

if (!process.versions.bun) throw new Error("Run this check with Bun.");

const binary = nativeTestBinary();
const cancelled = cancellation();
const delay = (milliseconds: number) =>
  sleep(milliseconds, undefined, { signal: cancelled.signal });
const output = resolve(
  process.env.TINYDASH_NATIVE_OUTPUT ?? "test-results/native",
);
let fixtures!: Awaited<ReturnType<typeof installFixtures>>;
let cleanupFixtures: (() => Promise<void>) | undefined;
let fixtureDirectory: string | undefined;
const elementKey = "element-6066-11e4-a52e-4f735466cecf";
let session: string | undefined;
let driver: ChildProcess | undefined;
let driverError: Error | undefined;
let application: ChildProcess | undefined;
let applicationError: Error | undefined;
let log: number | undefined;
let appLog: number | undefined;
const passed: string[] = [];
const reopenCheckMs: number[] = [];
const secondInstances = new Set<ChildProcess>();

function recordOwnedResources() {
  writeFileSync(
    resolve(output, "owned-resources.json"),
    JSON.stringify(
      {
        suite: process.pid,
        fixtureDirectory,
        driver: driver?.pid,
        application: application?.pid,
        secondInstances: [...secondInstances].map((child) => child.pid),
      },
      null,
      2,
    ) + "\n",
  );
}

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

let port: number;
let nativePort: number;

async function request<T>(
  path: string,
  method = "GET",
  body?: unknown,
  timeout = 15_000,
  cancellable = true,
): Promise<T> {
  const response = await fetch(`http://127.0.0.1:${port}${path}`, {
    method,
    headers: { "Content-Type": "application/json" },
    body: body === undefined ? undefined : JSON.stringify(body),
    signal: cancellable
      ? AbortSignal.any([cancelled.signal, AbortSignal.timeout(timeout)])
      : AbortSignal.timeout(timeout),
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
    cancelled.signal.throwIfAborted();
    if (driverError) throw driverError;
    if (applicationError) throw applicationError;
    if (driver && driver.exitCode !== null) {
      throw new Error(`tauri-driver exited with code ${driver.exitCode}`);
    }
    try {
      if (await check()) return;
    } catch (error) {
      cancelled.signal.throwIfAborted();
      lastError = error;
    }
    await delay(100);
  }
  throw new Error(`Timed out: ${description}`, { cause: lastError });
}

const observeArgs = <T>(script: string, args: unknown[]) =>
  request<T>(`/session/${session}/execute/sync`, "POST", { script, args });
const observe = <T>(script: string) => observeArgs<T>(script, []);

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
  if (process.platform === "linux") {
    // WebKit's snapshot request can stall while other driver requests still
    // work. Capture the focused X11 window without waiting on that renderer.
    execFileSync("scrot", ["--focused", "--overwrite", resolve(output, name)], {
      timeout: 5_000,
      stdio: "pipe",
    });
    return;
  }
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

async function clickButtonText(text: string) {
  const clicked = await observeArgs<boolean>(
    `const button = [...document.querySelectorAll('button')].find((item) => item.textContent?.trim() === arguments[0]);
     if (!button || button.disabled) return false;
     button.click();
     return true;`,
    [text],
  );
  assert(clicked, `Could not click button: ${text}`);
}

async function clickMenuItemText(text: string) {
  await until(`menu item is available: ${text}`, () =>
    observeArgs<boolean>(
      `const item = [...document.querySelectorAll('[role=menuitem]')].find((element) => element.textContent?.trim() === arguments[0]);
       if (!item || item.disabled || item.getAttribute('aria-disabled') === 'true') return false;
       item.click();
       return true;`,
      [text],
    ),
  );
}

async function selectMode(mode: "apps" | "clipboard" | "files" | "system") {
  // All mode also matches paths. Isolate the intended provider so temporary
  // file paths do not affect app or clipboard result assertions.
  const label = {
    apps: "Apps",
    clipboard: "Clipboard",
    files: "Files",
    system: "System",
  }[mode];
  await clickButtonText(label);
  const placeholder = {
    apps: "Search applications...",
    clipboard: "Search clipboard history...",
    files: "Search filenames and paths...",
    system: "Search system commands...",
  }[mode];
  await until(`${mode} mode is ready`, () =>
    observe<boolean>(
      `return document.querySelector('.category-tab[aria-pressed=true]')?.textContent === ${JSON.stringify(label)}
        && document.querySelector('input[role=combobox]')?.placeholder === ${JSON.stringify(placeholder)}
        && document.querySelector('[role=listbox]')?.getAttribute('aria-busy') === 'false'`,
    ),
  );
}

async function reopen() {
  cancelled.signal.throwIfAborted();
  const started = performance.now();
  // Start the executable as a desktop shortcut would. Its single-instance
  // handler must show the resident window and reset the search field.
  const child = spawn(binary, [], {
    env: fixtures.env,
    stdio: "ignore",
    detached: process.platform !== "win32",
  });
  secondInstances.add(child);
  try {
    recordOwnedResources();
    await waitForExit(child, cancelled.signal, 10_000);
  } finally {
    if (child.exitCode === null && child.signalCode === null)
      await stopProcessTree(child);
    secondInstances.delete(child);
  }
  await until("the existing window reopens on the welcome screen", () =>
    observe<boolean>(
      `return document.querySelector('input[role=combobox]')?.value === ''
      && document.activeElement?.getAttribute('role') === 'combobox'
      && document.querySelector('[role=listbox]')?.getAttribute('aria-busy') === 'false'
      && document.querySelector('.category-tab[aria-pressed=true]')?.textContent === 'All'
      && document.querySelector('.welcome-suggestions') !== null
      && document.querySelectorAll('[role=option]').length === 0`,
    ),
  );
  reopenCheckMs.push(performance.now() - started);
}

try {
  await mkdir(output, { recursive: true });
  recordOwnedResources();
  cancelled.signal.throwIfAborted();
  await access(binary);
  fixtures = await installFixtures(cancelled.signal, (cleanup, directory) => {
    cleanupFixtures = cleanup;
    fixtureDirectory = directory;
    recordOwnedResources();
  });
  cancelled.signal.throwIfAborted();
  const settingsPath = resolve(
    process.platform === "win32"
      ? (process.env.APPDATA ?? "")
      : (fixtures.env.XDG_CONFIG_HOME ?? ""),
    "dev.tinydash.launcher",
    "settings.json",
  );
  const fixtureSettings = JSON.parse(
    await readFile(settingsPath, "utf8"),
  ) as Record<string, unknown>;
  await writeFile(
    settingsPath,
    JSON.stringify({
      ...fixtureSettings,
      clipboardHistoryEnabled: false,
      clipboardHistoryDecided: false,
    }),
  );
  port = await freePort();
  nativePort = await freePort();
  while (nativePort === port) nativePort = await freePort();
  cancelled.signal.throwIfAborted();
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
    detached: true,
    windowsHide: true,
  });
  driver.once("error", (error) => (driverError = error));
  recordOwnedResources();
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
  let startupStarted: number | undefined;
  if (process.platform === "win32") {
    // WebView2's launch mode relies on finding a DevToolsActivePort file.
    // Attach to an explicit loopback port so app startup remains observable.
    const debugPort = process.env.TINYDASH_NATIVE_DEBUG_PORT
      ? Number(process.env.TINYDASH_NATIVE_DEBUG_PORT)
      : await freePort();
    assert(Number.isInteger(debugPort) && debugPort > 0 && debugPort <= 65535);
    appLog = openSync(resolve(output, "application.log"), "w");
    startupStarted = performance.now();
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
    recordOwnedResources();
    await until("TinyDash starts its WebView2 instance", async () => {
      const response = await fetch(
        `http://127.0.0.1:${debugPort}/json/version`,
        {
          signal: AbortSignal.any([
            cancelled.signal,
            AbortSignal.timeout(1_000),
          ]),
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
  startupStarted ??= performance.now();
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
        && document.querySelector('.welcome-suggestions') !== null
        && document.querySelectorAll('[role=option]').length === 0`,
    ),
  );
  const startupCheckMs = performance.now() - startupStarted;
  pass(
    "The application shows its welcome screen and selects the search field for input",
  );

  const input = await request<Record<string, string>>(
    `/session/${session}/element`,
    "POST",
    { using: "css selector", value: '[role="combobox"]' },
  );
  const inputId = input[elementKey];
  assert(inputId, "The search field has a WebDriver element ID");
  await until("the first-use clipboard choice is visible", () =>
    observe<boolean>(
      "return !!document.querySelector('.first-use[aria-label=\"Clipboard history choice\"]')",
    ),
  );
  // This value can be captured after consent. Keep it distinct from the
  // clipboard search fixture used below so it cannot change that selection.
  const beforeChoice = `TinyDash consent probe ${fixtures.nonce}`;
  setClipboardText(beforeChoice);
  await delay(1_000);
  const beforeChoiceEntries = await request<string[]>(
    `/session/${session}/execute/async`,
    "POST",
    {
      script: `const done = arguments[arguments.length - 1];
        window.__TAURI_INTERNALS__.invoke('search', { query: arguments[0], mode: 'clipboard' })
          .then(response => done(response.results.map(result => result.title)), error => done([String(error)]));`,
      args: [beforeChoice],
    },
  );
  assert.deepEqual(beforeChoiceEntries, []);
  pass("Clipboard capture stays off until the first-use choice is made");
  await clickButtonText("Enable history");
  await until(
    "the first-use clipboard choice closes after enabling history",
    () =>
      observe<boolean>(
        "return !document.querySelector('.first-use[aria-label=\"Clipboard history choice\"]')",
      ),
  );
  pass("The native smoke flow explicitly enables clipboard history");
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

  await selectMode("apps");
  if (process.platform === "linux") {
    await keys(inputId, "TinyDash");
    await until(
      "the installed desktop entry is excluded from app search",
      async () => {
        const names = await titles();
        return (
          expectedNames.every((name) => names.includes(name)) &&
          !names.includes("TinyDash") &&
          (await observe<boolean>(
            "return document.querySelector('[role=listbox]')?.getAttribute('aria-busy') === 'false'",
          ))
        );
      },
    );
    pass("Application search excludes TinyDash's installed desktop entry");
    await keys(inputId, "\uE009a\uE000\uE003");
  }
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
  await selectMode("apps");
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
  await click(".actions-button");
  await clickMenuItemText("Pin to Clipboard");
  await click(".actions-button");
  await clickMenuItemText("Pin to All");
  const thirdClip = `TinyDash third ${fixtures.nonce}`;
  setClipboardText(thirdClip);
  await click(".clear-query");
  await until(
    "the newer unpinned clipboard entry is selected after the pinned entry",
    async () => {
      const entries = await titles();
      return (
        entries[0] === secondClip &&
        entries.includes(thirdClip) &&
        (await selectedTitle()) === thirdClip
      );
    },
  );
  assert.equal((await titles())[0], secondClip);
  assert.equal(await selectedTitle(), thirdClip);
  assert.equal(
    await observe<string>(
      "return document.querySelector('input[role=combobox]')?.value ?? ''",
    ),
    "",
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
  await click("dialog .confirm-button");
  await until(
    "confirmed clear removes unpinned history but keeps pinned history",
    async () =>
      (await titles()).length === 1 && (await titles())[0] === secondClip,
  );
  assert.equal(await selectedTitle(), secondClip);
  await click(".actions-button");
  await clickMenuItemText("Clear all clipboard history");
  await until("clear all asks for its separate confirmation", () =>
    observe<boolean>(
      "return document.querySelector('dialog[open] h2')?.textContent === 'Clear all clipboard history?'",
    ),
  );
  await click("dialog .confirm-button");
  await until(
    "confirmed clear all removes pinned history",
    async () => (await titles()).length === 0,
  );
  assert.equal(clipboardText(), thirdClip);
  pass(
    "Clear unpinned protects pinned clipboard history, clear all removes it, and both preserve the OS clipboard",
  );

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
  await until(
    "the watcher removes the deleted file",
    async () => (await titles()).length === 0,
  );
  pass("Deleted files leave the results without a manual refresh");
  const replacement = `Replacement ${fixtures.nonce}.txt`;
  await writeFile(
    resolve(fixtures.fileRoot, replacement),
    "New file after the initial scan\n",
  );
  await keys(inputId, "\uE009a\uE000");
  await keys(inputId, replacement);
  await until(
    "the watcher finds the new document",
    async () => (await titles())[0] === replacement,
  );
  const renamed = `Renamed ${fixtures.nonce}.txt`;
  await rename(
    resolve(fixtures.fileRoot, replacement),
    resolve(fixtures.fileRoot, renamed),
  );
  await until(
    "the old filename leaves the results",
    async () => !(await titles()).includes(replacement),
  );
  await keys(inputId, "\uE009a\uE000");
  await keys(inputId, renamed);
  await until(
    "the renamed file is indexed",
    async () => (await titles())[0] === renamed,
  );
  const nested = `Nested ${fixtures.nonce}.txt`;
  await mkdir(resolve(fixtures.fileRoot, "new-folder"));
  await writeFile(
    resolve(fixtures.fileRoot, "new-folder", nested),
    "Created before the new directory watch\n",
  );
  await keys(inputId, "\uE009a\uE000");
  await keys(inputId, nested);
  await until(
    "new subdirectories are scanned and watched",
    async () => (await titles())[0] === nested,
  );
  await rename(fixtures.fileRoot, `${fixtures.fileRoot}-old`);
  await mkdir(fixtures.fileRoot);
  const recreated = `Recreated ${fixtures.nonce}.txt`;
  await writeFile(
    resolve(fixtures.fileRoot, recreated),
    "A replacement search root\n",
  );
  await until(
    "the old root leaves the results",
    // Fuzzy path matching can return the recreated file for the old query.
    // Only the stale file must disappear; unrelated matches remain valid.
    async () => !(await titles()).includes(nested),
  );
  await keys(inputId, "\uE009a\uE000");
  await keys(inputId, recreated);
  await until(
    "a recreated root is watched",
    async () => (await titles())[0] === recreated,
  );
  await keys(inputId, "\uE009r\uE000");
  await until(
    "manual refresh remains available",
    async () => (await titles())[0] === recreated,
  );
  pass(
    "File watching detects creation, renaming, new folders, and replacement roots; manual refresh still works",
  );

  await keys(inputId, "\uE00C");
  await until("Escape pauses the hidden launcher", () =>
    observe<boolean>(
      "return document.querySelector('.open-button')?.disabled === true",
    ),
  );
  await rm(resolve(fixtures.fileRoot, recreated));
  const hiddenFile = `Hidden ${fixtures.nonce}.txt`;
  await writeFile(
    resolve(fixtures.fileRoot, hiddenFile),
    "Created while TinyDash is hidden\n",
  );
  await until("the file index updates while the window is hidden", async () => {
    // Inspect the real Rust index without asking the hidden UI to render it.
    const found = await request<string[]>(
      `/session/${session}/execute/async`,
      "POST",
      {
        script: `const done = arguments[arguments.length - 1];
        window.__TAURI_INTERNALS__.invoke('search', { query: arguments[0], mode: 'files' })
          .then(response => done(response.results.map(result => result.title)), () => done([]));`,
        args: [hiddenFile],
      },
    );
    return found.includes(hiddenFile);
  });
  assert(
    (await titles()).includes(recreated),
    "Hidden results stay unchanged after index events",
  );
  await reopen();
  await selectMode("files");
  await keys(inputId, hiddenFile);
  await until(
    "reopening searches the current file index",
    async () => (await titles())[0] === hiddenFile,
  );
  pass(
    "Hidden windows pause result updates; the file index stays current and reopening shows new files",
  );

  await reopen();
  await selectMode("system");
  assert.equal((await titles()).length, 5);
  await keys(inputId, "reboot");
  await until("the system provider resolves the reboot alias", () =>
    observe<boolean>(
      "return document.querySelector('.result-title')?.textContent === 'Restart' && document.querySelector('[role=listbox]')?.getAttribute('aria-busy') === 'false'",
    ),
  );
  await keys(inputId, "\uE007");
  await until("restart asks for confirmation with Cancel selected", () =>
    observe<boolean>(
      "return document.querySelector('dialog[open] h2')?.textContent === 'Restart this computer?' && document.activeElement?.textContent === 'Cancel'",
    ),
  );
  await saveScreen("system-confirmation.png");
  await click("dialog .cancel-button");
  await until("cancel returns to search", () =>
    observe<boolean>(
      "return !document.querySelector('dialog[open]') && document.activeElement?.tagName === 'INPUT'",
    ),
  );
  pass(
    "System search resolves aliases and Cancel closes the native confirmation dialog",
  );
  // Intentionally omit consent. Never send confirmed:true for power actions
  // to a real backend: these tests must not disrupt the host or hosted runner.
  for (const id of ["system:restart", "system:shutdown", "system:sleep"]) {
    const rejection = await request<string>(
      `/session/${session}/execute/async`,
      "POST",
      {
        script: `const done = arguments[arguments.length - 1];
        window.__TAURI_INTERNALS__.invoke('execute_action', { id: arguments[0], action: 'run' })
          .then(() => done('unexpected success'), error => done(String(error)));`,
        args: [id],
      },
    );
    assert.match(rejection, /Confirm this system command before running it/);
  }
  pass(
    "Rust rejects sleep, restart, and shutdown IPC requests without explicit confirmation",
  );
  await reopen();
  const queryTimings: { query: string; elapsedMs: number }[] = [];
  for (let sample = 0; sample < 5; sample += 1) {
    for (const [query, expected] of [
      [expectedNames[0], expectedNames[0]],
      ["12 * 8", "96"],
      ["5 ft to cm", "152.4 cm"],
      [":rocket", "rocket"],
    ]) {
      const result: {
        elapsedMs: number;
        title: string;
        error?: string;
      } = await request(`/session/${session}/execute/async`, "POST", {
        // Measure inside the webview. This includes IPC, Rust search, and
        // Solid's DOM update, but not WebDriver transport or a screen paint.
        script: `const done = arguments[arguments.length - 1];
            const input = document.querySelector('input[role=combobox]');
            const list = document.querySelector('[role=listbox]');
            if (!input || !list || list.getAttribute('aria-busy') !== 'false') {
              done({ error: 'Search was not ready for the timing sample' }); return;
            }
            const started = performance.now();
            const observer = new MutationObserver(() => {
              if (list.getAttribute('aria-busy') !== 'false') return;
              observer.disconnect();
              clearTimeout(timer);
              done({ elapsedMs: performance.now() - started,
                title: list.querySelector('.result-title')?.textContent ?? '' });
            });
            const timer = setTimeout(() => {
              observer.disconnect(); done({ error: 'Search timing timed out' });
            }, 4000);
            observer.observe(list, { attributes: true, attributeFilter: ['aria-busy'] });
            input.value = arguments[0];
            input.dispatchEvent(new Event('input', { bubbles: true }));`,
        args: [query],
      });
      assert(!result.error, result.error);
      assert.equal(result.title, expected);
      assert(Number.isFinite(result.elapsedMs) && result.elapsedMs >= 0);
      queryTimings.push({ query, elapsedMs: result.elapsedMs });
    }
  }
  await writeFile(
    resolve(output, "performance.json"),
    JSON.stringify(
      {
        platform: process.platform,
        startupCheckMs,
        reopenCheckMs,
        queryTimings,
        method:
          "Startup and reopen include WebDriver and readiness polling. Query samples measure input-event dispatch through IPC and Rust search to settled DOM. None measures screen paint or physical shortcut latency. CI timings have no pass/fail threshold.",
      },
      null,
      2,
    ),
  );
  pass(
    "Native query timing samples return the expected app, calculation, unit conversion, and emoji",
  );
  await writeFile(
    resolve(output, "result.json"),
    JSON.stringify({ passed }, null, 2),
  );
} catch (error) {
  process.exitCode = 1;
  console.error(error);
  if (fixtures) {
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
  }
  if (process.platform === "win32" && !cancelled.signal.aborted) {
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
  if (session && !cancelled.signal.aborted) {
    await saveScreen("failure.png").catch(() => {});
    await request<string>(`/session/${session}/source`)
      .then((source) => writeFile(resolve(output, "failure.html"), source))
      .catch(() => {});
  }
} finally {
  const errors: string[] = [];
  const attempt = async (action: () => void | Promise<void>) => {
    try {
      await action();
      return true;
    } catch (error) {
      errors.push(errorDetails(error));
      return false;
    }
  };
  // Windows taskkill needs live roots to stop their descendants. Do not let
  // a WebDriver delete close the application before stopping its whole tree.
  if (session && process.platform !== "win32") {
    await request(
      `/session/${session}`,
      "DELETE",
      undefined,
      5_000,
      false,
    ).catch((error: unknown) =>
      console.error("Cannot close WebDriver session:", error),
    );
  }
  const processes = [application, driver, ...secondInstances].filter(
    (child): child is ChildProcess => child !== undefined,
  );
  const stopped = await attempt(() =>
    cleanupAll(processes.map((child) => () => stopProcessTree(child))),
  );
  for (const descriptor of [log, appLog]) {
    if (descriptor !== undefined) await attempt(() => closeSync(descriptor));
  }
  // A surviving application could overwrite restored settings. Keep the
  // fixtures and recovery evidence until every owned process has stopped.
  if (stopped && cleanupFixtures) await attempt(cleanupFixtures);
  if (errors.length) {
    process.exitCode = 1;
    console.error("Native cleanup failed:", errors.join("\n"));
    await writeFile(resolve(output, "cleanup-failure.txt"), errors.join("\n"));
  }
  if (cancelled.signal.aborted) {
    process.exitCode = 1;
    await writeFile(
      resolve(output, "failure.txt"),
      errorDetails(cancelled.signal.reason),
    );
  }
  try {
    await writeFile(
      resolve(output, "cleanup.json"),
      JSON.stringify(
        {
          complete: errors.length === 0,
          cancelled: cancelled.signal.aborted,
          processes: processes.map((child) => child.pid),
          errors,
        },
        null,
        2,
      ) + "\n",
    );
  } finally {
    cancelled.dispose();
  }
}
