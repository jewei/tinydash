import { createSignal, createUniqueId, For, type JSX } from "solid-js";

import { isComposing } from "../lib/keys";

/**
 * One labeled setting: text on the left, its control on the right. The row
 * is a group named and described by its text, so a screen reader reads the
 * description with the control.
 */
export function Row(props: { label: string; description?: string; children: JSX.Element }) {
  const id = createUniqueId();
  return (
    <div
      class="row"
      role="group"
      aria-labelledby={`${id}-label`}
      aria-describedby={props.description ? `${id}-description` : undefined}
    >
      <div class="row-text">
        <span id={`${id}-label`} class="row-label">
          {props.label}
        </span>
        {props.description && (
          <span id={`${id}-description`} class="row-description">
            {props.description}
          </span>
        )}
      </div>
      <div class="row-control">{props.children}</div>
    </div>
  );
}

export function Toggle(props: {
  label: string;
  checked: boolean;
  onChange: (on: boolean) => void;
}) {
  return (
    <input
      type="checkbox"
      role="switch"
      class="toggle"
      aria-label={props.label}
      checked={props.checked}
      onChange={(event) => props.onChange(event.currentTarget.checked)}
    />
  );
}

export function Select<T extends string | number>(props: {
  label: string;
  value: T;
  options: ReadonlyArray<{ value: T; label: string }>;
  onChange: (value: T) => void;
}) {
  return (
    <select
      class="field"
      aria-label={props.label}
      onChange={(event) => {
        const option = props.options[event.currentTarget.selectedIndex];
        if (option) props.onChange(option.value);
      }}
    >
      <For each={props.options}>
        {(option) => (
          <option value={String(option.value)} selected={option.value === props.value}>
            {option.label}
          </option>
        )}
      </For>
    </select>
  );
}

/** A number that saves when the field loses focus or Enter is pressed. */
export function NumberField(props: {
  label: string;
  value: number;
  min: number;
  max: number;
  onChange: (value: number) => void;
}) {
  // An empty or invalid field restores the saved value instead of saving 0.
  const commit = (input: HTMLInputElement) => {
    const value = input.value.trim() === "" ? Number.NaN : Math.round(Number(input.value));
    if (!Number.isFinite(value)) {
      input.value = String(props.value);
      return;
    }
    const clamped = Math.min(Math.max(value, props.min), props.max);
    input.value = String(clamped);
    if (clamped !== props.value) props.onChange(clamped);
  };
  return (
    <input
      type="number"
      class="field number"
      aria-label={props.label}
      min={props.min}
      max={props.max}
      value={props.value}
      onBlur={(event) => commit(event.currentTarget)}
      onKeyDown={(event) =>
        event.key === "Enter" && !isComposing(event) && commit(event.currentTarget)
      }
    />
  );
}

/**
 * A list of strings with Add and Remove. `onChange` saves the new list and
 * gives back its error, which shows next to the field: the page's own
 * error line may be scrolled out of view.
 */
export function ListEditor(props: {
  label: string;
  items: string[];
  max: number;
  placeholder: string;
  onChange: (items: string[]) => Promise<string | undefined>;
}) {
  const [draft, setDraft] = createSignal("");
  const [notice, setNotice] = createSignal<string>();
  const [saving, setSaving] = createSignal(false);
  const full = () => props.items.length >= props.max;
  // The field stays when the list is full, and Remove hands focus to it,
  // so keyboard focus never falls back to the page.
  let field!: HTMLInputElement;
  // One change at a time: each list starts from the saved one, so an entry
  // that the save refuses is never sent again with the next change.
  const change = (items: string[]) => {
    setSaving(true);
    setNotice(undefined);
    return props.onChange(items).then((failure) => {
      setNotice(failure);
      setSaving(false);
    });
  };
  const add = () => {
    const value = draft().trim();
    if (saving() || full() || !value) return;
    if (props.items.includes(value)) return setNotice(`“${value}” is already in the list.`);
    // The text stays until the save works, so a refused entry can be fixed.
    void change([...props.items, value]).then(() => {
      if (!notice() && draft().trim() === value) setDraft("");
    });
  };
  return (
    <div class="list-editor">
      <ul>
        <For each={props.items} fallback={<li class="list-empty">None</li>}>
          {(item) => (
            <li>
              <span>{item}</span>
              <button
                type="button"
                class="link"
                aria-label={`Remove ${item}`}
                onClick={() => {
                  if (!saving()) void change(props.items.filter((other) => other !== item));
                  field.focus();
                }}
              >
                Remove
              </button>
            </li>
          )}
        </For>
      </ul>
      <div class="list-add">
        <input
          ref={field}
          class="field"
          aria-label={props.label}
          placeholder={props.placeholder}
          value={draft()}
          onInput={(event) => {
            setDraft(event.currentTarget.value);
            setNotice(undefined);
          }}
          onKeyDown={(event) => event.key === "Enter" && !isComposing(event) && add()}
        />
        <button type="button" class="button" disabled={full()} onClick={add}>
          Add
        </button>
      </div>
      <p class="list-status" role="status">
        {notice() ?? (full() ? `The list is full (${props.max}). Remove one to add another.` : "")}
      </p>
    </div>
  );
}
