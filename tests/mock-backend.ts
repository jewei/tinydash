// Loaded only by the browser tests. Production always calls the Rust backend.
import { mockIPC } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import type { SearchResult, SearchMode } from "../src/bridge";

declare global {
  interface Window {
    isTauri: boolean;
    __launcherTest: {
      calls: { command: string; payload: unknown }[];
      rejectActions: boolean;
      storageError: string | null;
      usedAppFirst: boolean;
      clipboardDeleted: string[];
      clipboardCleared: boolean;
      rejectClear: boolean;
      slowPreview: boolean;
      fileIndexing: boolean;
      fileWarning: string | null;
      holdSearch: boolean;
      releaseSearch?: () => void;
      emit: typeof emit;
    };
  }
}

const names = [
  "Finder",
  "Safari",
  "Visual Studio Code",
  "Activity Monitor",
  "Calendar",
  "Ghostty",
  "Notes",
  "System Settings",
];
const apps: SearchResult[] = names.map((title, index) => ({
  id: `app-${index}`,
  kind: "app",
  title,
  subtitle: `/Applications/${title}.app`,
  score: 100 - index,
  icon: null,
  primaryAction: "launch",
  secondaryActions: ["reveal"],
}));

const calculation: SearchResult = {
  id: "calculation:1",
  kind: "calculation",
  title: "96",
  subtitle: "12 * 8",
  score: 100_000,
  icon: null,
  primaryAction: "copy",
  secondaryActions: [],
};
const file: SearchResult = {
  id: "file:/Documents/Launch notes.md",
  kind: "file",
  title: "Launch notes.md",
  subtitle: "/Documents/Launch notes.md",
  score: 2000,
  icon: null,
  primaryAction: "open",
  secondaryActions: ["reveal"],
};
const emoji: SearchResult = {
  id: "emoji:🚀",
  kind: "emoji",
  title: "rocket",
  subtitle: ":rocket: · Travel & places",
  score: 10_000,
  icon: "🚀",
  primaryAction: "copy",
  secondaryActions: [],
};

const clips: SearchResult[] = ["Meeting notes", "Project link"].map(
  (title, index) => ({
    id: `clipboard:${index + 1}`,
    kind: "clipboard",
    title,
    subtitle: "Text · 64 characters",
    score: 0,
    icon: null,
    primaryAction: "copy",
    secondaryActions: ["delete"],
  }),
);

window.isTauri = true;
window.__launcherTest = {
  calls: [],
  rejectActions: false,
  storageError: null,
  usedAppFirst: false,
  clipboardDeleted: [],
  clipboardCleared: false,
  rejectClear: false,
  slowPreview: false,
  fileIndexing: false,
  fileWarning: null,
  holdSearch: false,
  emit,
};
mockIPC(
  async (command, payload) => {
    const state = window.__launcherTest;
    state.calls.push({ command, payload });
    if (command === "launcher_ready") {
      return {
        platform: "macos",
        settings: {
          clearQueryOnOpen: true,
          hideOnBlur: true,
          shortcut: "CommandOrControl+Shift+Space",
          clipboardHistoryEnabled: true,
          clipboardHistoryLimit: 100,
          fileSearchRoots: null,
          fileSearchLimit: 50000,
          fileSearchExcludedDirs: ["node_modules", "target"],
        },
        warnings: [],
      };
    }
    if (command === "search") {
      const { query, mode } = payload as { query: string; mode: SearchMode };
      if (query === "slow" && mode !== "emoji") {
        if (state.holdSearch)
          await new Promise<void>((resolve) => {
            state.releaseSearch = resolve;
          });
        else await new Promise((resolve) => setTimeout(resolve, 250));
      }
      if (query === "error")
        throw new Error(
          "The application index is unavailable. Restart TinyDash.",
        );
      // These fixed responses test rendering and IPC order, not TypeScript search.
      const results =
        mode === "files" || query === "Launch notes.md"
          ? query === "missing"
            ? []
            : [file]
          : mode === "clipboard"
            ? state.clipboardCleared
              ? []
              : clips.filter(
                  (entry) => !state.clipboardDeleted.includes(entry.id),
                )
            : query === "=1 / 0" || (mode === "calculator" && !query)
              ? []
              : mode === "emoji" ||
                  query === ":rocket" ||
                  (query === "rocket" && mode !== "apps")
                ? [emoji]
                : query === "12 * 8" && mode !== "apps"
                  ? [calculation]
                  : query === "slow"
                    ? [apps[0]]
                    : query === "sa"
                      ? [apps[1]]
                      : query === "missing"
                        ? []
                        : state.usedAppFirst
                          ? [apps[1], apps[0], ...apps.slice(2)]
                          : apps;
      return {
        results,
        total: apps.length,
        indexing: false,
        indexError: null,
        notice: query === "=1 / 0" ? "Division by zero is not allowed." : null,
        storageError: state.storageError,
        files: {
          total: 1,
          indexing: state.fileIndexing,
          warning: state.fileWarning,
        },
      };
    }
    if (command === "execute_action" && state.rejectActions) {
      throw new Error(
        "Could not open the application. Refresh the application list.",
      );
    }
    if (
      command === "execute_action" &&
      (payload as { action: string }).action === "delete"
    ) {
      state.clipboardDeleted.push((payload as { id: string }).id);
    }
    if (command === "clipboard_preview") {
      const { id } = payload as { id: string };
      if (state.slowPreview && id === "clipboard:1")
        await new Promise((resolve) => setTimeout(resolve, 250));
      return {
        id: Number(id.split(":")[1]),
        content:
          id === "clipboard:1"
            ? "Meeting notes\n  Review the launcher.\n  Keep the original spacing. 🚀\n<script>literal text</script>"
            : "https://example.com/project",
        createdAt: 1_789_571_700,
        lastUsedAt: null,
      };
    }
    if (command === "clear_clipboard_history") {
      if (state.rejectClear)
        throw new Error("Could not delete clipboard history.");
      state.clipboardCleared = true;
    }
    return undefined;
  },
  { shouldMockEvents: true },
);
