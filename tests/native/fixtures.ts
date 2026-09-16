import { execFileSync } from "node:child_process";
import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { setTimeout as delay } from "node:timers/promises";

// Install real, temporary OS app entries. The launcher uses its normal scanner.
export async function installFixtures() {
  const directory = await mkdtemp(join(tmpdir(), "tinydash-native-"));
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
    if (association) fileAssociation(true);
    if (settingsPath) {
      if (originalSettings) await writeFile(settingsPath, originalSettings);
      else await rm(settingsPath, { force: true });
    }
    if (shortcuts) await rm(shortcuts, { recursive: true, force: true });
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
    execFileSync(
      "rustc",
      ["--edition=2024", "tests/native/fixture.rs", "-o", binary],
      { stdio: "inherit", timeout: 60_000 },
    );
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
      settingsPath = path;
      await writeFile(
        path,
        JSON.stringify({ ...settings, fileSearchRoots: [fileRoot] }),
      );
    } else {
      env.XDG_DATA_HOME = join(directory, "data");
      env.XDG_CONFIG_HOME = join(directory, "config");
      const applications = join(env.XDG_DATA_HOME, "applications");
      await mkdir(applications, { recursive: true });
      const config = join(env.XDG_CONFIG_HOME, "dev.tinydash.launcher");
      await mkdir(config, { recursive: true });
      await writeFile(
        join(config, "settings.json"),
        JSON.stringify({ fileSearchRoots: [fileRoot] }),
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
        `[Desktop Entry]\nType=Application\nName=TinyDash test document handler\nExec=${quote(binary)} %f\nMimeType=text/plain;\nNoDisplay=true\nTerminal=false\n`,
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
    await cleanup();
    throw error;
  }
}
