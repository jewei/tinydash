import { onCleanup, onSettled, Show } from "solid-js";

export default function ConfirmDialog(props: {
  title: string;
  description: string;
  confirmLabel: string;
  cancelLabel?: string;
  role?: "dialog" | "alertdialog";
  busyLabel: string;
  busy: boolean;
  error?: string;
  onClose: () => void;
  onConfirm: () => void;
}) {
  let dialog!: HTMLDialogElement;
  let cancel!: HTMLButtonElement;
  const previousFocus = document.activeElement;
  onSettled(() => {
    dialog.showModal();
    cancel.focus();
  });
  onCleanup(() => {
    dialog.close();
    if (previousFocus instanceof HTMLElement && previousFocus.isConnected)
      previousFocus.focus();
  });
  return (
    <dialog
      ref={dialog}
      class="confirm-dialog"
      role={props.role}
      aria-labelledby="confirm-title"
      aria-describedby="confirm-description"
      onKeyDown={(event) => {
        event.stopPropagation();
        if (event.key === "Tab") {
          const buttons = dialog.querySelectorAll<HTMLButtonElement>(
            "button:not(:disabled)",
          );
          const first = buttons[0];
          const last = buttons[buttons.length - 1];
          if (!first) {
            event.preventDefault();
          } else if (event.shiftKey && document.activeElement === first) {
            event.preventDefault();
            last.focus();
          } else if (!event.shiftKey && document.activeElement === last) {
            event.preventDefault();
            first.focus();
          }
        }
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
          {props.cancelLabel ?? "Cancel"}
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
