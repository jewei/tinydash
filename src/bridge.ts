import { invoke } from "@tauri-apps/api/core";

export type Action = "launch" | "reveal" | "copy" | "delete";
export type SearchMode = "all" | "apps" | "emoji" | "calculator" | "clipboard";

export interface SearchResult {
  id: string;
  kind: "app" | "emoji" | "calculation" | "clipboard";
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
}

export interface LauncherInfo {
  settings: {
    clearQueryOnOpen: boolean;
    hideOnBlur: boolean;
    shortcut: string;
    clipboardHistoryEnabled: boolean;
    clipboardHistoryLimit: number;
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
  quit: () => invoke<void>("quit_app"),
};
