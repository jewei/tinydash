import { expect, test } from "@playwright/test";
import { readFileSync, readdirSync } from "node:fs";
import { resolve } from "node:path";
import type {
  Action,
  LauncherWarning,
  SearchMode,
  SearchResult,
} from "../src/bridge";
import { contracts } from "./fixtures/ipc-contract";

// A new TS variant also needs a serialized Rust example, not just a widened union.
type MissingExamples =
  | Exclude<Action, (typeof contracts.actions)[number]>
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

test("canonical serde fixtures cover every variant and optional wire shape", () => {
  expect(complete).toBe(true);
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
  expect(contracts.defaults.fileSearchRoots).toBeNull();
  expect(contracts.settings.fileSearchRoots).toEqual(["/example"]);
  expect(contracts.warningResponse.storageError).toEqual({
    code: "storageUnavailable",
    message: "Storage unavailable",
    retryable: false,
  });
});

function commands() {
  const registrations = rust("lib.rs")
    .split("tauri::generate_handler![")[1]
    ?.split("]")[0];
  expect(registrations).toBeDefined();
  const names = registrations
    .split(",")
    .map((name) => name.trim().split("::").at(-1)!)
    .filter(Boolean)
    .sort();
  const signatures = new Map<string, { name: string; type: string }[]>();
  const paths = readdirSync(resolve(root, "src-tauri/src/launcher"), {
    recursive: true,
  }).filter((path) => typeof path === "string" && path.endsWith(".rs"));
  for (const path of paths) {
    const source = rust(`launcher/${path}`);
    // These internal commands use simple owned parameters. Fail closed when a
    // new signature is not understood instead of silently skipping the command.
    for (const match of source.matchAll(
      /#\[tauri::command\]\s*pub (?:async )?fn (\w+)\(([^)]*)\)/g,
    )) {
      const args = match[2]
        .split(",")
        .map((arg) => arg.trim())
        .filter(Boolean)
        .map((arg) => {
          const parameter = /^(\w+):\s*(.+)$/.exec(arg);
          if (!parameter)
            throw new Error(`Unrecognized command parameter: ${arg}`);
          return { name: camelCase(parameter[1]), type: parameter[2] };
        })
        .filter((arg) => !["AppHandle", "WebviewWindow"].includes(arg.type));
      signatures.set(match[1], args);
    }
  }
  expect([...signatures.keys()].sort()).toEqual(names);
  return { names, signatures };
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
  for (const { command, args } of calls) {
    const signature = signatures.get(command)!;
    const required = signature.filter((arg) => !arg.type.startsWith("Option<"));
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
        .type.replace(/^Option<(.+)>$/, "$1");
      switch (type) {
        case "String":
          expect(typeof value, `${command}.${name}`).toBe("string");
          break;
        case "bool":
          expect(typeof value, `${command}.${name}`).toBe("boolean");
          break;
        case "Vec<String>":
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
        case "settings::WebSearch":
          expect(value).toEqual(contracts.settings.webSearches[0]);
          break;
        case "LauncherAppearance":
          expect(value).toBe("dark");
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
