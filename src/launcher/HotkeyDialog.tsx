import { createUniqueId, onMount, Show } from "solid-js";

import { ShortcutRecorder } from "../ui/ShortcutRecorder";

/**
 * Records a result's hotkey: keys that run it from any app. Recording
 * starts at once; the keys save as soon as they are pressed. A failed save
 * keeps the dialog open with the reason.
 */
export function HotkeyDialog(props: {
  title: string;
  shortcut: string;
  error?: string;
  busy: boolean;
  onSave: (shortcut: string) => Promise<unknown>;
  onRecordError: (message: string) => void;
  onCancel: () => void;
}) {
  const helpId = createUniqueId();
  const errorId = createUniqueId();
  let dialog!: HTMLDivElement;
  let pressedBackdrop = false;

  // The recorder takes the keys while it records and stops them there; any
  // other key stays in the dialog, so the launcher never acts behind it.
  const onKeyDown = (event: KeyboardEvent) => {
    event.stopPropagation();
    if (event.key === "Escape" && !props.busy) {
      event.preventDefault();
      props.onCancel();
    } else if (event.key === "Tab") {
      event.preventDefault();
      const controls = Array.from(dialog.querySelectorAll<HTMLElement>("button:not(:disabled)"));
      const index = controls.indexOf(document.activeElement as HTMLElement);
      const step = event.shiftKey ? -1 : 1;
      const next = index < 0 ? 0 : (index + step + controls.length) % controls.length;
      controls[next]?.focus();
    }
  };
  onMount(() => dialog.querySelector<HTMLButtonElement>(".recorder")?.focus());

  return (
    <div
      class="backdrop"
      onMouseDown={(event) => (pressedBackdrop = event.target === event.currentTarget)}
      onClick={(event) => {
        if (pressedBackdrop && event.target === event.currentTarget && !props.busy) {
          props.onCancel();
        }
      }}
    >
      <div
        ref={dialog}
        class="dialog"
        role="dialog"
        aria-modal="true"
        aria-label={`Hotkey for ${props.title}`}
        aria-describedby={props.error ? `${errorId} ${helpId}` : helpId}
        onKeyDown={onKeyDown}
      >
        <div class="dialog-field">
          Hotkey for {props.title}
          <ShortcutRecorder
            label={`Hotkey for ${props.title}`}
            value={props.shortcut}
            empty="None"
            startNow
            onChange={props.onSave}
            onError={props.onRecordError}
          />
        </div>
        <Show when={props.error}>
          {(text) => (
            <p id={errorId} class="dialog-error" role="alert">
              {text()}
            </p>
          )}
        </Show>
        <p id={helpId} class="dialog-help">
          Press keys with a modifier, such as Control or Alt, or a function key. They run this
          result from any app. Escape stops recording.
        </p>
        <div class="dialog-buttons">
          <Show when={props.shortcut}>
            <button
              type="button"
              class="button"
              disabled={props.busy}
              onClick={() => void props.onSave("")}
            >
              Remove Hotkey
            </button>
          </Show>
          <button
            type="button"
            class="button"
            disabled={props.busy}
            onClick={() => props.onCancel()}
          >
            Close
          </button>
        </div>
      </div>
    </div>
  );
}
