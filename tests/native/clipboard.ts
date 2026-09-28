import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";

// Clipboard ownership is thread-affine. Keep every Win32 call in one
// synchronous managed frame; run no PowerShell pipeline while it is open.
const clearScript = String.raw`
$ErrorActionPreference = 'Stop'
Add-Type -TypeDefinition @'
using System;
using System.Globalization;
using System.ComponentModel;
using System.Runtime.InteropServices;
public static class TinyDashNativeClipboard {
  [DllImport("user32.dll", SetLastError = true, ExactSpelling = true)]
  private static extern bool OpenClipboard(IntPtr window);
  [DllImport("user32.dll", SetLastError = true, ExactSpelling = true)]
  private static extern bool EmptyClipboard();
  [DllImport("user32.dll", SetLastError = true, ExactSpelling = true)]
  private static extern bool CloseClipboard();
  [DllImport("user32.dll", SetLastError = true, ExactSpelling = true)]
  private static extern int CountClipboardFormats();
  [DllImport("user32.dll", ExactSpelling = true)]
  private static extern uint GetClipboardSequenceNumber();
  [DllImport("kernel32.dll", ExactSpelling = true)]
  private static extern void SetLastError(uint error);

  public static string Clear() {
    if (!OpenClipboard(IntPtr.Zero)) {
      throw new Win32Exception(Marshal.GetLastWin32Error());
    }
    int formats;
    uint sequence;
    try {
      if (!EmptyClipboard()) {
        throw new Win32Exception(Marshal.GetLastWin32Error());
      }
      SetLastError(0);
      formats = CountClipboardFormats();
      int error = Marshal.GetLastWin32Error();
      if (formats == 0 && error != 0) { throw new Win32Exception(error); }
      if (formats != 0) { throw new InvalidOperationException("Clipboard formats remain after EmptyClipboard"); }
      sequence = GetClipboardSequenceNumber();
    } finally {
      if (!CloseClipboard()) {
        throw new Win32Exception(Marshal.GetLastWin32Error());
      }
    }
    // Only construct/emit the acknowledgement after the clipboard is closed.
    return "{\"formats\":" + formats.ToString(CultureInfo.InvariantCulture)
      + ",\"sequence\":" + sequence.ToString(CultureInfo.InvariantCulture) + "}";
  }
}
'@
[TinyDashNativeClipboard]::Clear()
`;

function runWindows(script: string): string {
  assert.equal(process.platform, "win32");
  return execFileSync(
    "powershell.exe",
    ["-NoProfile", "-NonInteractive", "-Command", script],
    { encoding: "utf8", timeout: 5_000, windowsHide: true },
  );
}

export function clearWindowsClipboard(run = runWindows): {
  formats: number;
  sequence: number;
} {
  // A single attempt. Preserve subprocess errors and reject missing/malformed
  // evidence rather than treating an empty text read as a zero-format clear.
  const result: unknown = JSON.parse(run(clearScript));
  assert(result && typeof result === "object");
  const { formats, sequence } = result as Record<string, unknown>;
  assert.equal(formats, 0);
  assert(
    typeof sequence === "number" &&
      Number.isInteger(sequence) &&
      sequence >= 0 &&
      sequence <= 0xffff_ffff,
  );
  return { formats, sequence };
}
