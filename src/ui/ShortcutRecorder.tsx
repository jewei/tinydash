import { createSignal, onCleanup, onMount, Show } from "solid-js";

import * as ipc from "../lib/ipc";
import { acceleratorFromEvent, displayKeys } from "../lib/keys";
import { Keys } from "./Keys";

/**
 * Click, then press a key combination. The global shortcuts are paused while
 * recording, so pressing a shortcut that is set records it instead of
 * acting. `label` names it for screen readers; `startNow` records at once.
 * An empty `value` shows `empty`.
 */
export function ShortcutRecorder(props: {
  label: string;
  value: string;
  empty?: string;
  startNow?: boolean;
  onChange: (value: string) => Promise<unknown>;
  onError: (message: string) => void;
}) {
  const [recording, setRecording] = createSignal(false);
  let button!: HTMLButtonElement;
  // Pause, save, and resume run one after another: the backend does not
  // keep them in order, and a resume that lands after a newer pause would
  // turn the shortcut on while recording.
  let queue = Promise.resolve();
  const then = (work: () => Promise<void>) => (queue = queue.then(work));

  // If the current shortcut cannot be paused, pressing it would open the
  // launcher instead of being recorded, so recording stops and says why.
  const start = () => {
    setRecording(true);
    // WebKit on macOS does not focus a clicked button, and the keys and
    // the blur that ends recording arrive only while it has focus.
    button.focus();
    void then(() =>
      ipc.pauseShortcut(true).catch((failure: unknown) => {
        setRecording(false);
        props.onError(ipc.message(failure));
      }),
    );
  };

  const stop = (accelerator?: string) => {
    if (!recording()) return;
    setRecording(false);
    const changed = accelerator !== props.value ? accelerator : undefined;
    void then(async () => {
      if (changed) await props.onChange(changed);
      // Registers the saved shortcut again: the new one, or the old one on failure.
      await ipc
        .pauseShortcut(false)
        .catch((failure: unknown) => props.onError(ipc.message(failure)));
    });
  };

  onMount(() => {
    if (props.startNow) start();
  });
  // Closed while recording, as a dialog can be: turn the shortcuts back on.
  onCleanup(() => stop());

  const onKeyDown = (event: KeyboardEvent) => {
    if (!recording()) return;
    event.preventDefault();
    event.stopPropagation();
    if (event.key === "Escape") return stop();
    const accelerator = acceleratorFromEvent(event);
    if (accelerator) stop(accelerator);
  };

  return (
    <button
      ref={button}
      type="button"
      class="button recorder"
      classList={{ recording: recording() }}
      aria-label={
        recording()
          ? `${props.label}: press the new keys, or Escape to cancel`
          : `${props.label}: ${props.value ? displayKeys(props.value).join(" ") : (props.empty ?? "")}`
      }
      // A mouse press must not blur the recorder first: WebKit on macOS would
      // stop recording on the blur, and the click would then start again.
      onMouseDown={(event) => event.preventDefault()}
      onClick={() => (recording() ? stop() : start())}
      onKeyDown={onKeyDown}
      onBlur={() => stop()}
    >
      <Show when={!recording()} fallback="Press keys…">
        <Show when={props.value} fallback={props.empty}>
          <Keys keys={displayKeys(props.value)} />
        </Show>
      </Show>
    </button>
  );
}
