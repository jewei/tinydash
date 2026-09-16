import { createEffect, createSignal, onCleanup, Show } from "solid-js";
import { backend, type ClipboardEntry } from "../bridge";

export default function ClipboardPreview(props: { id: string }) {
  const [entry, setEntry] = createSignal<ClipboardEntry>();
  const [error, setError] = createSignal<string>();
  let sequence = 0;
  let disposed = false;
  createEffect(() => {
    const id = props.id;
    const request = ++sequence;
    setEntry(undefined);
    setError(undefined);
    void backend
      .clipboardPreview(id)
      .then((value) => {
        if (!disposed && request === sequence) setEntry(value);
      })
      .catch((reason) => {
        if (!disposed && request === sequence) setError(String(reason));
      });
  });
  onCleanup(() => {
    disposed = true;
    sequence += 1;
  });
  return (
    <aside class="clipboard-preview" aria-label="Clipboard preview">
      <h2>Clipboard text</h2>
      <Show
        when={entry()}
        fallback={<p role="status">{error() ?? "Loading text..."}</p>}
      >
        {(value) => (
          <>
            <pre tabIndex={0} aria-label="Saved clipboard text">
              {value().content}
            </pre>
            <p class="clipboard-date">
              Saved {new Date(value().createdAt * 1000).toLocaleString()}
            </p>
          </>
        )}
      </Show>
    </aside>
  );
}
