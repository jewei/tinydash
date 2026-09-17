import {
  createEffect,
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
import ClipboardPreview from "./components/ClipboardPreview";
import ConfirmDialog from "./components/ConfirmDialog";

export default function App() {
  const desktop = isTauri();
  const [query, setQuery] = createSignal("");
  const [mode, setMode] = createSignal<SearchMode>("all");
  const [results, setResults] = createSignal<SearchResult[]>([]);
  const [selected, setSelected] = createSignal(0);
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
  let sequence = 0;
  let disposed = false;
  let searchTask: Promise<void> | undefined;
  let queuedSearch:
    | {
        value: string;
        mode: SearchMode;
        selectedId: string | undefined;
        request: number;
      }
    | undefined;
  const unlisteners: UnlistenFn[] = [];

  const modifier = () => (info()?.platform === "macos" ? "⌘" : "Ctrl");
  const current = () => results()[selected()];
  const canOpen = () =>
    desktop &&
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
    current()?.kind === "systemCommand" || mode() === "system"
      ? "Run command"
      : current()?.kind === "emoji" || mode() === "emoji"
        ? "Copy emoji"
        : current()?.kind === "calculation" || mode() === "calculator"
          ? "Copy result"
          : current()?.kind === "clipboard" || mode() === "clipboard"
            ? "Copy text"
            : "Open";
  const placeholder = () =>
    mode() === "system"
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

  function startDragging(event: MouseEvent) {
    if (!desktop || event.button !== 0) return;
    event.preventDefault();
    void getCurrentWindow()
      .startDragging()
      .catch((reason: unknown) => setError(String(reason)));
  }

  function search(value = query(), preserveSelection = false) {
    if (!desktop || disposed) return Promise.resolve();
    queuedSearch = {
      value,
      mode: mode(),
      selectedId: preserveSelection ? current()?.id : undefined,
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
        const { value, mode: searchMode, selectedId, request } = queuedSearch;
        queuedSearch = undefined;
        try {
          const response = await backend.search(value, searchMode);
          if (disposed || request !== sequence) continue;
          setResults(response.results);
          setSelected(
            Math.max(
              0,
              response.results.findIndex((result) => result.id === selectedId),
            ),
          );
          setTotal(response.total);
          setIndexing(response.indexing);
          setFiles(response.files);
          setCurrency(response.currency);
          setIndexError(response.indexError ?? undefined);
          setStorageError(response.storageError ?? undefined);
          setNotice(response.notice ?? undefined);
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

  function changeMode(value: SearchMode) {
    setMode(value);
    setMenuOpen(false);
    setError(undefined);
    void search();
    focusInput();
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
    try {
      await backend.execute(result.id, action, confirmed);
      if (confirmed) setPendingAction(undefined);
      if (action === "delete") {
        await search();
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
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      if (results().length) {
        const step = event.key === "ArrowDown" ? 1 : -1;
        setSelected(
          (index) => (index + step + results().length) % results().length,
        );
      }
    } else if (event.key === "Enter") {
      event.preventDefault();
      if (command && current()?.secondaryActions.includes("reveal"))
        void run("reveal");
      else if (!command && !event.altKey && !event.shiftKey) runPrimary();
    }
  }

  createEffect(() => {
    const index = selected();
    results();
    queueMicrotask(() =>
      list?.children[index]?.scrollIntoView({ block: "nearest" }),
    );
  });

  function outsideClick(event: PointerEvent) {
    if (menuOpen() && !(event.target as Element).closest(".actions-area")) {
      setMenuOpen(false);
    }
  }

  onMount(() => {
    focusInput();
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
          register("apps-changed", () => {
            void search();
          }),
          register("files-changed", () => {
            void search(query(), true);
          }),
          register("currency-changed", () => {
            if (mode() === "calculator" || (mode() === "all" && query().trim()))
              void search(query(), true);
          }),
          register("usage-changed", () => {
            void search();
          }),
          register("clipboard-changed", () => {
            if (mode() === "clipboard" || (mode() === "all" && query().trim()))
              void search(query(), true);
          }),
          register("launcher-opened", (clear) => {
            setMenuOpen(false);
            setClearOpen(false);
            setPendingAction(undefined);
            setError(undefined);
            if (clear === true) {
              setMode("all");
              changeQuery("");
            }
            focusInput();
            if (clear !== true) {
              void search();
              input.select();
            }
          }),
        ]);
        if (disposed) return;
        setInfo(await backend.ready());
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
          <Icon name="search" size={23} />
          <select
            class="mode-select"
            aria-label="Search mode"
            value={mode()}
            onChange={(event) =>
              changeMode(event.currentTarget.value as SearchMode)
            }
          >
            <option value="all">All</option>
            <option value="apps">Apps</option>
            <option value="files">Files</option>
            <option value="clipboard">Clipboard</option>
            <option value="emoji">Emoji</option>
            <option value="calculator">Calculator</option>
            <option value="system">System</option>
          </select>
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
            maxLength={256}
            value={query()}
            onInput={(event) => changeQuery(event.currentTarget.value)}
          />
          <button
            class="escape-key"
            aria-label="Hide launcher"
            onClick={() => void hide()}
            disabled={!desktop}
          >
            <kbd>esc</kbd>
          </button>
        </div>
      </header>

      <div class="list-heading">
        <span>
          {query().trim()
            ? "Search results"
            : mode() === "system"
              ? "System commands"
              : mode() === "emoji"
                ? "Emoji"
                : mode() === "calculator"
                  ? "Calculator"
                  : mode() === "clipboard"
                    ? "Clipboard history"
                    : mode() === "files"
                      ? "Files"
                      : "Applications"}
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
                  mode() === "system"
                ? `${results().length} shown`
                : indexing()
                  ? "Finding applications..."
                  : pending()
                    ? "Searching..."
                    : `${total()} installed`}
        </span>
        <Show when={mode() === "clipboard"}>
          <button
            class="text-button clear-history"
            disabled={!desktop || busy()}
            onClick={confirmClear}
          >
            Clear history
          </button>
        </Show>
      </div>

      <section
        class="results-area"
        classList={{ "with-preview": current()?.kind === "clipboard" }}
        aria-label="Search results"
      >
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
              <li
                id={`result-${index()}`}
                role="option"
                aria-selected={index() === selected()}
                class="result-row"
                classList={{
                  selected: index() === selected(),
                  pending: pending(),
                  "calculation-row": result.kind === "calculation",
                }}
                onPointerMove={() => {
                  if (!pending()) setSelected(index());
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
                <Show when={index() < 9}>
                  <kbd class="result-shortcut">
                    <span>{modifier()}</span>
                    <span>{index() + 1}</span>
                  </kbd>
                </Show>
              </li>
            )}
          </For>
        </ul>
        <Show when={current()?.kind === "clipboard"}>
          <Show when={!pending()}>
            <ClipboardPreview id={current()!.id} />
          </Show>
        </Show>

        <Show when={results().length === 0}>
          <div class="empty-state">
            <div class="empty-icon">
              <Icon name={query() ? "search" : "apps"} size={28} />
            </div>
            <h1>
              {!desktop
                ? "TinyDash, one shortcut away."
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
                          : mode() === "all" && files().indexing && query()
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
                : mode() === "system"
                  ? "Try sleep, restart, or settings."
                  : mode() === "calculator"
                    ? "Try 12 * 8, 5 ft to cm, or 100 USD to MYR."
                    : mode() === "emoji"
                      ? "Try a name, shortcode, or category, such as coffee or food."
                      : mode() === "clipboard"
                        ? info()?.settings.clipboardHistoryEnabled === false
                          ? "Clipboard capture is off in settings.json."
                          : query()
                            ? "Try a word from the text you copied."
                            : "Copy text in any application. It will appear here."
                        : mode() === "files"
                          ? files().indexing
                            ? "You can search applications while the scan runs."
                            : query()
                              ? "Try a filename or part of a path."
                              : info()?.settings.fileSearchRoots?.length === 0
                                ? "File search is off in settings.json."
                                : "Check your folders in settings.json, then refresh the file list."
                          : mode() === "all" && files().indexing && query()
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
      </section>

      <div class="status-line">
        <Show
          when={message()}
          fallback={
            <>
              <span class="brand-mark" aria-hidden="true">
                <i />
                <i />
              </span>
              <span>TinyDash</span>
              <span class="status-separator">/</span>
              <span class="query-hint">
                {mode() === "all" && files().indexing
                  ? "Scanning files... You can search apps now."
                  : query()
                    ? `${results().length} ${results().length === 1 ? "result" : results().length === 30 ? "shown" : "results"}`
                    : mode() === "system"
                      ? "Power commands require confirmation."
                      : mode() === "calculator"
                        ? "Enter copies the result."
                        : mode() === "emoji"
                          ? "Enter copies the emoji."
                          : mode() === "clipboard"
                            ? "Enter copies text. Paste it with your usual shortcut."
                            : mode() === "files"
                              ? info()?.settings.fileWatchEnabled === false
                                ? "Enter opens the file. Refresh after files change."
                                : "Enter opens the file. Changes update automatically."
                              : "Type a name, : for emoji, or = to calculate."}
              </span>
            </>
          }
        >
          <span class="error-message" role="alert" title={message()}>
            {message()}
          </span>
        </Show>
      </div>

      <footer class="footer">
        <span class="navigation-hint">
          <span class="arrow-keys" aria-hidden="true">
            ↑ ↓
          </span>{" "}
          Navigate
        </span>
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
          <span class="footer-divider" />
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
              <div
                id="actions-menu"
                ref={menu}
                class="actions-menu"
                role="menu"
                aria-label="Launcher actions"
              >
                <div class="menu-heading">{current()?.title ?? "TinyDash"}</div>
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
                <div class="menu-divider" />
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
            </Show>
          </div>
        </div>
      </footer>
      <Show when={clearOpen()}>
        <ConfirmDialog
          title="Clear clipboard history?"
          description="This deletes all saved text entries. The current system clipboard stays available."
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
