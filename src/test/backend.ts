// A fake backend for component tests: records commands and answers them.
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";

import type { SearchResult } from "../generated/SearchResult";
import type { Settings } from "../generated/Settings";

/**
 * Rust's defaults on macOS, the platform this backend reports, except
 * clipboard history is on so its views have data.
 */
export const testSettings: Settings = {
  shortcut: "Control+Shift+Space",
  theme: "system",
  hideOnBlur: true,
  launchAtLogin: false,
  showTrayIcon: false,
  clipboardHistoryEnabled: true,
  clipboardHistoryLimit: 200,
  clipboardCaptureImages: false,
  clipboardCaptureFiles: false,
  fileSearchFolders: ["~/Desktop", "~/Documents", "~/Downloads"],
  fileSearchExcludedDirs: ["node_modules", "target"],
  emojiSkinTone: 0,
  emojiLanguages: [],
  currencyRatesEnabled: false,
  searchEngine: "google",
  checkForUpdates: true,
  launcherPosition: null,
  tabs: (["apps", "files", "clipboard", "snippets", "emoji", "system"] as const).map(
    (category) => ({
      category,
      shown: true,
      inAll: true,
    }),
  ),
  showClocks: true,
  clockCities: [],
  showDiskSpace: true,
  showNotepad: false,
  showFocusTimer: false,
  focusMinutes: 25,
  shortBreakMinutes: 5,
  longBreakMinutes: 15,
  sessionsBeforeLongBreak: 4,
  showWeather: false,
  weatherCity: "",
  temperatureUnit: "celsius",
  showClipboardCards: false,
};

/** Every widget off, so an empty All search shows the preview. */
export const noWidgets: Partial<Settings> = {
  showClocks: false,
  showDiskSpace: false,
  showNotepad: false,
  showFocusTimer: false,
  showWeather: false,
  showClipboardCards: false,
};

export const app: SearchResult = {
  id: "app:/Applications/Safari.app",
  kind: "app",
  title: "Safari",
  subtitle: "Application",
  icon: { type: "symbol", name: "app" },
  pinned: false,
  actions: [
    { label: "Open", action: { type: "launch", path: "/Applications/Safari.app" }, confirm: null },
    {
      label: "Show in Finder",
      action: { type: "reveal", path: "/Applications/Safari.app" },
      confirm: null,
    },
  ],
};

export const restart: SearchResult = {
  id: "system:restart",
  kind: "system",
  title: "Restart",
  subtitle: "System",
  icon: { type: "symbol", name: "restart" },
  pinned: false,
  actions: [
    {
      label: "Run",
      action: { type: "system", command: "restart" },
      confirm: "Restart the computer?",
    },
  ],
};

type Call = { command: string; args: Record<string, unknown> };
type Handler = (args: Record<string, unknown>) => unknown;

/**
 * Install the fake backend. `handlers` override the default replies, and
 * `initial` changes the settings it starts with.
 */
export function fakeBackend(
  handlers: Record<string, Handler> = {},
  initial: Partial<Settings> = {},
) {
  const calls: Call[] = [];
  // Like the backend, each save merges onto what earlier saves left.
  let settings: Settings = { ...testSettings, ...initial };
  const save = (changes: unknown) => (settings = { ...settings, ...(changes as object) });
  const defaults: Record<string, Handler> = {
    launcher_init: () => ({
      settings,
      platform: "macos",
      warnings: [],
      category: null,
      update: null,
    }),
    search: () => [],
    run_action: () => null,
    preview: () => null,
    hide_launcher: () => null,
    drag_launcher: () => null,
    get_settings: () => settings,
    update_settings: (args) => save(args.changes),
    pause_shortcut: () => null,
    library_items: () => [],
    save_library_item: (args) => ({ ...(args.item as object), id: 1 }),
    delete_library_item: () => null,
    check_for_update: () => null,
    install_update: () => null,
    widgets: () => ({
      clocks: [],
      disk: null,
      note: null,
      focus: null,
      weather: null,
      clipCard: null,
    }),
    save_note: () => null,
    hidden_results: () => [],
    unhide_result: () => null,
    about: () => ({
      version: "0.2.0",
      settingsFolder: "/data",
      dataFolder: "/data",
      richClipboard: true,
      selfUpdate: true,
      pendingUpdate: null,
    }),
  };
  clearMocks();
  mockIPC(
    (command, payload) => {
      const args = (payload ?? {}) as Record<string, unknown>;
      calls.push({ command, args });
      const handler = handlers[command] ?? defaults[command];
      if (!handler) throw new Error(`Unexpected command ${command}`);
      return handler(args);
    },
    { shouldMockEvents: true },
  );
  return {
    called: (command: string) => calls.filter((call) => call.command === command),
    /** Merges `changes` onto the stored settings, as `update_settings` does. */
    save,
  };
}
