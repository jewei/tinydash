import { describe, expect, it } from "vite-plus/test";

// The app's own code: no tests, test helpers, or generated types.
const sources = import.meta.glob<string>(
  ["./**/*.{ts,tsx}", "!./**/*.test.{ts,tsx}", "!./test/**", "!./generated/**"],
  { query: "?raw", import: "default", eager: true },
);

// macOS 12.0, the oldest that TinyDash supports, ships Safari 15.1. The
// build lowers most newer syntax for it, but not a regex lookbehind or a
// newer method, and the types (ES2022 and DOM) allow all of these. The list
// is not complete: add each one that you find.
const NEWER_THAN_SAFARI_15_1 = [
  // Safari 15.4
  /\.at\(/,
  /Object\.hasOwn\(/,
  /structuredClone\(/,
  /randomUUID\(/,
  /supportedValuesOf\(/,
  /\.showModal\(/,
  // Safari 16 and later, or no Safari yet
  /AbortSignal\.timeout\(/,
  /\(\?<[=!]/,
  /\.showPopover\(/,
  /\.togglePopover\(/,
  /\.checkVisibility\(/,
  /startViewTransition\(/,
  /requestIdleCallback\(/,
];

describe("app code", () => {
  it("uses nothing that the WebView of macOS 12.0 lacks", () => {
    expect(Object.keys(sources)).toContain("./launcher/Launcher.tsx");
    const found = Object.entries(sources).flatMap(([path, text]) =>
      NEWER_THAN_SAFARI_15_1.filter((pattern) => pattern.test(text)).map(
        (pattern) => `${path}: ${pattern.source}`,
      ),
    );
    expect(found).toEqual([]);
  });
});
