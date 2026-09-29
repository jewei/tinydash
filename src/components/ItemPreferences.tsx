import { createMemo, createSignal, For, onSettled, Show } from "solid-js";
import { backend, type ItemPreference, type SearchResult } from "../bridge";

export default function ItemPreferences(props: {
  value: Record<string, ItemPreference>;
  onChange: (value: Record<string, ItemPreference>) => void;
  onRecord: (id: string) => void;
  recording: string | null;
}) {
  const [items, setItems] = createSignal<SearchResult[]>([]);
  const [query, setQuery] = createSignal("");
  const [selected, setSelected] = createSignal("");
  const [error, setError] = createSignal("");
  const matches = createMemo(() =>
    items().filter((item) =>
      item.title.toLocaleLowerCase().includes(query().toLocaleLowerCase()),
    ),
  );
  const current = (): ItemPreference =>
    props.value[selected()] ?? {
      aliases: [],
      shortcut: "",
      hidden: false,
      disabled: false,
    };
  const update = (patch: Partial<ItemPreference>) =>
    props.onChange({
      ...props.value,
      [selected()]: { ...current(), ...patch },
    });
  onSettled(
    () =>
      void backend.itemCatalog().then(
        (items) => {
          setItems(items);
          setSelected(items[0]?.id ?? "");
        },
        (reason) => setError(String(reason)),
      ),
  );
  return (
    <div class="settings-group">
      <h2>Item shortcuts and aliases</h2>
      <p>
        Configure applications and built-in commands. Hide removes search
        results; Disable also stops its shortcut. Save changes to apply.
      </p>
      <Show when={error()}>
        <p role="alert">{error()}</p>
      </Show>
      <label class="settings-field-label">
        Find an item
        <input
          aria-label="Find an item"
          type="search"
          value={query()}
          onInput={(event) => setQuery(event.currentTarget.value)}
        />
      </label>
      <label class="settings-field-label">
        Item
        <select
          aria-label="Configure item"
          value={selected()}
          onChange={(event) => setSelected(event.currentTarget.value)}
        >
          <For each={matches()}>
            {(item) => <option value={item.id}>{item.title}</option>}
          </For>
        </select>
      </label>
      <Show when={selected()}>
        <label class="settings-field-label">
          Aliases
          <textarea
            aria-label="Item aliases"
            rows={2}
            value={current().aliases.join("\n")}
            onInput={(event) =>
              update({
                aliases: event.currentTarget.value
                  .split("\n")
                  .map((value) => value.trim())
                  .filter(Boolean),
              })
            }
          />
        </label>
        <label class="settings-field-label">
          Global shortcut
          <input
            aria-label="Item global shortcut"
            placeholder="Control+Alt+K"
            value={current().shortcut}
            onInput={(event) => update({ shortcut: event.currentTarget.value })}
          />
        </label>
        <div class="settings-buttons">
          <button type="button" onClick={() => props.onRecord(selected())}>
            {props.recording === `item:${selected()}`
              ? "Press shortcut keys…"
              : "Record item shortcut"}
          </button>
          <button
            type="button"
            disabled={!current().shortcut}
            onClick={() => update({ shortcut: "" })}
          >
            Clear item shortcut
          </button>
        </div>
        <label>
          <input
            type="checkbox"
            checked={current().hidden}
            onChange={(event) =>
              update({ hidden: event.currentTarget.checked })
            }
          />
          Hide item from search
        </label>
        <label>
          <input
            type="checkbox"
            checked={current().disabled}
            onChange={(event) =>
              update({ disabled: event.currentTarget.checked })
            }
          />
          Disable item and shortcut
        </label>
      </Show>
    </div>
  );
}
