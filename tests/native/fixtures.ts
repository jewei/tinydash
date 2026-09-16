import { execFileSync } from "node:child_process";
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

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
  let shortcuts: string | undefined;
  const env = { ...process.env };
  const cleanup = async () => {
    if (shortcuts) await rm(shortcuts, { recursive: true, force: true });
    await rm(directory, {
      recursive: true,
      force: true,
      maxRetries: 5,
      retryDelay: 100,
    });
  };

  try {
    execFileSync(
      "rustc",
      ["--edition=2024", "tests/native/fixture.rs", "-o", binary],
      { stdio: "inherit", timeout: 60_000 },
    );
    if (process.platform === "win32") {
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
    } else {
      env.XDG_DATA_HOME = join(directory, "data");
      env.XDG_CONFIG_HOME = join(directory, "config");
      const applications = join(env.XDG_DATA_HOME, "applications");
      await mkdir(applications, { recursive: true });
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
    }
    return { directory, prefix, nonce, marker, env, cleanup };
  } catch (error) {
    await cleanup();
    throw error;
  }
}
