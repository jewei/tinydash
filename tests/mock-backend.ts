// Loaded only by the browser tests. Production always calls the Rust backend.
import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import type { SearchResult, SearchMode, SettingsValues } from "../src/bridge";
import { defaultCategories } from "../src/categories";

declare global {
  interface Window {
    isTauri: boolean;
    __launcherTest: {
      calls: { command: string; payload: unknown }[];
      settings: SettingsValues;
      rejectSettings: string | null;
      pinnedIds: string[];
      rejectPin: boolean;
      toolRevision: number;
      rejectActions: boolean;
      storageError: string | null;
      usedAppFirst: boolean;
      clipboardDeleted: string[];
      clipboardCleared: boolean;
      rejectClear: boolean;
      slowPreview: boolean;
      fileIndexing: boolean;
      fileWarning: string | null;
      currencyDate: string | null;
      currencyRefreshing: boolean;
      currencyWarning: string | null;
      currencyMissing: boolean;
      holdSearch: boolean;
      holdNextSearch: boolean;
      reverseSystem: boolean;
      holdAction: boolean;
      releaseAction?: () => void;
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
  icon:
    index === 0
      ? "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+jC1sAAAAASUVORK5CYII="
      : index === 1
        ? "data:image/png;base64,broken"
        : null,
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
const systemCommands: SearchResult[] = [
  ["restart", "Restart", "Restart this computer?"],
  ["shutdown", "Shut down", "Shut down this computer?"],
  ["sleep", "Sleep", "Put this computer to sleep?"],
  ["settings", "Open system settings", ""],
].map(([id, title, question]) => ({
  id: `system:${id}`,
  kind: "systemCommand",
  title,
  subtitle: question
    ? "Confirmation required"
    : "Open your operating system settings",
  score: 1000,
  icon: null,
  primaryAction: "run",
  secondaryActions: [],
  confirmation: question
    ? {
        title: question,
        description: "Save your work before you continue.",
        confirmLabel: title,
      }
    : null,
}));
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
  settings: JSON.parse(
    localStorage.getItem("tinydash.test.settings") ?? "null",
  ) ?? {
    clearQueryOnOpen: true,
    hideOnBlur: true,
    shortcut: "Control+Shift+Space",
    clipboardHistoryEnabled: true,
    clipboardHistoryLimit: 100,
    fileSearchRoots: null,
    fileSearchLimit: 50000,
    fileSearchExcludedDirs: ["node_modules", "target"],
    fileWatchEnabled: true,
    currencyRatesEnabled: true,
    visibleCategories: [...defaultCategories],
  },
  rejectSettings: null,
  pinnedIds: JSON.parse(localStorage.getItem("tinydash.test.pins") ?? "[]"),
  rejectPin: false,
  toolRevision: 0,
  rejectActions: false,
  storageError: null,
  usedAppFirst: false,
  clipboardDeleted: [],
  clipboardCleared: false,
  rejectClear: false,
  slowPreview: false,
  fileIndexing: false,
  fileWarning: null,
  currencyDate: null,
  currencyRefreshing: false,
  currencyWarning: null,
  currencyMissing: false,
  holdSearch: false,
  holdNextSearch: false,
  reverseSystem: false,
  holdAction: false,
  emit,
};
mockWindows("main");
mockIPC(
  async (command, payload) => {
    const state = window.__launcherTest;
    state.calls.push({ command, payload });
    if (command === "hide_launcher") {
      await emit("launcher-hidden");
      return;
    }
    if (command === "launcher_ready") {
      return {
        platform: "macos",
        settings: state.settings,
        warnings: [],
      };
    }
    if (command === "get_settings") {
      return {
        settings: state.settings,
        defaults: { ...state.settings, shortcut: "Control+Shift+Space" },
        platform: "macos",
        version: "0.1.0",
        configPath:
          "/Users/test/Library/Application Support/dev.tinydash.launcher/settings.json",
        dataPath:
          "/Users/test/Library/Application Support/dev.tinydash.launcher/tinydash.sqlite3",
        shortcutsAvailable: true,
      };
    }
    if (command === "save_settings") {
      if (state.rejectSettings) throw new Error(state.rejectSettings);
      state.settings = (payload as { settings: SettingsValues }).settings;
      localStorage.setItem(
        "tinydash.test.settings",
        JSON.stringify(state.settings),
      );
      await emit("settings-changed", state.settings);
      return state.settings;
    }
    if (command === "search") {
      const { query, mode } = payload as { query: string; mode: SearchMode };
      if (state.holdNextSearch) {
        state.holdNextSearch = false;
        await new Promise<void>((resolve) => {
          state.releaseSearch = resolve;
        });
      }
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
        toolResults(query, mode) ??
        (mode === "system" || query === "reboot"
          ? query === "missing"
            ? []
            : state.reverseSystem
              ? [...systemCommands].reverse()
              : systemCommands
          : mode === "files" || query === "Launch notes.md"
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
                  ? query === "grid"
                    ? Array.from({ length: 20 }, (_, index) => ({
                        ...emoji,
                        id: `emoji:${index}`,
                        title: `Emoji ${index + 1}`,
                      }))
                    : [emoji]
                  : query === "12 * 8" && mode !== "apps"
                    ? [calculation]
                    : query === "100 USD to MYR" && mode !== "apps"
                      ? state.currencyMissing
                        ? []
                        : [
                            {
                              ...calculation,
                              id: "calculation:currency",
                              title: "400 MYR",
                              subtitle:
                                "100 USD to MYR · ECB 2026-09-16 · cached rates",
                            },
                          ]
                      : query === "slow"
                        ? [apps[0]]
                        : query === "sa"
                          ? [apps[1]]
                          : query === "missing"
                            ? []
                            : state.usedAppFirst
                              ? [apps[1], apps[0], ...apps.slice(2)]
                              : apps);
      return {
        results:
          !query.trim() && (mode === "all" || mode === "apps")
            ? [...results].sort(
                (a, b) =>
                  Number(state.pinnedIds.includes(b.id)) -
                  Number(state.pinnedIds.includes(a.id)),
              )
            : results,
        pinnedIds: state.pinnedIds,
        total: apps.length,
        indexing: false,
        indexError: null,
        notice:
          query === "100 USD to MYR" && state.currencyMissing
            ? "Currency rates are unavailable. Connect to the internet, then open Actions and choose Refresh currency rates."
            : query === "=1 / 0"
              ? "Division by zero is not allowed."
              : null,
        storageError: state.storageError,
        currency: {
          asOf: state.currencyDate,
          refreshing: state.currencyRefreshing,
          warning: state.currencyWarning,
        },
        files: {
          total: 1,
          indexing: state.fileIndexing,
          warning: state.fileWarning,
        },
      };
    }
    if (command === "set_app_pinned") {
      if (state.rejectPin) throw new Error("Could not save the app pin.");
      const { id, pinned } = payload as { id: string; pinned: boolean };
      state.pinnedIds = [
        ...state.pinnedIds.filter((value) => value !== id),
        ...(pinned ? [id] : []),
      ];
      localStorage.setItem(
        "tinydash.test.pins",
        JSON.stringify(state.pinnedIds),
      );
      await emit("pins-changed");
      return;
    }
    if (
      command === "execute_action" &&
      (payload as { action: string }).action === "regenerate"
    )
      state.toolRevision += 1;
    if (command === "execute_action" && state.holdAction) {
      await new Promise<void>((resolve) => {
        state.releaseAction = resolve;
      });
    }
    if (command === "execute_action" && state.rejectActions) {
      if ((payload as { action: string }).action === "run")
        throw new Error("The OS denied this system command.");
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

// Fixed tool values verify UI behavior. Rust tests verify generation and parsing.
function toolResults(
  query: string,
  mode: SearchMode,
): SearchResult[] | undefined {
  const base = {
    score: 100000,
    icon: null,
    primaryAction: "copy" as const,
    secondaryActions: [] as SearchResult["secondaryActions"],
  };
  if (mode === "password" || /^(password|passphrase|pin)\b/.test(query)) {
    return ["Symbols", "Letters and digits", "Word passphrase", "PIN"].map(
      (variant, index) => ({
        ...base,
        id: `password:${index}-${window.__launcherTest.toolRevision}`,
        kind: "password",
        title:
          index === 3
            ? "491027"
            : index === 2
              ? "sample words for a test only"
              : `SamplePassword${index}!Revision${window.__launcherTest.toolRevision}`,
        subtitle: `${variant} · Strength estimate`,
        secondaryActions: ["regenerate"],
        detail: {
          type: "password",
          variant,
          entropyBits: index === 3 ? 19 : 99,
          strength: index === 3 ? "Low" : "Strong",
        },
      }),
    );
  }
  if (mode === "timezone" || /^(time |tomorrow )/.test(query)) {
    return ["Asia/Tokyo", "Europe/London"].map((sourceZone, index) => ({
      ...base,
      id: `tool:time-${index}-${Date.now()}`,
      kind: "timezone",
      title: index === 0 ? "21:00 · Tokyo" : "13:00 · London",
      subtitle: "Thursday, 17 September 2026",
      detail: {
        type: "timezone",
        sourceZone,
        source: "Thu, 17 Sep 2026 · 21:00 · UTC+09:00",
        local: "Thu, 17 Sep 2026 · 20:00 · UTC+08:00",
        ambiguous: query.includes("ambiguous"),
      },
    }));
  }
  if (mode === "url" || query.startsWith("https://")) {
    if (!query) return [];
    return [
      {
        ...base,
        id: "tool:url",
        kind: "cleanedUrl",
        title: "https://www.youtube.com/watch?v=demo&t=30",
        subtitle: "2 tracking fields removed",
        secondaryActions: ["open"],
        detail: { type: "cleanedUrl", original: query, removed: 2 },
      },
    ];
  }
  if (mode === "web" || query.startsWith("web ")) {
    if (!query) return [];
    return ["Google", "DuckDuckGo", "Bing", "Brave", "YouTube", "GitHub"].map(
      (engine, index) => ({
        ...base,
        id: `tool:web-${index}`,
        kind: "webSearch",
        title: `Search ${engine}`,
        subtitle: query,
        primaryAction: "open",
        secondaryActions: ["copy"],
        detail: {
          type: "webSearch",
          engine,
          query,
          url: "https://example.com/search?q=sample",
        },
      }),
    );
  }
  return undefined;
}
