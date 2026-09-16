import { onMount, Show } from "solid-js";

export default function ClearHistoryDialog(props: {
  busy: boolean;
  error?: string;
  onClose: () => void;
  onClear: () => void;
}) {
  let dialog!: HTMLDialogElement;
  let cancel!: HTMLButtonElement;
  onMount(() => {
    dialog.showModal();
    cancel.focus();
  });
  return (
    <dialog
      ref={dialog}
      class="clear-dialog"
      aria-labelledby="clear-title"
      aria-describedby="clear-description"
      onCancel={(event) => {
        event.preventDefault();
        if (!props.busy) props.onClose();
      }}
    >
      <h2 id="clear-title">Clear clipboard history?</h2>
      <p id="clear-description">
        This deletes all saved text entries. The current system clipboard stays
        available.
      </p>
      <Show when={props.error}>
        <p class="dialog-error" role="alert">
          {props.error}
        </p>
      </Show>
      <div class="dialog-actions">
        <button
          ref={cancel}
          class="cancel-button"
          disabled={props.busy}
          onClick={props.onClose}
        >
          Cancel
        </button>
        <button
          class="clear-button"
          disabled={props.busy}
          onClick={props.onClear}
        >
          {props.busy ? "Clearing..." : "Clear history"}
        </button>
      </div>
    </dialog>
  );
}
