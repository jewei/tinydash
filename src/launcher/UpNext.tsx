import { For, Show } from "solid-js";

import type { ResultAction } from "../generated/ResultAction";
import type { UpNext } from "../generated/UpNext";

/** The rest of today's events and the reminders due today. */
export function UpNextCard(props: { upNext: UpNext; onRun: (action: ResultAction) => void }) {
  const nothing = () => props.upNext.items.length === 0 && props.upNext.problems.length === 0;
  return (
    <section class="widget wide up-next" aria-label="Up Next">
      <span class="widget-note widget-title">Up Next</span>
      <Show when={props.upNext.items.length > 0}>
        <ul class="up-next-list">
          <For each={props.upNext.items}>
            {(item) => (
              <li>
                <button
                  type="button"
                  class="up-next-item"
                  classList={{ reminder: item.kind === "reminder" }}
                  title={item.action.label}
                  onMouseDown={(event) => event.preventDefault()}
                  onClick={() => props.onRun(item.action)}
                >
                  {/* The spaces keep the words apart for screen readers;
                      the grid does not draw them. */}
                  <span class="up-next-time">{item.time}</span>{" "}
                  <span class="up-next-title">{item.title}</span>{" "}
                  <span class="widget-note up-next-source">{item.source}</span>
                </button>
              </li>
            )}
          </For>
        </ul>
      </Show>
      <For each={props.upNext.problems}>
        {(problem) => <p class="widget-note up-next-problem">{problem}</p>}
      </For>
      <Show when={props.upNext.ask}>
        {(ask) => (
          <button
            type="button"
            class="up-next-ask"
            onMouseDown={(event) => event.preventDefault()}
            onClick={() => props.onRun(ask())}
          >
            {ask().label}
          </button>
        )}
      </Show>
      <Show when={nothing()}>
        <p class="widget-note">Nothing else today</p>
      </Show>
    </section>
  );
}
