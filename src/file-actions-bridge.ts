import { invoke } from "@tauri-apps/api/core";
import type { SearchResult } from "./bridge";

export type FileAction =
  "copyPath" | "copyFile" | "openWith" | "openTerminal" | "quickLook" | "trash";

export interface FilePreviewData {
  name: string;
  path: string;
  folder: boolean;
  size: number;
  modifiedAt: number | null;
  readonly: boolean;
  content:
    | { type: "text"; text: string; truncated: boolean }
    | { type: "image"; dataUrl: string }
    | { type: "unavailable"; reason: string };
}

// File IPC never accepts a path or executable from the webview.
export const fileBackend = {
  preview: (id: string) => invoke<FilePreviewData>("file_preview", { id }),
  apps: () => invoke<SearchResult[]>("app_catalog"),
  execute: (
    id: string,
    action: FileAction,
    options: { appId?: string; confirmed?: boolean } = {},
  ) => invoke<void>("execute_file_action", { id, action, ...options }),
};
