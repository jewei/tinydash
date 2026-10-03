import { createEffect, createSignal, For, onCleanup, Show } from "solid-js";
import type { SearchResult } from "../bridge";
import {
  fileBackend,
  type FileAction,
  type FilePreviewData,
} from "../file-actions-bridge";
import "../styles/file-preview.css";

export default function FilePreview(props: {
  result: SearchResult;
  enabled?: boolean;
}) {
  const [preview, setPreview] = createSignal<FilePreviewData>();
  const [error, setError] = createSignal<string>();
  const [status, setStatus] = createSignal<string>();
  const [busy, setBusy] = createSignal(false);
  const [confirmTrash, setConfirmTrash] = createSignal(false);
  const [apps, setApps] = createSignal<SearchResult[]>();
  const [appId, setAppId] = createSignal("");
  let generation = 0;
  let disposed = false;
  // Keep at most one preview request in flight; replace queued selections.
  let loading = false;
  let pending: { id: string; generation: number } | undefined;
  async function load() {
    if (loading) return;
    loading = true;
    try {
      while (pending && !disposed) {
        const request = pending;
        pending = undefined;
        try {
          const value = await fileBackend.preview(request.id);
          if (!disposed && request.generation === generation) setPreview(value);
        } catch (reason) {
          if (!disposed && request.generation === generation)
            setError(String(reason));
        }
      }
    } finally {
      loading = false;
    }
  }
  createEffect(
    () => props.result.id,
    (id) => {
      generation += 1;
      setPreview(undefined);
      setError(undefined);
      setStatus(undefined);
      setConfirmTrash(false);
      setApps(undefined);
      setAppId("");
      setBusy(false);
      pending = { id, generation };
      void load();
    },
  );
  onCleanup(() => {
    disposed = true;
    generation += 1;
    pending = undefined;
  });
  const disabled = () => props.enabled === false || busy();
  const textContent = () => {
    const content = preview()?.content;
    return content?.type === "text" ? content : undefined;
  };
  const imageContent = () => {
    const content = preview()?.content;
    return content?.type === "image" &&
      /^data:image\/(png|jpeg|gif|webp);base64,/.test(content.dataUrl)
      ? content
      : undefined;
  };
  const unavailableContent = () => {
    const content = preview()?.content;
    return content?.type === "unavailable" ? content : undefined;
  };
  async function execute(action: FileAction, confirmed = false) {
    if (disabled()) return;
    const request = generation;
    const id = props.result.id;
    setBusy(true);
    setError(undefined);
    setStatus(undefined);
    try {
      await fileBackend.execute(id, action, {
        ...(action === "openWith" ? { appId: appId() } : {}),
        ...(confirmed ? { confirmed: true } : {}),
      });
      if (!disposed && request === generation) {
        setConfirmTrash(false);
        setStatus(
          action === "trash"
            ? "Moved to Trash."
            : action === "copyPath"
              ? "Path copied."
              : action === "copyFile"
                ? "File copied."
                : "Action completed.",
        );
      }
    } catch (reason) {
      if (!disposed && request === generation) setError(String(reason));
    } finally {
      if (!disposed && request === generation) setBusy(false);
    }
  }
  async function chooseApp() {
    if (disabled()) return;
    const request = generation;
    setBusy(true);
    setError(undefined);
    try {
      const catalog = await fileBackend.apps();
      if (!disposed && request === generation) setApps(catalog);
    } catch (reason) {
      if (!disposed && request === generation) setError(String(reason));
    } finally {
      if (!disposed && request === generation) setBusy(false);
    }
  }
  return (
    <section
      class="file-preview"
      aria-label="File preview"
      onKeyDown={(event) => {
        // The launcher must not interpret Enter on these controls as Open File.
        event.stopPropagation();
        if (event.key === "Escape") {
          setConfirmTrash(false);
          setApps(undefined);
        }
      }}
    >
      <Show
        when={preview()}
        fallback={
          <p role="status">
            {error() ? "Preview unavailable." : "Loading preview..."}
          </p>
        }
      >
        {(value) => (
          <>
            <dl class="file-preview-metadata">
              <Show when={!value().folder}>
                <div>
                  <dt>Size</dt>
                  <dd>{value().size.toLocaleString()} bytes</dd>
                </div>
              </Show>
              <Show when={value().modifiedAt !== null}>
                <div>
                  <dt>Modified</dt>
                  <dd>
                    {new Date(value().modifiedAt! * 1000).toLocaleString()}
                  </dd>
                </div>
              </Show>
              <Show when={value().readonly}>
                <div>
                  <dt>Access</dt>
                  <dd>Read-only</dd>
                </div>
              </Show>
            </dl>
            <Show when={textContent()}>
              {(content) => (
                <>
                  <pre tabindex={0} aria-label="File text preview">
                    {content().text}
                  </pre>
                  <Show when={content().truncated}>
                    <p>Preview truncated to 64 KiB.</p>
                  </Show>
                </>
              )}
            </Show>
            <Show when={imageContent()}>
              {(content) => (
                <img
                  class="file-preview-image"
                  src={content().dataUrl}
                  alt={`Preview of ${value().name}`}
                />
              )}
            </Show>
            <Show when={unavailableContent()}>
              {(content) => <p>{content().reason}</p>}
            </Show>
          </>
        )}
      </Show>
      <div class="file-action-buttons" role="group" aria-label="File actions">
        <button disabled={disabled()} onClick={() => void execute("copyPath")}>
          Copy Path
        </button>
        <button disabled={disabled()} onClick={() => void execute("copyFile")}>
          Copy File
        </button>
        <button disabled={disabled()} onClick={() => void chooseApp()}>
          Open With…
        </button>
        <button
          disabled={disabled()}
          onClick={() => void execute("openTerminal")}
        >
          Open in Terminal
        </button>
        <button disabled={disabled()} onClick={() => void execute("quickLook")}>
          Quick Look
        </button>
        <button disabled={disabled()} onClick={() => setConfirmTrash(true)}>
          Move to Trash…
        </button>
      </div>
      <Show when={apps()}>
        {(catalog) => (
          <div class="file-open-with">
            <label>
              Application
              <select
                aria-label="Open With application"
                value={appId()}
                disabled={disabled()}
                onChange={(event) => setAppId(event.currentTarget.value)}
              >
                <option value="">Choose an application</option>
                <For each={catalog()}>
                  {(app) => <option value={app.id}>{app.title}</option>}
                </For>
              </select>
            </label>
            <Show when={catalog().length === 0}>
              <p>No applications are indexed. Refresh Apps and try again.</p>
            </Show>
            <button
              disabled={disabled() || !appId()}
              onClick={() => void execute("openWith")}
            >
              Open with selected application
            </button>
          </div>
        )}
      </Show>
      <Show when={confirmTrash()}>
        <div
          role="alertdialog"
          aria-label="Move item to Trash?"
          aria-describedby="file-trash-description"
          class="file-trash-confirmation"
        >
          <p id="file-trash-description">
            Move “{props.result.title}” to Trash? You can restore it from the
            system Trash.
          </p>
          <button
            ref={(element) => queueMicrotask(() => element.focus())}
            disabled={disabled()}
            onClick={() => setConfirmTrash(false)}
          >
            Cancel
          </button>
          <button
            disabled={disabled()}
            onClick={() => void execute("trash", true)}
          >
            Confirm Move to Trash
          </button>
        </div>
      </Show>
      <Show when={error()}>
        <p role="alert">{error()}</p>
      </Show>
      <Show when={status()}>
        <p role="status">{status()}</p>
      </Show>
    </section>
  );
}
