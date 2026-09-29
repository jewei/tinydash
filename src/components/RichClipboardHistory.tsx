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
} from "../clipboardBridge";

export default function RichClipboardHistory(props: { onClose: () => void }) {
  const [history, setHistory] = createSignal<History>();
  const [selected, setSelected] = createSignal<number>();
  const [preview, setPreview] = createSignal<RichClipboardPreview>();
  const [imageUrl, setImageUrl] = createSignal<string>();
  const [message, setMessage] = createSignal<string>();
  const [busy, setBusy] = createSignal(false);
  let disposed = false;
  let refreshSequence = 0;
  let previewSequence = 0;
  let unlisten: (() => void) | undefined;
  let back!: HTMLButtonElement;
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
            setMessage(String(error));
        }
      }
    } finally {
      loadingPreview = false;
    }
  }

  async function refresh() {
    if (refreshing) {
      refreshAgain = true;
      return;
    }
    refreshing = true;
    const request = ++refreshSequence;
    try {
      const value = await richClipboardBackend.history();
      if (disposed || request !== refreshSequence) return;
      setHistory(value);
      if (!value.entries.some((entry) => entry.id === selected())) {
        setSelected(value.entries[0]?.id);
      }
    } catch (error) {
      if (!disposed && request === refreshSequence) setMessage(String(error));
    } finally {
      refreshing = false;
      if (refreshAgain && !disposed) {
        refreshAgain = false;
        void refresh();
      }
    }
  }

  onSettled(() => {
    back.focus();
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

  createEffect(selected, (id) => {
    const request = ++previewSequence;
    setPreview(undefined);
    setMessage(undefined);
    pendingPreview = id === undefined ? undefined : { id, sequence: request };
    void loadPreview();
  });
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

  async function action(kind: "copy" | "delete") {
    const id = selected();
    if (id === undefined || busy()) return;
    setBusy(true);
    setMessage(undefined);
    try {
      await richClipboardBackend[kind](id);
      if (!disposed) {
        setMessage(
          kind === "copy"
            ? "Copied. Paste in the target application."
            : "Deleted from history. The system clipboard is unchanged.",
        );
        if (kind === "delete") await refresh();
      }
    } catch (error) {
      if (!disposed) setMessage(String(error));
    } finally {
      if (!disposed) setBusy(false);
    }
  }

  return (
    <section
      class="rich-clipboard-panel"
      aria-label="Image and file clipboard history"
    >
      <header>
        <h2>Clipboard images and files</h2>
        <button ref={back} type="button" onClick={props.onClose}>
          Back
        </button>
        <button type="button" onClick={() => void refresh()} disabled={busy()}>
          Refresh
        </button>
      </header>
      <p>
        Enable image or file capture separately in Settings. Files are
        references, not backups. This local history is not encrypted.
      </p>
      <Show
        when={history()}
        fallback={
          <p role="status">
            {message()
              ? "History unavailable. Use Refresh to retry."
              : "Loading history…"}
          </p>
        }
      >
        {(value) => (
          <>
            <p>{value().supportNotice}</p>
            <Show
              when={value().entries.length > 0}
              fallback={<p>No saved images or file references.</p>}
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
                        disabled={busy()}
                        onClick={() => setSelected(entry.id)}
                      >
                        {entry.title} ·{" "}
                        {new Date(entry.createdAt * 1000).toLocaleString()}
                      </button>
                    </li>
                  )}
                </For>
              </ul>
            </Show>
            <Show when={preview()}>
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
              type="button"
              disabled={busy() || !preview() || !value().captureSupported}
              onClick={() => void action("copy")}
            >
              Copy original format
            </button>
            <button
              type="button"
              disabled={busy() || selected() === undefined}
              onClick={() => void action("delete")}
            >
              Delete saved entry
            </button>
          </>
        )}
      </Show>
      <Show when={message()}>{(text) => <p role="status">{text()}</p>}</Show>
    </section>
  );
}
