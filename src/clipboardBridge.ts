import { invoke } from "@tauri-apps/api/core";

export interface RichClipboardEntry {
  id: number;
  kind: "image" | "files";
  title: string;
  createdAt: number;
  sourceApp: string | null;
  bytes: number;
  pinned: boolean;
  customName: string | null;
}

export interface RichClipboardHistory {
  entries: RichClipboardEntry[];
  total: number;
  sourceApps: string[];
  captureSupported: boolean;
  supportNotice: string;
  storageNotice: string;
}

export interface RichClipboardPreview {
  entry: RichClipboardEntry;
  png: number[] | null;
  files: string[] | null;
}

export const richClipboardBackend = {
  history: (
    query: string,
    kind?: RichClipboardEntry["kind"],
    sourceApp?: string,
    pinnedOnly?: boolean,
  ) =>
    invoke<RichClipboardHistory>("rich_clipboard_history", {
      query,
      kind,
      sourceApp,
      pinnedOnly,
    }),
  preview: (id: number) =>
    invoke<RichClipboardPreview>("rich_clipboard_preview", { id }),
  saveImage: (id: number) =>
    invoke<boolean>("save_rich_clipboard_image", { id }),
  revealFile: (id: number, fileIndex: number) =>
    invoke<void>("reveal_rich_clipboard_file", { id, fileIndex }),
  paste: (id: number) => invoke<void>("paste_rich_clipboard", { id }),
  copy: (id: number) => invoke<void>("copy_rich_clipboard", { id }),
  setPinned: (id: number, pinned: boolean) =>
    invoke<void>("set_rich_clipboard_pinned", { id, pinned }),
  setName: (id: number, name: string) =>
    invoke<void>("set_rich_clipboard_name", { id, name }),
  delete: (id: number) => invoke<void>("delete_rich_clipboard", { id }),
};
