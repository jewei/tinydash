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

export default function RichClipboardHistory(props: { onClose: () => void }) {
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
      setHistory(value);
      const next = value.entries.some((entry) => entry.id === selected())
        ? selected()
        : value.entries[0]?.id;
      if (next !== undefined && next === selected()) requestPreview(next);
      else setSelected(next);
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

  async function action(kind: "copy" | "paste" | "delete") {
    const id = selected();
    if (id === undefined || unavailable()) return;
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
        if (kind === "paste")
          queueMicrotask(() => {
            if (!disposed) pasteButton?.focus();
          });
      }
    }
  }

  return (
    <section
      class="rich-clipboard-panel"
      aria-label="Image and file clipboard history"
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
            aria-label="Search images and files"
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
              <ul aria-label="Saved images and files">
                <For each={value().entries}>
                  {(entry) => (
                    <li>
                      <button
                        type="button"
                        aria-pressed={
                          selected() === entry.id ? "true" : "false"
                        }
                        disabled={unavailable()}
                        onClick={() => setSelected(entry.id)}
                      >
                        {entry.pinned ? "Pinned · " : ""}
                        {entry.title} ·{" "}
                        {new Date(entry.createdAt * 1000).toLocaleString()}
                      </button>
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
              disabled={
                unavailable() ||
                preview()?.entry.id !== selected() ||
                !preview() ||
                !value().captureSupported
              }
              onClick={() => void action("copy")}
            >
              Copy original format
            </button>
            <Show when={value().captureSupported}>
              <button
                ref={pasteButton}
                type="button"
                disabled={
                  unavailable() ||
                  !preview() ||
                  preview()?.entry.id !== selected()
                }
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
