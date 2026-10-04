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
      supported: boolean;
      pasteError: string | null;
      holdPaste: boolean;
      releasePaste?: () => void;
      pastedIds: number[];
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
  },
];
window.__richClipboardTest = {
  calls: [],
  missing: false,
  closed: false,
  entries,
  previewError: false,
  pinError: false,
  supported: true,
  pasteError: null,
  holdPaste: false,
  pastedIds: [],
};
mockWindows("main");
mockIPC(
  async (command, args) => {
    const state = window.__richClipboardTest;
    const id = args && "id" in args ? args.id : undefined;
    window.__richClipboardTest.calls.push(command);
    if (command === "rich_clipboard_history")
      return {
        entries: [...state.entries].sort(
          (a, b) => Number(b.pinned) - Number(a.pinned),
        ),
        captureSupported: state.supported,
        supportNotice:
          "Pinned entries survive automatic cleanup and Clear unpinned.",
        storageNotice: "Capture limit: 32 entries and 16 MiB, including pins.",
      };
    if (command === "rich_clipboard_preview") {
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
            : ["/fixtures/report.pdf", "/fixtures/design.png"],
      };
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
      if (window.__richClipboardTest.missing)
        throw new Error("A referenced file is no longer available.");
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
      onClose={() => {
        window.__richClipboardTest.closed = true;
      }}
    />
  ),
  document.getElementById("root")!,
);
