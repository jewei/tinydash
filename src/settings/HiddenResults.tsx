import { createResource, createSignal, For, onCleanup, Show } from "solid-js";

import * as ipc from "../lib/ipc";
import { modKey } from "../lib/keys";

/**
 * The results the user hid, each with a way to show it again. Results hidden
 * in the launcher while Settings is open appear at once.
 */
export function HiddenResults() {
  const [hidden, { refetch }] = createResource(ipc.hiddenResults);
  const [failure, setFailure] = createSignal<string>();
  const stop = ipc.onResultsStale(() => void refetch());
  onCleanup(() => void stop.then((unlisten) => unlisten()));

  let list!: HTMLUListElement;
  // The shown row's button had focus; the row now in its place, or the list
  // when it is empty, takes it, so keyboard users keep their place.
  const showAgain = (id: string, index: number) => {
    setFailure(undefined);
    ipc
      .unhideResult(id)
      .then(() => refetch())
      .then(() => {
        const buttons = list.querySelectorAll<HTMLButtonElement>("button");
        (buttons[Math.min(index, buttons.length - 1)] ?? list).focus();
      })
      .catch((error) => setFailure(ipc.message(error)));
  };

  return (
    <div class="list-editor">
      <Show when={hidden.error}>
        {(error) => (
          <p class="error" role="alert">
            Could not read the hidden results: {ipc.message(error())}
          </p>
        )}
      </Show>
      <Show when={!hidden.error}>
        <ul ref={list} tabIndex={-1} aria-label="Hidden results">
          <For
            each={hidden.latest ?? []}
            fallback={
              <li class="list-empty">
                None. To hide one, choose Hide from Results in its actions ({modKey()} K).
              </li>
            }
          >
            {(result, index) => (
              <li>
                <span class="hidden-result">
                  <span>{result.title}</span>
                  <span class="row-description">{result.subtitle}</span>
                </span>
                <button
                  type="button"
                  class="link"
                  aria-label={`Show ${result.title} again`}
                  onClick={() => showAgain(result.id, index())}
                >
                  Show Again
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
