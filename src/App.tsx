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
import ResultPreview from "./components/ResultPreview";
import ConfirmDialog from "./components/ConfirmDialog";
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
  const [clearOpen, setClearOpen] = createSignal(false);
  const [pendingAction, setPendingAction] = createSignal<SearchResult>();
  let input!: HTMLInputElement;
  let menu: HTMLDivElement | undefined;
  let list!: HTMLUListElement;
  let categoryBar!: HTMLDivElement;
  let sequence = 0;
  let disposed = false;
  let searchTask: Promise<void> | undefined;
  let displayedQuery: { value: string; mode: SearchMode } | undefined;
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
                        : "Search apps, files, emoji...";
  const focusInput = () => input.focus({ preventScroll: true });

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
    queuedSearch = {
      value,
      mode: mode(),
      preserveSelection,
      request: ++sequence,
    };
    setPending(true);
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
          const selectedId =
            preserveSelection &&
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
          displayedQuery = { value, mode: searchMode };
          batch(() => {
            setResults(response.results);
            setSelected(
              Math.max(0, matchedIndex >= 0 ? matchedIndex : stableToolIndex),
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

  function confirmClear() {
    setMenuOpen(false);
    setError(undefined);
    setClearOpen(true);
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
      await backend.clearClipboard();
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
    setMenuOpen(!menuOpen());
    if (menuOpen()) {
      queueMicrotask(() =>
        menu
          ?.querySelector<HTMLButtonElement>("button:not(:disabled)")
          ?.focus(),
      );
    } else {
      focusInput();
    }
  }

  function onKey(event: KeyboardEvent) {
    if (event.isComposing || event.keyCode === 229) return;
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
          (index + (event.key === "ArrowDown" ? 1 : -1) + buttons.length) %
            buttons.length
        ]?.focus();
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
                        !warning.startsWith("Could not register ") &&
                        !warning.startsWith("Could not read settings."),
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
        setInfo(await backend.ready());
        keepVisibleCategory();
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
      <Show when={!message() && mode() === "all" && files().indexing}>
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
          "emoji-results": mode() === "emoji",
          "clipboard-results": current()?.kind === "clipboard",
        }}
        aria-label="Search results"
      >
        <div class="result-column">
          <Show when={mode() === "clipboard"}>
            <div class="list-tools">
              <span>Saved on this device</span>
              <button
                class="text-button clear-history"
                disabled={!desktop || busy()}
                onClick={confirmClear}
              >
                Clear history
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
          <Show when={results().length === 0}>
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
          <span class="navigation-hint">
            <kbd>↑</kbd>
            <kbd>↓</kbd> navigate
          </span>
          <span class="list-count" role="status" aria-live="polite">
            {mode() === "calculator"
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
                <div class="menu-column">
                  <div class="menu-heading">
                    {current()?.title ?? "TinyDash"}
                  </div>
                  <button
                    role="menuitem"
                    disabled={!canOpen()}
                    onClick={runPrimary}
                  >
                    <Icon
                      name={
                        current()?.primaryAction === "copy" ? "copy" : "return"
                      }
                    />
                    {primaryLabel() === "Open"
                      ? current()?.kind === "file"
                        ? "Open file"
                        : "Open application"
                      : primaryLabel()}
                    <kbd>↵</kbd>
                  </button>
                  <Show when={current()?.secondaryActions.includes("reveal")}>
                    <button
                      role="menuitem"
                      disabled={!canOpen()}
                      onClick={() => void run("reveal")}
                    >
                      <Icon name="folder" />
                      Show in folder<kbd>{modifier()} ↵</kbd>
                    </button>
                  </Show>
                  <For each={pinOptions()}>
                    {(option) => (
                      <button
                        role="menuitem"
                        disabled={!canOpen() || pinBusy()}
                        onClick={() => void togglePin(option.category)}
                      >
                        <Icon name="pin" />
                        {option.label}
                      </button>
                    )}
                  </For>
                  <Show when={current()?.secondaryActions.includes("delete")}>
                    <button
                      role="menuitem"
                      disabled={!canOpen()}
                      onClick={() => void run("delete")}
                    >
                      <Icon name="delete" />
                      Delete entry<kbd>{modifier()} ⌫</kbd>
                    </button>
                  </Show>
                  <Show when={current()?.secondaryActions.includes("copy")}>
                    <button
                      role="menuitem"
                      disabled={!canOpen()}
                      onClick={() => void run("copy")}
                    >
                      <Icon name="copy" />
                      Copy URL
                    </button>
                  </Show>
                  <Show when={current()?.secondaryActions.includes("open")}>
                    <button
                      role="menuitem"
                      disabled={!canOpen()}
                      onClick={() => void run("open")}
                    >
                      <Icon name="globe" />
                      Open cleaned URL
                    </button>
                  </Show>
                  <Show
                    when={current()?.secondaryActions.includes("regenerate")}
                  >
                    <button
                      role="menuitem"
                      disabled={!canOpen()}
                      onClick={() => void run("regenerate")}
                    >
                      <Icon name="refresh" />
                      Generate another
                    </button>
                  </Show>
                  <Show
                    when={
                      mode() === "clipboard" || current()?.kind === "clipboard"
                    }
                  >
                    <button
                      role="menuitem"
                      disabled={!desktop || busy()}
                      onClick={confirmClear}
                    >
                      <Icon name="delete" />
                      Clear clipboard history
                    </button>
                  </Show>
                </div>
                <div class="menu-column">
                  <div
                    class="appearance-group"
                    role="group"
                    aria-label="Appearance"
                  >
                    <div class="menu-heading">Appearance</div>
                    <div class="appearance-options">
                      <For each={appearances}>
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
                  <button
                    role="menuitem"
                    disabled={!desktop}
                    onClick={() => void openSettings()}
                  >
                    <Icon name="system" />
                    Settings<kbd>{modifier()} ,</kbd>
                  </button>
                  <button
                    role="menuitem"
                    disabled={!desktop || currency().refreshing}
                    onClick={() => void refresh("currency")}
                  >
                    <Icon name="refresh" />
                    Refresh currency rates
                    <Show when={mode() === "calculator"}>
                      <kbd>{modifier()} R</kbd>
                    </Show>
                  </button>
                  <button
                    role="menuitem"
                    disabled={!desktop || indexing()}
                    onClick={() => void refresh("apps")}
                  >
                    <Icon name="refresh" />
                    Refresh applications
                    <Show when={mode() !== "files" && mode() !== "calculator"}>
                      <kbd>{modifier()} R</kbd>
                    </Show>
                  </button>
                  <button
                    role="menuitem"
                    disabled={!desktop || files().indexing}
                    onClick={() => void refresh("files")}
                  >
                    <Icon name="refresh" />
                    Refresh files
                    <Show when={mode() === "files"}>
                      <kbd>{modifier()} R</kbd>
                    </Show>
                  </button>
                  <button
                    role="menuitem"
                    disabled={!desktop}
                    onClick={() =>
                      void backend
                        .quit()
                        .catch((reason) => setError(String(reason)))
                    }
                  >
                    <Icon name="quit" />
                    Quit TinyDash<kbd>{modifier()} Q</kbd>
                  </button>
                </div>
              </div>
            </Show>
          </div>
        </div>
      </footer>
      <Show when={clearOpen()}>
        <ConfirmDialog
          title="Clear clipboard history?"
          description="This deletes all saved text entries, including pinned entries. The current system clipboard stays available."
          confirmLabel="Clear history"
          busyLabel="Clearing..."
          busy={busy()}
          error={error()}
          onClose={closeClear}
          onConfirm={() => void clearHistory()}
        />
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
