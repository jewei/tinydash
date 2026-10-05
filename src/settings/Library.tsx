import { createResource, createSignal, createUniqueId, For, Show } from "solid-js";

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
    "Opens an http, https, or mailto URL, or an absolute or ~ path. {query} is replaced by text typed after the keyword, as in “jira ABC-12”.",
};

/** Create, edit, and delete snippets or quicklinks. */
export function Library(props: { kind: LibraryKind }) {
  // A failed read must say so, not look like an empty library.
  const [loadError, setLoadError] = createSignal<string>();
  const load = () =>
    ipc.libraryItems().then(
      (list) => {
        setLoadError(undefined);
        return list;
      },
      (failure: unknown) => {
        setLoadError(ipc.message(failure));
        return [];
      },
    );
  const [items, { refetch }] = createResource(load);
  const [draft, setDraft] = createSignal<LibraryItem>(blank(props.kind));
  const [error, setError] = createSignal<string>();
  const [confirmingDelete, setConfirmingDelete] = createSignal(false);
  const [busy, setBusy] = createSignal(false);
  let nameField!: HTMLInputElement;
  // The inline confirmation replaces the button that had focus; move focus
  // to its counterpart so keyboard users keep their place.
  let deleteButton: HTMLButtonElement | undefined;
  let keepButton: HTMLButtonElement | undefined;
  // Focus moves to Keep, so Keep carries the question for screen readers.
  const questionId = createUniqueId();

  const ofKind = () => (items() ?? []).filter((item) => item.kind === props.kind);
  const noun = () => (props.kind === "snippet" ? "Snippet" : "Quicklink");
  // Delete removes the saved item, so ask with its saved name, not an
  // unsaved edit.
  const savedName = () => items()?.find((item) => item.id === draft().id)?.name ?? draft().name;
  const edit = (item: LibraryItem) => {
    setDraft({ ...item });
    setError(undefined);
    setConfirmingDelete(false);
  };
  const change = (field: "name" | "keyword" | "text", value: string) =>
    setDraft((item) => ({ ...item, [field]: value }));

  /** Run one write at a time, so a double click cannot save twice. */
  const write = async (work: () => Promise<void>) => {
    if (busy()) return;
    setBusy(true);
    try {
      await work();
      await refetch();
    } catch (failure) {
      setError(ipc.message(failure));
    } finally {
      setBusy(false);
    }
  };

  const save = () => write(async () => edit(await ipc.saveLibraryItem(draft())));

  const remove = () =>
    write(async () => {
      const id = draft().id;
      if (id === null) return;
      await ipc.deleteLibraryItem(id);
      edit(blank(props.kind));
      nameField.focus();
    });

  return (
    <div class="library">
      <div class="library-list">
        <button type="button" class="button" onClick={() => edit(blank(props.kind))}>
          New {noun()}
        </button>
        <Show when={loadError()}>
          {(text) => (
            <p class="error" role="alert">
              Could not load the list: {text()}
            </p>
          )}
        </Show>
        <ul>
          <For
            each={ofKind()}
            fallback={
              <Show when={!loadError()}>
                <li class="list-empty">No {noun().toLowerCase()}s yet</li>
              </Show>
            }
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
        {/* No maxLength: it counts UTF-16 units, so it would cut a name of
            emoji short. Rust checks the limits in characters on save. */}
        <label>
          Name
          <input
            ref={nameField}
            class="field"
            required
            value={draft().name}
            onInput={(event) => change("name", event.currentTarget.value)}
          />
        </label>
        <label>
          Keyword <span class="optional">(optional, one word)</span>
          <input
            class="field"
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
                <button
                  ref={deleteButton}
                  type="button"
                  class="button"
                  onClick={() => {
                    setConfirmingDelete(true);
                    keepButton?.focus();
                  }}
                >
                  Delete
                </button>
              }
            >
              <span id={questionId}>Delete “{savedName()}”?</span>
              <button
                ref={keepButton}
                type="button"
                class="button"
                aria-describedby={questionId}
                onClick={() => {
                  setConfirmingDelete(false);
                  deleteButton?.focus();
                }}
              >
                Keep
              </button>
              <button type="button" class="button danger" onClick={() => void remove()}>
                Delete
              </button>
            </Show>
          </Show>
          <button type="submit" class="button primary" disabled={busy()}>
            Save {noun()}
          </button>
        </div>
      </form>
    </div>
  );
}
