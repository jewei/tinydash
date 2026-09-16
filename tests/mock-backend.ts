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

window.isTauri = true;
window.__launcherTest = { calls: [], rejectActions: false, emit };
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
        },
        warnings: [],
      };
    }
    if (command === "search") {
      const { query, mode } = payload as { query: string; mode: SearchMode };
      if (query === "slow" && mode !== "emoji")
        await new Promise((resolve) => setTimeout(resolve, 250));
      if (query === "error")
        throw new Error(
          "The application index is unavailable. Restart TinyDash.",
        );
      // These fixed responses test rendering and IPC order, not TypeScript search.
      const results =
        query === "=1 / 0" || (mode === "calculator" && !query)
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
                    : apps;
      return {
        results,
        total: apps.length,
        indexing: false,
        indexError: null,
        notice: query === "=1 / 0" ? "Division by zero is not allowed." : null,
      };
    }
    if (command === "execute_action" && state.rejectActions) {
      throw new Error(
        "Could not open the application. Refresh the application list.",
      );
    }
    return undefined;
  },
  { shouldMockEvents: true },
);
