import { onMount } from "solid-js";

/** Asks before a destructive action. Cancel has focus, so Enter is safe. */
export function ConfirmDialog(props: {
  message: string;
  confirmLabel: string;
  onConfirm: () => void;
  onCancel: () => void;
}) {
  let cancel!: HTMLButtonElement;
  let confirm!: HTMLButtonElement;
  onMount(() => cancel.focus());

  const onKeyDown = (event: KeyboardEvent) => {
    event.stopPropagation();
    if (event.key === "Escape") {
      event.preventDefault();
      props.onCancel();
    } else if (event.key === "Tab") {
      // Keep focus inside the dialog.
      event.preventDefault();
      (document.activeElement === cancel ? confirm : cancel).focus();
    }
  };

  return (
    <div
      class="backdrop"
      onKeyDown={onKeyDown}
      onClick={(event) => event.target === event.currentTarget && props.onCancel()}
    >
      <div
        class="dialog"
        role="alertdialog"
        aria-modal="true"
        aria-label="Confirm"
        aria-describedby="dialog-message"
      >
        <p id="dialog-message">{props.message}</p>
        <div class="dialog-buttons">
          <button ref={cancel} type="button" class="button" onClick={() => props.onCancel()}>
            Cancel
          </button>
          <button
            ref={confirm}
            type="button"
            class="button danger"
            onClick={() => props.onConfirm()}
          >
            {props.confirmLabel}
          </button>
        </div>
      </div>
    </div>
  );
}
