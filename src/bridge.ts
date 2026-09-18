import { invoke } from "@tauri-apps/api/core";

export type Action =
  "launch" | "open" | "reveal" | "copy" | "delete" | "run" | "regenerate";
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

export interface FileStatus {
  total: number;
  indexing: boolean;
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
  } | null;
}

export interface SearchResponse {
  results: SearchResult[];
  total: number;
  indexing: boolean;
  indexError: string | null;
  notice: string | null;
  storageError: string | null;
  files: FileStatus;
  currency: CurrencyStatus;
}

export interface SettingsValues {
  clearQueryOnOpen: boolean;
  hideOnBlur: boolean;
  shortcut: string;
  clipboardHistoryEnabled: boolean;
  clipboardHistoryLimit: number;
  fileSearchRoots: string[] | null;
  fileSearchLimit: number;
  fileSearchExcludedDirs: string[];
  fileWatchEnabled: boolean;
  currencyRatesEnabled: boolean;
  visibleCategories: SearchMode[];
}

export interface LauncherInfo {
  settings: SettingsValues;
  platform: "macos" | "windows" | "linux";
  warnings: string[];
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
  ready: () => invoke<LauncherInfo>("launcher_ready"),
  openSettings: () => invoke<void>("open_settings"),
  settings: () => invoke<SettingsInfo>("get_settings"),
  saveSettings: (settings: SettingsValues) =>
    invoke<SettingsValues>("save_settings", { settings }),
  recordShortcut: (recording: boolean) =>
    invoke<void>("set_shortcut_recording", { recording }),
  revealSettings: (data = false) =>
    invoke<void>("reveal_settings_path", { data }),
  search: (query: string, mode: SearchMode) =>
    invoke<SearchResponse>("search", { query, mode }),
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
  clearClipboard: () => invoke<void>("clear_clipboard_history"),
  hide: () => invoke<void>("hide_launcher"),
  refresh: () => invoke<void>("refresh_apps"),
  refreshFiles: () => invoke<void>("refresh_files"),
  refreshCurrency: () => invoke<void>("refresh_currency"),
  quit: () => invoke<void>("quit_app"),
};
