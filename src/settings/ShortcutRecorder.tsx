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
}) {
  const [recording, setRecording] = createSignal(false);

  const start = () => {
    setRecording(true);
    void ipc.pauseShortcut(true);
  };

  const stop = async (accelerator?: string) => {
    if (!recording()) return;
    setRecording(false);
    if (accelerator && accelerator !== props.value) await props.onChange(accelerator);
    // Registers the saved shortcut again: the new one, or the old one on failure.
    await ipc.pauseShortcut(false).catch(() => {});
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
      aria-label="Launcher shortcut"
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
