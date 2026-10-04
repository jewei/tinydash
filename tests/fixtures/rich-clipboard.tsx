import { render } from "@solidjs/web";
import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import RichClipboardHistory from "../../src/components/RichClipboardHistory";
import type { RichClipboardEntry } from "../../src/clipboardBridge";

declare global {
  interface Window {
    __richClipboardTest: {
      calls: string[];
      missing: boolean;
      closed: boolean;
      entries: RichClipboardEntry[];
      previewError: boolean;
      pinError: boolean;
      nameError: boolean;
      namedEntries: { id: number; name: string }[];
      supported: boolean;
      pasteError: string | null;
      holdPaste: boolean;
      releasePaste?: () => void;
      pastedIds: number[];
      copiedIds: number[];
      revealedFiles: { id: number; fileIndex: number }[];
      revealError: string | null;
      holdReveal: boolean;
      releaseReveal?: () => void;
      filesById: Record<number, string[]>;
      historyError: string | null;
      holdHistory: boolean;
      releaseHistory?: () => void;
      historyQueries: string[];
      holdPreview: boolean;
      releasePreview?: () => void;
    };
  }
}

const entries: RichClipboardEntry[] = [
  {
    id: 1,
    kind: "files",
    title: "2 file references",
    createdAt: 100,
    sourceApp: "com.apple.finder",
    bytes: 64,
    pinned: false,
    customName: null,
  },
];
window.__richClipboardTest = {
  calls: [],
  missing: false,
  closed: false,
  entries,
  previewError: false,
  pinError: false,
  nameError: false,
  namedEntries: [],
  supported: true,
  pasteError: null,
  holdPaste: false,
  pastedIds: [],
  copiedIds: [],
  revealedFiles: [],
  revealError: null,
  holdReveal: false,
  filesById: { 1: ["/fixtures/report.pdf", "/fixtures/design.png"] },
  historyError: null,
  holdHistory: false,
  historyQueries: [],
  holdPreview: false,
};
mockWindows("main");
mockIPC(
  async (command, args) => {
    const state = window.__richClipboardTest;
    const id = args && "id" in args ? args.id : undefined;
    window.__richClipboardTest.calls.push(command);
    if (command === "rich_clipboard_history") {
      const query = String(args && "query" in args ? args.query : "");
      const kind = args && "kind" in args ? args.kind : undefined;
      const source = args && "sourceApp" in args ? args.sourceApp : undefined;
      state.historyQueries.push(query);
      const terms = query
        .normalize("NFC")
        .toLowerCase()
        .split(/\s+/)
        .filter(Boolean);
      const result = {
        entries: [...state.entries]
          .filter((entry) => {
            const text = [
              entry.title,
              entry.customName,
              entry.sourceApp,
              ...(state.filesById[entry.id] ?? []),
            ]
              .join(" ")
              .normalize("NFC")
              .toLowerCase();
            return (
              (!kind || entry.kind === kind) &&
              (!source || entry.sourceApp === source) &&
              terms.every((term) => text.includes(term))
            );
          })
          .sort((a, b) => Number(b.pinned) - Number(a.pinned)),
        total: state.entries.length,
        sourceApps: [
          ...new Set(
            state.entries.flatMap((entry) =>
              entry.sourceApp ? [entry.sourceApp] : [],
            ),
          ),
        ].sort(),
        captureSupported: state.supported,
        supportNotice:
          "Pinned entries survive automatic cleanup and Clear unpinned.",
        storageNotice: "Capture limit: 32 entries and 16 MiB, including pins.",
      };
      const error = state.historyError;
      if (state.holdHistory)
        await new Promise<void>((resolve) => {
          state.releaseHistory = resolve;
        });
      if (error) throw new Error(error);
      return result;
    }
    if (command === "rich_clipboard_preview") {
      if (state.holdPreview)
        await new Promise<void>((resolve) => {
          state.releasePreview = resolve;
        });
      if (state.previewError)
        throw new Error("The saved preview cannot be read.");
      return {
        entry: state.entries.find((entry) => entry.id === id),
        png:
          state.entries.find((entry) => entry.id === id)?.kind === "image"
            ? Array.from(
                atob(
                  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+jO9sAAAAASUVORK5CYII=",
                ),
                (c) => c.charCodeAt(0),
              )
            : null,
        files:
          state.entries.find((entry) => entry.id === id)?.kind === "image"
            ? null
            : (state.filesById[Number(id)] ?? [
                "/fixtures/report.pdf",
                "/fixtures/design.png",
              ]),
      };
    }
    if (command === "reveal_rich_clipboard_file") {
      state.revealedFiles.push({
        id: Number(id),
        fileIndex: Number(args && "fileIndex" in args ? args.fileIndex : -1),
      });
      if (state.holdReveal)
        await new Promise<void>((resolve) => {
          state.releaseReveal = resolve;
        });
      if (state.missing)
        throw new Error("A referenced file is no longer available.");
      if (state.revealError) throw new Error(state.revealError);
      return;
    }
    if (command === "paste_rich_clipboard") {
      state.pastedIds.push(Number(id));
      if (state.holdPaste)
        await new Promise<void>((resolve) => {
          state.releasePaste = resolve;
        });
      if (state.missing)
        throw new Error("A referenced file is no longer available.");
      if (state.pasteError) throw new Error(state.pasteError);
      return;
    }
    if (command === "copy_rich_clipboard") {
      state.copiedIds.push(Number(id));
      if (window.__richClipboardTest.missing)
        throw new Error("A referenced file is no longer available.");
      return;
    }
    if (command === "set_rich_clipboard_name") {
      const name = String(args && "name" in args ? args.name : "");
      state.namedEntries.push({ id: Number(id), name });
      if (state.nameError)
        throw new Error("Clipboard storage is busy. Try again.");
      const entry = state.entries.find((entry) => entry.id === id);
      if (!entry?.pinned)
        throw new Error("Pin an available entry before naming it.");
      state.entries = state.entries.map((entry) =>
        entry.id === id
          ? { ...entry, customName: name.trim().normalize("NFC") || null }
          : entry,
      );
      return;
    }
    if (command === "set_rich_clipboard_pinned") {
      if (state.pinError)
        throw new Error("Clipboard storage is busy. Try again.");
      const entry = state.entries.find((entry) => entry.id === id);
      if (!entry)
        throw new Error("This clipboard entry is no longer available.");
      state.entries = state.entries.map((entry) =>
        entry.id === id
          ? {
              ...entry,
              pinned: Boolean(args && "pinned" in args && args.pinned),
            }
          : entry,
      );
      return;
    }
    if (command === "delete_rich_clipboard") {
      state.entries = state.entries.filter((entry) => entry.id !== id);
      return;
    }
  },
  { shouldMockEvents: true },
);
render(
  () => (
    <RichClipboardHistory
      platform={new URLSearchParams(location.search).get("platform") ?? "macos"}
      onClose={() => {
        window.__richClipboardTest.closed = true;
      }}
    />
  ),
  document.getElementById("root")!,
);
