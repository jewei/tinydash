import { createSignal, onMount } from "solid-js";

/** Asks for a result's alias: one word that finds it first. Empty removes it. */
export function AliasDialog(props: {
  title: string;
  alias: string;
  onSave: (alias: string) => void;
  onCancel: () => void;
}) {
  const [alias, setAlias] = createSignal(props.alias);
  let field!: HTMLInputElement;
  onMount(() => {
    field.focus();
    field.select();
  });

  const onKeyDown = (event: KeyboardEvent) => {
    // The launcher's keys do not act while the dialog is open.
    event.stopPropagation();
    if (event.key === "Escape") {
      event.preventDefault();
      props.onCancel();
    } else if (event.key === "Enter" && event.repeat) {
      // A held Enter opened the dialog; its repeats must not save it.
      event.preventDefault();
    }
  };

  return (
    <div
      class="backdrop"
      onClick={(event) => event.target === event.currentTarget && props.onCancel()}
    >
      <form
        class="dialog"
        role="dialog"
        aria-modal="true"
        aria-label={`Alias for ${props.title}`}
        onKeyDown={onKeyDown}
        onSubmit={(event) => {
          event.preventDefault();
          props.onSave(alias());
        }}
      >
        <label class="dialog-field">
          Alias for {props.title}
          <input
            ref={field}
            class="field"
            value={alias()}
            onInput={(event) => setAlias(event.currentTarget.value)}
            autocomplete="off"
            autocorrect="off"
            autocapitalize="off"
            spellcheck={false}
          />
        </label>
        <p class="dialog-help">
          One word. Typing it puts this result first. Leave it empty to remove the alias.
        </p>
        <div class="dialog-buttons">
          <button type="button" class="button" onClick={() => props.onCancel()}>
            Cancel
          </button>
          <button type="submit" class="button primary">
            Save Alias
          </button>
        </div>
      </form>
    </div>
  );
}
