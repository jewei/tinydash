import { expect, test } from "@playwright/test";
import { clearWindowsClipboard } from "./native/clipboard";

test("native upstream clear requires a zero-format Win32 acknowledgement", () => {
  let calls = 0;
  const result = clearWindowsClipboard((script) => {
    calls += 1;
    expect(script).toContain("::OpenClipboard([IntPtr]::Zero)");
    expect(script).toContain("::EmptyClipboard()");
    expect(script).toContain("::CountClipboardFormats()");
    expect(script).toContain("} finally {");
    expect(script).toContain("::CloseClipboard()");
    expect(script).not.toContain("System.Windows.Forms");
    return '{"formats":0,"sequence":123}';
  });
  expect(result).toEqual({ formats: 0, sequence: 123 });
  expect(calls).toBe(1);
});

test("native upstream clear rejects missing, nonempty, or malformed evidence", () => {
  for (const value of [
    "null",
    "{}",
    '{"formats":1,"sequence":123}',
    '{"formats":0}',
    '{"formats":0,"sequence":-1}',
    '{"formats":0,"sequence":4294967296}',
    '{"formats":0,"sequence":1.5}',
    '{"formats":0,"sequence":"123"}',
    "not JSON",
  ]) {
    let calls = 0;
    expect(() =>
      clearWindowsClipboard(() => {
        calls += 1;
        return value;
      }),
    ).toThrow();
    expect(calls).toBe(1);
  }
});

test("native upstream clear preserves subprocess failure without retry", () => {
  const failure = new Error("fixture subprocess failed");
  let calls = 0;
  let observed: unknown;
  try {
    clearWindowsClipboard(() => {
      calls += 1;
      throw failure;
    });
  } catch (error) {
    observed = error;
  }
  expect(observed).toBe(failure);
  expect(calls).toBe(1);
});
