import assert from "node:assert/strict";
import { execFileSync, spawn, type ChildProcess } from "node:child_process";
import { openSync, closeSync } from "node:fs";
import {
  access,
  mkdir,
  readFile,
  readdir,
  rm,
  writeFile,
} from "node:fs/promises";
import { createServer } from "node:https";
import { homedir } from "node:os";
import { basename, join, resolve } from "node:path";
import { setTimeout as delay } from "node:timers/promises";
import { chromium, type Browser, type Page } from "@playwright/test";
import { fileHash, sourceIdentity } from "../../scripts/verify/identity.ts";
import {
  cancellation,
  stopProcessTree,
} from "../../scripts/verify/lifecycle.ts";
import {
  readUpgradeData,
  seedUpgradeData,
  upgradeText,
  writeUpgradeSettings,
} from "./upgrade-data.ts";

assert.equal(
  process.env.RUNNER_ENVIRONMENT,
  "github-hosted",
  "Version upgrade tests require a disposable hosted runner.",
);
assert(["win32", "linux"].includes(process.platform));
const windows = process.platform === "win32";
const output = resolve("test-results/upgrade");
const tag = process.env.TINYDASH_UPGRADE_TAG;
assert(tag && /^v\d+\.\d+\.\d+$/.test(tag), "Set TINYDASH_UPGRADE_TAG.");
const targetVersion = tag.slice(1);
assert.notEqual(targetVersion, "0.0.0");
const candidateCommit = execFileSync("git", ["rev-parse", `${tag}^{commit}`], {
  encoding: "utf8",
}).trim();
const config = windows
  ? join(process.env.APPDATA!, "dev.tinydash.launcher")
  : join(homedir(), ".config/dev.tinydash.launcher");
const data = windows
  ? config
  : join(homedir(), ".local/share/dev.tinydash.launcher");
const binary = windows
  ? join(process.env.LOCALAPPDATA!, "TinyDash/tinydash.exe")
  : "/usr/bin/tinydash";
const suffix = windows ? "-setup.exe" : ".deb";
const packageDir = resolve("native-build");
const olderDir = resolve(
  `src-tauri/target/release/bundle/${windows ? "nsis" : "deb"}`,
);
const cancelled = cancellation();
const children = new Set<ChildProcess>();
const passed: string[] = [];
const cleanupErrors: string[] = [];
let ownedProfile = false;
let installed = false;
let browser: Browser | undefined;
let page: Page | undefined;
let app: ChildProcess | undefined;
let driver: ChildProcess | undefined;
let session: string | undefined;
let feed: ReturnType<typeof createServer> | undefined;
let phase = "setup";
const requests: { phase: string; url: string }[] = [];
let failure: string | undefined;
let before: Awaited<ReturnType<typeof readUpgradeData>> | undefined;
let targetHash: string | undefined;
let installedHash: string | undefined;

await mkdir(output, { recursive: true });
const log = openSync(join(output, "processes.log"), "w");
const proof = {
  testSource: await sourceIdentity(),
  candidateCommit,
  tag,
  platform: process.platform,
};
const progress = setInterval(() => {
  console.log(JSON.stringify({ phase, passed, failure, cleanupErrors }));
}, 30_000);

function run(command: string, args: string[]) {
  cancelled.signal.throwIfAborted();
  return execFileSync(command, args, {
    encoding: "utf8",
    timeout: 180_000,
    windowsHide: true,
  });
}

function launch(command: string, args: string[], env = process.env) {
  const child = spawn(command, args, {
    env,
    stdio: ["ignore", log, log],
    windowsHide: true,
    detached: !windows,
  });
  children.add(child);
  child.on("error", (error) => {
    failure ??= error.message;
  });
  return child;
}

