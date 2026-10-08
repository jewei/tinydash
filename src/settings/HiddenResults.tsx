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

  const showAgain = (id: string) => {
    setFailure(undefined);
    ipc
      .unhideResult(id)
      .then(() => refetch())
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
      <ul>
        <For
          each={hidden.latest ?? []}
          fallback={
            <li class="list-empty">
              None. To hide one, choose Hide from Results in its actions ({modKey()} K).
            </li>
          }
        >
          {(result) => (
            <li>
              <span class="hidden-result">
                <span>{result.title}</span>
                <span class="row-description">{result.subtitle}</span>
              </span>
              <button
                type="button"
                class="link"
                aria-label={`Show ${result.title} again`}
                onClick={() => showAgain(result.id)}
              >
                Show Again
              </button>
            </li>
          )}
        </For>
      </ul>
      <p class="list-status" role="status">
        {failure() ?? ""}
      </p>
    </div>
  );
}
