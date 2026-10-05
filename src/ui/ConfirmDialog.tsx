import { onCleanup, onMount } from "solid-js";

/** Asks before a destructive action. Cancel has focus, so Enter is safe. */
export function ConfirmDialog(props: {
  message: string;
  confirmLabel: string;
  onConfirm: () => void;
  onCancel: () => void;
}) {
  let cancel!: HTMLButtonElement;
  let confirm!: HTMLButtonElement;

  // Handle keys on the window before anything else, so the dialog stays
  // modal even when a click moved focus out of it.
  const onKeyDown = (event: KeyboardEvent) => {
    const inside = document.activeElement === cancel || document.activeElement === confirm;
    if (event.repeat) {
      // A held Enter must not answer the dialog it just opened.
    } else if (event.key === "Escape") {
      props.onCancel();
    } else if (event.key === "Tab") {
      (document.activeElement === cancel ? confirm : cancel).focus();
    } else if (inside) {
      // Enter and Space keep their normal meaning on the focused button.
      event.stopPropagation();
      return;
    }
    event.preventDefault();
    event.stopPropagation();
  };
  onMount(() => {
    cancel.focus();
    window.addEventListener("keydown", onKeyDown, true);
  });
  onCleanup(() => window.removeEventListener("keydown", onKeyDown, true));

  return (
    <div
      class="backdrop"
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
