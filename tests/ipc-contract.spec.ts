import { expect, test } from "@playwright/test";
import { readFileSync } from "node:fs";
import { commandArguments } from "./fixtures/ipc-wire";
import { resolve } from "node:path";
import type {
  Action,
  FilePhase,
  LauncherWarning,
  SearchMode,
  SearchResult,
} from "../src/bridge";
import { contracts } from "./fixtures/ipc-contract";
import { commandWrappers } from "./ipc-types";

// A new TS variant also needs a serialized Rust example, not just a widened union.
type MissingExamples =
  | Exclude<Action, (typeof contracts.actions)[number]>
  | Exclude<FilePhase, (typeof contracts.fileStatuses)[number]["phase"]>
  | Exclude<SearchMode, (typeof contracts.modes)[number]>
  | Exclude<
      SearchResult["kind"],
      (typeof contracts.response.results)[number]["kind"]
    >
  | Exclude<
      NonNullable<SearchResult["detail"]>["type"],
      (typeof contracts.details)[number]["type"]
    >
  | Exclude<
      LauncherWarning["code"],
      (typeof contracts.launcher.warnings)[number]["code"]
    >;
const complete: [MissingExamples] extends [never] ? true : never = true;
const root = resolve(import.meta.dirname, "..");
const rust = (path: string) =>
  readFileSync(resolve(root, "src-tauri/src", path), "utf8");
const camelCase = (name: string) =>
  name.replace(/_([a-z])/g, (_, char: string) => char.toUpperCase());

function variants(path: string, name: string) {
  const body = rust(path).split(`pub enum ${name} {`)[1]?.split("\n}")[0];
  expect(body, `Missing Rust enum ${name}`).toBeDefined();
  return [...body.matchAll(/^    (\w+)(?:,| \{)/gm)]
    .map((match) => match[1][0].toLowerCase() + match[1].slice(1))
    .sort();
}

test("canonical serde fixtures cover variants and representative omitted/nullable values", () => {
  expect(complete).toBe(true);
  expect(contracts.fileStatuses.map((status) => status.phase).sort()).toEqual(
    variants("launcher/files.rs", "FilePhase"),
  );
  expect([...contracts.actions].sort()).toEqual(
    variants("launcher/result.rs", "Action"),
  );
  expect([...contracts.modes].sort()).toEqual(
    variants("launcher/query.rs", "SearchMode"),
  );
  expect(
    contracts.response.results.map((result) => result.kind).sort(),
  ).toEqual(variants("launcher/result.rs", "ResultKind"));
  expect(
    [...new Set(contracts.details.map((detail) => detail.type))].sort(),
  ).toEqual(variants("launcher/result.rs", "ToolDetail"));
  expect(
    contracts.launcher.warnings.map((warning) => warning.code).sort(),
  ).toEqual(variants("launcher/warning.rs", "WarningCode"));
  expect(contracts.response.results[0]).not.toHaveProperty("path");
  expect(contracts.response.results[0]).not.toHaveProperty("detail");
  expect(contracts.response.results[0].icon).toBeNull();
  expect(contracts.fullResult).toHaveProperty(
    "confirmation.confirmLabel",
    "Restart",
  );
  expect(contracts.fullResult.pin.categories).toEqual(contracts.modes);
  expect(contracts.clipboard.lastUsedAt).toBeNull();
  expect(contracts.usedClipboard.lastUsedAt).toBe(2);
  expect(contracts.update.notes).toBe("Release notes");
  expect(contracts.noUpdate.notes).toBeNull();
  expect(contracts.imported.followSystemGlass).toBeNull();
  expect(contracts.importedDefaults.followSystemGlass).toBe(true);
  expect(contracts.defaults.fileSearchRoots).toBeNull();
  expect(contracts.settings.fileSearchRoots).toEqual(["/example"]);
  expect(contracts.warningResponse.storageError).toEqual({
    code: "storageUnavailable",
    message: "Storage unavailable",
    retryable: false,
  });
});

function commands() {
  // Rust's syn-based generator checks this metadata against registered handlers.
  // Do not parse Rust again here: test strings are not command declarations.
  const signatures = new Map<string, { name: string; type: string }[]>(
    Object.entries(commandArguments).map(([command, args]) => [
      command,
      args.map(([name, type]) => ({ name: camelCase(name), type })),
    ]),
  );
  return { names: [...signatures.keys()].sort(), signatures };
}

test("every bridge wrapper invokes a registered Rust command with matching argument names and types", async ({
  page,
}) => {
  const { names, signatures } = commands();
  await page.goto("/");
  const calls = await page.evaluate(async (settings) => {
    const path = "/tests/ipc-probe.ts";
    const { probeCommands } = (await import(
      path
    )) as typeof import("./ipc-probe");
    return probeCommands(settings);
  }, contracts.settings);
  expect([...new Set(calls.map((call) => call.command))].sort()).toEqual(names);
  for (const { command, args, wrapper } of calls) {
    expect(wrapper).toBe(
      commandWrappers[command as keyof typeof commandWrappers],
    );
    const signature = signatures.get(command)!;
    const required = signature.filter((arg) => !arg.type.endsWith(" | null"));
    expect(
      Object.keys(args).filter(
        (name) => !signature.some((arg) => arg.name === name),
      ),
      command,
    ).toEqual([]);
    for (const arg of required) expect(args, command).toHaveProperty(arg.name);
    for (const [name, value] of Object.entries(args)) {
      const type = signature
        .find((arg) => arg.name === name)!
        .type.replace(/ \| null$/, "");
      switch (type) {
        case "string":
          expect(typeof value, `${command}.${name}`).toBe("string");
          break;
        case "boolean":
          expect(typeof value, `${command}.${name}`).toBe("boolean");
          break;
        case "Array<string>":
          expect(
            Array.isArray(value) &&
              value.every((item) => typeof item === "string"),
          ).toBe(true);
          break;
        case "SearchMode":
          expect(contracts.modes).toContain(value);
          break;
        case "Action":
          expect(contracts.actions).toContain(value);
          break;
        case "Settings":
          expect(value).toEqual(contracts.settings);
          break;
        case "WebSearch":
          expect(value).toEqual(contracts.settings.webSearches[0]);
          break;
        case "LauncherAppearance":
          expect(value).toBe("dark");
          break;
        case "AppearanceChange":
          expect(value).toEqual({ kind: "appearance", value: "dark" });
          break;
        case "number":
          expect(Number.isSafeInteger(value) && Number(value) > 0).toBe(true);
          break;
        default:
          throw new Error(`Add coverage for ${command}.${name}: ${type}`);
      }
    }
  }
  expect(
    calls
      .filter((call) => call.command === "execute_action")
      .map((call) => call.args),
  ).toEqual([
    { id: "app:example", action: "launch", confirmed: true },
    { id: "app:example", action: "launch" },
  ]);
});
