import {
  batch,
  createEffect,
  createMemo,
  createSignal,
  For,
  onCleanup,
  onMount,
  Show,
} from "solid-js";
import { isTauri } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  backend,
  type Action,
  type LauncherInfo,
  type SearchResult,
  type SearchMode,
  type FileStatus,
  type CurrencyStatus,
} from "./bridge";
import Icon from "./components/Icon";
import ResultIcon from "./components/ResultIcon";
import { disposeAppIcons } from "./app-icons";
import ResultPreview from "./components/ResultPreview";
import ConfirmDialog from "./components/ConfirmDialog";
import ClipboardCopyDialog from "./components/ClipboardCopyDialog";
import WelcomeSuggestions from "./components/WelcomeSuggestions";
import {
  appearances,
  readAppearance,
  saveAppearance,
  watchAppearance,
  type Appearance,
} from "./appearance";

import {
  categories,
  normalizeCategories,
  resultCategories,
} from "./categories";

const groupLabels: Record<SearchResult["kind"], string> = {
  app: "Applications",
  file: "Files",
  clipboard: "Clipboard history",
  emoji: "Emoji",
  calculation: "Calculator",
  systemCommand: "System commands",
  password: "Password generator",
  timezone: "Datetime",
  cleanedUrl: "URL cleaner",
  webSearch: "Web search",
};

