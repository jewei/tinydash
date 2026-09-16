import { invoke } from "@tauri-apps/api/core";

export type Action = "launch" | "reveal";

export interface SearchResult {
  id: string;
  kind: "app";
  title: string;
  subtitle: string;
  score: number;
  primaryAction: Action;
  secondaryActions: Action[];
}

export interface SearchResponse {
  results: SearchResult[];
  total: number;
  indexing: boolean;
  indexError: string | null;
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
  search: (query: string) => invoke<SearchResponse>("search_apps", { query }),
  execute: (id: string, action: Action) =>
    invoke<void>("execute_action", { id, action }),
  hide: () => invoke<void>("hide_launcher"),
  refresh: () => invoke<void>("refresh_apps"),
  quit: () => invoke<void>("quit_app"),
};
