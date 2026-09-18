import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import type { LauncherInfo, SearchMode, SearchResult } from "../src/bridge";
import { defaultCategories, resultCategories } from "../src/categories";

const savedPins = JSON.parse(
  localStorage.getItem("tinydash.preview.pins") ?? "{}",
);
const pins: Partial<Record<SearchMode, string[]>> = Array.isArray(savedPins)
  ? { all: savedPins, apps: savedPins }
  : savedPins;
const issued = new Map<string, SearchResult>();

function describePins(results: SearchResult[]): SearchResult[] {
  return results.map((result) => {
    const key =
      result.kind === "calculation"
        ? `calculation:${result.subtitle}`
        : result.id;
    const described = {
      ...result,
      pin: {
        key,
        categories: defaultCategories.filter((category) =>
          pins[category]?.includes(key),
        ),
      },
    };
    issued.set(result.id, described);
    return described;
  });
}

const apps: SearchResult[] = [
  "Safari",
  "Finder",
  "Notes",
  "Calendar",
  "Ghostty",
  "Visual Studio Code",
  "Activity Monitor",
  "System Settings",
].map((title, index) => ({
  id: `sample-app-${index}`,
  kind: "app",
  title,
  subtitle: `/Applications/${title}.app`,
  score: 100 - index,
  icon: null,
  primaryAction: "launch",
  secondaryActions: ["reveal"],
}));

const files: SearchResult[] = [
  ["Project notes.md", "Documents/Project notes.md"],
  ["TinyDash.sketch", "Documents/Design/TinyDash.sketch"],
  ["Weekend reading.pdf", "Downloads/Weekend reading.pdf"],
].map(([title, path], index) => ({
  id: `sample-file-${index}`,
  kind: "file",
  title,
  subtitle: `~/${path}`,
  score: 80 - index,
  icon: null,
  primaryAction: "open",
  secondaryActions: ["reveal"],
}));

const clipboardText = [
  "Design review notes\n\nKeep search in focus.\nUse the arrow keys to select a result.\nPress Enter to open it.",
  "https://github.com/jewei/tinydash",
  "bun run tauri dev",
];
let clips: SearchResult[] = clipboardText.map((content, index) => ({
  id: `sample-clip-${index}`,
  kind: "clipboard",
  title: content.split("\n")[0],
  subtitle: `Text · ${content.length} characters`,
  score: 60 - index,
  icon: null,
  primaryAction: "copy",
  secondaryActions: ["delete"],
}));

const emoji: SearchResult[] = [
  ["☕", "coffee", "Food & drink"],
  ["🚀", "rocket", "Travel & places"],
  ["🌿", "herb", "Animals & nature"],
  ["😀", "grinning", "Smileys & people"],
  ["😊", "smile", "Smileys & people"],
  ["😎", "sunglasses", "Smileys & people"],
  ["🥳", "party", "Smileys & people"],
  ["🤔", "thinking", "Smileys & people"],
  ["👋", "wave", "Smileys & people"],
  ["👏", "clap", "Smileys & people"],
  ["🙌", "raised hands", "Smileys & people"],
  ["👍", "thumbs up", "Smileys & people"],
  ["❤️", "heart", "Symbols"],
  ["✨", "sparkles", "Activities"],
  ["🔥", "fire", "Travel & places"],
  ["🌻", "sunflower", "Animals & nature"],
  ["🐈", "cat", "Animals & nature"],
  ["🦋", "butterfly", "Animals & nature"],
  ["🍋", "lemon", "Food & drink"],
  ["🍓", "strawberry", "Food & drink"],
  ["🍰", "cake", "Food & drink"],
  ["🌎", "earth", "Travel & places"],
  ["🎨", "palette", "Activities"],
  ["💡", "bulb", "Objects"],
].map(([icon, title, category]) => ({
  id: `sample-emoji-${title}`,
  kind: "emoji",
  title,
  subtitle: `:${title}: · ${category}`,
  score: 200,
  icon,
  primaryAction: "copy",
  secondaryActions: [],
}));

const system: SearchResult[] = [
  ["settings", "Open system settings"],
  ["sleep", "Sleep"],
  ["restart", "Restart"],
].map(([id, title]) => ({
  id: `system:${id}`,
  kind: "systemCommand",
  title,
  subtitle:
    id === "settings"
      ? "Open your operating system settings"
      : "Confirmation required",
  score: 50,
  icon: null,
  primaryAction: "run",
  secondaryActions: [],
  confirmation:
    id === "settings"
      ? null
      : {
          title: `${title} this computer?`,
          description: "This is a design preview. No system command will run.",
          confirmLabel: title,
        },
}));

const calculations = new Map([
  ["12*8", "96"],
  ["5fttocm", "152.4 cm"],
  ["sqrt(144)", "12"],
  ["32ctof", "89.6 °F"],
]);

