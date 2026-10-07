import { For } from "solid-js";

import type { Category } from "../generated/Category";
import type { LauncherTab } from "../generated/LauncherTab";
import { CATEGORY_LABELS } from "../lib/categories";
import { Toggle } from "./controls";

/**
 * The launcher's tabs after All: show or hide each one, keep its results in
 * or out of All, and move it. A tab is never both hidden and out of All, so
 * turning one switch off while the other is off turns the other on; no
 * switch is ever disabled, so focus never drops. Each change is an edit of
 * the latest saved list, so quick clicks never undo each other. Rows are keyed by category, so a switch keeps its row. A
 * moved row loses focus, also when a failed save moves it back; focus then
 * returns to its move buttons, unless the user has put it elsewhere.
 */
export function TabsEditor(props: {
  tabs: LauncherTab[];
  onChange: (edit: (tabs: LauncherTab[]) => LauncherTab[]) => Promise<unknown>;
}) {
  let editor!: HTMLOListElement;
  const order = () => props.tabs.map((tab) => tab.category);
  const tabOf = (category: Category) => props.tabs.find((tab) => tab.category === category);

  // When the moved row took focus with it, focus the first enabled one of
  // its move buttons in `directions` (at the top, Up is disabled, so Down).
  // Focus the user has since put elsewhere stays there.
  const focusMoveButton = (category: Category, directions: string[]) => () => {
    if (document.activeElement && document.activeElement !== document.body) return;
    for (const direction of directions) {
      const button = editor.querySelector<HTMLButtonElement>(
        `[data-tab="${category}"] [data-move="${direction}"]`,
      );
      if (button && !button.disabled) return button.focus();
    }
  };

  const move = (category: Category, by: 1 | -1) => {
    const saved = props.onChange((tabs) => {
      const from = tabs.findIndex((tab) => tab.category === category);
      const to = from + by;
      const moving = tabs[from];
      if (!moving || to < 0 || to >= tabs.length) return tabs;
      const next = tabs.filter((tab) => tab !== moving);
      next.splice(to, 0, moving);
      return next;
    });
    const focus = focusMoveButton(category, by < 0 ? ["up", "down"] : ["down", "up"]);
    queueMicrotask(focus);
    void saved.then(focus);
  };
  // The same rule as the backend's `normalized`, applied here so the form
  // shows it at once.
  const change = (
    category: Category,
    values: Pick<LauncherTab, "shown"> | Pick<LauncherTab, "inAll">,
  ) =>
    void props.onChange((tabs) =>
      tabs.map((tab) => {
        if (tab.category !== category) return tab;
        const next = { ...tab, ...values };
        if (!next.shown && !next.inAll) {
          if ("shown" in values) next.inAll = true;
          else next.shown = true;
        }
        return next;
      }),
    );

  return (
    <div class="list-editor">
      <ol ref={editor} aria-label="Tabs, in order">
        <li>
          <span>{CATEGORY_LABELS.all}</span>
          <span class="tabs-fixed">Always shown, always first</span>
        </li>
        <For each={order()}>
          {(category, index) => (
            <li data-tab={category}>
              <span>{CATEGORY_LABELS[category]}</span>
              <span class="tabs-buttons">
                <button
                  type="button"
                  class="link"
                  data-move="up"
                  aria-label={`Move ${CATEGORY_LABELS[category]} up`}
                  disabled={index() === 0}
                  onClick={() => move(category, -1)}
                >
                  ↑
                </button>
                <button
                  type="button"
                  class="link"
                  data-move="down"
                  aria-label={`Move ${CATEGORY_LABELS[category]} down`}
                  disabled={index() === order().length - 1}
                  onClick={() => move(category, 1)}
                >
                  ↓
                </button>
                <label class="tabs-switch">
                  <Toggle
                    label={`Show ${CATEGORY_LABELS[category]}`}
                    checked={tabOf(category)?.shown ?? false}
                    onChange={(shown) => change(category, { shown })}
                  />
                  Show
                </label>
                <label class="tabs-switch">
                  <Toggle
                    label={`${CATEGORY_LABELS[category]} in All`}
                    checked={tabOf(category)?.inAll ?? false}
                    onChange={(inAll) => change(category, { inAll })}
                  />
                  In All
                </label>
              </span>
            </li>
          )}
        </For>
      </ol>
    </div>
  );
}
