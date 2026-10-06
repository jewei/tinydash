import { createSignal, Show } from "solid-js";

import * as ipc from "../lib/ipc";
import { acceleratorFromEvent, displayKeys } from "../lib/keys";
import { Keys } from "../ui/Keys";

/**
 * Click, then press a key combination. The global shortcut is paused while
 * recording, so pressing the current shortcut records it instead of
 * opening the launcher.
 */
export function ShortcutRecorder(props: {
  value: string;
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
          ? "Launcher shortcut: press the new keys, or Escape to cancel"
          : `Launcher shortcut: ${displayKeys(props.value).join(" ")}`
      }
      // A mouse press must not blur the recorder first: WebKit on macOS would
      // stop recording on the blur, and the click would then start again.
      onMouseDown={(event) => event.preventDefault()}
      onClick={() => (recording() ? stop() : start())}
      onKeyDown={onKeyDown}
      onBlur={() => stop()}
    >
      <Show when={!recording()} fallback="Press keys…">
        <Keys keys={displayKeys(props.value)} />
      </Show>
    </button>
  );
}