export default function App(
  props: {
    initialAppearance?: Appearance;
    onAppearanceChange?: (appearance: Appearance) => void;
  } = {},
) {
  const desktop = isTauri();
  const [appearance, setAppearance] = createSignal(
    props.initialAppearance ?? readAppearance(),
  );
  const [visible, setVisible] = createSignal(true);
  const [query, setQuery] = createSignal("");
  const [mode, setMode] = createSignal<SearchMode>("all");
  const [results, setResults] = createSignal<SearchResult[]>([]);
  const [selected, setSelected] = createSignal(0);
  const [pinBusy, setPinBusy] = createSignal(false);
  const [info, setInfo] = createSignal<LauncherInfo>();
  const [total, setTotal] = createSignal(0);
  const [indexing, setIndexing] = createSignal(desktop);
  const [files, setFiles] = createSignal<FileStatus>({
    total: 0,
    indexing: desktop,
    warning: null,
  });
  const [pending, setPending] = createSignal(false);
  const [currency, setCurrency] = createSignal<CurrencyStatus>({
    asOf: null,
    refreshing: false,
    warning: null,
  });
  const [busy, setBusy] = createSignal(false);
  const [error, setError] = createSignal<string>();
  const [indexError, setIndexError] = createSignal<string>();
  const [storageError, setStorageError] = createSignal<string>();
  const [notice, setNotice] = createSignal<string>();
  const [menuOpen, setMenuOpen] = createSignal(false);
  const [menuFilter, setMenuFilter] = createSignal("");
  const [clipboardTool, setClipboardTool] = createSignal<{
    mode: "edit" | "combine";
    entry: SearchResult;
    entries: SearchResult[];
  }>();
  const [clearOpen, setClearOpen] = createSignal(false);
  const [keepPins, setKeepPins] = createSignal(true);
  const [pendingAction, setPendingAction] = createSignal<SearchResult>();
  let input!: HTMLInputElement;
  let menu: HTMLDivElement | undefined;
  let menuInput: HTMLInputElement | undefined;
  let list!: HTMLUListElement;
  let categoryBar!: HTMLDivElement;
  let sequence = 0;
  let disposed = false;
  let searchTask: Promise<void> | undefined;
  let displayedQuery: { value: string; mode: SearchMode } | undefined;
  let selectionChangedByUser = false;
  let queuedSearch:
    | {
        value: string;
        mode: SearchMode;
        preserveSelection: boolean;
        request: number;
      }
    | undefined;
  const unlisteners: UnlistenFn[] = [];

  const modifier = () => (info()?.platform === "macos" ? "⌘" : "Ctrl");
  const enabledCategories = createMemo(() =>
    normalizeCategories(info()?.settings.visibleCategories),
  );
  const visibleCategories = createMemo(() =>
    categories.filter(({ id }) => enabledCategories().includes(id)),
  );
  const current = () => results()[selected()];
  const welcome = () =>
    mode() === "all" && !query().trim() && results().length === 0;
  const resultKey = (result?: SearchResult) => result?.pin?.key ?? result?.id;
  const isPinned = (result?: SearchResult, category = mode()) =>
    result?.pin?.categories.includes(category) ?? false;
  const pinOptions = createMemo(() => {
    const result = current();
    if (!result?.pin) return [];
    return categories
      .filter(({ id }) => id === "all" || id === resultCategories[result.kind])
      .map(({ id, label }) => ({
        category: id,
        label: isPinned(result, id) ? `Unpin from ${label}` : `Pin to ${label}`,
        pinned: isPinned(result, id),
      }));
  });
  const groupLabel = (result: SearchResult) =>
    !query().trim() && isPinned(result) ? "Pinned" : groupLabels[result.kind];
  const canOpen = () =>
    desktop &&
    visible() &&
    !!current() &&
    !busy() &&
    !pending() &&
    !clearOpen() &&
    !clipboardTool() &&
    !pendingAction();
  const message = () =>
    error() ??
    notice() ??
    indexError() ??
    storageError() ??
    (mode() === "all" || mode() === "files" ? files().warning : undefined) ??
    (mode() === "calculator" ? currency().warning : undefined) ??
    info()?.warnings[0];
  const primaryLabel = () =>
    current()?.kind === "password"
      ? "Copy password"
      : current()?.kind === "timezone"
        ? current()?.detail?.type === "dateCalculation"
          ? "Copy date"
          : "Copy time"
        : current()?.kind === "cleanedUrl"
          ? "Copy URL"
          : current()?.kind === "webSearch"
            ? "Search web"
            : current()?.kind === "systemCommand" || mode() === "system"
              ? "Run command"
              : current()?.kind === "emoji" || mode() === "emoji"
                ? "Copy emoji"
                : current()?.kind === "calculation" || mode() === "calculator"
                  ? "Copy result"
                  : current()?.kind === "clipboard" || mode() === "clipboard"
                    ? "Copy text"
                    : "Open";
  const placeholder = () =>
    mode() === "password"
      ? "password 32, passphrase 6, pin 6..."
      : mode() === "timezone"
        ? "next Friday + 2 weeks, 10am Pacific Time..."
        : mode() === "url"
          ? "Paste a URL to remove tracking..."
          : mode() === "web"
            ? "Search the web..."
            : mode() === "system"
              ? "Search system commands..."
              : mode() === "apps"
                ? "Search applications..."
                : mode() === "files"
                  ? "Search filenames and paths..."
                  : mode() === "emoji"
                    ? "Search emoji..."
                    : mode() === "calculator"
                      ? "Calculate or convert..."
                      : mode() === "clipboard"
                        ? "Search clipboard history..."
                        : "What are you looking for?";
  const focusInput = () => input.focus({ preventScroll: true });

  const matchesMenu = (label: string) =>
    label.toLocaleLowerCase().includes(menuFilter().trim().toLocaleLowerCase());
  const resultActions = createMemo(() => {
    type MenuAction = {
      label: string;
      icon: Parameters<typeof Icon>[0]["name"];
      run: () => void;
      disabled: boolean;
      key?: string;
    };
    const actions: MenuAction[] = [
      {
        label:
          primaryLabel() === "Open"
            ? current()?.kind === "file"
              ? "Open file"
              : "Open application"
            : primaryLabel(),
        icon: current()?.primaryAction === "copy" ? "copy" : "return",
        run: runPrimary,
        disabled: !canOpen(),
        key: "↵",
      },
    ];
    if (current()?.secondaryActions.includes("reveal"))
      actions.push({
        label: "Show in folder",
        icon: "folder",
        run: () => void run("reveal"),
        disabled: !canOpen(),
        key: `${modifier()} ↵`,
      });
    for (const option of pinOptions())
      actions.push({
        label: option.label,
        icon: "pin",
        run: () => void togglePin(option.category),
        disabled: !canOpen() || pinBusy(),
      });
    if (current()?.kind === "app")
      actions.push({
        label: "Hide application",
        icon: "apps",
        run: () => void hideApplication(),
        disabled: !canOpen(),
      });
    if (current()?.kind === "clipboard") {
      actions.push(
        {
          label: "Edit a copy",
          icon: "clipboard",
          run: () => openClipboardTool("edit"),
          disabled: !canOpen(),
        },
        {
          label: "Copy selected entries",
          icon: "copy",
          run: () => openClipboardTool("combine"),
          disabled: !canOpen(),
        },
        {
          label: "Save text as a file",
          icon: "file",
          run: () => void saveClipboardFile(),
          disabled: !canOpen(),
        },
      );
    }
    for (const [action, label, icon] of [
      ["delete", "Delete entry", "delete"],
      ["copy", "Copy URL", "copy"],
      ["open", "Open cleaned URL", "globe"],
      ["regenerate", "Generate another", "refresh"],
    ] as const) {
      if (current()?.secondaryActions.includes(action))
        actions.push({
          label,
          icon,
          run: () => void run(action),
          disabled: !canOpen(),
          key: action === "delete" ? `${modifier()} ⌫` : undefined,
        });
    }
    if (mode() === "clipboard" || current()?.kind === "clipboard") {
      actions.push(
        {
          label: "Clear unpinned history",
          icon: "delete",
          run: () => confirmClear(true),
          disabled: !desktop || busy(),
        },
        {
          label: "Clear all clipboard history",
          icon: "delete",
          run: () => confirmClear(false),
          disabled: !desktop || busy(),
        },
      );
    }
    return actions.filter((action) => matchesMenu(action.label));
  });
  const launcherActions = createMemo(() =>
    [
      {
        label: "Settings",
        icon: "system" as const,
        run: () => void openSettings(),
        disabled: !desktop,
        key: `${modifier()} ,`,
      },
      {
        label: "Refresh currency rates",
        icon: "refresh" as const,
        run: () => void refresh("currency"),
        disabled: !desktop || currency().refreshing,
        key: mode() === "calculator" ? `${modifier()} R` : undefined,
      },
      {
        label: "Refresh applications",
        icon: "refresh" as const,
        run: () => void refresh("apps"),
        disabled: !desktop || indexing(),
        key:
          mode() !== "files" && mode() !== "calculator"
            ? `${modifier()} R`
            : undefined,
      },
      {
        label: "Refresh files",
        icon: "refresh" as const,
        run: () => void refresh("files"),
        disabled: !desktop || files().indexing,
        key: mode() === "files" ? `${modifier()} R` : undefined,
      },
      {
        label: "Quit TinyDash",
        icon: "quit" as const,
        run: () =>
          void backend.quit().catch((reason) => setError(String(reason))),
        disabled: !desktop,
        key: `${modifier()} Q`,
      },
    ].filter((action) => matchesMenu(action.label)),
  );

  function openClipboardTool(tool: "edit" | "combine") {
    const entry = current();
    if (!entry || entry.kind !== "clipboard" || !canOpen()) return;
    setMenuOpen(false);
    setClipboardTool({
      mode: tool,
      entry,
      entries: results().filter((entry) => entry.kind === "clipboard"),
    });
  }

  function closeClipboardTool() {
    setClipboardTool(undefined);
    focusInput();
  }

  async function saveClipboardFile() {
    const entry = current();
    if (!entry || !canOpen()) return;
    setMenuOpen(false);
    setBusy(true);
    setError(undefined);
    try {
      if (await backend.saveClipboardFile(entry.id))
        setNotice("Text file saved.");
    } catch (reason) {
      setError(String(reason));
    } finally {
      setBusy(false);
      focusInput();
    }
  }

  async function hideApplication() {
    const entry = current();
    if (!entry || entry.kind !== "app" || !canOpen()) return;
    setMenuOpen(false);
    setBusy(true);
    setError(undefined);
    try {
      const settings = await backend.setAppPreference(
        entry.id,
        info()?.settings.appPreferences[entry.id]?.aliases ?? [],
        true,
      );
      setInfo((current) => (current ? { ...current, settings } : current));
      await search();
      setNotice("Application hidden. You can show it again in Settings.");
    } catch (reason) {
      setError(String(reason));
    } finally {
      setBusy(false);
      focusInput();
    }
  }

  async function chooseClipboardHistory(enabled: boolean) {
    if (busy()) return;
    setBusy(true);
    setError(undefined);
    try {
      const settings = await backend.chooseClipboardHistory(enabled);
      setInfo((current) => (current ? { ...current, settings } : current));
      await search();
      focusInput();
    } catch (reason) {
      setError(String(reason));
    } finally {
      setBusy(false);
    }
  }

  createEffect(() => {
    document.documentElement.dataset.appearance = appearance();
  });

  function changeAppearance(value: Appearance) {
    setAppearance(value);
    saveAppearance(value);
    setMenuOpen(false);
    focusInput();
    props.onAppearanceChange?.(value);
  }

  function startDragging(event: MouseEvent) {
    if (!desktop || event.button !== 0) return;
    event.preventDefault();
    void getCurrentWindow()
      .startDragging()
      .catch((reason: unknown) => setError(String(reason)));
  }

  async function openSettings() {
    setMenuOpen(false);
    try {
      await backend.openSettings();
    } catch (reason) {
      setError(String(reason));
      focusInput();
    }
  }

  function search(value = query(), preserveSelection = false) {
    if (!desktop || !visible() || disposed) return Promise.resolve();
    if (!preserveSelection) selectionChangedByUser = false;
    queuedSearch = {
      value,
      mode: mode(),
      preserveSelection,
      request: ++sequence,
    };
    setPending(true);
    if (
      mode() === "all" &&
      !value.trim() &&
      (displayedQuery?.mode !== "all" || displayedQuery.value.trim())
    ) {
      setResults([]);
    }
    setNotice(undefined);
    // Native IPC calls can arrive out of order. Keep one call in flight and
    // replace waiting input with the newest query, without a debounce timer.
    return (searchTask ??= drainSearch());
  }

  async function drainSearch() {
    try {
      while (queuedSearch && !disposed) {
        const {
          value,
          mode: searchMode,
          preserveSelection,
          request,
        } = queuedSearch;
        queuedSearch = undefined;
        try {
          const response = await backend.search(value, searchMode);
          if (disposed || request !== sequence) continue;
          // Read selection after the response. The user can navigate while a
          // refresh runs, but a different query must select its first result.
          const followNewestClipboard =
            searchMode === "clipboard" &&
            !value.trim() &&
            !selectionChangedByUser &&
            response.preferredSelectionId != null;
          const selectedId =
            preserveSelection &&
            !followNewestClipboard &&
            displayedQuery?.value === value &&
            displayedQuery.mode === searchMode
              ? resultKey(current())
              : undefined;
          const stableToolIndex =
            selectedId &&
            ["timezone", "webSearch"].includes(current()?.kind ?? "")
              ? Math.min(selected(), response.results.length - 1)
              : 0;
          const matchedIndex = response.results.findIndex(
            (result) => resultKey(result) === selectedId,
          );
          const preferredIndex = response.results.findIndex(
            (result) => result.id === response.preferredSelectionId,
          );
          displayedQuery = { value, mode: searchMode };
          batch(() => {
            setResults(response.results);
            setSelected(
              Math.max(
                0,
                matchedIndex >= 0
                  ? matchedIndex
                  : selectedId
                    ? stableToolIndex
                    : preferredIndex,
              ),
            );
            setTotal(response.total);
            setIndexing(response.indexing);
            setFiles(response.files);
            setCurrency(response.currency);
            setIndexError(response.indexError ?? undefined);
            setStorageError(response.storageError ?? undefined);
            setNotice(response.notice ?? undefined);
          });
        } catch (reason) {
          if (disposed || request !== sequence) continue;
          setResults([]);
          setError(String(reason));
        }
      }
    } finally {
      searchTask = undefined;
      if (!disposed) setPending(false);
    }
  }

  function changeQuery(value: string) {
    setQuery(value);
    setError(undefined);
    void search(value);
  }

  async function togglePin(category: SearchMode) {
    const result = current();
    if (!canOpen() || pinBusy() || !result?.pin) return;
    const pinned = !isPinned(result, category);
    setPinBusy(true);
    setMenuOpen(false);
    focusInput();
    setError(undefined);
    try {
      await backend.setPinned(result.id, category, pinned);
      if (!disposed) await search(query(), true);
    } catch (reason) {
      if (!disposed) setError(String(reason));
    } finally {
      if (!disposed) setPinBusy(false);
    }
  }

  function changeMode(value: SearchMode) {
    setMode(value);
    setMenuOpen(false);
    setError(undefined);
    void search();
    focusInput();
  }

  function cycleCategory(step: number) {
    const values = enabledCategories();
    const next =
      values[(values.indexOf(mode()) + step + values.length) % values.length];
    if (next !== mode()) changeMode(next);
  }

  function keepVisibleCategory() {
    if (!enabledCategories().includes(mode())) setMode(enabledCategories()[0]);
  }

  function runPrimary() {
    const result = current();
    if (result) void run(result.primaryAction, result);
  }

  async function run(action: Action, result = current()) {
    if (!result || !canOpen()) return;
    setMenuOpen(false);
    setError(undefined);
    if (result.confirmation && action === result.primaryAction) {
      // Capture the issued result. Background search updates must not change
      // the command that the dialog asks the user to confirm.
      setPendingAction(result);
      return;
    }
    await execute(result, action);
  }

  async function execute(
    result: SearchResult,
    action: Action,
    confirmed = false,
  ) {
    if (busy()) return;
    setBusy(true);
    setError(undefined);
    const previousQuery = query();
    const previousMode = mode();
    const previousSelection = selected();
    try {
      await backend.execute(result.id, action, confirmed);
      if (confirmed) setPendingAction(undefined);
      if (action === "delete") {
        await search();
        focusInput();
      }
      if (action === "regenerate") {
        await search();
        if (query() === previousQuery && mode() === previousMode)
          setSelected(
            Math.max(0, Math.min(previousSelection, results().length - 1)),
          );
        focusInput();
      }
    } catch (reason) {
      setError(String(reason));
      if (!pendingAction()) focusInput();
    } finally {
      setBusy(false);
    }
  }

  function closeConfirmation() {
    setPendingAction(undefined);
    setError(undefined);
    focusInput();
  }

  function confirmAction() {
    const result = pendingAction();
    if (result) void execute(result, result.primaryAction, true);
  }

  function confirmClear(keepPinned = true) {
    setMenuOpen(false);
    setError(undefined);
    setClearOpen(true);
    setKeepPins(keepPinned);
  }

  function closeClear() {
    setClearOpen(false);
    setError(undefined);
    focusInput();
  }

  async function clearHistory() {
    if (busy()) return;
    setBusy(true);
    setError(undefined);
    try {
      await backend.clearClipboard(keepPins());
      closeClear();
      await search();
    } catch (reason) {
      setError(String(reason));
    } finally {
      setBusy(false);
    }
  }

  async function hide() {
    if (!desktop) return;
    try {
      await backend.hide();
    } catch (reason) {
      setError(String(reason));
    }
  }

  async function refresh(
    target: "apps" | "files" | "currency" = mode() === "calculator"
      ? "currency"
      : mode() === "files"
        ? "files"
        : "apps",
  ) {
    setMenuOpen(false);
    setError(undefined);
    if (target === "files") setFiles((state) => ({ ...state, indexing: true }));
    else if (target === "apps") setIndexing(true);
    focusInput();
    try {
      if (target === "files") await backend.refreshFiles();
      else if (target === "currency") await backend.refreshCurrency();
      else await backend.refresh();
      await search();
    } catch (reason) {
      if (target === "files")
        setFiles((state) => ({ ...state, indexing: false }));
      else if (target === "apps") setIndexing(false);
      setError(String(reason));
    }
  }

  function toggleMenu() {
    setMenuFilter("");
    setMenuOpen(!menuOpen());
    if (menuOpen()) {
      queueMicrotask(() => menuInput?.focus());
    } else {
      focusInput();
    }
  }

  function onKey(event: KeyboardEvent) {
    if (event.isComposing || event.keyCode === 229) return;
    if (clipboardTool()) return;
    if (clearOpen() || pendingAction()) {
      if (event.key === "Escape") {
        event.preventDefault();
        if (!busy()) {
          if (clearOpen()) closeClear();
          else closeConfirmation();
        }
      }
      return;
    }
    const command =
      info()?.platform === "macos" ? event.metaKey : event.ctrlKey;
    if (command && event.code === "Comma" && desktop) {
      event.preventDefault();
      void openSettings();
      return;
    }
    if (event.key === "Escape") {
      event.preventDefault();
      if (menuOpen()) {
        setMenuOpen(false);
        focusInput();
      } else void hide();
      return;
    }
    if (command && event.key.toLowerCase() === "k") {
      event.preventDefault();
      toggleMenu();
      return;
    }
    if (command && event.key.toLowerCase() === "r" && desktop) {
      event.preventDefault();
      void refresh();
      return;
    }
    if (command && event.key.toLowerCase() === "q" && desktop) {
      event.preventDefault();
      void backend.quit().catch((reason) => setError(String(reason)));
      return;
    }
    if (menuOpen()) {
      if (event.key === "ArrowDown" || event.key === "ArrowUp") {
        event.preventDefault();
        const buttons = Array.from(
          menu?.querySelectorAll<HTMLButtonElement>("button:not(:disabled)") ??
            [],
        );
        const index = buttons.indexOf(
          document.activeElement as HTMLButtonElement,
        );
        buttons[
          (index < 0
            ? event.key === "ArrowDown"
              ? 0
              : buttons.length - 1
            : index + (event.key === "ArrowDown" ? 1 : -1) + buttons.length) %
            buttons.length
        ]?.focus();
      } else if (event.key === "Enter" && event.target === menuInput) {
        event.preventDefault();
        if (!event.repeat)
          menu
            ?.querySelector<HTMLButtonElement>("button:not(:disabled)")
            ?.click();
      } else if (event.key === "Tab") {
        setMenuOpen(false);
        focusInput();
      }
      return;
    }
    if (command && /^[1-9]$/.test(event.key)) {
      event.preventDefault();
      const result = results()[Number(event.key) - 1];
      if (result) void run(result.primaryAction, result);
      return;
    }
    if (
      command &&
      event.key === "Backspace" &&
      current()?.secondaryActions.includes("delete")
    ) {
      event.preventDefault();
      void run("delete");
      return;
    }
    // Enter on a focused button must keep the button's native action.
    if (
      welcome() &&
      !command &&
      !event.altKey &&
      !event.shiftKey &&
      (event.key === "ArrowDown" || event.key === "ArrowUp") &&
      (event.target === input ||
        (event.target instanceof Element &&
          event.target.closest(".suggestion-button")))
    ) {
      event.preventDefault();
      const buttons = Array.from(
        document.querySelectorAll<HTMLButtonElement>(".suggestion-button"),
      );
      const index = buttons.indexOf(
        document.activeElement as HTMLButtonElement,
      );
      const next =
        index < 0
          ? event.key === "ArrowDown"
            ? 0
            : buttons.length - 1
          : (index + (event.key === "ArrowDown" ? 1 : -1) + buttons.length) %
            buttons.length;
      buttons[next]?.focus();
      return;
    }
    if (event.target !== input) return;
    if (
      event.altKey &&
      !event.metaKey &&
      !event.ctrlKey &&
      (event.key === "ArrowLeft" || event.key === "ArrowRight")
    ) {
      event.preventDefault();
      cycleCategory(event.key === "ArrowRight" ? 1 : -1);
      return;
    }
    const plainKey = !event.metaKey && !event.ctrlKey && !event.altKey;
    if (event.key === "Tab" && plainKey) {
      event.preventDefault();
      cycleCategory(event.shiftKey ? -1 : 1);
      return;
    }
    const emojiGrid = mode() === "emoji";
    if (
      event.key === "ArrowDown" ||
      event.key === "ArrowUp" ||
      (emojiGrid &&
        !command &&
        !event.altKey &&
        !event.shiftKey &&
        (event.key === "ArrowLeft" || event.key === "ArrowRight"))
    ) {
      event.preventDefault();
      if (results().length) {
        const count = results().length;
        selectionChangedByUser = true;
        setSelected((index) => {
          if (
            emojiGrid &&
            (event.key === "ArrowDown" || event.key === "ArrowUp")
          ) {
            const cells = Array.from(
              list.querySelectorAll<HTMLElement>('[role="option"]'),
            );
            const left = cells[index].getBoundingClientRect().left;
            const column = cells
              .map((cell, position) => ({
                position,
                left: cell.getBoundingClientRect().left,
              }))
              .filter((cell) => Math.abs(cell.left - left) < 1);
            const position = column.findIndex(
              (cell) => cell.position === index,
            );
            const step = event.key === "ArrowDown" ? 1 : -1;
            return column[(position + step + column.length) % column.length]
              .position;
          }
          const step =
            event.key === "ArrowDown" || event.key === "ArrowRight" ? 1 : -1;
          return (index + step + count) % count;
        });
      }
    } else if (event.key === "Enter") {
      event.preventDefault();
      if (command && current()?.secondaryActions.includes("reveal"))
        void run("reveal");
      else if (!command && !event.altKey && !event.shiftKey) runPrimary();
    }
  }

  createEffect(() => {
    mode();
    visibleCategories();
    queueMicrotask(() =>
      categoryBar
        ?.querySelector('[aria-pressed="true"]')
        ?.scrollIntoView({ block: "nearest", inline: "nearest" }),
    );
  });

  createEffect(() => {
    const index = selected();
    results();
    appearance();
    queueMicrotask(() =>
      list
        ?.querySelector(`#result-${index}`)
        ?.scrollIntoView({ block: "nearest" }),
    );
  });

  createEffect(() => {
    if (!visible() || busy() || current()?.kind !== "timezone") return;
    // Refresh the displayed clock at the next minute without polling other tools.
    const timer = window.setTimeout(
      () => {
        if (!busy()) void search(query(), true);
      },
      60000 - (Date.now() % 60000) + 50,
    );
    onCleanup(() => window.clearTimeout(timer));
  });

  function outsideClick(event: PointerEvent) {
    if (menuOpen() && !(event.target as Element).closest(".actions-area")) {
      setMenuOpen(false);
    }
  }

  onMount(() => {
    focusInput();
    void watchAppearance(setAppearance).then((stop) => {
      if (disposed) stop();
      else unlisteners.push(stop);
    });
    document.addEventListener("keydown", onKey);
    document.addEventListener("pointerdown", outsideClick);
    if (!desktop) return;
    void (async () => {
      try {
        const register = async (
          name: string,
          callback: (payload: unknown) => void,
        ) => {
          const stop = await listen(name, (event) => callback(event.payload));
          if (disposed) stop();
          else unlisteners.push(stop);
        };
        await Promise.all([
          register("settings-changed", (payload) => {
            setInfo((current) =>
              current
                ? {
                    ...current,
                    settings: payload as LauncherInfo["settings"],
                    warnings: current.warnings.filter(
                      (warning) =>
                        !(
                          warning.startsWith("Could not register ") &&
                          (current.settings.shortcut !==
                            (payload as LauncherInfo["settings"]).shortcut ||
                            JSON.stringify(
                              current.settings.categoryShortcuts,
                            ) !==
                              JSON.stringify(
                                (payload as LauncherInfo["settings"])
                                  .categoryShortcuts,
                              ))
                        ) && !warning.startsWith("Could not read settings."),
                    ),
                  }
                : current,
            );
            keepVisibleCategory();
            void search(query(), true);
          }),
          register("apps-changed", () => {
            void search(query(), true);
          }),
          register("pins-changed", () => {
            void search(query(), true);
          }),
          register("files-changed", () => {
            void search(query(), true);
          }),
          register("currency-changed", () => {
            if (mode() === "calculator" || mode() === "all")
              void search(query(), true);
          }),
          register("usage-changed", () => {
            void search(query(), true);
          }),
          register("clipboard-changed", () => {
            if (mode() === "clipboard" || mode() === "all")
              void search(query(), true);
          }),
          register("launcher-opened", (clear) => {
            // Show the preview only after the new search settles. Publishing
            // visibility first would briefly request the previous preview.
            batch(() => {
              setVisible(true);
              setMenuOpen(false);
              setClearOpen(false);
              setPendingAction(undefined);
              setClipboardTool(undefined);
              setError(undefined);
              if (clear === true) {
                setMode(enabledCategories()[0]);
                changeQuery("");
              } else {
                void search();
              }
            });
            focusInput();
            if (clear !== true) input.select();
          }),
          register("launcher-category", (payload) => {
            const category = categories.find(({ id }) => id === payload);
            if (category) {
              setQuery("");
              changeMode(category.id);
            }
          }),
          register("launcher-hidden", () => {
            setVisible(false);
            // One running Rust search may finish. Ignore its reply and drop
            // waiting input. Opening the window always requests current data.
            queuedSearch = undefined;
            sequence += 1;
            setPending(false);
          }),
        ]);
        if (disposed) return;
        const initial = await backend.ready();
        setInfo(initial);
        setVisible(initial.visible ?? true);
        if (initial.initialMode) setMode(initial.initialMode);
        else keepVisibleCategory();
        await search();
        focusInput();
      } catch (reason) {
        setIndexing(false);
        setError(`Could not connect to TinyDash. ${String(reason)}`);
      }
    })();
  });

  onCleanup(() => {
    disposed = true;
    disposeAppIcons();
    sequence += 1;
    unlisteners.forEach((stop) => stop());
    document.removeEventListener("keydown", onKey);
    document.removeEventListener("pointerdown", outsideClick);
  });

  return (
    <main class="launcher" aria-label="TinyDash launcher">
      <div
        class="window-drag-handle"
        title="Drag to move window"
        aria-hidden="true"
        onMouseDown={startDragging}
      />
      <header class="search-header">
        <div class="search-field">
          <span class="search-symbol">
            <Icon name="search" size={22} />
          </span>
          <input
            ref={input}
            type="text"
            role="combobox"
            aria-label="Search TinyDash"
            aria-autocomplete="list"
            aria-expanded={results().length > 0}
            aria-controls="search-results"
            aria-activedescendant={
              current() ? `result-${selected()}` : undefined
            }
            placeholder={placeholder()}
            autocomplete="off"
            autocapitalize="off"
            spellcheck={false}
            maxLength={8192}
            value={query()}
            onInput={(event) => changeQuery(event.currentTarget.value)}
          />
          <Show when={query() && results().length > 0}>
            <button
              class="clear-query"
              aria-label="Clear search"
              title="Clear search"
              onClick={() => {
                changeQuery("");
                focusInput();
              }}
            >
              <Icon name="close" size={16} />
            </button>
          </Show>
          <button
            class="escape-key"
            aria-label="Hide launcher"
            title="Hide launcher (Esc)"
            onClick={() => void hide()}
            disabled={!desktop}
          >
            <kbd>esc</kbd>
          </button>
        </div>
        <nav class="category-bar" aria-label="Search categories">
          <div class="category-tabs" ref={categoryBar}>
            <For each={visibleCategories()}>
              {(category) => (
                <button
                  class="category-tab"
                  aria-pressed={mode() === category.id}
                  onClick={() => changeMode(category.id)}
                >
                  {category.label}
                </button>
              )}
            </For>
          </div>
          <span
            class="category-hint"
            title="Tab: next category. Shift+Tab: previous category."
          >
            <kbd>Tab</kbd> to switch
          </span>
        </nav>
      </header>
      <Show
        when={desktop && info()?.settings.clipboardHistoryDecided === false}
      >
        <aside class="first-use" aria-label="Clipboard history choice">
          <div>
            <strong>Save clipboard history?</strong>
            <p>
              TinyDash saves copied text on this device as plain text. It can
              include passwords and other private text. Choose before capture
              starts.
            </p>
            <p>
              Launcher shortcut: <kbd>{info()?.settings.shortcut}</kbd>. You can
              change this in Settings.
            </p>
            <Show when={info()?.platform === "linux"}>
              <p>On Wayland, set a desktop shortcut to run tinydash.</p>
            </Show>
            <p>
              Currency tools can download exchange rates. Turn off these
              requests in Settings &gt; Currency.
            </p>
          </div>
          <div class="first-use-actions">
            <button
              disabled={busy()}
              onClick={() => void chooseClipboardHistory(false)}
            >
              Keep history off
            </button>
            <button
              disabled={busy()}
              onClick={() => void chooseClipboardHistory(true)}
            >
              Enable history
            </button>
          </div>
        </aside>
      </Show>
      <Show
        when={!message() && !welcome() && mode() === "all" && files().indexing}
      >
        <div class="status-line indexing-status">
          <span class="query-hint" role="status">
            Scanning files... You can search apps now.
          </span>
        </div>
      </Show>
      <Show when={message()}>
        <div class="status-line">
          <span class="error-message" role="alert">
            {message()}
          </span>
          <Show when={mode() === "calculator" && currency().warning}>
            <button
              class="text-button"
              disabled={!desktop || currency().refreshing}
              onClick={() => void refresh("currency")}
            >
              Retry
            </button>
          </Show>
        </div>
      </Show>

      <section
        class="results-area"
        classList={{
          "welcome-results": welcome(),
          "emoji-results": mode() === "emoji",
          "clipboard-results": current()?.kind === "clipboard",
        }}
        aria-label="Search results"
      >
        <div class="result-column">
          <Show when={welcome()}>
            <WelcomeSuggestions
              appsAvailable={enabledCategories().includes("apps")}
              onBrowseApps={() => changeMode("apps")}
              onQuery={(value) => {
                changeQuery(value);
                focusInput();
              }}
            />
          </Show>
          <Show when={mode() === "clipboard"}>
            <div class="list-tools">
              <span>Saved on this device</span>
              <button
                class="text-button clear-history"
                disabled={!desktop || busy()}
                onClick={() => confirmClear(true)}
              >
                Clear unpinned
              </button>
            </div>
          </Show>
          <ul
            id="search-results"
            ref={list}
            class="result-list"
            role="listbox"
            aria-label="Search results"
            aria-busy={pending()}
          >
            <For each={results()}>
              {(result, index) => (
                <li role="presentation" class="result-entry">
                  <Show
                    when={
                      index() === 0 ||
                      groupLabel(results()[index() - 1]) !== groupLabel(result)
                    }
                  >
                    <div class="result-group" aria-hidden="true">
                      {groupLabel(result)}
                      <span class="group-count">
                        {
                          results().filter(
                            (item) => groupLabel(item) === groupLabel(result),
                          ).length
                        }
                      </span>
                    </div>
                  </Show>
                  <div
                    id={`result-${index()}`}
                    role="option"
                    aria-selected={index() === selected()}
                    class="result-row"
                    classList={{
                      selected: index() === selected(),
                      pending: pending(),
                      "calculation-row": result.kind === "calculation",
                      "password-row": result.kind === "password",
                    }}
                    onPointerMove={() => {
                      if (!pending()) {
                        selectionChangedByUser = true;
                        setSelected(index());
                      }
                    }}
                    onMouseDown={(event) => event.preventDefault()}
                    onClick={() => void run(result.primaryAction, result)}
                  >
                    <ResultIcon result={result} active={visible()} />
                    <span class="result-copy">
                      <span class="result-title" title={result.title}>
                        {result.title}
                      </span>
                      <span class="result-subtitle" title={result.subtitle}>
                        {result.subtitle}
                      </span>
                    </span>
                    <Show when={isPinned(result)}>
                      <span
                        class="result-pin"
                        title={`Pinned to ${categories.find(({ id }) => id === mode())?.label}`}
                        aria-label={`Pinned to ${categories.find(({ id }) => id === mode())?.label}`}
                      >
                        <Icon name="pin" size={13} />
                      </span>
                    </Show>
                    <Show when={index() < 9}>
                      <kbd class="result-shortcut">
                        <span>{modifier()}</span>
                        <span>{index() + 1}</span>
                      </kbd>
                    </Show>
                  </div>
                </li>
              )}
            </For>
          </ul>
          <Show when={results().length === 0 && !welcome()}>
            <div class="empty-state">
              <div class="empty-icon">
                <Icon name={query() ? "search" : "apps"} size={28} />
              </div>
              <h1>
                {!desktop
                  ? "TinyDash, one shortcut away."
                  : mode() === "password"
                    ? "Generate a password"
                    : mode() === "timezone"
                      ? "Calculate a date or time"
                      : mode() === "url"
                        ? "Clean a URL"
                        : mode() === "web"
                          ? "Search the web"
                          : mode() === "system"
                            ? "No system commands found"
                            : mode() === "calculator"
                              ? "Calculate and convert"
                              : mode() === "emoji"
                                ? "No emoji found"
                                : mode() === "clipboard"
                                  ? query()
                                    ? "No clipboard entries found"
                                    : "No saved clipboard text"
                                  : mode() === "files"
                                    ? files().indexing
                                      ? "Finding your files"
                                      : query()
                                        ? "No files found"
                                        : "No files in the index"
                                    : mode() === "all" &&
                                        files().indexing &&
                                        query()
                                      ? "No results yet"
                                      : indexing()
                                        ? "Finding your applications"
                                        : query()
                                          ? "No results found"
                                          : "No applications in the index"}
              </h1>
              <p>
                {!desktop
                  ? "Start the TinyDash desktop app to search this computer."
                  : mode() === "password"
                    ? "Try password 32, passphrase 6, or pin 6."
                    : mode() === "timezone"
                      ? "Try next Friday + 2 weeks, time in Tokyo, or 10am Pacific Time."
                      : mode() === "url"
                        ? "Paste a full http:// or https:// URL."
                        : mode() === "web"
                          ? "Type a search, then choose an engine."
                          : mode() === "system"
                            ? "Try sleep, restart, or settings."
                            : mode() === "calculator"
                              ? "Try 12 * 8, 5 ft to cm, or 100 USD to MYR."
                              : mode() === "emoji"
                                ? "Try a name, shortcode, or category, such as coffee or food."
                                : mode() === "clipboard"
                                  ? info()?.settings.clipboardHistoryEnabled ===
                                    false
                                    ? "Clipboard capture is off in Settings."
                                    : query()
                                      ? "Try a word from the text you copied."
                                      : "Copy text in any application. It will appear here."
                                  : mode() === "files"
                                    ? files().indexing
                                      ? "You can search applications while the scan runs."
                                      : query()
                                        ? "Try a filename or part of a path."
                                        : info()?.settings.fileSearchRoots
                                              ?.length === 0
                                          ? "File search is off in settings.json."
                                          : "Check your folders in settings.json, then refresh the file list."
                                    : mode() === "all" &&
                                        files().indexing &&
                                        query()
                                      ? "The file scan is still running. You can search applications now."
                                      : indexing()
                                        ? "You can start typing while the list loads."
                                        : query()
                                          ? "Try a name, a file path, or a calculation."
                                          : "Refresh the list after you install an application."}
              </p>
              <Show
                when={
                  desktop &&
                  !(mode() === "files" ? files().indexing : indexing()) &&
                  (query() ||
                    mode() === "all" ||
                    mode() === "apps" ||
                    mode() === "files")
                }
              >
                <button
                  class="text-button"
                  onClick={() => {
                    if (query()) {
                      changeQuery("");
                      focusInput();
                    } else {
                      void refresh();
                    }
                  }}
                >
                  {query()
                    ? "Clear search"
                    : mode() === "files"
                      ? "Refresh files"
                      : "Refresh applications"}
                </button>
              </Show>
            </div>
          </Show>
        </div>
        <Show when={mode() !== "emoji"}>
          <ResultPreview
            active={visible()}
            result={current()}
            welcome={!current()}
            previewReady={visible() && !pending()}
            enabled={canOpen()}
            modifier={modifier()}
            pinOptions={pinOptions()}
            pinBusy={pinBusy()}
            onPin={(category) => void togglePin(category)}
            onAction={(action) => void run(action)}
          />
        </Show>
      </section>

      <footer class="footer">
        <div class="footer-actions">
          <Show
            when={!welcome()}
            fallback={<span class="footer-prompt">Type to search</span>}
          >
            <button
              class="open-button"
              disabled={!canOpen()}
              onClick={runPrimary}
            >
              {busy()
                ? current()?.primaryAction === "run"
                  ? "Running..."
                  : current()?.primaryAction === "copy"
                    ? "Copying..."
                    : "Opening..."
                : primaryLabel()}
              <Icon name="return" size={17} />
            </button>
            <span class="footer-selection" title={current()?.title}>
              {current()?.title ?? "TinyDash"}
            </span>
          </Show>
          <span class="navigation-hint">
            <kbd>↑</kbd>
            <kbd>↓</kbd> navigate
          </span>
          <span class="list-count" role="status" aria-live="polite">
            {welcome()
              ? "0 results"
              : mode() === "calculator"
                ? currency().refreshing
                  ? "Updating rates..."
                  : currency().asOf
                    ? `Rates ${currency().asOf}`
                    : "Arithmetic and units offline"
                : mode() === "files"
                  ? files().indexing
                    ? "Scanning files..."
                    : `${files().total} ${files().total === 1 ? "file" : "files"} indexed`
                  : mode() === "emoji" ||
                      mode() === "clipboard" ||
                      mode() === "system" ||
                      ["password", "timezone", "url", "web"].includes(mode())
                    ? `${results().length} shown`
                    : indexing()
                      ? "Finding applications..."
                      : pending()
                        ? "Searching..."
                        : query().trim()
                          ? `${results().length} ${results().length === 1 ? "result" : "results"}`
                          : mode() === "all"
                            ? `${results().length} pinned`
                            : `${total()} installed`}
          </span>

          <div class="actions-area">
            <button
              class="actions-button"
              aria-haspopup="menu"
              aria-expanded={menuOpen()}
              aria-controls="actions-menu"
              onClick={toggleMenu}
            >
              Actions <kbd>{modifier()} K</kbd>
            </button>
            <Show when={menuOpen()}>
              <button
                class="menu-scrim"
                aria-label="Close actions"
                onClick={toggleMenu}
              />
              <div
                id="actions-menu"
                ref={menu}
                class="actions-menu"
                role="menu"
                aria-label="Launcher actions"
              >
                <input
                  ref={menuInput}
                  class="actions-filter"
                  type="search"
                  aria-label="Search actions"
                  placeholder="Search actions..."
                  autocomplete="off"
                  value={menuFilter()}
                  onInput={(event) => setMenuFilter(event.currentTarget.value)}
                />
                <div class="menu-column">
                  <Show when={resultActions().length > 0}>
                    <div class="menu-heading">
                      {current()?.title ?? "TinyDash"}
                    </div>
                  </Show>
                  <For each={resultActions()}>
                    {(action) => (
                      <button
                        role="menuitem"
                        disabled={action.disabled}
                        onClick={action.run}
                      >
                        <Icon name={action.icon} />
                        {action.label}
                        <Show when={action.key}>
                          <kbd>{action.key}</kbd>
                        </Show>
                      </button>
                    )}
                  </For>
                </div>
                <div class="menu-column">
                  <Show
                    when={appearances.some((item) =>
                      matchesMenu(`Appearance ${item.label}`),
                    )}
                  >
                    <div
                      class="appearance-group"
                      role="group"
                      aria-label="Appearance"
                    >
                      <div class="menu-heading">Appearance</div>
                      <div class="appearance-options">
                        <For
                          each={appearances.filter((item) =>
                            matchesMenu(`Appearance ${item.label}`),
                          )}
                        >
                          {(item) => (
                            <button
                              role="menuitemradio"
                              aria-checked={appearance() === item.id}
                              title={item.description}
                              onClick={() => changeAppearance(item.id)}
                            >
                              <span
                                class={`appearance-swatch swatch-${item.id}`}
                                aria-hidden="true"
                              />
                              {item.label}
                            </button>
                          )}
                        </For>
                      </div>
                    </div>
                    <div class="menu-divider" />
                  </Show>
                  <For each={launcherActions()}>
                    {(action) => (
                      <button
                        role="menuitem"
                        disabled={action.disabled}
                        onClick={action.run}
                      >
                        <Icon name={action.icon} />
                        {action.label}
                        <Show when={action.key}>
                          <kbd>{action.key}</kbd>
                        </Show>
                      </button>
                    )}
                  </For>
                </div>
                <Show
                  when={
                    !resultActions().length &&
                    !launcherActions().length &&
                    !appearances.some((item) =>
                      matchesMenu(`Appearance ${item.label}`),
                    )
                  }
                >
                  <p role="status" class="actions-empty">
                    No matching actions
                  </p>
                </Show>
              </div>
            </Show>
          </div>
        </div>
      </footer>
      <Show when={clearOpen()}>
        <ConfirmDialog
          title={
            keepPins()
              ? "Clear unpinned history?"
              : "Clear all clipboard history?"
          }
          description={
            keepPins()
              ? "This deletes unpinned text. Entries pinned in All or Clipboard stay saved. The system clipboard does not change."
              : "This deletes all saved text, including entries pinned in All and Clipboard. The system clipboard does not change."
          }
          confirmLabel={keepPins() ? "Clear unpinned" : "Clear all history"}
          busyLabel="Clearing..."
          busy={busy()}
          error={error()}
          onClose={closeClear}
          onConfirm={() => void clearHistory()}
        />
      </Show>
      <Show when={clipboardTool()} keyed>
        {(tool) => (
          <ClipboardCopyDialog
            {...tool}
            onClose={closeClipboardTool}
            onCopied={() => {
              closeClipboardTool();
              void search(query(), true).then(() =>
                setNotice("Text copied. Paste it in the target app."),
              );
            }}
          />
        )}
      </Show>
      <Show when={pendingAction()?.confirmation} keyed>
        {(confirmation) => (
          <ConfirmDialog
            title={confirmation.title}
            description={confirmation.description}
            confirmLabel={confirmation.confirmLabel}
            busyLabel="Running..."
            busy={busy()}
            error={error()}
            onClose={closeConfirmation}
            onConfirm={confirmAction}
          />
        )}
      </Show>
    </main>
  );
}
