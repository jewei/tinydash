import { For, Match, Show, Switch } from "solid-js";

import type { CardContent } from "../generated/CardContent";
import type { ClipCard as Card } from "../generated/ClipCard";
import type { CardRow } from "../generated/CardRow";
import type { Contrast } from "../generated/Contrast";
import type { JsonLine } from "../generated/JsonLine";
import type { ResultAction } from "../generated/ResultAction";
import { formatBytes } from "../lib/format";
import { Glyph } from "../ui/Icon";
import { Keys } from "../ui/Keys";

type Json = Extract<CardContent, { type: "json" }>;

/** "Object · 8 keys · depth 3 · 312 bytes" */
export const jsonSummary = (json: Json) => {
  const noun = json.array
    ? json.count === 1
      ? "item"
      : "items"
    : json.count === 1
      ? "key"
      : "keys";
  return `${json.array ? "Array" : "Object"} · ${json.count} ${noun} · depth ${json.depth} · ${formatBytes(json.bytes)}`;
};

const KIND: Record<CardContent["type"], string> = {
  color: "Color",
  unixTime: "Unix time",
  json: "JSON",
};

/** The label of the card's main action, which Mod+Shift+Enter runs. */
export const mainLabel = (card: Card) =>
  card.content.type === "json" ? "View Full JSON" : (card.actions[0]?.label ?? "");

/**
 * A card for what the clipboard holds. Copies run the card's actions;
 * the JSON view opens over the pane.
 */
export function ClipCard(props: {
  card: Card;
  mainKeys: string[];
  onRun: (action: ResultAction) => void;
  onOpenJson: () => void;
  onDismiss: () => void;
}) {
  const action = (index: number) => props.card.actions[index];
  const run = (index: number) => {
    const chosen = action(index);
    if (chosen) props.onRun(chosen);
  };
  return (
    <section
      class="widget wide clip-card"
      aria-label={`${KIND[props.card.content.type]} on the clipboard`}
    >
      <div class="clip-head">
        <Glyph name="clipboard" size={13} />
        <span class="clip-kind">{KIND[props.card.content.type]}</span>
        <Show when={props.card.content.type === "color" && props.card.content.copiedAs}>
          {(form) => <span class="badge">from {form()}</span>}
        </Show>
        <span class="clip-main">
          <Keys keys={props.mainKeys} />
          <span class="clip-main-label">{mainLabel(props.card)}</span>
        </span>
        <button
          type="button"
          class="icon-button"
          aria-label="Dismiss"
          onMouseDown={(event) => event.preventDefault()}
          onClick={() => props.onDismiss()}
        >
          <Glyph name="close" size={12} />
        </button>
      </div>
      <Switch>
        <Match when={props.card.content.type === "color" && props.card.content}>
          {(color) => (
            <>
              <div class="clip-color">
                <span class="swatch" style={{ background: color().hex }} />
                <Rows rows={color().rows} onCopy={run} />
              </div>
              <div class="contrast">
                <ContrastBox on="white" hex={color().hex} contrast={color().onWhite} />
                <ContrastBox on="black" hex={color().hex} contrast={color().onBlack} />
              </div>
            </>
          )}
        </Match>
        <Match when={props.card.content.type === "unixTime" && props.card.content}>
          {(time) => (
            <>
              <div class="clip-time">
                <span class="mono">{time().raw}</span>
                <span class="badge">{time().milliseconds ? "milliseconds" : "seconds"}</span>
                <span class="clip-relative">{time().relative}</span>
              </div>
              <Rows rows={time().rows} onCopy={run} />
            </>
          )}
        </Match>
        <Match when={props.card.content.type === "json" && props.card.content}>
          {(json) => (
            <>
              <span class="widget-note">{jsonSummary(json())}</span>
              <Show when={json().keys.length > 0}>
                <ul class="json-keys">
                  <For each={json().keys}>
                    {(key) => (
                      <li class="mono">
                        {key.name}
                        <span class="json-hint">{key.hint}</span>
                      </li>
                    )}
                  </For>
                </ul>
              </Show>
              <div class="json-minified mono">{json().minified}</div>
              <div class="clip-buttons">
                <button
                  type="button"
                  class="clip-button main"
                  onMouseDown={(event) => event.preventDefault()}
                  onClick={() => props.onOpenJson()}
                >
                  View Full
                </button>
                <For each={props.card.actions}>
                  {(copy) => (
                    <button
                      type="button"
                      class="clip-button"
                      onMouseDown={(event) => event.preventDefault()}
                      onClick={() => props.onRun(copy)}
                    >
                      {copy.label.replace(" JSON", "")}
                    </button>
                  )}
                </For>
              </div>
            </>
          )}
        </Match>
      </Switch>
    </section>
  );
}

/** Labeled values, each with a copy button for the action of its index. */
function Rows(props: { rows: CardRow[]; onCopy: (index: number) => void }) {
  return (
    <dl class="clip-rows">
      <For each={props.rows}>
        {(row, index) => (
          <div class="clip-row">
            <dt>{row.label}</dt>
            <dd class="mono">{row.value}</dd>
            <button
              type="button"
              class="icon-button bordered"
              aria-label={`Copy ${row.label}`}
              onMouseDown={(event) => event.preventDefault()}
              onClick={() => props.onCopy(index())}
            >
              <Glyph name="copy" size={12} />
            </button>
          </div>
        )}
      </For>
    </dl>
  );
}

function ContrastBox(props: { on: "white" | "black"; hex: string; contrast: Contrast }) {
  return (
    <div class={`contrast-box ${props.on}`} aria-label={`Contrast on ${props.on}`}>
      <span class="contrast-sample" style={{ color: props.hex }}>
        Aa
      </span>
      <span>
        {props.contrast.ratio} on {props.on}
      </span>
      <strong>{props.contrast.grade}</strong>
    </div>
  );
}

/** The whole JSON over the pane, with line numbers. Escape closes it. */
export function JsonView(props: {
  json: Json;
  copyPretty: ResultAction | undefined;
  onRun: (action: ResultAction) => void;
  onClose: () => void;
}) {
  return (
    <div class="json-view" role="dialog" aria-label="Clipboard JSON">
      <div class="json-view-head">
        <button
          type="button"
          class="icon-button bordered"
          aria-label="Back to widgets"
          onMouseDown={(event) => event.preventDefault()}
          onClick={() => props.onClose()}
        >
          <Glyph name="back" size={14} />
        </button>
        <span class="json-view-title">
          <strong>Clipboard JSON</strong>
          <span class="widget-note">{jsonSummary(props.json)}</span>
        </span>
        <Show when={props.copyPretty}>
          {(copy) => (
            <button
              type="button"
              class="clip-button"
              onMouseDown={(event) => event.preventDefault()}
              onClick={() => props.onRun(copy())}
            >
              Copy Pretty
            </button>
          )}
        </Show>
        <Keys keys={["esc"]} />
      </div>
      <ol class="json-lines mono">
        <For each={props.json.lines}>{(line) => <Line line={line} />}</For>
        <Show when={props.json.moreLines > 0}>
          <li class="json-more">
            {props.json.moreLines.toLocaleString()} more lines. Copy Pretty has all of them.
          </li>
        </Show>
      </ol>
    </div>
  );
}

function Line(props: { line: JsonLine }) {
  return (
    <li>
      <span class="json-code" style={{ "padding-left": `${props.line.indent * 2}ch` }}>
        <For each={props.line.tokens}>
          {(token) => <span class={`json-${token.kind}`}>{token.text}</span>}
        </For>
      </span>
    </li>
  );
}
