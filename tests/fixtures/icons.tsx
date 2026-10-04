import { render } from "@solidjs/web";
import { createSignal } from "solid-js";
import Icon, { type IconName } from "../../src/components/Icon";

render(() => {
  const [name, setName] = createSignal<IconName>("window");
  const [size, setSize] = createSignal(20);
  return (
    <div>
      <Icon name={name()} size={size()} />
      <button
        onClick={() => setName(name() === "window" ? "search" : "window")}
      >
        Change glyph
      </button>
      <button onClick={() => setSize(32)}>Change size</button>
    </div>
  );
}, document.getElementById("root")!);
