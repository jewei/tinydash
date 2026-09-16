import { onMount, Show } from "solid-js";

export default function ConfirmDialog(props: {
  title: string;
  description: string;
  confirmLabel: string;
  busyLabel: string;
  busy: boolean;
  error?: string;
  onClose: () => void;
  onConfirm: () => void;
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
      class="confirm-dialog"
      aria-labelledby="confirm-title"
      aria-describedby="confirm-description"
      onKeyDown={(event) => {
        if (event.key === "Enter" && event.repeat) event.preventDefault();
      }}
      onCancel={(event) => {
        event.preventDefault();
        if (!props.busy) props.onClose();
      }}
    >
      <h2 id="confirm-title">{props.title}</h2>
      <p id="confirm-description">{props.description}</p>
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
          class="confirm-button"
          disabled={props.busy}
          onClick={props.onConfirm}
        >
          {props.busy ? props.busyLabel : props.confirmLabel}
        </button>
      </div>
    </dialog>
  );
}
