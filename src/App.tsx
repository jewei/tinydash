import {
  createEffect,
  createMemo,
  createSignal,
  For,
  flush,
  onCleanup,
  onSettled,
  snapshot,
  Show,
} from "solid-js";
import {
  createLauncherController,
  receiveLauncherSettings,
} from "./launcherController";
import { isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { createNativeSubscriptions } from "./nativeSubscriptions";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  backend,
  type Action,
  type LauncherInfo,
  type SearchResult,
  type SearchMode,
} from "./bridge";
import Icon from "./components/Icon";
import ResultIcon from "./components/ResultIcon";
import ResultPreview from "./components/ResultPreview";
import ConfirmDialog from "./components/ConfirmDialog";
import ClipboardCopyDialog from "./components/ClipboardCopyDialog";
import WelcomeSuggestions from "./components/WelcomeSuggestions";
import LibraryPanel from "./components/LibraryPanel";
import RichClipboardHistory from "./components/RichClipboardHistory";
import UtilitiesPanel, {
  UtilitiesAwakeIndicator,
} from "./components/UtilitiesPanel";
import "./styles/native-views.css";
import {
  appearances,
  readAppearance,
  readCompact,
  readFollowSystemGlass,
  saveAppearance,
  saveCompact,
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
  folder: "Files",
  clipboard: "Clipboard history",
  emoji: "Emoji",
  calculation: "Calculator",
  systemCommand: "System commands",
  password: "Password generator",
  timezone: "Datetime",
  cleanedUrl: "URL cleaner",
  webSearch: "Web search",
};

