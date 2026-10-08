import { createSignal, onCleanup, onMount } from "solid-js";

import * as ipc from "../lib/ipc";
import { Glyph } from "../ui/Icon";

/** The message window: one short line, such as "Copied". Rust shows and hides it. */
export function Hud() {
  const [message, setMessage] = createSignal("");
  onMount(() => {
    const stop = ipc.onHudMessage(setMessage);
    onCleanup(() => void stop.then((unlisten) => unlisten()));
  });
  return (
    <div class="hud" role="status">
      <Glyph name="check" size={16} />
      <span>{message()}</span>
    </div>
  );
}
