import { createEffect, For, Show } from "solid-js";

import type { SearchResult } from "../generated/SearchResult";
import { modKey } from "../lib/keys";
import { Glyph, ResultIcon } from "../ui/Icon";
import { Keys } from "../ui/Keys";
import { FALLBACK_GLYPHS, KIND_LABELS } from "./describe";

export const optionId = (index: number) => `result-${index}`;

export function ResultList(props: {
  results: SearchResult[];
  selectedIndex: number;
  onSelect: (index: number) => void;
  onRun: (index: number) => void;
}) {
  let list!: HTMLUListElement;

  // Also on new results: they often keep the same selected index, while
  // the list may still be scrolled away from it.
  createEffect(() => {
    void props.results;
    const row = list.children[props.selectedIndex];
    row?.scrollIntoView({ block: "nearest" });
  });

  return (
    <ul ref={list} id="results" class="results" role="listbox" aria-label="Results">
      <For each={props.results}>
        {(result, index) => (
          <li
            id={optionId(index())}
            class="result"
            role="option"
            aria-selected={index() === props.selectedIndex}
            onMouseMove={() => index() !== props.selectedIndex && props.onSelect(index())}
            onClick={() => props.onRun(index())}
          >
            <ResultIcon icon={result.icon} size={32} fallback={FALLBACK_GLYPHS[result.kind]} />
            <span class="result-text">
              <span class="result-title">{result.title}</span>
              <span class="result-subtitle">{result.subtitle}</span>
            </span>
            <span class="result-meta">
              <Show when={result.pinned}>
                <span class="pinned" title="Pinned">
                  <Glyph name="pin" size={13} />
                </span>
              </Show>
              <span class="result-kind">{KIND_LABELS[result.kind]}</span>
              <Show when={index() < 9}>
                <Keys keys={[modKey(), String(index() + 1)]} />
              </Show>
            </span>
          </li>
        )}
      </For>
    </ul>
  );
}
