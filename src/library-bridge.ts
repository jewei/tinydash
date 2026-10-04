import { invoke } from "@tauri-apps/api/core";

export type LibraryKind = "quicklink" | "snippet";
export type LibraryAction = "open" | "copy" | "paste";

export interface LibraryDraft {
  kind: LibraryKind;
  name: string;
  keywords: string;
  content: string;
}

export interface LibraryEntry extends LibraryDraft {
  id: string;
}

/** Metadata only: never index snippet contents or resolved clipboard values. */
export interface LibraryItem {
  id: string;
  kind: LibraryKind;
  name: string;
  keywords: string;
  arguments: string[];
  usesClipboard: boolean;
}

export const libraryBackend = {
  list: (query = "") => invoke<LibraryItem[]>("library_list", { query }),
  get: (id: string) => invoke<LibraryEntry>("library_get", { id }),
  save: (id: string | null, draft: LibraryDraft) =>
    invoke<LibraryEntry>("library_save", { id, draft }),
  delete: (id: string) =>
    invoke<void>("library_delete", { id, confirmed: true }),
  execute: (
    id: string,
    action: LibraryAction,
    arguments_: Record<string, string> = {},
    allowClipboard = false,
  ) =>
    invoke<void>("library_execute", {
      id,
      action,
      arguments: arguments_,
      allowClipboard,
    }),
};
