import { createResource, createSignal, For, onCleanup, Show } from "solid-js";

import * as ipc from "../lib/ipc";
import { modKey } from "../lib/keys";

/**
 * The aliases the user set, each with a way to remove it. Aliases set in the
 * launcher while Settings is open appear at once.
 */
export function AliasList() {
  const [aliases, { refetch }] = createResource(ipc.aliases);
  const [failure, setFailure] = createSignal<string>();
  const stop = ipc.onResultsStale(() => void refetch());
  onCleanup(() => void stop.then((unlisten) => unlisten()));

  const remove = (id: string) => {
    setFailure(undefined);
    ipc
      .runAction({ type: "setAlias", id, alias: "" })
      .then(() => refetch())
      .catch((error) => setFailure(ipc.message(error)));
  };

  return (
    <div class="list-editor">
      <Show when={aliases.error}>
        {(error) => (
          <p class="error" role="alert">
            Could not read the aliases: {ipc.message(error())}
          </p>
        )}
      </Show>
      <ul>
        <For
          each={aliases.error ? [] : (aliases.latest ?? [])}
          fallback={
            <li class="list-empty">
              None. To add one, choose Add Alias in a result's actions ({modKey()} K).
            </li>
          }
        >
          {(entry) => (
            <li>
              <span class="hidden-result">
                <span>
                  <code>{entry.alias}</code> {entry.title}
                </span>
                <span class="row-description">{entry.subtitle}</span>
              </span>
              <button
                type="button"
                class="link"
                aria-label={`Remove the alias ${entry.alias} of ${entry.title}`}
                onClick={() => remove(entry.id)}
              >
                Remove
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