async function wait(
  label: string,
  check: () => Promise<boolean>,
  timeout = 30_000,
) {
  const deadline = Date.now() + timeout;
  let error: unknown;
  while (Date.now() < deadline) {
    cancelled.signal.throwIfAborted();
    try {
      if (await check()) return;
    } catch (cause) {
      error = cause;
    }
    await delay(100, undefined, { signal: cancelled.signal });
  }
  throw new Error(`Timed out: ${label}`, { cause: error });
}

async function exists(path: string) {
  try {
    await access(path);
    return true;
  } catch {
    return false;
  }
}

async function onlyPackage(directory: string) {
  const files = (await readdir(directory)).filter((name) =>
    name.endsWith(suffix),
  );
  assert.equal(files.length, 1, `Expected one installer in ${directory}`);
  return join(directory, files[0]);
}

function install(path: string) {
  if (windows) {
    // Pass paths as positional arguments, never as PowerShell source text.
    run("powershell.exe", [
      "-NoProfile",
      "-File",
      "tests/native/upgrade-installer.ps1",
      "Install",
      path,
    ]);
  } else run("sudo", ["apt-get", "install", "-y", path]);
  installed = true;
}

async function wd<T>(path: string, method = "GET", body?: unknown): Promise<T> {
  const response = await fetch(`http://127.0.0.1:18444${path}`, {
    method,
    headers: { "Content-Type": "application/json" },
    body: body === undefined ? undefined : JSON.stringify(body),
    signal: AbortSignal.any([cancelled.signal, AbortSignal.timeout(30_000)]),
  });
  const result = (await response.json()) as {
    value: T & { error?: string; message?: string };
  };
  assert(
    response.ok && !result.value?.error,
    result.value?.message ?? response.statusText,
  );
  return result.value;
}

async function text() {
  return windows
    ? page!.locator("body").innerText()
    : wd<string>(`/session/${session}/execute/sync`, "POST", {
        script: "return document.body.innerText",
        args: [],
      });
}

async function click(name: string) {
  console.log(`Click: ${name}`);
  if (windows) {
    await page!.getByRole("button", { name, exact: true }).click();
    return;
  }
  const element = await wd<Record<string, string>>(
    `/session/${session}/element`,
    "POST",
    {
      using: "xpath",
      value: `//button[normalize-space(.)='${name}']`,
    },
  );
  await wd(
    `/session/${session}/element/${element["element-6066-11e4-a52e-4f735466cecf"]}/click`,
    "POST",
    {},
  );
}

