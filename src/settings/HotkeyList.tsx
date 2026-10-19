import { createResource, createSignal, For, onCleanup, Show } from "solid-js";

import * as ipc from "../lib/ipc";
import { displayKeys, modKey } from "../lib/keys";
import { Keys } from "../ui/Keys";

/**
 * The hotkeys the user set, each with a way to remove it. Hotkeys set in
 * the launcher while Settings is open appear at once.
 */
export function HotkeyList() {
  const [hotkeys, { refetch }] = createResource(ipc.hotkeys);
  const [failure, setFailure] = createSignal<string>();
  const stop = ipc.onResultsStale(() => void refetch());
  onCleanup(() => void stop.then((unlisten) => unlisten()));

  let list!: HTMLUListElement;
  // The removed row's button had focus; the row now in its place, or the
  // list when it is empty, takes it, so keyboard users keep their place.
  const remove = (id: string, index: number) => {
    setFailure(undefined);
    ipc
      .removeHotkey(id)
      .then(() => refetch())
      .then(() => {
        const buttons = list.querySelectorAll<HTMLButtonElement>("button");
        (buttons[Math.min(index, buttons.length - 1)] ?? list).focus();
      })
      .catch((error) => setFailure(ipc.message(error)));
  };

  return (
    <div class="list-editor">
      <Show when={hotkeys.error}>
        {(error) => (
          <p class="error" role="alert">
            Could not read the hotkeys: {ipc.message(error())}
          </p>
        )}
      </Show>
      <Show when={!hotkeys.error}>
        <ul ref={list} tabIndex={-1} aria-label="Hotkeys">
          <For
            each={hotkeys.latest ?? []}
            fallback={
              <li class="list-empty">
                None. To add one, choose Set Hotkey in a result's actions ({modKey()} K).
              </li>
            }
          >
            {(entry, index) => (
              <li>
                <span class="hidden-result">
                  <span>
                    <Keys keys={displayKeys(entry.shortcut)} /> {entry.title}
                  </span>
                  <span class="row-description">{entry.subtitle}</span>
                </span>
                <button
                  type="button"
                  class="link"
                  aria-label={`Remove the hotkey of ${entry.title}`}
                  onClick={() => remove(entry.id, index())}
                >
                  Remove
                </button>
              </li>
            )}
          </For>
        </ul>
      </Show>
      <p class="list-status" role="status">
        {failure() ?? ""}
      </p>
    </div>
  );
}
