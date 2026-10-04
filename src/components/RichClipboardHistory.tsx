import {
  createEffect,
  createSignal,
  For,
  onCleanup,
  onSettled,
  Show,
} from "solid-js";
import { listen } from "@tauri-apps/api/event";
import {
  richClipboardBackend,
  type RichClipboardHistory as History,
  type RichClipboardPreview,
  type RichClipboardEntry,
} from "../clipboardBridge";

export default function RichClipboardHistory(props: {
  onClose: () => void;
  platform?: string;
}) {
  const [history, setHistory] = createSignal<History>();
  const [query, setQuery] = createSignal("");
  const [kind, setKind] = createSignal<RichClipboardEntry["kind"]>();
  const [sourceApp, setSourceApp] = createSignal<string>();
  const [loadingHistory, setLoadingHistory] = createSignal(true);
  const [historyError, setHistoryError] = createSignal<string>();
  const [selected, setSelected] = createSignal<number>();
  const [preview, setPreview] = createSignal<RichClipboardPreview>();
  const [imageUrl, setImageUrl] = createSignal<string>();
  const [message, setMessage] = createSignal<string>();
  const [previewError, setPreviewError] = createSignal<string>();
  const [busy, setBusy] = createSignal(false);
  let disposed = false;
  let refreshSequence = 0;
  let previewSequence = 0;
  let unlisten: (() => void) | undefined;
  let searchInput!: HTMLInputElement;
  let pasteButton!: HTMLButtonElement;
  let resultList!: HTMLUListElement;
  let composing = false;
  let compositionTimer: number | undefined;
  let refreshing = false;
  let refreshAgain = false;
  let loadingPreview = false;
  let pendingPreview: { id: number; sequence: number } | undefined;

  async function loadPreview() {
    if (loadingPreview) return;
    loadingPreview = true;
    try {
      while (pendingPreview && !disposed) {
        const request = pendingPreview;
        pendingPreview = undefined;
        try {
          const value = await richClipboardBackend.preview(request.id);
          if (!disposed && request.sequence === previewSequence)
            setPreview(value);
        } catch (error) {
          if (!disposed && request.sequence === previewSequence)
            setPreviewError(String(error));
        }
      }
    } finally {
      loadingPreview = false;
    }
  }

  async function refresh() {
    const request = ++refreshSequence;
    setLoadingHistory(true);
    setHistoryError(undefined);
    if (refreshing) {
      refreshAgain = true;
      return;
    }
    refreshing = true;
    try {
      const value = await richClipboardBackend.history(
        query(),
        kind(),
        sourceApp(),
      );
      if (disposed || request !== refreshSequence) return;
      const focused = document.activeElement;
      const keepResultFocus =
        focused instanceof HTMLElement &&
        focused.getAttribute("role") === "option" &&
        resultList?.contains(focused);
      setHistory(value);
      const next = value.entries.some((entry) => entry.id === selected())
        ? selected()
        : value.entries[0]?.id;
      if (next !== undefined && next === selected()) requestPreview(next);
      else setSelected(next);
      if (keepResultFocus)
        queueMicrotask(() => {
          if (
            disposed ||
            (document.activeElement !== document.body &&
              document.activeElement !== focused)
          )
            return;
          const row =
            next === undefined
              ? undefined
              : resultList?.querySelector<HTMLElement>(`#${resultId(next)}`);
          (row ?? searchInput).focus({ preventScroll: true });
        });
    } catch (error) {
      if (!disposed && request === refreshSequence)
        setHistoryError(String(error));
    } finally {
      refreshing = false;
      if (refreshAgain && !disposed) {
        refreshAgain = false;
        void refresh();
      } else if (!disposed && request === refreshSequence) {
        setLoadingHistory(false);
      }
    }
  }

  onSettled(() => {
    searchInput.focus();
    void refresh();
    void listen("clipboard-changed", () => void refresh()).then(
      (stop) => {
        if (disposed) stop();
        else unlisten = stop;
      },
      () => {}, // Manual refresh remains available outside the native runtime.
    );
  });
  onCleanup(() => {
    disposed = true;
    pendingPreview = undefined;
    unlisten?.();
    window.clearTimeout(compositionTimer);
  });

  function requestPreview(id: number | undefined) {
    const request = ++previewSequence;
    setPreview(undefined);
    setPreviewError(undefined);
    pendingPreview = id === undefined ? undefined : { id, sequence: request };
    void loadPreview();
  }
  createEffect(selected, requestPreview);
  createEffect(
    () => preview()?.png,
    (png) => {
      if (!png) {
        setImageUrl(undefined);
        return;
      }
      const url = URL.createObjectURL(
        new Blob([new Uint8Array(png)], { type: "image/png" }),
      );
      setImageUrl(url);
      return () => URL.revokeObjectURL(url);
    },
  );

  const unavailable = () => busy() || loadingHistory() || !!historyError();
  const hasFilters = () => !!query().trim() || !!kind() || !!sourceApp();
  function clearFilters() {
    setQuery("");
    setKind(undefined);
    setSourceApp(undefined);
    void refresh();
    searchInput.focus();
  }
  const selectedEntry = () =>
    history()?.entries.find((entry) => entry.id === selected());

  const canTransfer = () =>
    !unavailable() &&
    !!history()?.captureSupported &&
    !!selectedEntry() &&
    preview()?.entry.id === selected();
  const resultId = (id: number) => `rich-clipboard-entry-${id}`;
  const modifier = () => (props.platform === "macos" ? "⌘" : "Ctrl");

  function startComposition() {
    window.clearTimeout(compositionTimer);
    composing = true;
  }
  function endComposition() {
    // WebKit can deliver the committing Enter just after compositionend.
    window.clearTimeout(compositionTimer);
    compositionTimer = window.setTimeout(() => {
      composing = false;
    }, 0);
  }

  function onKey(event: KeyboardEvent) {
    if (event.defaultPrevented) return;
    if (composing || event.isComposing || event.keyCode === 229) {
      event.stopPropagation();
      return;
    }
    if (event.key === "Escape" && busy()) {
      event.preventDefault();
      event.stopPropagation();
      return;
    }
    const target = event.target as HTMLElement;
    const fromSearch = target === searchInput;
    const fromResult =
      target.getAttribute("role") === "option" && resultList?.contains(target);
    if (!fromSearch && !fromResult) return;
    const plain =
      !event.metaKey && !event.ctrlKey && !event.altKey && !event.shiftKey;
    const move =
      plain && (event.key === "ArrowDown" || event.key === "ArrowUp");
    const paste =
      event.key === "Enter" &&
      event.shiftKey &&
      !event.altKey &&
      (props.platform === "macos"
        ? event.metaKey && !event.ctrlKey
        : event.ctrlKey && !event.metaKey);
    const copy = plain && event.key === "Enter";
    const select = plain && fromResult && event.key === " ";
    if (!move && !paste && !copy && !select) return;
    event.preventDefault();
    event.stopPropagation();
    if (unavailable()) return;
    if (move) {
      const entries = history()?.entries ?? [];
      if (!entries.length) return;
      const current = entries.findIndex((entry) => entry.id === selected());
      const step = event.key === "ArrowDown" ? 1 : -1;
      const next =
        current < 0
          ? step > 0
            ? 0
            : entries.length - 1
          : Math.max(0, Math.min(entries.length - 1, current + step));
      const id = entries[next].id;
      setSelected(id);
      queueMicrotask(() => {
        if (disposed || selected() !== id) return;
        const row = resultList?.querySelector<HTMLElement>(`#${resultId(id)}`);
        if (fromResult) row?.focus({ preventScroll: true });
        row?.scrollIntoView({ block: "nearest" });
      });
    } else if (!event.repeat && canTransfer() && !select) {
      void action(paste ? "paste" : "copy", target);
    }
  }

  async function togglePin() {
    const entry = selectedEntry();
    if (!entry || unavailable()) return;
    const pinned = !entry.pinned;
    setBusy(true);
    setMessage(undefined);
    try {
      await richClipboardBackend.setPinned(entry.id, pinned);
      if (!disposed) {
        setMessage(
          pinned
            ? "Pinned. Automatic cleanup will keep this entry."
            : "Unpinned. Current retention and storage limits now apply.",
        );
        await refresh();
      }
    } catch (error) {
      if (!disposed) setMessage(String(error));
    } finally {
      if (!disposed) setBusy(false);
    }
  }

  async function action(
    kind: "copy" | "paste" | "delete",
    restoreFocus?: HTMLElement,
  ) {
    const id = selected();
    if (
      id === undefined ||
      unavailable() ||
      (kind !== "delete" && !canTransfer())
    )
      return;
    setBusy(true);
    setMessage(undefined);
    try {
      await richClipboardBackend[kind](id);
      if (!disposed) {
        setMessage(
          kind === "copy"
            ? "Copied. Paste in the target application."
            : kind === "paste"
              ? "Paste sent to the previous app. The receiving app decides whether to accept this format."
              : "Deleted from history. The system clipboard is unchanged.",
        );
        if (kind === "delete") await refresh();
      }
    } catch (error) {
      if (!disposed) setMessage(String(error));
    } finally {
      if (!disposed) {
        setBusy(false);
        if (restoreFocus || kind === "paste")
          queueMicrotask(() => {
            if (disposed) return;
            if (restoreFocus) {
              (restoreFocus.isConnected ? restoreFocus : searchInput).focus({
                preventScroll: true,
              });
            } else pasteButton?.focus();
          });
      }
    }
  }

  return (
    <section
      class="rich-clipboard-panel"
      aria-label="Image and file clipboard history"
      onKeyDown={onKey}
      onCompositionStart={startComposition}
      onCompositionEnd={endComposition}
    >
      <header>
        <h2>Clipboard images and files</h2>
        <button type="button" disabled={busy()} onClick={props.onClose}>
          Back
        </button>
        <button type="button" onClick={() => void refresh()} disabled={busy()}>
          Refresh
        </button>
      </header>
      <div
        class="rich-clipboard-filters"
        role="search"
        aria-label="Search image and file history"
      >
        <label>
          Search history
          <input
            ref={searchInput}
            type="search"
            role="combobox"
            aria-label="Search images and files"
            aria-autocomplete="list"
            aria-expanded={history()?.entries.length ? "true" : "false"}
            aria-controls={
              history()?.entries.length ? "rich-clipboard-results" : undefined
            }
            aria-activedescendant={
              !unavailable() && selected() !== undefined
                ? resultId(selected()!)
                : undefined
            }
            aria-describedby="rich-clipboard-shortcuts"
            placeholder="Filename, path, title, or source app"
            maxlength={256}
            value={query()}
            disabled={busy()}
            onInput={(event) => {
              setQuery(event.currentTarget.value);
              void refresh();
            }}
          />
        </label>
        <label>
          Content type
          <select
            aria-label="Clipboard content type"
            value={kind() ?? ""}
            disabled={busy()}
            onChange={(event) => {
              const value = event.currentTarget.value;
              setKind(
                value === "image" || value === "files" ? value : undefined,
              );
              void refresh();
            }}
          >
            <option value="">All types</option>
            <option value="image">Images</option>
            <option value="files">Files</option>
          </select>
        </label>
        <label>
          Source app
          <select
            aria-label="Clipboard source app"
            value={sourceApp() ?? ""}
            disabled={busy()}
            onChange={(event) => {
              setSourceApp(event.currentTarget.value || undefined);
              void refresh();
            }}
          >
            <option value="">All apps</option>
            <For each={history()?.sourceApps ?? []}>
              {(source) => <option value={source}>{source}</option>}
            </For>
            <Show
              when={
                sourceApp() && !history()?.sourceApps.includes(sourceApp()!)
              }
            >
              <option value={sourceApp()}>{sourceApp()}</option>
            </Show>
          </select>
        </label>
        <button
          type="button"
          disabled={busy() || !hasFilters()}
          onClick={clearFilters}
        >
          Clear filters
        </button>
      </div>
      <p id="rich-clipboard-shortcuts" class="rich-clipboard-shortcuts">
        In search or results: <kbd>↑</kbd> <kbd>↓</kbd> select
        <Show when={history()?.captureSupported}>
          {" · "}
          <kbd>Enter</kbd> copy{" · "}
          <kbd>{modifier()} + Shift + Enter</kbd> paste
        </Show>
      </p>
      <Show when={historyError()}>
        {(error) => (
          <p role="alert">
            History unavailable. {error()} Use Refresh to retry.
          </p>
        )}
      </Show>
      <p aria-live="polite">
        {loadingHistory()
          ? "Searching history…"
          : historyError()
            ? "Search failed."
            : `${history()?.entries.length ?? 0} of ${history()?.total ?? 0} entries`}
      </p>
      <p>
        Enable image or file capture separately in Settings. Files are
        references, not backups. This local history is not encrypted.
      </p>
      <Show
        when={history()}
        fallback={
          <p role="status">
            {historyError()
              ? "History unavailable. Use Refresh to retry."
              : "Loading history…"}
          </p>
        }
      >
        {(value) => (
          <>
            <p>{value().supportNotice}</p>
            <p>{value().storageNotice}</p>
            <Show
              when={value().entries.length > 0}
              fallback={
                <p>
                  {value().total
                    ? "No entries match these filters."
                    : "No saved images or file references."}
                </p>
              }
            >
              <ul
                ref={resultList}
                id="rich-clipboard-results"
                role="listbox"
                aria-label="Saved images and files"
                aria-describedby="rich-clipboard-shortcuts"
                aria-busy={loadingHistory() ? "true" : "false"}
              >
                <For each={value().entries}>
                  {(entry) => (
                    <li
                      id={resultId(entry.id)}
                      role="option"
                      aria-selected={selected() === entry.id ? "true" : "false"}
                      aria-disabled={unavailable() ? "true" : "false"}
                      tabindex={
                        !unavailable() && selected() === entry.id ? 0 : -1
                      }
                      onFocus={() => {
                        if (!unavailable()) setSelected(entry.id);
                      }}
                      onClick={() => {
                        if (!unavailable()) setSelected(entry.id);
                      }}
                    >
                      {entry.pinned ? "Pinned · " : ""}
                      {entry.title} ·{" "}
                      {new Date(entry.createdAt * 1000).toLocaleString()}
                    </li>
                  )}
                </For>
              </ul>
            </Show>
            <Show
              when={
                !loadingHistory() && !historyError() ? preview() : undefined
              }
            >
              {(detail) => (
                <>
                  <Show when={imageUrl()}>
                    {(url) => (
                      <img
                        src={url()}
                        alt="Saved clipboard image"
                        style={{
                          "max-width": "100%",
                          "max-height": "240px",
                          "object-fit": "contain",
                        }}
                      />
                    )}
                  </Show>
                  <Show when={detail().files}>
                    {(files) => (
                      <ul aria-label="Saved file references">
                        <For each={files()}>{(file) => <li>{file}</li>}</For>
                      </ul>
                    )}
                  </Show>
                  <Show when={detail().entry.sourceApp}>
                    {(source) => <p>Source: {source()}</p>}
                  </Show>
                </>
              )}
            </Show>
            <button
              class="panel-primary"
              type="button"
              disabled={!canTransfer()}
              onClick={() => void action("copy")}
            >
              Copy original format
            </button>
            <Show when={value().captureSupported}>
              <button
                ref={pasteButton}
                type="button"
                disabled={!canTransfer()}
                onClick={() => void action("paste")}
              >
                Paste to previous app
              </button>
              <p>
                Direct paste needs Accessibility access on macOS. If it fails,
                use Copy original format and paste manually.
              </p>
            </Show>
            <button
              type="button"
              disabled={unavailable() || !selectedEntry()}
              onClick={() => void togglePin()}
            >
              {selectedEntry()?.pinned
                ? "Unpin saved entry"
                : "Pin saved entry"}
            </button>
            <button
              class="panel-danger"
              type="button"
              disabled={unavailable() || selected() === undefined}
              onClick={() => void action("delete")}
            >
              Delete saved entry
            </button>
          </>
        )}
      </Show>
      <Show when={previewError()}>
        {(text) => <p role="alert">Preview unavailable. {text()}</p>}
      </Show>
      <Show when={message()}>{(text) => <p role="status">{text()}</p>}</Show>
    </section>
  );
}
