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
  onChange: (value: string) => Promise<void>;
  onError: (message: string) => void;
}) {
  const [recording, setRecording] = createSignal(false);

  // If the current shortcut cannot be paused, pressing it would open the
  // launcher instead of being recorded, so recording stops and says why.
  const start = () => {
    setRecording(true);
    ipc.pauseShortcut(true).catch((failure: unknown) => {
      setRecording(false);
      props.onError(ipc.message(failure));
    });
  };

  const stop = async (accelerator?: string) => {
    if (!recording()) return;
    setRecording(false);
    if (accelerator && accelerator !== props.value) await props.onChange(accelerator);
    // Registers the saved shortcut again: the new one, or the old one on failure.
    await ipc.pauseShortcut(false).catch((failure: unknown) => props.onError(ipc.message(failure)));
  };

  const onKeyDown = (event: KeyboardEvent) => {
    if (!recording()) return;
    event.preventDefault();
    event.stopPropagation();
    if (event.key === "Escape") return void stop();
    const accelerator = acceleratorFromEvent(event);
    if (accelerator) void stop(accelerator);
  };

  return (
    <button
      type="button"
      class="button recorder"
      classList={{ recording: recording() }}
      aria-label={
        recording()
          ? "Launcher shortcut: press the new keys, or Escape to cancel"
          : `Launcher shortcut: ${displayKeys(props.value).join(" ")}`
      }
      onClick={() => (recording() ? void stop() : start())}
      onKeyDown={onKeyDown}
      onBlur={() => void stop()}
    >
      <Show when={!recording()} fallback="Press keys…">
        <Keys keys={displayKeys(props.value)} />
      </Show>
    </button>
  );
}
