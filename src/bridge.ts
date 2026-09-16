import { invoke } from "@tauri-apps/api/core";

export type Action = "launch" | "open" | "reveal" | "copy" | "delete";
export type SearchMode =
  "all" | "apps" | "files" | "emoji" | "calculator" | "clipboard";

export interface FileStatus {
  total: number;
  indexing: boolean;
  warning: string | null;
}

export interface SearchResult {
  id: string;
  kind: "app" | "file" | "emoji" | "calculation" | "clipboard";
  title: string;
  subtitle: string;
  score: number;
  icon: string | null;
  primaryAction: Action;
  secondaryActions: Action[];
}

export interface SearchResponse {
  results: SearchResult[];
  total: number;
  indexing: boolean;
  indexError: string | null;
  notice: string | null;
  storageError: string | null;
  files: FileStatus;
}

export interface LauncherInfo {
  settings: {
    clearQueryOnOpen: boolean;
    hideOnBlur: boolean;
    shortcut: string;
    clipboardHistoryEnabled: boolean;
    clipboardHistoryLimit: number;
    fileSearchRoots: string[] | null;
    fileSearchLimit: number;
    fileSearchExcludedDirs: string[];
  };
  platform: "macos" | "windows" | "linux";
  warnings: string[];
}

export interface ClipboardEntry {
  id: number;
  content: string;
  createdAt: number;
  lastUsedAt: number | null;
}

export const backend = {
  ready: () => invoke<LauncherInfo>("launcher_ready"),
  search: (query: string, mode: SearchMode) =>
    invoke<SearchResponse>("search", { query, mode }),
  execute: (id: string, action: Action) =>
    invoke<void>("execute_action", { id, action }),
  clipboardPreview: (id: string) =>
    invoke<ClipboardEntry>("clipboard_preview", { id }),
  clearClipboard: () => invoke<void>("clear_clipboard_history"),
  hide: () => invoke<void>("hide_launcher"),
  refresh: () => invoke<void>("refresh_apps"),
  refreshFiles: () => invoke<void>("refresh_files"),
  quit: () => invoke<void>("quit_app"),
};
