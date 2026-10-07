import { createResource, createSignal, For, onCleanup, onMount, Show } from "solid-js";

import type { CityClock } from "../generated/CityClock";
import type { Settings } from "../generated/Settings";
import * as ipc from "../lib/ipc";

/** Whether any widget is on, so the pane has something to show. */
export const hasWidgets = (settings: Settings) => settings.showClocks;

/** A signal of the current time that ticks every second while mounted. */
function createNow() {
  const [now, setNow] = createSignal(Date.now());
  const timer = setInterval(() => setNow(Date.now()), 1000);
  onCleanup(() => clearInterval(timer));
  return now;
}

const hourMinute = (timeZone?: string) =>
  new Intl.DateTimeFormat(undefined, {
    hour: "2-digit",
    minute: "2-digit",
    hourCycle: "h23",
    timeZone,
  });

/**
 * The cards to the right of an empty All search. They load when the pane
 * appears, each time the launcher opens, and when settings change.
 */
export function WidgetPane() {
  const [widgets, { refetch }] = createResource(ipc.widgets);
  const now = createNow();

  onMount(() => {
    const listeners = [
      ipc.onLauncherShown(() => void refetch()),
      ipc.onSettingsChanged(() => void refetch()),
    ];
    onCleanup(() => listeners.forEach((listener) => void listener.then((stop) => stop())));
  });

  return (
    <aside class="widgets" aria-label="Widgets">
      <Show when={widgets.error}>
        {(error) => (
          <p class="widgets-error" role="alert">
            Could not load the widgets: {ipc.message(error())}
          </p>
        )}
      </Show>
      <Show when={widgets.latest?.clocks}>
        {(cities) => <Clocks cities={cities()} now={now()} />}
      </Show>
    </aside>
  );
}

const localTime = hourMinute();
// A city's time is UTC shifted by its offset, so a fixed offset such as
// UTC+8 works too, which Intl cannot name.
const shiftedTime = hourMinute("UTC");
const shortDate = new Intl.DateTimeFormat(undefined, {
  weekday: "short",
  day: "numeric",
  month: "short",
});
const weekday = new Intl.DateTimeFormat(undefined, { weekday: "long" });
const longDate = new Intl.DateTimeFormat(undefined, {
  day: "numeric",
  month: "long",
  year: "numeric",
});

function Clocks(props: { cities: CityClock[]; now: number }) {
  return (
    <section class="widget wide clocks" aria-label="Clocks">
      <Show
        when={props.cities.length > 0}
        fallback={
          <div class="clock-alone">
            <span class="clock-time large">{localTime.format(props.now)}</span>
            <span class="clock-day">
              <strong>{weekday.format(props.now)}</strong>
              <span>{longDate.format(props.now)}</span>
            </span>
          </div>
        }
      >
        <div class="clock-local">
          <span class="clock-time">{localTime.format(props.now)}</span>
          <span class="widget-note">{shortDate.format(props.now)}</span>
        </div>
        <ul class="clock-cities">
          <For each={props.cities}>
            {(city) => (
              <li>
                <span class="widget-note clock-city">{city.name}</span>
                <span class="clock-city-time">
                  {shiftedTime.format(props.now + city.offsetSeconds * 1000)}
                </span>
                <span class="widget-note">{city.difference}</span>
              </li>
            )}
          </For>
        </ul>
      </Show>
    </section>
  );
}
