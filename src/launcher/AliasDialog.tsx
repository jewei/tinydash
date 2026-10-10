import { createEffect, createSignal, createUniqueId, on, onCleanup, onMount, Show } from "solid-js";

import { isComposing } from "../lib/keys";

/**
 * Asks for a result's alias: one word that finds it first. Empty removes
 * it. A failed save keeps the dialog open with the reason.
 */
export function AliasDialog(props: {
  title: string;
  alias: string;
  error?: string;
  busy: boolean;
  onSave: (alias: string) => void;
  onCancel: () => void;
}) {
  const [alias, setAlias] = createSignal(props.alias);
  const helpId = createUniqueId();
  const errorId = createUniqueId();
  let form!: HTMLFormElement;
  let field!: HTMLInputElement;

  // Handle keys on the window before anything else, so the dialog stays
  // modal even when a click moved focus out of it, and the launcher's keys
  // never act behind it.
  const onKeyDown = (event: KeyboardEvent) => {
    if (isComposing(event)) return;
    const controls = Array.from(form.querySelectorAll<HTMLElement>("input, button:not(:disabled)"));
    const index = controls.indexOf(document.activeElement as HTMLElement);
    if (event.key === "Escape") {
      // A save already sent cannot be taken back, so it is not cancelled.
      if (!props.busy) props.onCancel();
    } else if (event.key === "Tab") {
      const step = event.shiftKey ? -1 : 1;
      const next = index < 0 ? 0 : (index + step + controls.length) % controls.length;
      controls[next]?.focus();
    } else if (index < 0) {
      // Focus left the dialog; the key goes to the field, as if typed there.
      field.focus();
      event.stopPropagation();
      return;
    } else if (!(event.key === "Enter" && event.repeat)) {
      // Keys keep their normal meaning in the field and on the buttons. A
      // held Enter opened the dialog; its repeats must not save it.
      event.stopPropagation();
      return;
    }
    event.preventDefault();
    event.stopPropagation();
  };
  onMount(() => {
    field.focus();
    field.select();
    window.addEventListener("keydown", onKeyDown, true);
  });
  onCleanup(() => window.removeEventListener("keydown", onKeyDown, true));
  // The Save button is disabled while it saves, which drops its focus; the
  // field, which the reason describes, takes it back.
  createEffect(
    on(
      () => props.error,
      (error) => {
        if (error) field.focus();
      },
      { defer: true },
    ),
  );

  return (
    <div
      class="backdrop"
      onClick={(event) => event.target === event.currentTarget && !props.busy && props.onCancel()}
    >
      <form
        ref={form}
        class="dialog"
        role="dialog"
        aria-modal="true"
        aria-label={`Alias for ${props.title}`}
        onSubmit={(event) => {
          event.preventDefault();
          if (!props.busy) props.onSave(alias());
        }}
      >
        <label class="dialog-field">
          Alias for {props.title}
          <input
            ref={field}
            class="field"
            value={alias()}
            aria-describedby={props.error ? `${errorId} ${helpId}` : helpId}
            onInput={(event) => setAlias(event.currentTarget.value)}
            autocomplete="off"
            autocorrect="off"
            autocapitalize="off"
            spellcheck={false}
          />
        </label>
        <Show when={props.error}>
          {(text) => (
            <p id={errorId} class="dialog-error" role="alert">
              {text()}
            </p>
          )}
        </Show>
        <p id={helpId} class="dialog-help">
          One word. Typing it puts this result first. Leave it empty to remove the alias.
        </p>
        <div class="dialog-buttons">
          <button
            type="button"
            class="button"
            disabled={props.busy}
            onClick={() => props.onCancel()}
          >
            Cancel
          </button>
          <button type="submit" class="button primary" disabled={props.busy}>
            Save Alias
          </button>
        </div>
      </form>
    </div>
  );
}
