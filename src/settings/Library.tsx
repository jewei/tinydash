import {
  createEffect,
  createResource,
  createSignal,
  createUniqueId,
  For,
  on,
  Show,
} from "solid-js";

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
  snippet:
    "Enter copies the text. Placeholders: {date}, {time}, {datetime}, {clipboard}, and {query}, the text typed after the keyword, as in “hi Sam”.",
  quicklink:
    "Opens an http, https, or mailto URL, or an absolute or ~ path. {query} is replaced by text typed after the keyword, as in “jira ABC-12”. Also: {clipboard}, {date}, {time}, and {datetime}.",
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
  // Each item opened in the editor, a new one included, is its own session;
  // the reply to a save or delete, and its error, change only the session it
  // came from.
  let session = 0;
  const edit = (item: LibraryItem) => {
    session += 1;
    setDraft({ ...item });
    setError(undefined);
    setConfirmingDelete(false);
  };
  // An open item that a delete removed stays open as a new one, with its
  // text, so Save adds it again instead of failing. Checked only when the
  // list changes, so a save's new ID never looks deleted before the refetch.
  createEffect(
    on(
      items,
      (list) => {
        const id = draft().id;
        if (list && id !== null && !list.some((item) => item.id === id)) {
          setDraft((item) => ({ ...item, id: null }));
        }
      },
      { defer: true },
    ),
  );
  const change = (field: "name" | "keyword" | "text", value: string) =>
    setDraft((item) => ({ ...item, [field]: value }));

  /**
   * Run one write at a time, so a double click cannot save twice. `open`
   * says whether the session the write came from is still in the editor.
   */
  const write = async (work: (open: () => boolean) => Promise<void>) => {
    if (busy()) return;
    setBusy(true);
    setError(undefined);
    const from = session;
    const open = () => session === from;
    try {
      await work(open);
    } catch (failure) {
      if (open()) setError(ipc.message(failure));
    } finally {
      // Also after a failure: the list shows what the backend now holds.
      await refetch();
      setBusy(false);
    }
  };

  // Typing goes on while a save runs: the reply replaces the draft only if
  // nothing changed, and otherwise gives a new item its ID. A different or
  // new item opened meanwhile stays as it is.
  const save = () =>
    write(async (open) => {
      const sent = draft();
      const saved = await ipc.saveLibraryItem(sent);
      if (draft() === sent) edit(saved);
      else if (open()) setDraft((item) => ({ ...item, id: saved.id }));
    });

  const remove = () =>
    write(async (open) => {
      const id = draft().id;
      if (id === null) return;
      await ipc.deleteLibraryItem(id);
      if (!open()) return;
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
            emoji short. Rust checks the name and keyword limits in
            characters on save. */}
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
