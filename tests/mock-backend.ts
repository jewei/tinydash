// Loaded only by the browser tests. Production always calls the Rust backend.
import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import type {
  SearchResult,
  SearchMode,
  SettingsImport,
  SettingsValues,
  UpdateStatus,
} from "../src/bridge";
import { defaultCategories, resultCategories } from "../src/categories";

declare global {
  interface Window {
    isTauri: boolean;
    __launcherTest: {
      calls: { command: string; payload: unknown }[];
      settings: SettingsValues;
      platform: "macos" | "windows" | "linux";
      nativeGlass: boolean;
      rejectNativeGlass: boolean;
      holdNativeGlass: boolean;
      releaseNativeGlass?: () => void;
      rejectSettings: string | null;
      rejectResetPosition: string | null;
      pins: Partial<Record<SearchMode, string[]>>;
      rejectPin: boolean;
      emojiGrid: boolean;
      toolRevision: number;
      rejectActions: boolean | string;
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
      importPreview: SettingsImport | null;
      rejectImport: string | null;
      backupRevealed: boolean;
      rejectBackup: string | null;
      updateStatus: UpdateStatus;
      updateInstalled: boolean;
      rejectUpdate: string | null;
      clipboardClearKeepPinned: boolean;
      editedClipboard: { id: string; text: string } | null;
      copiedSelection: { ids: string[]; separator: string } | null;
      rejectClipboardCopy: string | null;
      initialVisible?: boolean;
      initialMode?: SearchMode | null;
      holdSearch: boolean;
      holdNextSearch: boolean;
      reverseSystem: boolean;
      resultOverrides: Record<string, Partial<SearchResult>>;
      holdAction: boolean;
      releaseAction?: () => void;
      releaseSearch?: () => void;
      emit: typeof emit;
    };
  }
}

