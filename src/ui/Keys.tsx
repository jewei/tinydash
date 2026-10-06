import { For } from "solid-js";

/** Key caps for a shortcut hint, for example ⌘ K. */
export function Keys(props: { keys: string[] }) {
  return (
    <span class="keys">
      <For each={props.keys}>{(key) => <kbd>{key}</kbd>}</For>
    </span>
  );
}
