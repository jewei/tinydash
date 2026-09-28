import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";

// Use Win32's zero-format clear, not an OLE wrapper whose format ownership is
// otherwise an unverified precondition of the application's clear heuristic.
const clearScript = String.raw`
$ErrorActionPreference = 'Stop'
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class TinyDashNativeClipboard {
  [DllImport("user32.dll", SetLastError = true)]
  public static extern bool OpenClipboard(IntPtr window);
  [DllImport("user32.dll", SetLastError = true)]
  public static extern bool EmptyClipboard();
  [DllImport("user32.dll", SetLastError = true)]
  public static extern bool CloseClipboard();
  [DllImport("user32.dll")]
  public static extern int CountClipboardFormats();
  [DllImport("user32.dll")]
  public static extern uint GetClipboardSequenceNumber();
}
'@
if (-not [TinyDashNativeClipboard]::OpenClipboard([IntPtr]::Zero)) {
  throw [System.ComponentModel.Win32Exception]::new([Runtime.InteropServices.Marshal]::GetLastWin32Error())
}
try {
  if (-not [TinyDashNativeClipboard]::EmptyClipboard()) {
    throw [System.ComponentModel.Win32Exception]::new([Runtime.InteropServices.Marshal]::GetLastWin32Error())
  }
  $formats = [TinyDashNativeClipboard]::CountClipboardFormats()
  if ($formats -ne 0) { throw 'Clipboard formats remain after EmptyClipboard' }
  [pscustomobject]@{
    formats = $formats
    sequence = [TinyDashNativeClipboard]::GetClipboardSequenceNumber()
  } | ConvertTo-Json -Compress
} finally {
  if (-not [TinyDashNativeClipboard]::CloseClipboard()) {
    throw [System.ComponentModel.Win32Exception]::new([Runtime.InteropServices.Marshal]::GetLastWin32Error())
  }
}
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