const names = [
  ["Finder", "Files and folders"],
  ["Safari", "Web browser"],
  ["Visual Studio Code", "Code editor"],
  ["Activity Monitor", "System activity"],
  ["Calendar", "Events and reminders"],
  ["Ghostty", "Terminal emulator"],
  ["Notes", "Notes and checklists"],
  ["System Settings", "System preferences"],
];
const apps: SearchResult[] = names.map(([title, subtitle], index) => ({
  id: `app-${index}`,
  kind: "app",
  title,
  subtitle,
  path: `/Applications/${title}.app`,
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
  ["lock", "Lock screen", ""],
  ["appearance", "Toggle system appearance", ""],
  ["empty-trash", "Empty Trash", "Empty the Trash?"],
  ["logout", "Log out", "Log out of your account?"],
  ["desktop", "Show desktop", ""],
  ["mute", "Toggle mute", ""],
].map(([id, title, question]) => ({
  id: `system:${id}`,
  kind: "systemCommand",
  title,
  subtitle: question
    ? "Confirmation required"
    : id === "mute"
      ? "Mute or unmute system sound output"
      : title,
  score: 1000,
  icon: null,
  primaryAction: "run",
  secondaryActions: [],
  confirmation: question
    ? {
        title: question,
        description:
          id === "empty-trash"
            ? "This permanently deletes all trashed items, including items on connected drives. You cannot undo this action."
            : "Save your work before you continue.",
        confirmLabel: title,
      }
    : null,
}));
const commandQueries = new Map([
  ["sleep", systemCommands[2]],
  ["slee", systemCommands[2]],
  ["sle", systemCommands[2]],
  ["sl", systemCommands[2]],
  ["re", systemCommands[0]],
  ["res", systemCommands[0]],
  ["sh", systemCommands[1]],
  ["shu", systemCommands[1]],
  ["loc", systemCommands[4]],
  ["lock screen", systemCommands[4]],
  ["dar", systemCommands[5]],
  ["dark mode", systemCommands[5]],
  ["toggle system appearance", systemCommands[5]],
  ["em", systemCommands[6]],
  ["emp", systemCommands[6]],
  ["empty trash", systemCommands[6]],
  ["log", systemCommands[7]],
  ["log out", systemCommands[7]],
  ["des", systemCommands[8]],
  ["show desktop", systemCommands[8]],
  ["mu", systemCommands[9]],
  ["toggle mute", systemCommands[9]],
]);
const file: SearchResult = {
  id: "file:/Documents/Launch notes.md",
  kind: "file",
  title: "Launch notes.md",
  subtitle: "/Documents/Launch notes.md",
  path: "/Documents/Launch notes.md",
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
const gridEmoji = Array.from({ length: 20 }, (_, index) => ({
  ...emoji,
  id: `emoji:${index}`,
  title: `Emoji ${index + 1}`,
}));

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

const issued = new Map<string, SearchResult>();
const savedPins = JSON.parse(
  localStorage.getItem("tinydash.test.pins") ?? "{}",
);

function describePins(results: SearchResult[], query: string): SearchResult[] {
  const indices = new Map<SearchMode, number>();
  return results.map((result) => {
    const category = resultCategories[result.kind];
    const index = indices.get(category) ?? 0;
    indices.set(category, index + 1);
    const key = ["calculator", "password", "timezone", "url", "web"].includes(
      category,
    )
      ? `query:${JSON.stringify([category, query.trim().replace(/^=/, "").trim(), index])}`
      : result.id;
    const value = {
      ...result,
      pin: {
        key,
        categories: defaultCategories.filter((mode) =>
          window.__launcherTest.pins[mode]?.includes(key),
        ),
      },
    };
    issued.set(result.id, value);
    return value;
  });
}

function restorePin(key: string): SearchResult | undefined {
  if (key.startsWith("query:")) {
    const [mode, query, index] = JSON.parse(key.slice(6)) as [
      SearchMode,
      string,
      number,
    ];
    const results =
      toolResults(query, mode) ?? (mode === "calculator" ? [calculation] : []);
    return describePins(results, query)[index];
  }
  const state = window.__launcherTest;
  const available = [
    ...apps,
    file,
    emoji,
    ...gridEmoji,
    ...systemCommands,
    ...clips.filter((clip) => {
      const pinned = Object.values(state.pins).some((keys) =>
        keys?.includes(clip.id),
      );
      return (
        (!state.clipboardCleared ||
          (state.clipboardClearKeepPinned && pinned)) &&
        !state.clipboardDeleted.includes(clip.id)
      );
    }),
  ];
  const result = available.find((result) => result.id === key);
  return result ? describePins([result], "")[0] : undefined;
}

window.isTauri = true;
const defaultSettings: SettingsValues = {
  clearQueryOnOpen: true,
  hideOnBlur: true,
  shortcut: "Control+Shift+Space",
  categoryShortcuts: [],
  startAtLogin: false,
  appPreferences: {},
  webSearches: [],
  clipboardHistoryEnabled: true,
  clipboardHistoryDecided: true,
  clipboardHistoryLimit: 100,
  fileSearchRoots: null,
  fileSearchLimit: 50000,
  fileSearchExcludedDirs: ["node_modules", "target"],
  fileWatchEnabled: true,
  currencyRatesEnabled: true,
  visibleCategories: [...defaultCategories],
};
const savedSettings = JSON.parse(
  localStorage.getItem("tinydash.test.settings") ?? "null",
) as Partial<SettingsValues> | null;
window.__launcherTest = {
  calls: [],
  nativeGlass: localStorage.getItem("tinydash.test.nativeGlass") === "true",
  rejectNativeGlass: false,
  holdNativeGlass: false,
  platform:
    (localStorage.getItem("tinydash.test.platform") as
      "macos" | "windows" | "linux") ?? "macos",
  settings: { ...defaultSettings, ...savedSettings },
  rejectSettings: null,
  rejectResetPosition: null,
  pins: Array.isArray(savedPins)
    ? { all: savedPins, apps: savedPins }
    : savedPins,
  rejectPin: false,
  emojiGrid: false,
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
  importPreview: null,
  rejectImport: null,
  backupRevealed: false,
  rejectBackup: null,
  updateStatus: {
    available: false,
    version: null,
    notes: null,
    message: "TinyDash is up to date.",
  },
  updateInstalled: false,
  rejectUpdate: null,
  clipboardClearKeepPinned: false,
  editedClipboard: null,
  copiedSelection: null,
  rejectClipboardCopy: null,
  initialVisible:
    localStorage.getItem("tinydash.test.initialVisible") === "false"
      ? false
      : undefined,
  initialMode:
    (localStorage.getItem("tinydash.test.initialMode") as SearchMode | null) ??
    null,
  holdSearch: false,
  holdNextSearch: false,
  reverseSystem: false,
  resultOverrides: {},
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
    if (command === "reset_launcher_position") {
      if (state.rejectResetPosition) throw new Error(state.rejectResetPosition);
      return;
    }
    if (command === "launcher_ready") {
      return {
        platform: state.platform,
        settings: state.settings,
        warnings: [],
        visible: state.initialVisible,
        initialMode: state.initialMode,
      };
    }
    if (command === "set_launcher_appearance") {
      if (state.holdNativeGlass)
        await new Promise<void>((resolve) => {
          state.releaseNativeGlass = resolve;
        });
      if (state.rejectNativeGlass) throw new Error("Native glass unavailable");
      return state.platform === "macos" && state.nativeGlass;
    }
    if (command === "get_settings") {
      return {
        settings: state.settings,
        defaults: { ...state.settings, shortcut: "Control+Shift+Space" },
        platform: state.platform,
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
    if (command === "choose_clipboard_history") {
      const { enabled } = payload as { enabled: boolean };
      state.settings = {
        ...state.settings,
        clipboardHistoryEnabled: enabled,
        clipboardHistoryDecided: true,
      };
      localStorage.setItem(
        "tinydash.test.settings",
        JSON.stringify(state.settings),
      );
      return state.settings;
    }
    if (command === "app_catalog") return apps;
    if (command === "set_app_preference") {
      const { id, aliases, hidden } = payload as {
        id: string;
        aliases: string[];
        hidden: boolean;
      };
      const appPreferences = { ...state.settings.appPreferences };
      if (aliases.length || hidden) appPreferences[id] = { aliases, hidden };
      else delete appPreferences[id];
      state.settings = { ...state.settings, appPreferences };
      return state.settings;
    }
    if (command === "preview_web_search") {
      const { search, query } = payload as {
        search: { template: string };
        query: string;
      };
      if ((search.template.match(/\{query\}/g) ?? []).length !== 1)
        throw new Error("Use {query} exactly once in the URL.");
      return search.template.replace("{query}", encodeURIComponent(query));
    }
    if (command === "export_settings") return true;
    if (command === "preview_settings_import") {
      if (state.rejectImport) throw new Error(state.rejectImport);
      return state.importPreview;
    }
    if (command === "reveal_backup") {
      if (state.rejectBackup) throw new Error(state.rejectBackup);
      state.backupRevealed = true;
      return;
    }
    if (command === "check_update") {
      if (state.rejectUpdate) throw new Error(state.rejectUpdate);
      return state.updateStatus;
    }
    if (command === "install_update") {
      if (state.rejectUpdate) throw new Error(state.rejectUpdate);
      state.updateInstalled = true;
      return;
    }
    if (command === "edit_clipboard_history") {
      if (state.rejectClipboardCopy) throw new Error(state.rejectClipboardCopy);
      state.editedClipboard = payload as { id: string; text: string };
      return;
    }
    if (command === "copy_clipboard_selection") {
      if (state.rejectClipboardCopy) throw new Error(state.rejectClipboardCopy);
      state.copiedSelection = payload as { ids: string[]; separator: string };
      return;
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
      const command = commandQueries.get(query);
      const commandResults =
        mode === "all" && command
          ? [command, apps[0], { ...clips[0], title: `${command.title} notes` }]
          : mode === "system" && command
            ? [command]
            : null;
      const results =
        toolResults(query, mode) ??
        commandResults ??
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
              ? clips.filter((entry) => {
                  const pinned = Object.values(state.pins).some((keys) =>
                    keys?.includes(entry.id),
                  );
                  return (
                    (!state.clipboardCleared ||
                      (state.clipboardClearKeepPinned && pinned)) &&
                    !state.clipboardDeleted.includes(entry.id) &&
                    `${entry.title} ${entry.subtitle}`
                      .toLocaleLowerCase()
                      .includes(query.trim().toLocaleLowerCase())
                  );
                })
              : query === "=1 / 0" || (mode === "calculator" && !query)
                ? []
                : mode === "emoji" ||
                    query === ":rocket" ||
                    query === ":smile" ||
                    (query === "rocket" && mode !== "apps")
                  ? query === "grid" || (!query && state.emojiGrid)
                    ? gridEmoji
                    : [emoji]
                  : ["12 * 8", "128 * 1.08"].includes(query) && mode !== "apps"
                    ? [
                        {
                          ...calculation,
                          title: query === "128 * 1.08" ? "138.24" : "96",
                          subtitle: query,
                        },
                      ]
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
                        : query === "sa" || query === "saf"
                          ? [apps[1]]
                          : query === "missing"
                            ? []
                            : state.usedAppFirst
                              ? [apps[1], apps[0], ...apps.slice(2)]
                              : apps);
      const described = describePins(
        mode === "all" && !query.trim() ? [] : results,
        query,
      );
      if (!query.trim()) {
        for (const key of state.pins[mode] ?? []) {
          if (described.some((result) => result.pin?.key === key)) continue;
          const result = restorePin(key);
          if (result) described.push(result);
        }
        described.sort(
          (a, b) =>
            Number(b.pin?.categories.includes(mode)) -
            Number(a.pin?.categories.includes(mode)),
        );
      }
      const preferredSelectionId =
        !query.trim() && (mode === "clipboard" || mode === "all")
          ? (clips.find((entry) => {
              const pinned = Object.values(state.pins).some((keys) =>
                keys?.includes(entry.id),
              );
              return (
                (!state.clipboardCleared ||
                  (state.clipboardClearKeepPinned && pinned)) &&
                !state.clipboardDeleted.includes(entry.id)
              );
            })?.id ?? null)
          : null;
      return {
        preferredSelectionId,
        // Native IPC returns independent objects, including nested action data.
        results: structuredClone(
          described.map((result) => ({
            ...result,
            ...state.resultOverrides[result.id],
          })),
        ),
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
    if (command === "set_pinned") {
      if (state.rejectPin) throw new Error("Could not save the pin.");
      const { id, category, pinned } = payload as {
        id: string;
        category: SearchMode;
        pinned: boolean;
      };
      const result = issued.get(id)!;
      const key = result.pin!.key;
      state.pins[category] = [
        ...(state.pins[category] ?? []).filter((value) => value !== key),
        ...(pinned ? [key] : []),
      ];
      localStorage.setItem("tinydash.test.pins", JSON.stringify(state.pins));
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
      if (typeof state.rejectActions === "string") throw state.rejectActions;
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
      state.clipboardClearKeepPinned = Boolean(
        (payload as { keepPinned?: boolean }).keepPinned,
      );
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
  if (/^next friday \+ 2 weeks?$/i.test(query)) {
    return [
      {
        ...base,
        id: "tool:date-calculation",
        kind: "timezone",
        title: "Fri, 02 Oct 2026",
        subtitle:
          "next friday + 2 week · based on your local date, Thu, 17 Sep 2026",
        detail: {
          type: "dateCalculation",
          expression: query,
          basedOn: "Thu, 17 Sep 2026",
          result: "Fri, 02 Oct 2026",
        },
      },
    ];
  }
  if (query === "10:00 a.m. Pacific Time") {
    return [
      {
        ...base,
        id: "tool:pacific-conversion",
        kind: "timezone",
        title: "01:00 · your local time",
        subtitle:
          "Thu, 17 Sep 2026 · 10:00 · Los Angeles · UTC-07:00 · 18 Sep locally",
        detail: {
          type: "timezone",
          sourceZone: "America/Los_Angeles",
          source: "Thu, 17 Sep 2026 · 10:00 · Los Angeles · UTC-07:00",
          local: "Fri, 18 Sep 2026 · 01:00 · UTC+08:00",
          ambiguous: false,
        },
      },
    ];
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
