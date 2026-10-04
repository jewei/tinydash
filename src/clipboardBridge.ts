import { invoke } from "@tauri-apps/api/core";

export interface RichClipboardEntry {
  id: number;
  kind: "image" | "files";
  title: string;
  createdAt: number;
  sourceApp: string | null;
  bytes: number;
  pinned: boolean;
}

export interface RichClipboardHistory {
  entries: RichClipboardEntry[];
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
  history: () => invoke<RichClipboardHistory>("rich_clipboard_history"),
  preview: (id: number) =>
    invoke<RichClipboardPreview>("rich_clipboard_preview", { id }),
  paste: (id: number) => invoke<void>("paste_rich_clipboard", { id }),
  copy: (id: number) => invoke<void>("copy_rich_clipboard", { id }),
  setPinned: (id: number, pinned: boolean) =>
    invoke<void>("set_rich_clipboard_pinned", { id, pinned }),
  delete: (id: number) => invoke<void>("delete_rich_clipboard", { id }),
};
