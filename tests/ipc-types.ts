import type * as Bridge from "../src/bridge";
import type * as Wire from "./fixtures/ipc-wire";

// Expand aliases recursively before equality: assignability alone accepts extra
// optional keys, narrower null/enum domains, and nested changes.
type Expand<T> = T extends object ? { [K in keyof T]: Expand<T[K]> } : T;
type Equal<A, B> =
  (<T>() => T extends Expand<A> ? 1 : 2) extends <T>() => T extends Expand<B>
    ? 1
    : 2
    ? true
    : false;
type Assert<T extends true> = T;

export type WireTypesMatch = [
  Assert<Equal<Bridge.Action, Wire.Action>>,
  Assert<Equal<Bridge.SearchMode, Wire.SearchMode>>,
  Assert<Equal<Bridge.SearchResult, Wire.SearchResult>>,
  Assert<Equal<Bridge.SearchResponse, Wire.SearchResponse>>,
  Assert<Equal<Bridge.SettingsValues, Wire.Settings>>,
  Assert<Equal<Bridge.WebSearch, Wire.WebSearch>>,
  Assert<Equal<Bridge.SettingsImport, Wire.SettingsImport>>,
  Assert<Equal<Bridge.ClipboardEntry, Wire.ClipboardEntry>>,
  Assert<Equal<Bridge.UpdateStatus, Wire.UpdateStatus>>,
  Assert<Equal<Bridge.LauncherInfo, Wire.LauncherInfo>>,
  Assert<Equal<Bridge.SettingsInfo, Wire.SettingsInfo>>,
  Assert<Equal<Bridge.LauncherWarning, Wire.LauncherWarning>>,
  Assert<Equal<Bridge.FileStatus, Wire.FileStatus>>,
  Assert<Equal<Bridge.CurrencyStatus, Wire.CurrencyStatus>>,
];

// This only binds wrapper names to commands. Domains and success types come
// from parsed Rust signatures, and the runtime probe verifies these bindings.
export const commandWrappers = {
  set_launcher_appearance: "setLauncherAppearance",
  sync_appearance: "syncAppearance",
  cancel_search: "cancelSearch",
  launcher_ready: "ready",
  open_settings: "openSettings",
  get_settings: "settings",
  save_settings: "saveSettings",
  choose_clipboard_history: "chooseClipboardHistory",
  app_catalog: "appCatalog",
  set_app_preference: "setAppPreference",
  preview_web_search: "previewWebSearch",
  export_settings: "exportSettings",
  preview_settings_import: "importSettings",
  reveal_backup: "revealBackup",
  check_update: "checkUpdate",
  install_update: "installUpdate",
  set_shortcut_recording: "recordShortcut",
  reveal_settings_path: "revealSettings",
  search: "search",
  set_pinned: "setPinned",
  execute_action: "execute",
  clipboard_preview: "clipboardPreview",
  clear_clipboard_history: "clearClipboard",
  edit_clipboard_history: "editClipboardCopy",
  copy_clipboard_selection: "copyClipboardSelection",
  save_clipboard_file: "saveClipboardFile",
  hide_launcher: "hide",
  reset_launcher_position: "resetPosition",
  refresh_apps: "refresh",
  refresh_files: "refreshFiles",
  refresh_currency: "refreshCurrency",
  quit_app: "quit",
} as const satisfies Record<keyof Wire.Commands, keyof typeof Bridge.backend>;

type Wrappers = typeof commandWrappers;
type Actual = {
  [C in keyof Wire.Commands]: {
    args: Parameters<(typeof Bridge.backend)[Wrappers[C]]>;
    result: Awaited<ReturnType<(typeof Bridge.backend)[Wrappers[C]]>>;
  };
};

// The bridge represents Rust Option arguments by omission (not explicit null).
// revealSettings additionally supplies a false default for Rust's required bool.
// No other argument-domain or success-type adaptations are permitted.
type WrapperArgs<T extends unknown[]> = T extends [infer First, ...infer Rest]
  ? null extends First
    ? [arg?: NonNullable<First>, ...WrapperArgs<Rest>]
    : [arg: First, ...WrapperArgs<Rest>]
  : [];
type Expected = {
  [C in keyof Wire.Commands]: {
    args: C extends "reveal_settings_path"
      ? Partial<Wire.Commands[C]["args"]>
      : WrapperArgs<Wire.Commands[C]["args"]>;
    result: Wire.Commands[C]["result"];
  };
};
export type BridgeCommandsMatch = [
  Assert<Equal<keyof typeof Bridge.backend, Wrappers[keyof Wrappers]>>,
  Assert<Equal<Actual, Expected>>,
];
