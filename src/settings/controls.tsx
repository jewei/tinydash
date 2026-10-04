import { createSignal, For, type JSX } from "solid-js";

import { isComposing } from "../lib/keys";

/** One labeled setting: text on the left, its control on the right. */
export function Row(props: { label: string; description?: string; children: JSX.Element }) {
  return (
    <div class="row">
      <div class="row-text">
        <span class="row-label">{props.label}</span>
        {props.description && <span class="row-description">{props.description}</span>}
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

/** A list of strings with Add and Remove. */
export function ListEditor(props: {
  label: string;
  items: string[];
  placeholder: string;
  onChange: (items: string[]) => void;
}) {
  const [draft, setDraft] = createSignal("");
  const add = () => {
    const value = draft().trim();
    if (!value || props.items.includes(value)) return;
    props.onChange([...props.items, value]);
    setDraft("");
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
                onClick={() => props.onChange(props.items.filter((other) => other !== item))}
              >
                Remove
              </button>
            </li>
          )}
        </For>
      </ul>
      <div class="list-add">
        <input
          class="field"
          aria-label={props.label}
          placeholder={props.placeholder}
          value={draft()}
          onInput={(event) => setDraft(event.currentTarget.value)}
          onKeyDown={(event) => event.key === "Enter" && !isComposing(event) && add()}
        />
        <button type="button" class="button" onClick={add}>
          Add
        </button>
      </div>
    </div>
  );
}
