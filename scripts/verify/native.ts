import { resolve } from "node:path";

export function nativeTestBinary(): string {
  if (process.platform !== "win32" && process.platform !== "linux")
    throw new Error(
      "Native WebDriver checks require Windows or Linux. Use docs/how-to/desktop-checks.md on macOS.",
    );
  return resolve(
    process.env.TINYDASH_NATIVE_BINARY ??
      `src-tauri/target/release/tinydash${process.platform === "win32" ? ".exe" : ""}`,
  );
}
