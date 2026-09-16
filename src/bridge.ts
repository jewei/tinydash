import { invoke } from "@tauri-apps/api/core";

export type Action = "launch" | "reveal" | "copy";
export type SearchMode = "all" | "apps" | "emoji" | "calculator";

export interface SearchResult {
  id: string;
  kind: "app" | "emoji" | "calculation";
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
}

export interface LauncherInfo {
  settings: {
    clearQueryOnOpen: boolean;
    hideOnBlur: boolean;
    shortcut: string;
  };
  platform: "macos" | "windows" | "linux";
  warnings: string[];
}

export const backend = {
  ready: () => invoke<LauncherInfo>("launcher_ready"),
  search: (query: string, mode: SearchMode) =>
    invoke<SearchResponse>("search", { query, mode }),
  execute: (id: string, action: Action) =>
    invoke<void>("execute_action", { id, action }),
  hide: () => invoke<void>("hide_launcher"),
  refresh: () => invoke<void>("refresh_apps"),
  quit: () => invoke<void>("quit_app"),
};
