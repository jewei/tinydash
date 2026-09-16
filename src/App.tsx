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
import {
  backend,
  type Action,
  type LauncherInfo,
  type SearchResult,
  type SearchMode,
} from "./bridge";
import Icon from "./components/Icon";
import ResultIcon from "./components/ResultIcon";

export default function App() {
  const desktop = isTauri();
  const [query, setQuery] = createSignal("");
  const [mode, setMode] = createSignal<SearchMode>("all");
  const [results, setResults] = createSignal<SearchResult[]>([]);
  const [selected, setSelected] = createSignal(0);
  const [info, setInfo] = createSignal<LauncherInfo>();
  const [total, setTotal] = createSignal(0);
  const [indexing, setIndexing] = createSignal(desktop);
  const [pending, setPending] = createSignal(false);
  const [busy, setBusy] = createSignal(false);
  const [error, setError] = createSignal<string>();
  const [indexError, setIndexError] = createSignal<string>();
  const [notice, setNotice] = createSignal<string>();
  const [menuOpen, setMenuOpen] = createSignal(false);
  let input!: HTMLInputElement;
  let menu: HTMLDivElement | undefined;
  let list!: HTMLUListElement;
  let sequence = 0;
  let disposed = false;
  const unlisteners: UnlistenFn[] = [];

  const modifier = () => (info()?.platform === "macos" ? "⌘" : "Ctrl");
  const current = () => results()[selected()];
  const canOpen = () => desktop && !!current() && !busy() && !pending();
  const message = () =>
    error() ?? notice() ?? indexError() ?? info()?.warnings[0];
  const primaryLabel = () =>
    current()?.kind === "emoji" || mode() === "emoji"
      ? "Copy emoji"
      : current()?.kind === "calculation" || mode() === "calculator"
        ? "Copy result"
        : "Open";
  const placeholder = () =>
    mode() === "apps"
      ? "Search applications..."
      : mode() === "emoji"
        ? "Search emoji..."
        : mode() === "calculator"
          ? "Calculate or convert..."
          : "Search apps, emoji, or calculate...";
  const focusInput = () => input.focus({ preventScroll: true });

  async function search(value = query()) {
    if (!desktop) return;
    const request = ++sequence;
    setPending(true);
    setNotice(undefined);
    try {
      const response = await backend.search(value, mode());
      if (disposed || request !== sequence) return;
      setResults(response.results);
      setSelected(0);
      setTotal(response.total);
      setIndexing(response.indexing);
      setIndexError(response.indexError ?? undefined);
      setNotice(response.notice ?? undefined);
    } catch (reason) {
      if (disposed || request !== sequence) return;
      setResults([]);
      setError(String(reason));
    } finally {
      if (!disposed && request === sequence) setPending(false);
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
    setBusy(true);
    setError(undefined);
    try {
      await backend.execute(result.id, action);
    } catch (reason) {
      setError(String(reason));
      focusInput();
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

  async function refresh() {
    setMenuOpen(false);
    setError(undefined);
    setIndexing(true);
    focusInput();
    try {
      await backend.refresh();
      await search();
    } catch (reason) {
      setIndexing(false);
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
          register("launcher-opened", (clear) => {
            setMenuOpen(false);
            setError(undefined);
            if (clear === true) {
              setMode("all");
              changeQuery("");
            }
            focusInput();
            if (clear !== true) input.select();
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
            <option value="emoji">Emoji</option>
            <option value="calculator">Calculator</option>
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
            : mode() === "emoji"
              ? "Emoji"
              : mode() === "calculator"
                ? "Calculator"
                : "Applications"}
        </span>
        <span class="list-count" role="status" aria-live="polite">
          {mode() === "calculator"
            ? "Offline"
            : mode() === "emoji"
              ? `${results().length} shown`
              : indexing()
                ? "Finding applications..."
                : pending()
                  ? "Searching..."
                  : `${total()} installed`}
        </span>
      </div>

      <section class="results-area" aria-label="Search results">
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

        <Show when={results().length === 0}>
          <div class="empty-state">
            <div class="empty-icon">
              <Icon name={query() ? "search" : "apps"} size={28} />
            </div>
            <h1>
              {!desktop
                ? "TinyDash, one shortcut away."
                : mode() === "calculator"
                  ? "Calculate and convert"
                  : mode() === "emoji"
                    ? "No emoji found"
                    : indexing()
                      ? "Finding your applications"
                      : query()
                        ? "No results found"
                        : "No applications in the index"}
            </h1>
            <p>
              {!desktop
                ? "Start the TinyDash desktop app to search this computer."
                : mode() === "calculator"
                  ? "Try 12 * 8, sqrt(144), or 5 ft to cm."
                  : mode() === "emoji"
                    ? "Try a name, shortcode, or category, such as coffee or food."
                    : indexing()
                      ? "You can start typing while the list loads."
                      : query()
                        ? "Try an app name, an emoji name, or a calculation."
                        : "Refresh the list after you install an application."}
            </p>
            <Show
              when={
                desktop &&
                !indexing() &&
                (query() || mode() === "all" || mode() === "apps")
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
                {query() ? "Clear search" : "Refresh applications"}
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
                {query()
                  ? `${results().length} ${results().length === 1 ? "result" : results().length === 30 ? "shown" : "results"}`
                  : mode() === "calculator"
                    ? "Enter copies the result."
                    : mode() === "emoji"
                      ? "Enter copies the emoji."
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
              ? current()?.primaryAction === "copy"
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
                    ? "Open application"
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
                <div class="menu-divider" />
                <button
                  role="menuitem"
                  disabled={!desktop || indexing()}
                  onClick={() => void refresh()}
                >
                  <Icon name="refresh" />
                  Refresh applications<kbd>{modifier()} R</kbd>
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
    </main>
  );
}
