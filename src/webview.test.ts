import { describe, expect, it } from "vite-plus/test";

// The app's own code: no tests, test helpers, or generated types.
const sources = import.meta.glob<string>(
  ["./**/*.{ts,tsx}", "!./**/*.test.{ts,tsx}", "!./test/**", "!./generated/**"],
  { query: "?raw", import: "default", eager: true },
);

// macOS 12.0, the oldest that TinyDash supports, ships Safari 15.1. The
// build lowers newer syntax for it, but not newer methods, and the types
// (ES2022 and DOM) allow these, which Safari 15.4 added.
const NEWER_THAN_SAFARI_15_1 = [
  /\.at\(/,
  /Object\.hasOwn\(/,
  /structuredClone\(/,
  /randomUUID\(/,
  /supportedValuesOf\(/,
];

describe("app code", () => {
  it("calls no method that the WebView of macOS 12.0 lacks", () => {
    expect(Object.keys(sources)).toContain("./launcher/Launcher.tsx");
    const found = Object.entries(sources).flatMap(([path, text]) =>
      NEWER_THAN_SAFARI_15_1.filter((pattern) => pattern.test(text)).map(
        (pattern) => `${path}: ${pattern.source}`,
      ),
    );
    expect(found).toEqual([]);
  });
});
