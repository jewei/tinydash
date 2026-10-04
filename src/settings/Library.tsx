import { createResource, createSignal, For, Show } from "solid-js";

import type { LibraryItem } from "../generated/LibraryItem";
import type { LibraryKind } from "../generated/LibraryKind";
import * as ipc from "../lib/ipc";

const blank = (kind: LibraryKind): LibraryItem => ({
  id: null,
  kind,
  name: "",
  keyword: "",
  text: "",
});

const HELP: Record<LibraryKind, string> = {
  snippet: "Enter copies the text. Placeholders: {date}, {time}, {datetime}, and {clipboard}.",
  quicklink:
    "Opens an http, https, or mailto URL, or a file path. {query} is replaced by text typed after the keyword, as in “jira ABC-12”.",
};

/** Create, edit, and delete snippets or quicklinks. */
export function Library(props: { kind: LibraryKind }) {
  const [items, { refetch }] = createResource(ipc.libraryItems);
  const [draft, setDraft] = createSignal<LibraryItem>(blank(props.kind));
  const [error, setError] = createSignal<string>();
  const [confirmingDelete, setConfirmingDelete] = createSignal(false);

  const ofKind = () => (items() ?? []).filter((item) => item.kind === props.kind);
  const noun = () => (props.kind === "snippet" ? "Snippet" : "Quicklink");
  const edit = (item: LibraryItem) => {
    setDraft({ ...item });
    setError(undefined);
    setConfirmingDelete(false);
  };
  const change = (field: "name" | "keyword" | "text", value: string) =>
    setDraft((item) => ({ ...item, [field]: value }));

  const save = async () => {
    try {
      edit(await ipc.saveLibraryItem(draft()));
      await refetch();
    } catch (failure) {
      setError(ipc.message(failure));
    }
  };

  const remove = async () => {
    const id = draft().id;
    if (id === null) return;
    try {
      await ipc.deleteLibraryItem(id);
      edit(blank(props.kind));
      await refetch();
    } catch (failure) {
      setError(ipc.message(failure));
    }
  };

  return (
    <div class="library">
      <div class="library-list">
        <button type="button" class="button" onClick={() => edit(blank(props.kind))}>
          New {noun()}
        </button>
        <ul>
          <For
            each={ofKind()}
            fallback={<li class="list-empty">No {noun().toLowerCase()}s yet</li>}
          >
            {(item) => (
              <li>
                <button
                  type="button"
                  class="library-item"
                  aria-current={item.id === draft().id}
                  onClick={() => edit(item)}
                >
                  <span>{item.name}</span>
                  <Show when={item.keyword}>
                    <code>{item.keyword}</code>
                  </Show>
                </button>
              </li>
            )}
          </For>
        </ul>
      </div>

      <form
        class="library-editor"
        onSubmit={(event) => {
          event.preventDefault();
          void save();
        }}
      >
        <label>
          Name
          <input
            class="field"
            required
            maxLength={100}
            value={draft().name}
            onInput={(event) => change("name", event.currentTarget.value)}
          />
        </label>
        <label>
          Keyword <span class="optional">(optional, one word)</span>
          <input
            class="field"
            maxLength={32}
            value={draft().keyword}
            onInput={(event) => change("keyword", event.currentTarget.value)}
          />
        </label>
        <label>
          {props.kind === "snippet" ? "Text" : "URL or path"}
          <Show
            when={props.kind === "snippet"}
            fallback={
              <input
                class="field"
                required
                placeholder="https://example.com/search?q={query}"
                value={draft().text}
                onInput={(event) => change("text", event.currentTarget.value)}
              />
            }
          >
            <textarea
              class="field"
              required
              rows={8}
              value={draft().text}
              onInput={(event) => change("text", event.currentTarget.value)}
            />
          </Show>
        </label>
        <p class="help">{HELP[props.kind]}</p>
        <Show when={error()}>
          <p class="error" role="alert">
            {error()}
          </p>
        </Show>
        <div class="library-buttons">
          <Show when={draft().id !== null}>
            <Show
              when={confirmingDelete()}
              fallback={
                <button type="button" class="button" onClick={() => setConfirmingDelete(true)}>
                  Delete
                </button>
              }
            >
              <span>Delete “{draft().name}”?</span>
              <button type="button" class="button" onClick={() => setConfirmingDelete(false)}>
                Keep
              </button>
              <button type="button" class="button danger" onClick={() => void remove()}>
                Delete
              </button>
            </Show>
          </Show>
          <button type="submit" class="button primary">
            Save {noun()}
          </button>
        </div>
      </form>
    </div>
  );
}
