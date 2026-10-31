import { createEffect, createUniqueId, on, onCleanup, onMount, Show } from "solid-js";

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
  const recorder = () => dialog.querySelector<HTMLElement>(".recorder");

  // Handle keys on the window before anything else, so the dialog stays
  // modal even when focus left it (a click on its text, or a button that a
  // save disabled), and the launcher never acts behind it.
  const onKeyDown = (event: KeyboardEvent) => {
    // The recorder takes the keys while it records and stops them there.
    if (recorder()?.classList.contains("recording")) return;
    const controls = Array.from(dialog.querySelectorAll<HTMLElement>("button:not(:disabled)"));
    const index = controls.indexOf(document.activeElement as HTMLElement);
    if (event.repeat && event.key !== "Tab") {
      // A held key acts once. Its repeats must not click a button again,
      // close the dialog after Escape stopped recording, or start recording.
    } else if (event.key === "Escape") {
      // A save already sent cannot be taken back, so it is not cancelled.
      if (!props.busy) props.onCancel();
    } else if (event.key === "Tab") {
      const step = event.shiftKey ? -1 : 1;
      const next = index < 0 ? 0 : (index + step + controls.length) % controls.length;
      controls[next]?.focus();
    } else if (index < 0) {
      // Focus left the dialog; it goes back to the recorder.
      recorder()?.focus();
    } else {
      // Enter and Space keep their normal meaning on the focused button.
      event.stopPropagation();
      return;
    }
    event.preventDefault();
    event.stopPropagation();
  };
  onMount(() => {
    recorder()?.focus();
    window.addEventListener("keydown", onKeyDown, true);
  });
  onCleanup(() => window.removeEventListener("keydown", onKeyDown, true));
  // A button that a save disables drops its focus. When the save ends and
  // the dialog stays open, the recorder takes it back.
  createEffect(
    on(
      () => props.busy,
      (busy) => {
        if (!busy && !dialog.contains(document.activeElement)) recorder()?.focus();
      },
      { defer: true },
    ),
  );

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
