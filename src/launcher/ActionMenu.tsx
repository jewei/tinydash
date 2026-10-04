import { createMemo, createSignal, For, onMount, Show } from "solid-js";

import { hasMod } from "../lib/keys";
import { Keys } from "../ui/Keys";

export interface MenuItem {
  label: string;
  keys?: string[];
  run: () => void;
}

/** A filterable list of actions, opened with Mod+K. */
export function ActionMenu(props: { items: MenuItem[]; onClose: () => void }) {
  const [filter, setFilter] = createSignal("");
  const [active, setActive] = createSignal(0);
  const visible = createMemo(() => {
    const words = filter().toLowerCase().split(/\s+/).filter(Boolean);
    return props.items.filter((item) =>
      words.every((word) => item.label.toLowerCase().includes(word)),
    );
  });
  let input!: HTMLInputElement;
  onMount(() => input.focus());

  const choose = (item: MenuItem | undefined) => {
    if (!item) return;
    props.onClose();
    item.run();
  };

  const onKeyDown = (event: KeyboardEvent) => {
    event.stopPropagation();
    const last = visible().length - 1;
    if (event.key === "ArrowDown") setActive(Math.min(active() + 1, last));
    else if (event.key === "ArrowUp") setActive(Math.max(active() - 1, 0));
    else if (event.key === "Enter" && !event.isComposing) choose(visible()[active()]);
    else if (event.key === "Escape" || (hasMod(event) && event.key === "k")) props.onClose();
    else return;
    event.preventDefault();
  };

  return (
    <div class="menu" role="dialog" aria-label="Actions" onKeyDown={onKeyDown}>
      <input
        ref={input}
        class="menu-filter"
        placeholder="Search actions"
        value={filter()}
        onInput={(event) => {
          setFilter(event.currentTarget.value);
          setActive(0);
        }}
        aria-controls="menu-items"
        aria-activedescendant={`menu-item-${active()}`}
        autocomplete="off"
        spellcheck={false}
      />
      <ul id="menu-items" class="menu-items" role="listbox">
        <For each={visible()} fallback={<li class="menu-empty">No matching actions</li>}>
          {(item, index) => (
            <li
              id={`menu-item-${index()}`}
              class="menu-item"
              role="option"
              aria-selected={index() === active()}
              onMouseMove={() => setActive(index())}
              onClick={() => choose(item)}
            >
              <span>{item.label}</span>
              <Show when={item.keys}>{(keys) => <Keys keys={keys()} />}</Show>
            </li>
          )}
        </For>
      </ul>
    </div>
  );
}
