import { invoke } from "@tauri-apps/api/core";
import type { Appearance } from "./appearance";

export type AppearanceChange =
  | { kind: "appearance"; value: Appearance }
  | { kind: "compact"; value: boolean }
  | { kind: "systemGlass"; value: boolean };

export type Action =
  | "launch"
  | "open"
  | "reveal"
  | "copy"
  | "paste"
  | "delete"
  | "run"
  | "regenerate";
export type SearchMode =
  | "all"
  | "apps"
  | "files"
  | "emoji"
  | "calculator"
  | "clipboard"
  | "system"
  | "password"
  | "timezone"
  | "url"
  | "web";

export type FilePhase = "disabled" | "idle" | "queued" | "scanning" | "failed";

export interface FileStatus {
  total: number;
  phase: FilePhase;
  warning: string | null;
}

export interface CurrencyStatus {
  asOf: string | null;
  refreshing: boolean;
  warning: string | null;
}

export interface SearchResult {
  id: string;
  kind:
    | "app"
    | "file"
    | "folder"
    | "emoji"
    | "calculation"
    | "clipboard"
    | "systemCommand"
    | "password"
    | "timezone"
    | "cleanedUrl"
    | "webSearch";
  title: string;
  subtitle: string;
  path?: string;
  score: number;
  icon: string | null;
  primaryAction: Action;
  secondaryActions: Action[];
  pin?: {
    key: string;
    categories: SearchMode[];
  };
  detail?:
    | {
        type: "password";
        variant: string;
        entropyBits: number;
        strength: string;
      }
    | {
        type: "timezone";
        source: string;
        local: string;
        sourceZone: string;
        targetZone?: string;
        ambiguous: boolean;
      }
    | {
        type: "dateCalculation";
        expression: string;
        basedOn: string;
        result: string;
      }
    | { type: "cleanedUrl"; original: string; removed: number }
    | { type: "webSearch"; engine: string; query: string; url: string };
  confirmation?: {
    title: string;
    description: string;
    confirmLabel: string;
  };
}

export interface LauncherWarning {
  code:
    | "settingsRead"
    | "shortcutRegistration"
    | "shortcutsUnavailable"
    | "clipboardLimited"
    | "trayUnavailable"
    | "storageUnavailable"
    | "clipboardUnavailable";
  message: string;
  retryable: boolean;
}

export interface SearchResponse {
  preferredSelectionId: string | null;
  results: SearchResult[];
  total: number;
  indexing: boolean;
  indexError: string | null;
  notice: string | null;
  storageError: LauncherWarning | null;
  files: FileStatus;
  currency: CurrencyStatus;
}

export interface SettingsValues {
  clearQueryOnOpen: boolean;
  hideOnBlur: boolean;
  shortcut: string;
  categoryShortcuts: { mode: SearchMode; shortcut: string }[];
  startAtLogin: boolean;
  showMenuBarIcon: boolean;
  appPreferences: Record<string, { aliases: string[]; hidden: boolean }>;
  webSearches: WebSearch[];
  itemPreferences: Record<string, ItemPreference>;
  clipboardHistoryEnabled: boolean;
  clipboardHistoryDecided: boolean;
  clipboardHistoryLimit: number;
  clipboardRetentionDays: number;
  clipboardExcludedApps: string[];
  clipboardCaptureImages: boolean;
  clipboardCaptureFiles: boolean;
  fileSearchRoots: string[] | null;
  fileSearchLimit: number;
  fileSearchExcludedDirs: string[];
  fileWatchEnabled: boolean;
  fileSearchIncludeHidden: boolean;
  fileSearchIgnorePatterns: string[];
  currencyRatesEnabled: boolean;
  visibleCategories: SearchMode[];
}

export interface ItemPreference {
  aliases: string[];
  shortcut: string;
  hidden: boolean;
  disabled: boolean;
}

export interface WebSearch {
  name: string;
  keyword: string;
  template: string;
  enabled: boolean;
}

export interface SettingsImport {
  settings: SettingsValues;
  ignoredKeys: string[];
  appearance: string | null;
  compact: boolean | null;
  followSystemGlass: boolean | null;
}

