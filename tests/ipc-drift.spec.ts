import { expect, test } from "@playwright/test";
import { spawnSync } from "node:child_process";
import {
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const bridge = readFileSync(resolve(root, "src/bridge.ts"), "utf8");
const wire = readFileSync(resolve(root, "tests/fixtures/ipc-wire.ts"), "utf8");
const checks = readFileSync(resolve(root, "tests/ipc-types.ts"), "utf8");

// Compile isolated copies, never mutate the checkout or reuse stale diagnostics.
// A clean baseline and a contract-specific TS2344 are required for every probe.
const mutations = [
  [
    "optional result addition",
    "bridge",
    "export interface SearchResult {",
    "export interface SearchResult { extra?: string;",
  ],
  [
    "nested optional addition",
    "bridge",
    "    key: string;",
    "    key: string; extra?: boolean;",
  ],
  [
    "nested required becomes optional",
    "bridge",
    "    categories: SearchMode[];",
    "    categories?: SearchMode[];",
  ],
  [
    "nullable narrowing",
    "bridge",
    "lastUsedAt: number | null",
    "lastUsedAt: null",
  ],
  [
    "nullable widening",
    "bridge",
    "lastUsedAt: number | null",
    "lastUsedAt: number | string | null",
  ],
  ["notes narrowing", "bridge", "notes: string | null", "notes: null"],
  [
    "system glass narrowing",
    "bridge",
    "followSystemGlass: boolean | null",
    "followSystemGlass: null",
  ],
  [
    "required nullable becomes optional",
    "bridge",
    "preferredSelectionId: string | null",
    "preferredSelectionId?: string | null",
  ],
  [
    "result enum widening",
    "bridge",
    '| "systemCommand"',
    '| "systemCommand" | "invalid"',
  ],
  ["nested enum narrowing", "bridge", 'type: "cleanedUrl"', 'type: "password"'],
  [
    "success return widening",
    "bridge",
    'invoke<void>("hide_launcher")',
    'invoke<number>("hide_launcher")',
  ],
  [
    "argument enum widening",
    "bridge",
    'appearance: "light" | "dark" | "sage" | "rose" | "ink"',
    'appearance: "light" | "dark" | "sage" | "rose" | "ink" | "invalid"',
  ],
  [
    "argument enum narrowing",
    "bridge",
    'appearance: "light" | "dark" | "sage" | "rose" | "ink"',
    'appearance: "dark"',
  ],
  [
    "argument optionality",
    "bridge",
    "recordShortcut: (recording: boolean)",
    "recordShortcut: (recording?: boolean)",
  ],
  [
    "Rust-derived return drift",
    "wire",
    "hide_launcher: { args: []; result: void }",
    "hide_launcher: { args: []; result: number }",
  ],
  [
    "Rust-derived field drift",
    "wire",
    "lastUsedAt: number | null",
    "lastUsedAt: string | null",
  ],
  [
    "Rust-derived enum drift",
    "wire",
    'export type LauncherAppearance = "light"',
    'export type LauncherAppearance = "invalid" | "light"',
  ],
] as const;

test("exact Rust-derived contracts reject deliberate wire and bridge drift", async ({}, testInfo) => {
  test.setTimeout(120_000);
  mkdirSync(resolve(root, "test-results"), { recursive: true });
  const dir = mkdtempSync(resolve(root, "test-results/ipc-drift-"));
  mkdirSync(resolve(dir, "src"));
  mkdirSync(resolve(dir, "tests/fixtures"), { recursive: true });
  writeFileSync(
    resolve(dir, "src/appearance.ts"),
    readFileSync(resolve(root, "src/appearance.ts"), "utf8"),
  );
  writeFileSync(resolve(dir, "tests/ipc-types.ts"), checks);
  writeFileSync(
    resolve(dir, "tsconfig.json"),
    JSON.stringify({
      compilerOptions: {
        strict: true,
        noEmit: true,
        target: "ES2022",
        module: "ESNext",
        moduleResolution: "Bundler",
        types: [],
      },
      include: ["src", "tests"],
    }),
  );
  const compile = (source: string, canonical: string) => {
    writeFileSync(resolve(dir, "src/bridge.ts"), source);
    writeFileSync(resolve(dir, "tests/fixtures/ipc-wire.ts"), canonical);
    const result = spawnSync(
      "bun",
      [
        resolve(root, "node_modules/typescript/bin/tsc"),
        "--pretty",
        "false",
        "--project",
        resolve(dir, "tsconfig.json"),
      ],
      { encoding: "utf8", timeout: 20_000 },
    );
    expect(result.error).toBeUndefined();
    return { status: result.status, output: result.stdout + result.stderr };
  };
  try {
    expect(compile(bridge, wire)).toEqual({ status: 0, output: "" });
    for (const [name, target, from, to] of mutations) {
      await test.step(name, async () => {
        const original = target === "bridge" ? bridge : wire;
        expect(original.split(from).length, `${name}: unique mutation`).toBe(2);
        const changed = original.replace(from, to);
        const result = compile(
          target === "bridge" ? changed : bridge,
          target === "wire" ? changed : wire,
        );
        await testInfo.attach(name, {
          body: result.output,
          contentType: "text/plain",
        });
        expect(result.status, `${name}: ${result.output}`).not.toBe(0);
        expect(result.output, name).toMatch(
          /ipc-types\.ts\(\d+,\d+\): error TS2344/,
        );
      });
    }
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});