function snapshotResult(result: SearchResult): SearchResult {
  return structuredClone(snapshot(result));
}

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
  const [nativeGlass, setNativeGlass] = createSignal(false);
  const [followSystemGlass, setFollowSystemGlass] = createSignal(
    readFollowSystemGlass(),
  );
  const [compact, setCompact] = createSignal(readCompact());
  const controller = createLauncherController({
    desktop,
    send: backend.search,
    cancelBackend: backend.cancelSearch,
  });
  const {
    visible,
    setVisible,
    query,
    setQuery,
    mode,
    setMode,
    results,
    selected,
    setSelected,
    current,
    pending,
    total,
    indexing,
    setIndexing,
    files,
    currency,
    searchError,
    clearSearchError,
    indexError,
    storageError,
    notice,
    setNotice,
    search,
    changeQuery: changeSearchQuery,
    markSelectionChanged,
  } = controller;
  // Actions and startup own their failures independently of search refreshes.
  const [operationError, setOperationError] = createSignal<string>();
  const error = () => operationError() ?? searchError();
  function setError(reason: string | undefined) {
    setOperationError(reason);
    // Existing explicit dismissals (input, actions, reopen) clear both owners.
    // Search delivery never uses this setter.
    if (reason === undefined) clearSearchError();
  }
  function changeQuery(value: string) {
    setOperationError(undefined);
    changeSearchQuery(value);
  }
  const [pinBusy, setPinBusy] = createSignal(false);
  const [info, setInfo] = createSignal<LauncherInfo>();
  const [busy, setBusy] = createSignal(false);
  const [menuOpen, setMenuOpen] = createSignal(false);
  const [panel, setPanel] = createSignal<string>();
  const [panelNotice, setPanelNotice] = createSignal<string>();
  function openPanel(name: string) {
    if (panel()) {
      if (panel() !== name)
        setPanelNotice(
          "Close the current view before opening another command.",
        );
      return;
    }
    setMenuOpen(false);
    setPanelNotice(undefined);
    setPanel(name);
  }
  function closePanel() {
    setPanelNotice(undefined);
    setPanel(undefined);
    void search(query(), true);
    queueMicrotask(() => input?.focus());
  }
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
  let disposed = false;
  const subscriptions = createNativeSubscriptions(listen);

  const modifier = () => (info()?.platform === "macos" ? "⌘" : "Ctrl");
  const enabledCategories = createMemo(() =>
    normalizeCategories(info()?.settings.visibleCategories),
  );
  const visibleCategories = createMemo(() =>
    categories.filter(({ id }) => enabledCategories().includes(id)),
  );
  const welcome = () =>
    mode() === "all" &&
    !query().trim() &&
    results().length === 0 &&
    !searchError();
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
  const filePending = () =>
    files().phase === "queued" || files().phase === "scanning";
  const fileActivity = () =>
    files().phase === "queued"
      ? "Waiting to refresh files..."
      : "Scanning files...";
  const message = () =>
    error() ??
    notice() ??
    indexError() ??
    storageError()?.message ??
    (mode() === "all" || mode() === "files" ? files().warning : undefined) ??
    (mode() === "calculator" ? currency().warning : undefined) ??
    info()?.warnings[0]?.message;
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
  const focusInput = () =>
    queueMicrotask(() => input.focus({ preventScroll: true }));

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
    const actions: MenuAction[] = current()
      ? [
          {
            label:
              primaryLabel() === "Open"
                ? current()?.kind === "file"
                  ? "Open file"
                  : current()?.kind === "folder"
                    ? "Open folder"
                    : "Open application"
                : primaryLabel(),
            icon: current()?.primaryAction === "copy" ? "copy" : "return",
            run: runPrimary,
            disabled: !canOpen(),
            key: "↵",
          },
        ]
      : [];
    if (
      current()?.primaryAction === "copy" ||
      current()?.secondaryActions.includes("copy")
    )
      actions.push({
        label: "Paste to previous app",
        icon: "clipboard",
        run: () => void run("paste"),
        disabled: !canOpen(),
        key: `${modifier()} ⇧ ↵`,
      });
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
        label: "Quicklinks and snippets",
        icon: "file" as const,
        run: () => openPanel("quicklinks"),
        disabled: !desktop,
      },
      {
        label: "Utilities",
        icon: "system" as const,
        run: () => openPanel("processes"),
        disabled: !desktop,
      },
      {
        label: "Images and files clipboard",
        icon: "clipboard" as const,
        run: () => openPanel("rich-clipboard"),
        disabled: !desktop,
      },
      {
        label: "Settings",
        icon: "system" as const,
        run: () => void openSettings(),
        disabled: !desktop,
        key: `${modifier()} ,`,
      },
      {
        label: "Reset window position",
        icon: "refresh" as const,
        run: () => void resetPosition(),
        disabled: !desktop,
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
        disabled: !desktop || filePending() || files().phase === "disabled",
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

  async function resetPosition() {
    setMenuOpen(false);
    setError(undefined);
    try {
      await backend.resetPosition();
    } catch (reason) {
      setError(String(reason));
    } finally {
      focusInput();
    }
  }

  function openClipboardTool(tool: "edit" | "combine") {
    const entry = current();
    if (!entry || entry.kind !== "clipboard" || !canOpen()) return;
    setMenuOpen(false);
    setClipboardTool({
      mode: tool,
      entry: snapshotResult(entry),
      entries: results()
        .filter((entry) => entry.kind === "clipboard")
        .map(snapshotResult),
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

  createEffect(
    () => ({ appearance: appearance(), glass: followSystemGlass() }),
    ({ appearance, glass }) => {
      document.documentElement.dataset.appearance = appearance;
      if (!desktop || !glass) {
        setNativeGlass(false);
        return;
      }
      let current = true;
      void backend.setLauncherAppearance(appearance).then(
        (enabled) => {
          if (current) setNativeGlass(enabled);
        },
        (reason: unknown) => {
          if (current) setNativeGlass(false);
          console.warn(
            "Could not apply the native launcher background.",
            reason,
          );
        },
      );
      return () => {
        current = false;
      };
    },
  );

  createEffect(compact, (value) => {
    document.documentElement.dataset.compact = String(value);
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
    flush();
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
      setPendingAction(snapshotResult(result));
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
    if (target === "apps") setIndexing(true);
    focusInput();
    try {
      if (target === "files") await backend.refreshFiles();
      else if (target === "currency") await backend.refreshCurrency();
      else await backend.refresh();
      await search();
    } catch (reason) {
      if (target === "apps") setIndexing(false);
      setError(String(reason));
    }
  }

  function toggleMenu() {
    const opening = !menuOpen();
    setMenuFilter("");
    setMenuOpen(opening);
    if (opening) {
      queueMicrotask(() => menuInput?.focus());
    } else {
      focusInput();
    }
  }

  let composing = false;
  let compositionTimer: number | undefined;
  function clearComposition() {
    window.clearTimeout(compositionTimer);
    compositionTimer = undefined;
    composing = false;
  }
  function startComposition() {
    clearComposition();
    composing = true;
  }
  function endComposition() {
    // WebKit can end composition before delivering the Enter that commits it.
    // Keep that event out of launcher navigation until this event turn finishes.
    window.clearTimeout(compositionTimer);
    compositionTimer = window.setTimeout(clearComposition, 0);
  }

  function onKey(event: KeyboardEvent) {
    if (composing || event.isComposing || event.keyCode === 229) return;
    if (panel()) return;
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
    if (
      command &&
      event.shiftKey &&
      event.key === "Enter" &&
      (current()?.primaryAction === "copy" ||
        current()?.secondaryActions.includes("copy"))
    ) {
      event.preventDefault();
      void run("paste");
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
        markSelectionChanged();
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

  createEffect(
    () => [mode(), visibleCategories()],
    () => {
      queueMicrotask(() =>
        categoryBar
          ?.querySelector('[aria-pressed="true"]')
          ?.scrollIntoView({ block: "nearest", inline: "nearest" }),
      );
    },
  );

  createEffect(
    () => ({ index: selected(), results: results(), appearance: appearance() }),
    ({ index }) => {
      queueMicrotask(() =>
        list
          ?.querySelector(`#result-${index}`)
          ?.scrollIntoView({ block: "nearest" }),
      );
    },
  );

  createEffect(
    () => visible() && !busy() && current()?.kind === "timezone",
    (active) => {
      if (!active) return;
      // Refresh the displayed clock at the next minute without polling other tools.
      const timer = window.setTimeout(
        () => {
          if (!busy()) void search(query(), true);
        },
        60000 - (Date.now() % 60000) + 50,
      );
      return () => window.clearTimeout(timer);
    },
  );

  function outsideClick(event: PointerEvent) {
    if (menuOpen() && !(event.target as Element).closest(".actions-area")) {
      setMenuOpen(false);
    }
  }

  onSettled(() => {
    focusInput();
    void subscriptions
      .own(
        watchAppearance(
          subscriptions.guard(setAppearance),
          subscriptions.guard(setCompact),
          subscriptions.guard(setFollowSystemGlass),
        ),
      )
      .catch((reason) => {
        if (!disposed) setError(String(reason));
      });
    document.addEventListener("keydown", onKey);
    document.addEventListener("compositionstart", startComposition);
    document.addEventListener("compositionend", endComposition);
    document.addEventListener("pointerdown", outsideClick);
    if (!desktop) return;
    void (async () => {
      try {
        const { register } = subscriptions;
        await Promise.all([
          register<LauncherInfo["settings"]>("settings-changed", (settings) => {
            setInfo((current) =>
              current ? receiveLauncherSettings(current, settings) : current,
            );
            keepVisibleCategory();
            void search(query(), true);
          }),
          register("open-panel", (payload) => openPanel(String(payload))),
          register("library-changed", () => {
            if (!panel()) void search(query(), true);
          }),
          register("confirm-item", (payload) => {
            if (panel()) {
              setPanelNotice(
                "Close the current view, then press the shortcut again to confirm this command.",
              );
              return;
            }
            setPendingAction(payload as SearchResult);
            setError(undefined);
          }),
          register("action-error", (payload) => setError(String(payload))),
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
            clearComposition();
            controller.hidden();
          }),
        ]);
        if (disposed) return;
        const initial = await backend.ready();
        if (disposed) return;
        setInfo(initial);
        setVisible(initial.visible ?? true);
        if (initial.initialMode) setMode(initial.initialMode);
        else keepVisibleCategory();
        await search();
        if (!disposed) focusInput();
      } catch (reason) {
        if (disposed) return;
        setIndexing(false);
        setError(`Could not connect to TinyDash. ${String(reason)}`);
      }
    })();
  });

  onCleanup(() => {
    disposed = true;
    controller.dispose();
    subscriptions.dispose();
    document.removeEventListener("keydown", onKey);
    document.removeEventListener("compositionstart", startComposition);
    document.removeEventListener("compositionend", endComposition);
    clearComposition();
    document.removeEventListener("pointerdown", outsideClick);
  });

  return (
    <>
      <Show when={panel()} keyed>
        {(name) => (
          <div
            class="native-view"
            onKeyDown={(event) => {
              if (event.key === "Escape" && !event.defaultPrevented) {
                event.preventDefault();
                event.stopPropagation();
                closePanel();
              }
            }}
          >
            <Show when={panelNotice()}>
              <p class="native-view-notice" role="status">
                {panelNotice()}
              </p>
            </Show>
            <Show
              when={name !== "rich-clipboard"}
              fallback={<RichClipboardHistory onClose={closePanel} />}
            >
              <Show
                when={
                  name === "quicklinks" ||
                  name === "snippets" ||
                  name.startsWith("library:")
                }
                fallback={
                  <UtilitiesPanel initialTab={name} onClose={closePanel} />
                }
              >
                <LibraryPanel
                  initialId={name.startsWith("library:") ? name : undefined}
                  onClose={closePanel}
                />
              </Show>
            </Show>
          </div>
        )}
      </Show>
      <main
        style={{ display: panel() ? "none" : undefined }}
        class="launcher"
        data-native-glass={nativeGlass() ? "true" : undefined}
        aria-label="TinyDash launcher"
      >
        <div
          class="window-drag-handle"
          title="Drag to move window"
          aria-hidden="true"
          onMouseDown={startDragging}
        />
        <header class="search-header">
          <Show when={desktop}>
            <UtilitiesAwakeIndicator />
          </Show>
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
              aria-expanded={results().length > 0 ? "true" : "false"}
              aria-controls="search-results"
              aria-activedescendant={
                current() ? `result-${selected()}` : undefined
              }
              placeholder={placeholder()}
              autocomplete="off"
              autocorrect="off"
              autocapitalize="off"
              writingsuggestions="false"
              spellcheck={false}
              maxlength={8192}
              value={query()}
              onInput={(event) => changeQuery(event.currentTarget.value)}
            />
            <Show when={query()}>
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
                    aria-pressed={mode() === category.id ? "true" : "false"}
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
                Launcher shortcut: <kbd>{info()?.settings.shortcut}</kbd>. You
                can change this in Settings.
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
          when={!message() && !welcome() && mode() === "all" && filePending()}
        >
          <div class="status-line indexing-status">
            <span class="query-hint" role="status">
              {fileActivity()} You can search apps now.
            </span>
          </div>
        </Show>
        <Show when={message() && !pendingAction() && !clearOpen()}>
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
          class={{
            "results-area": true,
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
              aria-busy={pending() ? "true" : "false"}
            >
              <For each={results()}>
                {(result, index) => (
                  <li role="presentation" class="result-entry">
                    <Show
                      when={
                        index() === 0 ||
                        groupLabel(results()[index() - 1]) !==
                          groupLabel(result)
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
                      aria-selected={index() === selected() ? "true" : "false"}
                      class={{
                        "result-row": true,
                        selected: index() === selected(),
                        pending: pending(),
                        "calculation-row": result.kind === "calculation",
                        "password-row": result.kind === "password",
                      }}
                      onPointerMove={() => {
                        if (!pending()) {
                          markSelectionChanged();
                          setSelected(index());
                        }
                      }}
                      onMouseDown={(event) => event.preventDefault()}
                      onClick={() => void run(result.primaryAction, result)}
                    >
                      <ResultIcon result={result} />
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
                    : searchError()
                      ? "Search unavailable"
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
                                        ? files().phase === "queued"
                                          ? "Waiting to refresh files"
                                          : files().phase === "scanning"
                                            ? "Finding your files"
                                            : files().phase === "disabled"
                                              ? "File search is off"
                                              : files().phase === "failed"
                                                ? "File scan unavailable"
                                                : query()
                                                  ? "No files found"
                                                  : "No files in the index"
                                        : mode() === "all" &&
                                            filePending() &&
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
                    : searchError()
                      ? "Check the message above, then change or clear your search to try again."
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
                                  ? "Try 12 * 8, 5 ft to cm, or 100 USD MYR."
                                  : mode() === "emoji"
                                    ? "Try a name, shortcode, or category, such as coffee or food."
                                    : mode() === "clipboard"
                                      ? info()?.settings
                                          .clipboardHistoryEnabled === false
                                        ? "Clipboard capture is off in Settings."
                                        : query()
                                          ? "Try a word from the text you copied."
                                          : "Copy text in any application. It will appear here."
                                      : mode() === "files"
                                        ? filePending()
                                          ? files().phase === "queued"
                                            ? "The refresh is queued. You can search applications while waiting."
                                            : "You can search applications while the scan runs."
                                          : files().phase === "disabled"
                                            ? "File search is off. Choose folders in Settings, File search."
                                            : files().phase === "failed"
                                              ? "Use Refresh files to try again."
                                              : query()
                                                ? "Try a filename or part of a path."
                                                : "Check your folders in Settings, File search, then refresh the file list."
                                        : mode() === "all" &&
                                            filePending() &&
                                            query()
                                          ? `${fileActivity()} You can search applications now.`
                                          : indexing()
                                            ? "You can start typing while the list loads."
                                            : query()
                                              ? "Try a name, a file path, or a calculation."
                                              : "Refresh the list after you install an application."}
                </p>
                <Show
                  when={
                    desktop &&
                    !(mode() === "files"
                      ? filePending() || files().phase === "disabled"
                      : indexing()) &&
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
                      ? "Reset search"
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
              result={current()}
              welcome={!current()}
              hasQuery={query().trim().length > 0}
              searchFailed={!!searchError()}
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
                    ? filePending()
                      ? fileActivity()
                      : files().phase === "disabled"
                        ? "File search is off"
                        : files().phase === "failed"
                          ? "File scan unavailable"
                          : `${files().total} ${files().total === 1 ? "item" : "items"} indexed`
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
                aria-expanded={menuOpen() ? "true" : "false"}
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
                  data-has-result-actions={
                    resultActions().length > 0 ? "true" : "false"
                  }
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
                    onInput={(event) =>
                      setMenuFilter(event.currentTarget.value)
                    }
                  />
                  <Show when={resultActions().length > 0}>
                    <div class="menu-column">
                      <div class="menu-heading">
                        {current()?.title ?? "TinyDash"}
                      </div>
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
                  </Show>
                  <div class="menu-column launcher-menu-column">
                    <Show
                      when={appearances.some((item) =>
                        matchesMenu(`Appearance ${item.label}`),
                      )}
                    >
                      <div class="launcher-menu-appearance">
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
                                  aria-checked={
                                    appearance() === item.id ? "true" : "false"
                                  }
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
                      </div>
                    </Show>
                    <div class="launcher-menu-commands">
                      <Show
                        when={
                          launcherActions().length > 0 ||
                          matchesMenu("Compact layout")
                        }
                      >
                        <div class="menu-heading">TinyDash</div>
                      </Show>
                      <Show when={matchesMenu("Compact layout")}>
                        <button
                          role="menuitemcheckbox"
                          aria-checked={compact() ? "true" : "false"}
                          onClick={() => {
                            const next = !compact();
                            setCompact(next);
                            saveCompact(next);
                            setMenuOpen(false);
                            focusInput();
                          }}
                        >
                          <span class="menu-check" aria-hidden="true">
                            {compact() ? "✓" : ""}
                          </span>
                          Compact
                        </button>
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
                  </div>
                  <Show
                    when={
                      !resultActions().length &&
                      !launcherActions().length &&
                      !matchesMenu("Compact layout") &&
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
                ? "This deletes unpinned text and all saved images and file references. Text pinned in All or Clipboard stays saved. The system clipboard does not change."
                : "This deletes all saved text, images, and file references, including pinned text. The system clipboard does not change."
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
    </>
  );
}
