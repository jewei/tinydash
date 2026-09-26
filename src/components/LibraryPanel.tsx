import {
  createEffect,
  createSignal,
  For,
  onCleanup,
  onMount,
  Show,
} from "solid-js";
import {
  libraryBackend,
  type LibraryAction,
  type LibraryDraft,
  type LibraryEntry,
  type LibraryItem,
  type LibraryKind,
} from "../library-bridge";
import "../styles/library.css";

const empty = (kind: LibraryKind): LibraryDraft => ({
  kind,
  name: "",
  keywords: "",
  content: "",
});

/** The parent owns routing and focus restoration. initialId supports root search. */
export default function LibraryPanel(props: {
  onClose: () => void;
  initialId?: string;
  onChanged?: () => void;
}) {
  const [query, setQuery] = createSignal("");
  const [items, setItems] = createSignal<LibraryItem[]>([]);
  const [entry, setEntry] = createSignal<LibraryEntry>();
  const [draft, setDraft] = createSignal<LibraryDraft>(empty("quicklink"));
  const [editing, setEditing] = createSignal(false);
  const [dirty, setDirty] = createSignal(false);
  const [confirmDiscard, setConfirmDiscard] = createSignal(false);
  const [confirmDelete, setConfirmDelete] = createSignal(false);
  const [values, setValues] = createSignal<Record<string, string>>({});
  const [allowClipboard, setAllowClipboard] = createSignal(false);
  const [busy, setBusy] = createSignal(false);
  const [error, setError] = createSignal("");
  const [notice, setNotice] = createSignal("");
  const [loaded, setLoaded] = createSignal(false);
  let alive = true;
  let filter!: HTMLInputElement;
  let listRequest = 0;
  let pendingNavigation: (() => void) | undefined;
  onCleanup(() => {
    alive = false;
    listRequest++;
  });

  const selected = () => items().find((item) => item.id === entry()?.id);
  // Selection metadata is retained while the searchable list is filtered.
  const [metadata, setMetadata] = createSignal<LibraryItem>();
  const current = () => selected() ?? metadata();

  async function refresh(search = query()) {
    const request = ++listRequest;
    try {
      const result = await libraryBackend.list(search);
      if (alive && request === listRequest) {
        setItems(result);
        setLoaded(true);
      }
    } catch (reason) {
      if (alive && request === listRequest) setError(String(reason));
    }
  }

  createEffect(() => {
    const search = query();
    const timer = setTimeout(() => void refresh(search), 100);
    onCleanup(() => clearTimeout(timer));
  });

  async function run(task: () => Promise<void>) {
    if (busy()) return;
    setBusy(true);
    setError("");
    setNotice("");
    try {
      await task();
    } catch (reason) {
      if (alive) setError(String(reason));
    } finally {
      if (alive) setBusy(false);
    }
  }

  function navigate(action: () => void) {
    if (busy()) return;
    if (dirty()) {
      pendingNavigation = action;
      setConfirmDiscard(true);
    } else action();
  }

  async function select(id: string) {
    await run(async () => {
      const [value, all] = await Promise.all([
        libraryBackend.get(id),
        libraryBackend.list(),
      ]);
      if (!alive) return;
      setEntry(value);
      setMetadata(all.find((item) => item.id === id));
      setEditing(false);
      setDirty(false);
      setValues({});
      setAllowClipboard(false);
      setConfirmDelete(false);
    });
  }

  onMount(() => {
    filter.focus();
    if (props.initialId) void select(props.initialId);
  });

  function create(kind: LibraryKind) {
    navigate(() => {
      setEntry(undefined);
      setMetadata(undefined);
      setDraft(empty(kind));
      setEditing(true);
      setDirty(false);
      setError("");
      setNotice("");
      setConfirmDelete(false);
    });
  }

  function update(field: keyof LibraryDraft, value: string) {
    setDraft((previous) => ({ ...previous, [field]: value }));
    setDirty(true);
  }

  async function save() {
    await run(async () => {
      const saved = await libraryBackend.save(entry()?.id ?? null, draft());
      if (!alive) return;
      // Keep the committed ID even if metadata refresh fails, so retry updates
      // this item rather than creating a duplicate after a successful write.
      setEntry(saved);
      setDirty(false);
      props.onChanged?.();
      const all = await libraryBackend.list();
      if (!alive) return;
      setMetadata(all.find((item) => item.id === saved.id));
      setEditing(false);
      setDirty(false);
      setValues({});
      setAllowClipboard(false);
      setNotice("Saved");
      await refresh();
    });
  }

  async function execute(action: LibraryAction) {
    const id = entry()?.id;
    if (!id || busy()) return;
    const clipboardConsent = allowClipboard();
    setAllowClipboard(false);
    await run(async () => {
      await libraryBackend.execute(id, action, values(), clipboardConsent);
      if (!alive) return;
      setNotice(
        action === "copy"
          ? "Snippet copied"
          : action === "paste"
            ? "Snippet pasted"
            : "Quicklink opened",
      );
      if (action === "paste") props.onClose();
    });
  }

  return (
    <section
      class="library-panel"
      aria-label="Quicklinks and snippets"
      onKeyDown={(event) => {
        if (event.key === "Escape") {
          event.preventDefault();
          event.stopPropagation();
          if (confirmDelete()) setConfirmDelete(false);
          else if (confirmDiscard()) setConfirmDiscard(false);
          else navigate(props.onClose);
        }
      }}
    >
      <header class="library-header">
        <h2>Quicklinks &amp; snippets</h2>
        <button
          type="button"
          disabled={busy()}
          onClick={() => navigate(props.onClose)}
        >
          Close library
        </button>
      </header>
      <p class="library-hint">
        Run items explicitly. Keywords help search; they do not expand as you
        type. Stored locally as plaintext, not a secrets vault.
      </p>
      <Show when={error()}>
        <p role="alert">{error()}</p>
      </Show>
      <p role="status" class="library-status">
        {notice()}
      </p>
      <Show when={confirmDiscard()}>
        <div
          role="alertdialog"
          aria-label="Discard unsaved changes?"
          class="library-confirm"
        >
          <p>Discard unsaved changes?</p>
          <button
            type="button"
            onClick={() => {
              setConfirmDiscard(false);
              setDirty(false);
              pendingNavigation?.();
              pendingNavigation = undefined;
            }}
          >
            Discard changes
          </button>
          <button
            type="button"
            onClick={() => {
              setConfirmDiscard(false);
              pendingNavigation = undefined;
            }}
            autofocus
          >
            Keep editing
          </button>
        </div>
      </Show>
      <div class="library-layout" inert={confirmDiscard()}>
        <aside class="library-sidebar">
          <label>
            Search library
            <input
              ref={filter}
              type="search"
              value={query()}
              maxLength={512}
              onInput={(event) => setQuery(event.currentTarget.value)}
            />
          </label>
          <div class="library-toolbar">
            <button
              type="button"
              disabled={busy()}
              onClick={() => create("quicklink")}
            >
              New quicklink
            </button>
            <button
              type="button"
              disabled={busy()}
              onClick={() => create("snippet")}
            >
              New snippet
            </button>
          </div>
          <ul class="library-items" aria-label="Library items">
            <For each={items()}>
              {(item) => (
                <li>
                  <button
                    type="button"
                    disabled={busy()}
                    aria-current={entry()?.id === item.id ? "true" : undefined}
                    onClick={() => navigate(() => void select(item.id))}
                  >
                    <strong>{item.name}</strong>
                    <small>
                      {item.kind === "quicklink" ? "Quicklink" : "Snippet"}
                      {item.keywords ? ` · ${item.keywords}` : ""}
                    </small>
                  </button>
                </li>
              )}
            </For>
          </ul>
          <Show when={loaded() && !items().length}>
            <p class="library-hint">
              No matching items. Create a quicklink or snippet to get started.
            </p>
          </Show>
        </aside>
        <main class="library-editor" aria-busy={busy()}>
          <Show
            when={editing()}
            fallback={
              <Show
                when={entry()}
                fallback={
                  <p class="library-hint">Choose an item to run or edit.</p>
                }
              >
                {(saved) => (
                  <>
                    <h3>{saved().name}</h3>
                    <p class="library-hint">{saved().keywords}</p>
                    <pre class="library-content">{saved().content}</pre>
                    <For each={current()?.arguments ?? []}>
                      {(name) => (
                        <label>
                          Argument: {name}
                          <input
                            value={values()[name] ?? ""}
                            maxLength={4096}
                            onInput={(event) =>
                              setValues((previous) => ({
                                ...previous,
                                [name]: event.currentTarget.value,
                              }))
                            }
                          />
                        </label>
                      )}
                    </For>
                    <Show when={current()?.usesClipboard}>
                      <label class="library-checkbox">
                        <input
                          type="checkbox"
                          checked={allowClipboard()}
                          onChange={(event) =>
                            setAllowClipboard(event.currentTarget.checked)
                          }
                        />
                        Allow reading the clipboard for this invocation
                      </label>
                    </Show>
                    <div class="library-toolbar">
                      <Show
                        when={saved().kind === "quicklink"}
                        fallback={
                          <>
                            <button
                              type="button"
                              disabled={
                                busy() ||
                                (!!current()?.usesClipboard &&
                                  !allowClipboard())
                              }
                              onClick={() => void execute("copy")}
                            >
                              Copy snippet
                            </button>
                            <button
                              type="button"
                              disabled={
                                busy() ||
                                (!!current()?.usesClipboard &&
                                  !allowClipboard())
                              }
                              onClick={() => void execute("paste")}
                            >
                              Paste snippet
                            </button>
                          </>
                        }
                      >
                        <button
                          type="button"
                          disabled={
                            busy() ||
                            (current()?.arguments ?? []).some(
                              (name) => !values()[name],
                            )
                          }
                          onClick={() => void execute("open")}
                        >
                          Open quicklink
                        </button>
                      </Show>
                      <button
                        type="button"
                        disabled={busy()}
                        onClick={() => {
                          setDraft({
                            kind: saved().kind,
                            name: saved().name,
                            keywords: saved().keywords,
                            content: saved().content,
                          });
                          setEditing(true);
                          setDirty(false);
                          setConfirmDelete(false);
                        }}
                      >
                        Edit item
                      </button>
                      <button
                        type="button"
                        disabled={busy()}
                        onClick={() => setConfirmDelete(true)}
                      >
                        Delete item
                      </button>
                    </div>
                    <Show when={saved().kind === "snippet"}>
                      <p class="library-hint">
                        Paste targets the previously focused app. If focus or
                        accessibility permission cannot be verified, use Copy
                        instead. Selection capture is not supported.
                      </p>
                    </Show>
                    <Show when={confirmDelete()}>
                      <div
                        role="alertdialog"
                        aria-label="Delete library item?"
                        class="library-confirm"
                      >
                        <p>Delete “{saved().name}”? This cannot be undone.</p>
                        <button
                          type="button"
                          disabled={busy()}
                          onClick={() =>
                            void run(async () => {
                              await libraryBackend.delete(saved().id);
                              if (!alive) return;
                              setEntry(undefined);
                              setMetadata(undefined);
                              setConfirmDelete(false);
                              setNotice("Item deleted");
                              props.onChanged?.();
                              await refresh();
                            })
                          }
                        >
                          Confirm delete
                        </button>
                        <button
                          type="button"
                          disabled={busy()}
                          onClick={() => setConfirmDelete(false)}
                          autofocus
                        >
                          Cancel deletion
                        </button>
                      </div>
                    </Show>
                  </>
                )}
              </Show>
            }
          >
            <form
              onSubmit={(event) => {
                event.preventDefault();
                void save();
              }}
            >
              <h3>
                {entry() ? "Edit" : "Create"} {draft().kind}
              </h3>
              <fieldset disabled={busy()}>
                <label>
                  Name
                  <input
                    required
                    maxLength={120}
                    value={draft().name}
                    onInput={(event) =>
                      update("name", event.currentTarget.value)
                    }
                  />
                </label>
                <label>
                  Keywords
                  <input
                    maxLength={512}
                    value={draft().keywords}
                    onInput={(event) =>
                      update("keywords", event.currentTarget.value)
                    }
                  />
                </label>
                <label>
                  {draft().kind === "quicklink"
                    ? "URL template"
                    : "Snippet content"}
                  <textarea
                    required
                    rows={7}
                    maxLength={32768}
                    value={draft().content}
                    onInput={(event) =>
                      update("content", event.currentTarget.value)
                    }
                    spellcheck={draft().kind === "snippet"}
                  />
                </label>
                <Show
                  when={draft().kind === "quicklink"}
                  fallback={
                    <p class="library-hint">
                      Placeholders: {"{date}"} (local YYYY-MM-DD), {"{time}"}{" "}
                      (local HH:mm), {"{clipboard}"} (explicit permission each
                      time). Other braces remain literal.
                    </p>
                  }
                >
                  <p class="library-hint">
                    Use {"{query}"} or {"{argument:name}"} in the URL path or
                    query. Arguments are URL-encoded. Allowed: https, http,
                    file, mailto, tel, spotify. For files or folders use an
                    absolute file:/// URL without arguments. No shell commands
                    or custom app routing.
                  </p>
                </Show>
                <div class="library-toolbar">
                  <button type="submit">Save item</button>
                  <button
                    type="button"
                    onClick={() =>
                      navigate(() => {
                        setEditing(false);
                        setDirty(false);
                      })
                    }
                  >
                    Cancel editing
                  </button>
                </div>
              </fieldset>
            </form>
          </Show>
        </main>
      </div>
    </section>
  );
}
