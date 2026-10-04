import type * as Bridge from "../src/bridge";
import type * as Wire from "./fixtures/ipc-wire";
import type * as Rich from "../src/clipboardBridge";
import type * as Files from "../src/file-actions-bridge";
import type * as Library from "../src/library-bridge";
import type * as Utilities from "../src/utilities-bridge";
import type { ipcBridges } from "./ipc-probe";

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
  Assert<Equal<Bridge.DragOutcome, Wire.DragOutcome>>,
  Assert<Equal<Bridge.SearchMode, Wire.SearchMode>>,
  Assert<Equal<Bridge.SearchResult, Wire.SearchResult>>,
  Assert<Equal<Bridge.SearchResponse, Wire.SearchResponse>>,
  Assert<Equal<Bridge.SettingsValues, Wire.Settings>>,
  Assert<Equal<Bridge.WebSearch, Wire.WebSearch>>,
  Assert<Equal<Bridge.SettingsImport, Wire.SettingsImport>>,
  Assert<Equal<Bridge.ClipboardEntry, Wire.ClipboardEntry>>,
  Assert<Equal<Bridge.PasteQueueAction, Wire.PasteQueueAction>>,
  Assert<Equal<Bridge.PasteQueueStatus, Wire.PasteQueueStatus>>,
  Assert<Equal<Bridge.UpdateStatus, Wire.UpdateStatus>>,
  Assert<Equal<Bridge.LauncherInfo, Wire.LauncherInfo>>,
  Assert<Equal<Bridge.SettingsInfo, Wire.SettingsInfo>>,
  Assert<Equal<Bridge.LauncherWarning, Wire.LauncherWarning>>,
  Assert<Equal<Bridge.FileStatus, Wire.FileStatus>>,
  Assert<Equal<Bridge.CurrencyStatus, Wire.CurrencyStatus>>,
  Assert<Equal<Rich.RichClipboardEntry, Wire.RichEntry>>,
  Assert<Equal<Rich.RichClipboardHistory, Wire.RichHistory>>,
  Assert<Equal<Rich.RichClipboardPreview, Wire.RichPreview>>,
  Assert<Equal<Files.FileAction, Wire.FileAction>>,
  Assert<Equal<Files.FilePreviewData, Wire.FilePreview>>,
  Assert<Equal<Library.LibraryDraft, Wire.LibraryDraft>>,
  Assert<Equal<Library.LibraryEntry, Wire.LibraryEntry>>,
  Assert<Equal<Library.LibraryItem, Wire.LibraryItem>>,
  Assert<Equal<Library.LibraryAction, Wire.LibraryAction>>,
  Assert<Equal<Utilities.UtilityCapabilities, Wire.Capabilities>>,
  Assert<Equal<Utilities.UtilityProcess, Wire.ProcessInfo>>,
  Assert<Equal<Utilities.ProcessConfirmation, Wire.ProcessConfirmation>>,
  Assert<Equal<Utilities.UtilityColor, Wire.Color>>,
  Assert<Equal<Utilities.AwakeStatus, Wire.Status>>,
  Assert<Equal<Utilities.MediaAction, Wire.MediaAction>>,
  Assert<Equal<Utilities.WindowAction, Wire.WindowAction>>,
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
  app_icon: "appIcon",
  cancel_app_icon: "cancelAppIcon",
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
  item_catalog: "itemCatalog",
  paste_result: "paste",
  drag_result: "drag",
  share_result: "share",
  paste_queue: "pasteQueue",
  rich_clipboard_history: "richClipboardHistory",
  rich_clipboard_preview: "richClipboardPreview",
  reveal_rich_clipboard_file: "revealRichClipboardFile",
  save_rich_clipboard_image: "saveRichClipboardImage",
  copy_rich_clipboard: "copyRichClipboard",
  paste_rich_clipboard: "pasteRichClipboard",
  delete_rich_clipboard: "deleteRichClipboard",
  set_rich_clipboard_pinned: "setRichClipboardPinned",
  set_rich_clipboard_name: "setRichClipboardName",
  file_preview: "filePreview",
  execute_file_action: "executeFileAction",
  library_list: "libraryList",
  library_get: "libraryGet",
  library_save: "librarySave",
  library_delete: "libraryDelete",
  library_execute: "libraryExecute",
  utility_capabilities: "utilityCapabilities",
  utility_processes: "utilityProcesses",
  utility_prepare_process: "utilityPrepareProcess",
  utility_prepare_app: "utilityPrepareApp",
  utility_confirm_process: "utilityConfirmProcess",
  utility_cancel_process: "utilityCancelProcess",
  utility_color: "utilityColor",
  utility_copy_color: "utilityCopyColor",
  utility_eyedropper: "utilityEyedropper",
  utility_awake_status: "utilityAwakeStatus",
  utility_set_awake: "utilitySetAwake",
  utility_media: "utilityMedia",
  utility_capture_window: "utilityCaptureWindow",
  utility_window: "utilityWindow",
} as const satisfies Record<keyof Wire.Commands, keyof typeof ipcBridges>;

