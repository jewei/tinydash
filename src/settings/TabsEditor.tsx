import { For, Show } from "solid-js";

import type { Category } from "../generated/Category";
import { CATEGORY_LABELS, OPTIONAL_TABS } from "../lib/categories";

/**
 * The launcher's tabs after All: move, remove, and add them. Each change is
 * an edit of the latest saved list, so quick clicks never undo each other.
 * Focus follows the tab it acted on, so the keyboard never falls back to
 * the page.
 */
export function TabsEditor(props: {
  tabs: Category[];
  onChange: (edit: (tabs: Category[]) => Category[]) => void;
}) {
  let editor!: HTMLDivElement;
  const hidden = () => OPTIONAL_TABS.filter((tab) => !props.tabs.includes(tab));
  // The list redraws after a change; focus the first of these that is enabled.
  const focus = (...selectors: string[]) =>
    queueMicrotask(() => {
      for (const selector of selectors) {
        const button = editor.querySelector<HTMLButtonElement>(selector);
        if (button && !button.disabled) return button.focus();
      }
    });

  const move = (tab: Category, by: 1 | -1) => {
    props.onChange((tabs) => {
      const from = tabs.indexOf(tab);
      const to = from + by;
      if (from < 0 || to < 0 || to >= tabs.length) return tabs;
      const next = tabs.filter((other) => other !== tab);
      next.splice(to, 0, tab);
      return next;
    });
    const [same, other] = by < 0 ? ["up", "down"] : ["down", "up"];
    focus(
      `[data-tab="${tab}"] [data-move="${same}"]`,
      `[data-tab="${tab}"] [data-move="${other}"]`,
    );
  };
  const remove = (tab: Category) => {
    props.onChange((tabs) => tabs.filter((other) => other !== tab));
    focus(`[data-add="${tab}"]`);
  };
  const add = (tab: Category) => {
    props.onChange((tabs) => (tabs.includes(tab) ? tabs : [...tabs, tab]));
    focus(`[data-tab="${tab}"] [data-remove]`);
  };

  return (
    <div ref={editor} class="list-editor tabs-editor">
      <ol aria-label="Tabs, in order">
        <li>
          <span>{CATEGORY_LABELS.all}</span>
          <span class="tabs-fixed">Always first</span>
        </li>
        <For each={props.tabs}>
          {(tab, index) => (
            <li data-tab={tab}>
              <span>{CATEGORY_LABELS[tab]}</span>
              <span class="tabs-buttons">
                <button
                  type="button"
                  class="link"
                  data-move="up"
                  aria-label={`Move ${CATEGORY_LABELS[tab]} up`}
                  disabled={index() === 0}
                  onClick={() => move(tab, -1)}
                >
                  ↑
                </button>
                <button
                  type="button"
                  class="link"
                  data-move="down"
                  aria-label={`Move ${CATEGORY_LABELS[tab]} down`}
                  disabled={index() === props.tabs.length - 1}
                  onClick={() => move(tab, 1)}
                >
                  ↓
                </button>
                <button
                  type="button"
                  class="link"
                  data-remove
                  aria-label={`Remove ${CATEGORY_LABELS[tab]}`}
                  onClick={() => remove(tab)}
                >
                  Remove
                </button>
              </span>
            </li>
          )}
        </For>
      </ol>
      <Show when={hidden().length > 0}>
        <div class="tabs-add">
          <span>Add:</span>
          <For each={hidden()}>
            {(tab) => (
              <button
                type="button"
                class="button"
                data-add={tab}
                aria-label={`Add ${CATEGORY_LABELS[tab]} tab`}
                onClick={() => add(tab)}
              >
                {CATEGORY_LABELS[tab]}
              </button>
            )}
          </For>
        </div>
      </Show>
    </div>
  );
}
