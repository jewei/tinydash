import { createResource, For, Match, Show, Switch } from "solid-js";

import type { Preview } from "../generated/Preview";
import type { ResultAction } from "../generated/ResultAction";
import type { ResultKind } from "../generated/ResultKind";
import type { SearchResult } from "../generated/SearchResult";
import { formatBytes } from "../lib/format";
import { clipboardImageUrl, preview as loadPreview } from "../lib/ipc";
import { ResultIcon } from "../ui/Icon";
import { Keys } from "../ui/Keys";
import { FALLBACK_GLYPHS, isAnswer, KIND_LABELS, shortcutFor } from "./describe";

/** Kinds whose details live in the backend and load on selection. */
const LOADED = new Set<ResultKind>(["app", "clipboard", "file", "folder", "snippet"]);

export function PreviewPane(props: {
  result: SearchResult | undefined;
  /** Changes whenever results refresh, so an edited item reloads. */
  revision: number;
  disabled: boolean;
  onRun: (action: ResultAction) => void;
}) {
  const [details] = createResource(
    () =>
      props.result && LOADED.has(props.result.kind)
        ? { id: props.result.id, revision: props.revision }
        : false,
    async ({ id }) => ({ id, preview: await loadPreview(id).catch(() => null) }),
  );
  // The resource keeps its last value; show it only for the result it belongs to.
  const loaded = () => details.latest?.id === props.result?.id;
  const current = () => (loaded() ? (details.latest?.preview ?? null) : null);
  // A copied text's title is its first line, and its preview shows all of it,
  // so the title would say the same thing twice. Until the preview loads, it
  // stays hidden too, so a long title never flashes and then disappears.
  const showsTitle = (result: SearchResult) =>
    result.kind !== "clipboard" || (loaded() && current()?.type !== "text");

  return (
    <Show when={props.result} fallback={<aside class="preview" />}>
      {(result) => (
        <aside class="preview" aria-label="Details">
          <div class="preview-header">
            <ResultIcon icon={result().icon} size={56} fallback={FALLBACK_GLYPHS[result().kind]} />
            <span class="preview-kind">{KIND_LABELS[result().kind]}</span>
            <Show when={showsTitle(result())}>
              <h2
                class="preview-title"
                classList={{ answer: isAnswer(result().kind), mono: result().kind === "password" }}
              >
                {result().title}
              </h2>
            </Show>
            <p class="preview-subtitle">{result().subtitle}</p>
          </div>
          <Show when={current()}>{(preview) => <Details preview={preview()} />}</Show>
          <ul class="preview-actions">
            <For each={result().actions}>
              {(action) => (
                <li>
                  <button
                    type="button"
                    class="preview-action"
                    disabled={props.disabled}
                    onClick={() => props.onRun(action)}
                  >
                    <span>{action.label}</span>
                    <Show when={shortcutFor(result(), action.action)}>
                      {(keys) => <Keys keys={keys()} />}
                    </Show>
                  </button>
                </li>
              )}
            </For>
          </ul>
        </aside>
      )}
    </Show>
  );
}

function Details(props: { preview: Preview }) {
  return (
    <div class="preview-details">
      <Switch>
        <Match when={props.preview.type === "text" && props.preview}>
          {(text) => <pre class="preview-text">{text().text}</pre>}
        </Match>
        <Match when={props.preview.type === "image" && props.preview}>
          {(image) => (
            <img
              class="preview-image"
              src={clipboardImageUrl(image().id)}
              width={image().width}
              height={image().height}
              alt=""
            />
          )}
        </Match>
        <Match when={props.preview.type === "files" && props.preview}>
          {(files) => (
            <ul class="preview-files">
              <For each={files().paths}>{(path) => <li>{path}</li>}</For>
            </ul>
          )}
        </Match>
        <Match when={props.preview.type === "file" && props.preview}>
          {(file) => (
            <dl class="preview-facts">
              <dt>Where</dt>
              <dd>{file().path}</dd>
              <Show when={!file().isDir}>
                <dt>Size</dt>
                <dd>{formatBytes(file().size)}</dd>
              </Show>
              <Show when={file().modified}>
                {(modified) => (
                  <>
                    <dt>Modified</dt>
                    <dd>{new Date(modified() * 1000).toLocaleString()}</dd>
                  </>
                )}
              </Show>
            </dl>
          )}
        </Match>
      </Switch>
    </div>
  );
}