type Wrappers = typeof commandWrappers;
type Actual = {
  [C in keyof Wire.Commands]: {
    args: Parameters<(typeof ipcBridges)[Wrappers[C]]>;
    result: Awaited<ReturnType<(typeof ipcBridges)[Wrappers[C]]>>;
  };
};

// The bridge represents Rust Option arguments by omission (not explicit null).
// revealSettings additionally supplies a false default for Rust's required bool.
// Feature wrappers also supply confirmation/default fields, group file action
// options, and turn canceled native color sampling into a rejected promise.
type WrapperArgs<T extends unknown[]> = T extends [infer First, ...infer Rest]
  ? null extends First
    ? [arg?: NonNullable<First>, ...WrapperArgs<Rest>]
    : [arg: First, ...WrapperArgs<Rest>]
  : [];
type OptionField<K extends string, T> = null extends T
  ? { [P in K]?: NonNullable<T> }
  : { [P in K]: T };
type FileActionArgs<T> = T extends [
  infer Id,
  infer Action,
  infer App,
  infer Confirmed,
]
  ? [
      id: Id,
      action: Action,
      options?: OptionField<"appId", App> & OptionField<"confirmed", Confirmed>,
    ]
  : never;
type FirstArg<T> = T extends [infer First, boolean] ? [value: First] : never;
type LibraryExecuteArgs<T> = T extends [
  infer Id,
  infer Action,
  infer Arguments,
  infer Clipboard,
]
  ? [id: Id, action: Action, arguments_?: Arguments, allowClipboard?: Clipboard]
  : never;
type Expected = {
  [C in keyof Wire.Commands]: {
    args: C extends
      | "reveal_settings_path"
      | "library_list"
      | "utility_capture_window"
      ? Partial<Wire.Commands[C]["args"]>
      : C extends "library_save"
        ? Wire.Commands[C]["args"]
        : C extends "library_delete" | "utility_confirm_process"
          ? FirstArg<Wire.Commands[C]["args"]>
          : C extends "library_execute"
            ? LibraryExecuteArgs<Wire.Commands[C]["args"]>
            : C extends "execute_file_action"
              ? FileActionArgs<Wire.Commands[C]["args"]>
              : C extends "utility_eyedropper"
                ? Wire.Commands[C]["args"] extends []
                  ? [signal: AbortSignal, native?: boolean]
                  : never
                : WrapperArgs<Wire.Commands[C]["args"]>;
    result: C extends "utility_eyedropper"
      ? NonNullable<Wire.Commands[C]["result"]>
      : Wire.Commands[C]["result"];
  };
};
export type BridgeCommandsMatch = [
  Assert<Equal<keyof typeof ipcBridges, Wrappers[keyof Wrappers]>>,
  Assert<Equal<Actual, Expected>>,
];
