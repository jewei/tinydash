import {
  createEffect,
  createMemo,
  createSignal,
  For,
  Index,
  onCleanup,
  onMount,
  Show,
} from "solid-js";
import {
  backend,
  type SearchResult,
  type SettingsValues,
  type WebSearch,
} from "../bridge";

export function AppPreferences(props: {
  value: SettingsValues["appPreferences"];
  onChange: (value: SettingsValues["appPreferences"]) => void;
}) {
  const [apps, setApps] = createSignal<SearchResult[]>([]);
  const [filter, setFilter] = createSignal("");
  const [selected, setSelected] = createSignal("");
  const [error, setError] = createSignal("");
  let disposed = false;
  onCleanup(() => {
    disposed = true;
  });
  const visible = createMemo(() =>
    apps().filter((app) =>
      `${app.title} ${app.path}`
        .toLocaleLowerCase()
        .includes(filter().toLocaleLowerCase()),
    ),
  );
  const current = () =>
    props.value[selected()] ?? { aliases: [], hidden: false };
  function update(aliases: string[], hidden: boolean) {
    const next = { ...props.value };
    if (aliases.length || hidden) next[selected()] = { aliases, hidden };
    else delete next[selected()];
    props.onChange(next);
  }
  onMount(() => {
    void backend
      .appCatalog()
      .then((values) => {
        if (!disposed) setApps(values);
      })
      .catch((reason) => {
        if (!disposed) setError(String(reason));
      });
  });
  return (
    <>
      <p class="settings-hint">
        Give an app another search name, or hide it from results. Save changes
        to apply.
      </p>
      <Show when={error()}>
        <p role="alert">{error()}</p>
      </Show>
      <label class="settings-field">
        Find an app
        <input
          value={filter()}
          onInput={(event) => setFilter(event.currentTarget.value)}
        />
      </label>
      <label class="settings-field">
        Application
        <select
          size={6}
          value={selected()}
          onChange={(event) => setSelected(event.currentTarget.value)}
        >
          <For each={visible()}>
            {(app) => (
              <option value={app.id}>
                {app.title}
                {props.value[app.id]?.hidden ? " (hidden)" : ""}
              </option>
            )}
          </For>
        </select>
      </label>
      <Show when={selected()}>
        <label class="settings-field">
          Aliases, one per line
          <textarea
            rows={3}
            value={current().aliases.join("\n")}
            onChange={(event) =>
              update(
                event.currentTarget.value
                  .split(/\r?\n/)
                  .map((value) => value.trim())
                  .filter(Boolean),
                current().hidden,
              )
            }
          />
        </label>
        <label class="settings-row">
          <span>Hide this app from search</span>
          <input
            type="checkbox"
            checked={current().hidden}
            onChange={(event) =>
              update(current().aliases, event.currentTarget.checked)
            }
          />
        </label>
      </Show>
      <Show when={Object.keys(props.value).length}>
        <h3>Saved app preferences</h3>
        <For each={Object.keys(props.value)}>
          {(id) => (
            <div class="settings-row">
              <span>
                {apps().find((app) => app.id === id)?.title ?? id.slice(4)}
                <span class="settings-hint">
                  {props.value[id]?.hidden ? "Hidden. " : ""}
                  {props.value[id]?.aliases.join(", ")}
                </span>
              </span>
              <button
                type="button"
                onClick={() => {
                  const next = { ...props.value };
                  delete next[id];
                  props.onChange(next);
                }}
              >
                Remove preference
              </button>
            </div>
          )}
        </For>
      </Show>
    </>
  );
}

export function WebSearchPreferences(props: {
  value: WebSearch[];
  onChange: (value: WebSearch[]) => void;
}) {
  const [preview, setPreview] = createSignal("");
  const [error, setError] = createSignal("");
  let sequence = 0;
  createEffect(() => {
    props.value;
    sequence += 1;
    setPreview("");
    setError("");
  });
  onCleanup(() => {
    sequence += 1;
  });
  function update(
    index: number,
    field: keyof WebSearch,
    value: string | boolean,
  ) {
    props.onChange(
      props.value.map((search, position) =>
        position === index ? { ...search, [field]: value } : search,
      ),
    );
    setPreview("");
    setError("");
  }
  async function showPreview(search: WebSearch) {
    const request = ++sequence;
    try {
      const url = await backend.previewWebSearch(search, "coffee & 東京");
      if (request !== sequence) return;
      setPreview(url);
      setError("");
    } catch (reason) {
      if (request !== sequence) return;
      setPreview("");
      setError(String(reason));
    }
  }
  return (
    <>
      <p class="settings-hint">
        Add a website with one {"{query}"} placeholder. Type its keyword
        followed by search text. Searches open only when you choose a result.
      </p>
      <Index each={props.value}>
        {(search, index) => (
          <fieldset class="settings-editor">
            <legend>Web search {index + 1}</legend>
            <label class="settings-field">
              Search name
              <input
                maxlength={80}
                value={search().name}
                onInput={(event) =>
                  update(index, "name", event.currentTarget.value)
                }
              />
            </label>
            <label class="settings-field">
              Keyword
              <input
                maxlength={24}
                placeholder="docs"
                value={search().keyword}
                onInput={(event) =>
                  update(index, "keyword", event.currentTarget.value)
                }
              />
            </label>
            <label class="settings-field">
              Search URL
              <input
                maxlength={2048}
                placeholder="https://example.com/search?q={query}"
                value={search().template}
                onInput={(event) =>
                  update(index, "template", event.currentTarget.value)
                }
              />
            </label>
            <label class="settings-row">
              <span>Enable this search</span>
              <input
                type="checkbox"
                checked={search().enabled}
                onChange={(event) =>
                  update(index, "enabled", event.currentTarget.checked)
                }
              />
            </label>
            <div class="settings-inline-actions">
              <button type="button" onClick={() => void showPreview(search())}>
                Preview URL
              </button>
              <button
                type="button"
                onClick={() => {
                  props.onChange(
                    props.value.filter((_, position) => position !== index),
                  );
                  setPreview("");
                }}
              >
                Remove search
              </button>
            </div>
          </fieldset>
        )}
      </Index>
      <button
        type="button"
        disabled={props.value.length >= 24}
        onClick={() =>
          props.onChange([
            ...props.value,
            {
              name: "",
              keyword: "",
              template: "https://example.com/search?q={query}",
              enabled: true,
            },
          ])
        }
      >
        Add web search
      </button>
      <Show when={preview()}>
        <p class="settings-hint">Preview for coffee &amp; 東京</p>
        <pre class="settings-preview" aria-label="Web search URL preview">
          {preview()}
        </pre>
      </Show>
      <Show when={error()}>
        <p role="alert">{error()}</p>
      </Show>
    </>
  );
}