export interface UpdateStatus {
  available: boolean;
  version: string | null;
  notes: string | null;
  message: string;
}

export interface LauncherInfo {
  settings: SettingsValues;
  platform: string;
  warnings: LauncherWarning[];
  visible: boolean;
  initialMode: SearchMode | null;
}

export interface SettingsInfo {
  settings: SettingsValues;
  defaults: SettingsValues;
  platform: LauncherInfo["platform"];
  version: string;
  configPath: string;
  dataPath: string;
  shortcutsAvailable: boolean;
}

export interface ClipboardEntry {
  id: number;
  content: string;
  createdAt: number;
  lastUsedAt: number | null;
}

export const backend = {
  syncAppearance: (change: AppearanceChange) =>
    invoke<void>("sync_appearance", { change }),
  setLauncherAppearance: (
    appearance: "light" | "dark" | "sage" | "rose" | "ink",
  ) => invoke<boolean>("set_launcher_appearance", { appearance }),
  ready: () => invoke<LauncherInfo>("launcher_ready"),
  openSettings: () => invoke<void>("open_settings"),
  settings: () => invoke<SettingsInfo>("get_settings"),
  saveSettings: (settings: SettingsValues) =>
    invoke<SettingsValues>("save_settings", { settings }),
  chooseClipboardHistory: (enabled: boolean) =>
    invoke<SettingsValues>("choose_clipboard_history", { enabled }),
  appCatalog: () => invoke<SearchResult[]>("app_catalog"),
  itemCatalog: () => invoke<SearchResult[]>("item_catalog"),
  paste: (id: string) => invoke<void>("paste_result", { id }),
  setAppPreference: (id: string, aliases: string[], hidden: boolean) =>
    invoke<SettingsValues>("set_app_preference", { id, aliases, hidden }),
  previewWebSearch: (search: WebSearch, query: string) =>
    invoke<string>("preview_web_search", { search, query }),
  exportSettings: (
    appearance: string,
    compact: boolean,
    followSystemGlass: boolean,
  ) =>
    invoke<boolean>("export_settings", {
      appearance,
      compact,
      followSystemGlass,
    }),
  importSettings: () =>
    invoke<SettingsImport | null>("preview_settings_import"),
  revealBackup: () => invoke<void>("reveal_backup"),
  checkUpdate: () => invoke<UpdateStatus>("check_update"),
  installUpdate: () => invoke<void>("install_update"),
  recordShortcut: (recording: boolean) =>
    invoke<void>("set_shortcut_recording", { recording }),
  revealSettings: (data = false) =>
    invoke<void>("reveal_settings_path", { data }),
  search: (query: string, mode: SearchMode, requestId?: number) =>
    invoke<SearchResponse>("search", { query, mode, requestId }),
  cancelSearch: (requestId: number) =>
    invoke<void>("cancel_search", { requestId }),
  setPinned: (id: string, category: SearchMode, pinned: boolean) =>
    invoke<void>("set_pinned", { id, category, pinned }),
  execute: (id: string, action: Action, confirmed = false) =>
    invoke<void>("execute_action", {
      id,
      action,
      ...(confirmed ? { confirmed } : {}),
    }),
  clipboardPreview: (id: string) =>
    invoke<ClipboardEntry>("clipboard_preview", { id }),
  clearClipboard: (keepPinned = false) =>
    invoke<void>("clear_clipboard_history", { keepPinned }),
  editClipboardCopy: (id: string, text: string) =>
    invoke<void>("edit_clipboard_history", { id, text }),
  copyClipboardSelection: (ids: string[], separator: string) =>
    invoke<void>("copy_clipboard_selection", { ids, separator }),
  saveClipboardFile: (id: string) =>
    invoke<boolean>("save_clipboard_file", { id }),
  hide: () => invoke<void>("hide_launcher"),
  resetPosition: () => invoke<void>("reset_launcher_position"),
  refresh: () => invoke<void>("refresh_apps"),
  refreshFiles: () => invoke<void>("refresh_files"),
  refreshCurrency: () => invoke<void>("refresh_currency"),
  quit: () => invoke<void>("quit_app"),
};
