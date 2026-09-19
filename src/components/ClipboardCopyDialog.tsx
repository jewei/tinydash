import { createSignal, For, onCleanup, onMount, Show } from "solid-js";
import { backend, type SearchResult } from "../bridge";

export default function ClipboardCopyDialog(props: {
  mode: "edit" | "combine";
  entry: SearchResult;
  entries: SearchResult[];
  onClose: () => void;
  onCopied: () => void;
}) {
  const [text, setText] = createSignal("");
  const [selected, setSelected] = createSignal<string[]>([]);
  const [separator, setSeparator] = createSignal("\n");
  const [loading, setLoading] = createSignal(props.mode === "edit");
  const [busy, setBusy] = createSignal(false);
  const [error, setError] = createSignal<string>();
  let dialog!: HTMLDialogElement;
  let disposed = false;
  onMount(() => {
    dialog.showModal();
    if (props.mode === "edit") {
      void backend
        .clipboardPreview(props.entry.id)
        .then((entry) => {
          if (!disposed) setText(entry.content);
        })
        .catch((reason) => {
          if (!disposed) setError(String(reason));
        })
        .finally(() => {
          if (!disposed) setLoading(false);
        });
    }
  });
  onCleanup(() => {
    disposed = true;
  });

  function select(id: string, checked: boolean) {
    setSelected((values) =>
      checked ? [...values, id] : values.filter((value) => value !== id),
    );
  }

  async function copy() {
    if (busy() || loading()) return;
    setBusy(true);
    setError(undefined);
    try {
      if (props.mode === "edit")
        await backend.editClipboardCopy(props.entry.id, text());
      else await backend.copyClipboardSelection(selected(), separator());
      if (!disposed) props.onCopied();
    } catch (reason) {
      if (!disposed) setError(String(reason));
    } finally {
      if (!disposed) setBusy(false);
    }
  }

  return (
    <dialog
      ref={dialog}
      class="confirm-dialog clipboard-copy-dialog"
      aria-labelledby="clipboard-copy-title"
      onCancel={(event) => {
        event.preventDefault();
        if (!busy()) props.onClose();
      }}
    >
      <h2 id="clipboard-copy-title">
        {props.mode === "edit" ? "Edit a copy" : "Copy selected entries"}
      </h2>
      <Show
        when={props.mode === "edit"}
        fallback={
          <>
            <p>
              Select entries in the order to copy them. The number shows each
              entry's position.
            </p>
            <div class="clipboard-choices">
              <For each={props.entries}>
                {(entry) => (
                  <label>
                    <input
                      type="checkbox"
                      checked={selected().includes(entry.id)}
                      disabled={busy()}
                      onChange={(event) =>
                        select(entry.id, event.currentTarget.checked)
                      }
                    />
                    <span class="copy-order">
                      {selected().includes(entry.id)
                        ? selected().indexOf(entry.id) + 1
                        : ""}
                    </span>
                    <span title={entry.title}>{entry.title}</span>
                  </label>
                )}
              </For>
            </div>
            <label class="copy-separator">
              Separate entries with
              <select
                value={separator()}
                disabled={busy()}
                onChange={(event) => setSeparator(event.currentTarget.value)}
              >
                <option value={"\n"}>New line</option>
                <option value={"\n\n"}>Blank line</option>
                <option value=" ">Space</option>
                <option value={"\t"}>Tab</option>
                <option value=", ">Comma and space</option>
              </select>
            </label>
          </>
        }
      >
        <p>
          The original entry stays in history. Copy the changed text, then paste
          it where you need it.
        </p>
        <textarea
          aria-label="Edited clipboard text"
          value={text()}
          maxLength={16384}
          disabled={loading() || busy()}
          spellcheck={false}
          onInput={(event) => setText(event.currentTarget.value)}
        />
      </Show>
      <Show when={loading()}>
        <p role="status">Loading text...</p>
      </Show>
      <Show when={error()}>
        <p class="dialog-error" role="alert">
          {error()}
        </p>
      </Show>
      <div class="dialog-actions">
        <button disabled={busy()} onClick={props.onClose}>
          Cancel
        </button>
        <button
          class="confirm-button"
          disabled={
            busy() ||
            loading() ||
            (props.mode === "edit" ? !text().trim() : !selected().length)
          }
          onClick={() => void copy()}
        >
          {busy()
            ? "Copying..."
            : props.mode === "edit"
              ? "Copy edited text"
              : `Copy ${selected().length} entries`}
        </button>
      </div>
    </dialog>
  );
}
