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
  },
];
window.__richClipboardTest = {
  calls: [],
  missing: false,
  closed: false,
  entries,
  previewError: false,
};
mockWindows("main");
mockIPC(
  (command, args) => {
    const state = window.__richClipboardTest;
    const id = args && "id" in args ? args.id : undefined;
    window.__richClipboardTest.calls.push(command);
    if (command === "rich_clipboard_history")
      return {
        entries: state.entries,
        captureSupported: true,
        supportNotice: "Rich history: up to 32 entries and 16 MiB; no pins.",
      };
    if (command === "rich_clipboard_preview") {
      if (state.previewError)
        throw new Error("The saved preview cannot be read.");
      return {
        entry: state.entries.find((entry) => entry.id === id),
        png: null,
        files: ["/fixtures/report.pdf", "/fixtures/design.png"],
      };
    }
    if (command === "copy_rich_clipboard") {
      if (window.__richClipboardTest.missing)
        throw new Error("A referenced file is no longer available.");
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