function sampleSearch(query: string, mode: SearchMode): SearchResult[] {
  const normalized = query.trim().toLowerCase();
  const calculation = calculations.get(
    normalized.replace(/^=/, "").replace(/\s+/g, ""),
  );
  if (calculation && (mode === "all" || mode === "calculator")) {
    return [
      {
        id: "sample-calculation",
        kind: "calculation",
        title: calculation,
        subtitle: query,
        score: 1000,
        icon: null,
        primaryAction: "copy",
        secondaryActions: [],
      },
    ];
  }
  const source =
    mode === "apps"
      ? apps
      : mode === "files"
        ? files
        : mode === "clipboard"
          ? clips
          : mode === "emoji" || normalized.startsWith(":")
            ? emoji
            : mode === "system"
              ? system
              : mode === "calculator"
                ? []
                : normalized
                  ? [...apps, ...files, ...clips, ...emoji, ...system]
                  : apps;
  const value = normalized.replace(/^:/, "");
  return source.filter((item) =>
    `${item.title} ${item.subtitle}`.toLowerCase().includes(value),
  );
}

function report(message: string) {
  window.parent.postMessage(
    { type: "tinydash:preview-action", message },
    window.location.origin,
  );
}

Object.assign(window, { isTauri: true });
mockWindows("main");
mockIPC(
  async (command, payload) => {
    if (command === "launcher_ready") {
      return {
        platform: "macos",
        settings: {
          clearQueryOnOpen: true,
          hideOnBlur: true,
          shortcut: "Control+Shift+Space",
          clipboardHistoryEnabled: true,
          clipboardHistoryLimit: 100,
          fileSearchRoots: null,
          fileSearchLimit: 50000,
          fileSearchExcludedDirs: [],
          fileWatchEnabled: true,
          currencyRatesEnabled: false,
          visibleCategories: [...defaultCategories],
        },
        warnings: [],
      } satisfies LauncherInfo;
    }
    if (command === "search") {
      const { query, mode } = payload as { query: string; mode: SearchMode };
      const results = describePins(sampleSearch(query, mode));
      if (!query.trim()) {
        for (const key of pins[mode] ?? []) {
          if (results.some((result) => result.pin?.key === key)) continue;
          const result = key.startsWith("calculation:")
            ? sampleSearch(key.slice(12), "calculator")[0]
            : [...apps, ...files, ...clips, ...emoji, ...system].find(
                (result) => result.id === key,
              );
          if (result) results.push(...describePins([result]));
        }
        results.sort(
          (a, b) =>
            Number(b.pin?.categories.includes(mode)) -
            Number(a.pin?.categories.includes(mode)),
        );
      }
      return {
        results,
        total: apps.length,
        indexing: false,
        indexError: null,
        storageError: null,
        notice:
          mode === "calculator" && query && !sampleSearch(query, mode).length
            ? "The preview supports 12 * 8, 5 ft to cm, sqrt(144), and 32 C to F."
            : null,
        files: { total: files.length, indexing: false, warning: null },
        currency: { asOf: null, refreshing: false, warning: null },
      };
    }
    if (command === "set_pinned") {
      const { id, category, pinned } = payload as {
        id: string;
        category: SearchMode;
        pinned: boolean;
      };
      const result = issued.get(id);
      if (
        !result?.pin ||
        (category !== "all" && category !== resultCategories[result.kind])
      )
        throw new Error("This pin is not available.");
      const key = result.pin.key;
      pins[category] = [
        ...(pins[category] ?? []).filter((value) => value !== key),
        ...(pinned ? [key] : []),
      ];
      localStorage.setItem("tinydash.preview.pins", JSON.stringify(pins));
      await emit("pins-changed");
      return;
    }
    if (command === "clipboard_preview") {
      const { id } = payload as { id: string };
      return {
        id: Number(id.split("-").at(-1)),
        content: clipboardText[Number(id.split("-").at(-1))] ?? "",
        createdAt: Math.floor(Date.now() / 1000),
        lastUsedAt: null,
      };
    }
    if (command === "execute_action") {
      const { id, action } = payload as { id: string; action: string };
      const item = [...apps, ...files, ...clips, ...emoji, ...system].find(
        (entry) => entry.id === id,
      );
      if (action === "delete") clips = clips.filter((entry) => entry.id !== id);
      report(
        `Sample action: ${action} ${item?.title ?? "result"}. Your computer is unchanged.`,
      );
    }
    if (command === "clear_clipboard_history") {
      clips = [];
      await emit("clipboard-changed");
      report("Sample clipboard cleared. Your clipboard is unchanged.");
    }
    if (command === "hide_launcher" || command === "quit_app") {
      report(
        "The preview stays open. Use the desktop app to test hiding and quitting.",
      );
    }
  },
  { shouldMockEvents: true },
);
