import {
  createResource,
  createSignal,
  For,
  Match,
  onCleanup,
  onMount,
  Show,
  Switch,
} from "solid-js";

import type { CityClock } from "../generated/CityClock";
import type { Disk } from "../generated/Disk";
import type { Settings } from "../generated/Settings";
import { formatBytes } from "../lib/format";
import * as ipc from "../lib/ipc";
import { isComposing, modKey } from "../lib/keys";
import { Glyph } from "../ui/Icon";
import { Keys } from "../ui/Keys";

/** Whether any widget is on, so the pane has something to show. */
export const hasWidgets = (settings: Settings) =>
  settings.showClocks || settings.showDiskSpace || settings.showNotepad;

/** What the launcher's shortcuts do in the pane. Each says whether it acted. */
export interface PaneControls {
  focusNote: () => boolean;
}

/** The note's field, so the launcher leaves its keys alone. */
export const inNote = (target: EventTarget | null) =>
  target instanceof Element && target.closest(".notepad") !== null;

/** 10,000 matches MAX_NOTE_CHARS in the Rust code. */
const MAX_NOTE_CHARS = 10_000;
/** Typing pauses this long before the note saves. */
const SAVE_DELAY_MS = 400;

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
export function WidgetPane(props: {
  controls: (controls: PaneControls) => void;
  /** Leave the note for the search field. */
  onLeave: () => void;
}) {
  const [widgets, { refetch }] = createResource(ipc.widgets);
  const now = createNow();
  let noteField: HTMLTextAreaElement | undefined;
  props.controls({
    focusNote: () => {
      noteField?.focus();
      return noteField !== undefined;
    },
  });

  onMount(() => {
    const listeners = [
      ipc.onLauncherShown(() => void refetch()),
      ipc.onSettingsChanged(() => void refetch()),
    ];
    onCleanup(() => listeners.forEach((listener) => void listener.then((stop) => stop())));
  });

  return (
    <aside class="widgets" aria-label="Widgets">
      <div class="widget-grid">
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
        <Show when={widgets.latest?.disk}>{(disk) => <DiskSpace disk={disk()} />}</Show>
      </div>
      {/* An empty note is still a note, so test for null. */}
      <Show when={widgets.latest && widgets.latest.note !== null}>
        <Notepad
          initial={widgets.latest?.note ?? ""}
          field={(field) => (noteField = field)}
          onLeave={props.onLeave}
        />
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

function DiskSpace(props: { disk: Disk }) {
  return (
    <section class="widget wide disk" aria-label="Disk space">
      <Switch>
        <Match when={props.disk.type === "ready" && props.disk}>
          {(disk) => {
            const used = () =>
              disk().totalBytes > 0
                ? Math.round((1 - disk().freeBytes / disk().totalBytes) * 100)
                : 0;
            return (
              <>
                <div class="disk-line">
                  <Glyph name="disk" size={14} />
                  <span class="widget-title">{disk().name}</span>
                  <Show when={disk().low}>
                    <span class="disk-low">
                      <Glyph name="warning" size={12} /> Low space
                    </span>
                  </Show>
                  <span class="disk-free">
                    <strong>{formatBytes(disk().freeBytes)}</strong> free of{" "}
                    {formatBytes(disk().totalBytes)}
                  </span>
                </div>
                <div
                  class="meter"
                  classList={{ low: disk().low }}
                  role="meter"
                  aria-label="Disk used"
                  aria-valuemin={0}
                  aria-valuemax={100}
                  aria-valuenow={used()}
                >
                  <span style={{ width: `${used()}%` }} />
                </div>
              </>
            );
          }}
        </Match>
        <Match when={props.disk.type === "unavailable" && props.disk}>
          {(disk) => <p class="widget-note">Could not read the disk. {disk().message}</p>}
        </Match>
      </Switch>
    </section>
  );
}

/**
 * One scratch note. It starts from the saved text and then keeps its own,
 * since only this field edits it; a pause in typing, leaving the field,
 * and closing the card save it.
 */
function Notepad(props: {
  initial: string;
  field: (field: HTMLTextAreaElement | undefined) => void;
  onLeave: () => void;
}) {
  const [text, setText] = createSignal(props.initial);
  // Typed text that is not saved yet, and why the last save failed.
  const [pending, setPending] = createSignal(false);
  const [failure, setFailure] = createSignal<string>();
  let timer: ReturnType<typeof setTimeout> | undefined;
  let unsaved = false;
  // Saves run one at a time, in order, so an older text never lands last.
  let saving = Promise.resolve();

  const save = () => {
    clearTimeout(timer);
    if (!unsaved) return;
    unsaved = false;
    const value = text();
    saving = saving.then(() =>
      ipc.saveNote(value).then(
        () => {
          setPending(unsaved);
        },
        (error) => {
          setFailure(ipc.message(error));
          setPending(unsaved);
        },
      ),
    );
  };
  onCleanup(() => {
    save();
    props.field(undefined);
  });

  const words = () => text().split(/\s+/).filter(Boolean).length;
  const status = () => {
    const reason = failure();
    if (reason) return `Not saved. ${reason}`;
    return pending() ? "Saving…" : "Saved";
  };

  return (
    <section class="widget notepad" aria-label="Notepad">
      <div class="notepad-head">
        <label for="tinydash-note" class="widget-note widget-title">
          Notepad
        </label>
        <Keys keys={[modKey(), "J"]} />
        <span
          class="widget-note notepad-status"
          classList={{ failed: failure() !== undefined }}
          role="status"
        >
          {words() === 1 ? "1 word" : `${words()} words`} · {status()}
        </span>
      </div>
      <textarea
        id="tinydash-note"
        ref={props.field}
        class="notepad-text"
        placeholder="Jot something down…"
        maxLength={MAX_NOTE_CHARS}
        spellcheck={false}
        value={text()}
        onInput={(event) => {
          setText(event.currentTarget.value);
          setPending(true);
          setFailure(undefined);
          unsaved = true;
          clearTimeout(timer);
          timer = setTimeout(save, SAVE_DELAY_MS);
        }}
        onBlur={save}
        onKeyDown={(event) => {
          if (event.key !== "Escape" || isComposing(event)) return;
          event.preventDefault();
          props.onLeave();
        }}
      />
    </section>
  );
}
