import { execFileSync, spawn, type ChildProcess } from "node:child_process";
import {
  mkdir,
  mkdtemp,
  readFile,
  realpath,
  rm,
  writeFile,
} from "node:fs/promises";
import { tmpdir } from "node:os";
import { delimiter, join, resolve } from "node:path";
import { setTimeout as delay } from "node:timers/promises";
import {
  cleanupAll,
  stopProcessTree,
  waitForExit,
} from "../../scripts/verify/lifecycle.ts";

// Install real, temporary OS app entries. The launcher uses its normal scanner.
export async function installFixtures(
  signal: AbortSignal,
  registerCleanup: (cleanup: () => Promise<void>, directory: string) => void,
) {
  signal.throwIfAborted();
  const temporary = await mkdtemp(join(tmpdir(), "tinydash-native-"));
  registerCleanup(
    () => rm(temporary, { recursive: true, force: true }),
    temporary,
  );
  // Resolve Windows 8.3 temp aliases before comparing paths from native APIs.
  const directory = await realpath(temporary);
  const marker = join(directory, "launched.txt");
  const binary = join(
    directory,
    process.platform === "win32" ? "fixture.exe" : "fixture",
  );
  const nonce = `${process.pid}${Date.now()}`;
  const prefix = `TinyDash Smoke ${nonce}`;
  const fileRoot = join(directory, "files");
  const extension = process.platform === "win32" ? `.tinydash${nonce}` : ".txt";
  const fileName = `Budget notes café ${nonce}${extension}`;
  const filePath = join(fileRoot, fileName);
  const fileMarker = join(fileRoot, `Budget notes café ${nonce}.opened`);
  let association = false;
  let settingsPath: string | undefined;
  let originalSettings: Buffer | undefined;
  let compiler: ChildProcess | undefined;
  const fileAssociation = (remove = false) =>
    execFileSync(
      "powershell.exe",
      [
        "-NoProfile",
        "-NonInteractive",
        "-ExecutionPolicy",
        "Bypass",
        "-File",
        resolve("tests/native/file-association.ps1"),
        "-Extension",
        extension,
        "-Binary",
        binary,
        ...(remove ? ["-Remove"] : []),
      ],
      { timeout: 15_000, stdio: "inherit" },
    );
  let shortcuts: string | undefined;
  const env = { ...process.env };
  const cleanup = async () => {
    if (compiler) {
      if (
        process.platform !== "win32" ||
        (compiler.exitCode === null && compiler.signalCode === null)
      )
        await stopProcessTree(compiler);
      compiler = undefined;
    }
    await cleanupAll([
      () => {
        if (association) {
          fileAssociation(true);
          association = false;
        }
      },
      async () => {
        if (settingsPath) {
          if (originalSettings) await writeFile(settingsPath, originalSettings);
          else await rm(settingsPath, { force: true });
          settingsPath = undefined;
        }
      },
      async () => {
        if (shortcuts) {
          await rm(shortcuts, { recursive: true, force: true });
          shortcuts = undefined;
        }
      },
    ]);
    // Keep this directory and its settings backup if restoration failed.
    // WebView2 can retain profile files briefly after its host process exits.
    // Retry explicitly: Bun does not reliably apply rm's maxRetries option here.
    const deadline = Date.now() + 10_000;
    for (;;) {
      try {
        await rm(directory, { recursive: true, force: true });
        break;
      } catch (error) {
        const code = (error as NodeJS.ErrnoException).code;
        if (
          !["EBUSY", "ENOTEMPTY", "EPERM"].includes(code ?? "") ||
          Date.now() >= deadline
        ) {
          throw error;
        }
        await delay(250);
      }
    }
  };

  try {
    registerCleanup(cleanup, directory);
    signal.throwIfAborted();
    await mkdir(fileRoot, { recursive: true });
    await writeFile(
      filePath,
      `This is a file-search test document. Content-only-${nonce}\n`,
    );
    await writeFile(join(fileRoot, ".hidden.txt"), "Hidden test file\n");
    await mkdir(join(fileRoot, "node_modules"));
    await writeFile(
      join(fileRoot, "node_modules", "excluded.txt"),
      "Excluded test file\n",
    );
    compiler = spawn(
      "rustc",
      ["--edition=2024", "tests/native/fixture.rs", "-o", binary],
      { stdio: "inherit", detached: true, windowsHide: true },
    );
    await waitForExit(compiler, signal, 60_000);
    if (process.platform !== "win32") await stopProcessTree(compiler);
    compiler = undefined;
    signal.throwIfAborted();
    if (process.platform === "win32") {
      fileAssociation();
      association = true;
      shortcuts = execFileSync(
        "powershell.exe",
        [
          "-NoProfile",
          "-NonInteractive",
          "-ExecutionPolicy",
          "Bypass",
          "-File",
          resolve("tests/native/install-shortcuts.ps1"),
          "-Binary",
          binary,
          "-Marker",
          marker,
          "-Prefix",
          prefix,
          "-DirectoryName",
          `TinyDash-Native-${nonce}`,
        ],
        { encoding: "utf8", timeout: 15_000 },
      ).trim();
      if (!process.env.APPDATA)
        throw new Error("APPDATA is required for Windows test settings");
      const config = join(process.env.APPDATA, "dev.tinydash.launcher");
      await mkdir(config, { recursive: true });
      const path = join(config, "settings.json");
      try {
        originalSettings = await readFile(path);
      } catch (error) {
        if ((error as NodeJS.ErrnoException).code !== "ENOENT") throw error;
      }
      const settings = originalSettings
        ? JSON.parse(originalSettings.toString("utf8"))
        : {};
      if (originalSettings)
        await writeFile(
          join(directory, "settings-backup.json"),
          originalSettings,
        );
      settingsPath = path;
      await writeFile(
        path,
        JSON.stringify({
          ...settings,
          fileSearchRoots: [fileRoot],
          fileWatchEnabled: true,
          currencyRatesEnabled: false,
        }),
      );
    } else {
      env.XDG_DATA_HOME = join(directory, "data");
      env.XDG_CONFIG_HOME = join(directory, "config");
      // xdg-open's generic desktop parser cannot resolve a quoted Exec binary.
      // Use a PATH command for this handler, as normal desktop entries do.
      env.PATH = `${directory}${delimiter}${env.PATH ?? ""}`;
      const applications = join(env.XDG_DATA_HOME, "applications");
      await mkdir(applications, { recursive: true });
      const config = join(env.XDG_CONFIG_HOME, "dev.tinydash.launcher");
      await mkdir(config, { recursive: true });
      await writeFile(
        join(config, "settings.json"),
        JSON.stringify({
          fileSearchRoots: [fileRoot],
          fileWatchEnabled: true,
          currencyRatesEnabled: false,
        }),
      );
      // Desktop Exec quoting has two escape layers and treats % as a field code.
      const quote = (value: string) =>
        `"${value
          .replace(/[%]/g, "%%")
          .replace(/[\\"`$]/g, "\\$&")
          .replace(/\\/g, "\\\\")}"`;
      for (const label of ["Alpha", "Beta"]) {
        await writeFile(
          join(applications, `tinydash-native-${label.toLowerCase()}.desktop`),
          `[Desktop Entry]\nType=Application\nName=${prefix} ${label}\nExec=${quote(binary)} ${quote(marker)} ${label}\nTerminal=false\n`,
        );
      }
      await writeFile(
        join(applications, "tinydash-file.desktop"),
        `[Desktop Entry]\nType=Application\nName=TinyDash test document handler\nExec=fixture %f\nMimeType=text/plain;\nNoDisplay=true\nTerminal=false\n`,
      );
      await writeFile(
        join(env.XDG_CONFIG_HOME, "mimeapps.list"),
        "[Default Applications]\ntext/plain=tinydash-file.desktop\n",
      );
      execFileSync("update-desktop-database", [applications], {
        env,
        timeout: 15_000,
      });
    }
    signal.throwIfAborted();
    // Check the OS association before starting the launcher. A missing handler
    // is a fixture failure, separate from a failure in TinyDash's open action.
    if (process.platform === "win32") {
      execFileSync(
        "powershell.exe",
        [
          "-NoProfile",
          "-NonInteractive",
          "-Command",
          "Start-Process -FilePath $env:TINYDASH_TEST_DOCUMENT",
        ],
        { env: { ...env, TINYDASH_TEST_DOCUMENT: filePath }, timeout: 10_000 },
      );
    } else {
      console.log(
        "Document MIME type:",
        execFileSync("xdg-mime", ["query", "filetype", filePath], {
          env,
          encoding: "utf8",
          timeout: 10_000,
        }).trim(),
      );
      console.log(
        "Document handler:",
        execFileSync("xdg-mime", ["query", "default", "text/plain"], {
          env,
          encoding: "utf8",
          timeout: 10_000,
        }).trim(),
      );
      execFileSync("xdg-open", [filePath], {
        env,
        timeout: 10_000,
        stdio: "inherit",
      });
    }
    let opened: string | undefined;
    for (let attempt = 0; attempt < 50; attempt++) {
      signal.throwIfAborted();
      try {
        opened = await readFile(fileMarker, "utf8");
        break;
      } catch (error) {
        if ((error as NodeJS.ErrnoException).code !== "ENOENT") throw error;
      }
      await delay(100, undefined, { signal });
    }
    if (opened !== filePath)
      throw new Error(
        `Document fixture failed: expected ${filePath}, received ${opened ?? "no marker"}`,
      );
    await rm(fileMarker);
    signal.throwIfAborted();
    return {
      directory,
      prefix,
      nonce,
      marker,
      fileRoot,
      fileName,
      filePath,
      fileMarker,
      env,
      cleanup,
    };
  } catch (error) {
    try {
      await cleanup();
    } catch (cleanupError) {
      throw new AggregateError(
        [error, cleanupError],
        `Fixture setup failed. Recovery files: ${directory}`,
      );
    }
    throw error;
  }
}