async function start() {
  console.log("Start installed app");
  if (windows) {
    const port = process.env.TINYDASH_NATIVE_DEBUG_PORT;
    assert(
      port,
      "The hosted Windows policy must select a loopback debug port.",
    );
    app = launch(binary, [], {
      ...process.env,
      WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${port}`,
    });
    await wait("WebView2 starts", async () => {
      const response = await fetch(`http://127.0.0.1:${port}/json/version`, {
        signal: AbortSignal.timeout(1_000),
      });
      return response.ok;
    });
    browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`);
    for (const context of browser.contexts()) {
      context.setDefaultTimeout(15_000);
      context.setDefaultNavigationTimeout(15_000);
    }
    await wait("launcher page", async () => {
      page = browser!
        .contexts()
        .flatMap((context) => context.pages())
        .find((candidate) => !candidate.url().includes("view=settings"));
      return (
        !!page &&
        (await page
          .getByRole("combobox", { name: "Search TinyDash" })
          .isVisible())
      );
    });
  } else {
    driver = launch("tauri-driver", [
      "--port",
      "18444",
      "--native-port",
      "18445",
    ]);
    await wait(
      "native driver",
      async () => (await wd<{ ready: boolean }>("/status")).ready,
    );
    session = (
      await wd<{ sessionId: string }>("/session", "POST", {
        capabilities: {
          alwaysMatch: { "tauri:options": { application: binary } },
        },
      })
    ).sessionId;
    assert(session);
    await wait("launcher opens", async () =>
      (await text()).includes("Clipboard"),
    );
  }
  await wait("the installed app creates its database", () =>
    exists(join(data, "tinydash.sqlite3")),
  );
}

async function stop() {
  console.log("Stop owned processes");
  if (windows && app?.pid) {
    const record = join(output, `process-${app.pid}.json`);
    if (app.exitCode === null && app.signalCode === null) {
      execFileSync(
        "powershell.exe",
        [
          "-NoProfile",
          "-File",
          "tests/native/upgrade-processes.ps1",
          "Record",
          String(app.pid),
          record,
        ],
        { windowsHide: true, timeout: 15_000 },
      );
    }
    execFileSync(
      "powershell.exe",
      [
        "-NoProfile",
        "-File",
        "tests/native/upgrade-processes.ps1",
        "Stop",
        String(app.pid),
        record,
      ],
      { windowsHide: true, timeout: 30_000 },
    );
    children.delete(app);
    app = undefined;
  }
  if (browser) {
    await browser.close().catch(() => {});
    browser = undefined;
    page = undefined;
  }
  if (session) {
    await wd(`/session/${session}`, "DELETE").catch(() => {});
    session = undefined;
  }
  if (app) {
    await stopProcessTree(app);
    children.delete(app);
    app = undefined;
  }
  if (driver) {
    await stopProcessTree(driver);
    children.delete(driver);
    driver = undefined;
  }
}

async function checkFixture(label: string) {
  await click("Clipboard");
  await wait("saved clipboard text appears", async () =>
    (await text()).includes(upgradeText),
  );
  if (windows) {
    await page!.getByLabel("Pinned to Clipboard", { exact: true }).waitFor();
    await page!.screenshot({ path: join(output, `${label}.png`) });
  } else {
    assert(
      await wd<boolean>(`/session/${session}/execute/sync`, "POST", {
        script:
          "return !!document.querySelector('[aria-label=\"Pinned to Clipboard\"]')",
        args: [],
      }),
    );
    run("scrot", ["--focused", "--overwrite", join(output, `${label}.png`)]);
  }
  passed.push(
    `${label}: the installed UI shows the saved clipboard entry and pin`,
  );
}

async function about() {
  const second = launch(binary, ["--settings"]);
  await wait("settings window", async () => {
    page = browser!
      .contexts()
      .flatMap((context) => context.pages())
      .find((candidate) => candidate.url().includes("view=settings"));
    return (
      !!page &&
      (await page
        .getByRole("button", { name: "About", exact: true })
        .isVisible())
    );
  });
  await click("About");
  await wait("About version", async () => (await text()).includes("Version "));
  await wait("second instance exits", async () => second.exitCode !== null);
  children.delete(second);
}

async function checkFeed(expected: string) {
  const count = requests.length;
  await click("Check for updates");
  await wait(
    expected,
    async () =>
      requests.length > count &&
      (await text()).includes(expected) &&
      (await page!
        .getByRole("button", { name: "Check for updates", exact: true })
        .isEnabled()),
  );
}

try {
  assert(!(await exists(binary)), "An installed TinyDash app already exists.");
  assert(
    !(await exists(config)) && !(await exists(data)),
    "The test profile must be new.",
  );
  const target = await onlyPackage(packageDir);
  const older = await onlyPackage(olderDir);
  const metadata = JSON.parse(
    await readFile(join(packageDir, "build.json"), "utf8"),
  );
  assert.equal(
    metadata.source.commit,
    candidateCommit,
    "The candidate tag and package source differ.",
  );
  assert(basename(target).includes(`_${targetVersion}_`));
  assert(basename(older).includes("_0.0.0_"));
  const extracted = resolve(".local/upgrade/target");
  if (windows)
    run("7z", ["x", target, `-o${extracted}`, "-y", "tinydash.exe", "-r"]);
  else run("dpkg-deb", ["--extract", target, extracted]);
  targetHash = await fileHash(
    join(extracted, windows ? "tinydash.exe" : "usr/bin/tinydash"),
  );
  phase = "install-older";
  install(older);
  await mkdir(config, { recursive: true });
  await mkdir(data, { recursive: true });
  ownedProfile = true;
  await writeUpgradeSettings(config);
  await start();
  await stop();
  seedUpgradeData(data);
  before = await readUpgradeData(config, data);
  assert.equal(before.clipboard.length, 1);
  assert.equal(before.pins.length, 2);
  assert.equal(before.usage.length, 1);
  await writeFile(join(output, "before.json"), JSON.stringify(before, null, 2));
  const olderHash = await fileHash(binary);
  assert.notEqual(olderHash, targetHash);
  phase = "before-upgrade";
  await start();
  await checkFixture("before-upgrade");

  if (windows) {
    const payload = await readFile(target);
    const changed = Buffer.from(payload);
    changed[Math.floor(changed.length / 2)] ^= 1;
    const signature = (await readFile(`${target}.sig`, "utf8")).trim();
    feed = createServer(
      {
        key: await readFile(".local/upgrade/localhost.key"),
        cert: await readFile(".local/upgrade/localhost.crt"),
      },
      (request, response) => {
        requests.push({ phase, url: request.url ?? "" });
        if (phase === "offline") {
          request.socket.destroy();
          return;
        }
        if (request.url === "/latest.json") {
          response.setHeader("Content-Type", "application/json");
          response.end(
            phase === "invalid-metadata"
              ? "invalid JSON"
              : JSON.stringify({
                  version: phase === "same-version" ? "0.0.0" : targetVersion,
                  notes: "Disposable local update test",
                  platforms: {
                    "windows-x86_64": {
                      url: "https://localhost:18443/package",
                      signature:
                        phase === "invalid-signature"
                          ? "invalid signature"
                          : signature,
                    },
                  },
                }),
          );
        } else if (request.url === "/package") {
          if (phase === "missing-asset") {
            response.writeHead(404).end();
            return;
          }
          response.setHeader("Content-Length", payload.length);
          if (phase === "interrupted-download") {
            response.write(payload.subarray(0, Math.floor(payload.length / 2)));
            setTimeout(() => response.destroy(), 100);
          } else response.end(phase === "changed-bytes" ? changed : payload);
        } else response.writeHead(404).end();
      },
    );
    await new Promise<void>((done, reject) => {
      feed!.once("error", reject);
      feed!.listen(18443, "localhost", done);
    });
    await about();
    assert((await text()).includes("Version 0.0.0"));
    for (phase of ["same-version", "offline", "invalid-metadata"]) {
      await checkFeed(
        phase === "same-version"
          ? "TinyDash is up to date."
          : "Update check failed:",
      );
      assert.equal(await fileHash(binary), olderHash);
      passed.push(`${phase}: no app replacement`);
    }
    for (phase of [
      "missing-asset",
      "invalid-signature",
      "changed-bytes",
      "interrupted-download",
    ]) {
      await checkFeed(`TinyDash ${targetVersion} is ready to install.`);
      await click(`Install ${targetVersion}`);
      await wait(
        "failed update leaves the app usable",
        async () =>
          (await text()).includes("Update download failed:") &&
          (await page!
            .getByRole("button", { name: "Check for updates", exact: true })
            .isEnabled()),
      );
      assert.equal(await fileHash(binary), olderHash);
      assert.deepEqual(await readUpgradeData(config, data), before);
      await page!.screenshot({ path: join(output, `${phase}.png`) });
      passed.push(
        `${phase}: error displayed; executable and saved data unchanged`,
      );
    }
    phase = "valid-update";
    await checkFeed(`TinyDash ${targetVersion} is ready to install.`);
    assert.equal(
      await fileHash(binary),
      olderHash,
      "Checking alone must not install.",
    );
    await page!.screenshot({ path: join(output, "update-ready.png") });
    assert(app?.pid);
    run("powershell.exe", [
      "-NoProfile",
      "-File",
      "tests/native/upgrade-processes.ps1",
      "Record",
      String(app.pid),
      join(output, `process-${app.pid}.json`),
    ]);
    await click(`Install ${targetVersion}`).catch(() => {});
    await wait(
      "the signed candidate replaces the older executable",
      async () => (await fileHash(binary)) === targetHash,
      120_000,
    );
    await wait(
      "the installer records the candidate version",
      async () =>
        run("powershell.exe", [
          "-NoProfile",
          "-File",
          "tests/native/upgrade-installer.ps1",
          "Version",
        ]).trim() === targetVersion,
    );
    passed.push(
      "The Settings install action downloaded and installed the exact signed candidate",
    );
    await stop();
  } else {
    await stop();
    install(target);
    assert.equal(
      run("dpkg-query", ["-W", "-f=${Version}", "tiny-dash"]),
      targetVersion,
    );
    passed.push(
      "APT replaced the older installed version with the exact candidate package",
    );
  }
  installedHash = await fileHash(binary);
  assert.equal(installedHash, targetHash);
  assert.deepEqual(await readUpgradeData(config, data), before);
  phase = "after-upgrade";
  await start();
  await checkFixture("after-upgrade");
  if (windows) {
    await about();
    assert((await text()).includes(`Version ${targetVersion}`));
    await page!.screenshot({ path: join(output, "installed-version.png") });
  }
  await stop();
  const after = await readUpgradeData(config, data);
  assert.deepEqual(after, before);
  await writeFile(join(output, "after.json"), JSON.stringify(after, null, 2));
  passed.push(
    "Settings, clipboard consent, clipboard text, both pins, and usage history survive update and restart",
  );
} catch (error) {
  failure = String(error);
  if (page) {
    await page
      .screenshot({ path: join(output, "failure.png"), timeout: 5_000 })
      .catch(() => {});
    await page
      .content()
      .then((html) => writeFile(join(output, "failure.html"), html))
      .catch(() => {});
  }
} finally {
  try {
    await stop();
  } catch (error) {
    cleanupErrors.push(String(error));
  }
  for (const child of children) {
    try {
      await stopProcessTree(child);
    } catch (error) {
      cleanupErrors.push(String(error));
    }
  }
  feed?.closeAllConnections();
  if (feed) await new Promise<void>((done) => feed!.close(() => done()));
  if (installed) {
    try {
      // Cleanup must also run after cancellation.
      if (windows)
        execFileSync(
          "powershell.exe",
          [
            "-NoProfile",
            "-File",
            "tests/native/upgrade-installer.ps1",
            "Remove",
          ],
          { timeout: 180_000, windowsHide: true, stdio: ["ignore", log, log] },
        );
      else
        execFileSync("sudo", ["apt-get", "remove", "-y", "tiny-dash"], {
          timeout: 180_000,
          stdio: ["ignore", log, log],
        });
      assert(!(await exists(binary)), "Removal left the executable installed.");
      if (before) assert.deepEqual(await readUpgradeData(config, data), before);
      passed.push("Removal deletes the app and retains the saved user data");
    } catch (error) {
      cleanupErrors.push(String(error));
    }
  }
  if (ownedProfile && cleanupErrors.length === 0) {
    await rm(config, { recursive: true });
    if (data !== config) await rm(data, { recursive: true });
  }
  closeSync(log);
  clearInterval(progress);
  cancelled.dispose();
  await writeFile(
    join(output, "result.json"),
    JSON.stringify(
      {
        ...proof,
        phase,
        targetHash,
        installedHash,
        passed,
        requests,
        failure,
        cleanupComplete: cleanupErrors.length === 0,
        cleanupErrors,
      },
      null,
      2,
    ) + "\n",
  );
}
console.log(JSON.stringify({ passed, failure, cleanupErrors }, null, 2));
if (failure || cleanupErrors.length) process.exitCode = 1;
