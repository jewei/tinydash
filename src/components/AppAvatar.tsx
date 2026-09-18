import { createSignal, Show } from "solid-js";
import Icon from "./Icon";

export default function AppAvatar(props: { icon: string | null }) {
  const [failedSource, setFailedSource] = createSignal<string>();
  const source = () =>
    props.icon?.startsWith("data:image/png;base64,") &&
    props.icon !== failedSource()
      ? props.icon
      : undefined;
  return (
    <span
      class="app-avatar"
      classList={{ "has-app-icon": !!source() }}
      aria-hidden="true"
    >
      <Show when={source()} fallback={<Icon name="window" size={20} />}>
        {(url) => (
          <img
            src={url()}
            alt=""
            draggable={false}
            onError={(event) =>
              setFailedSource(
                event.currentTarget.getAttribute("src") ?? undefined,
              )
            }
          />
        )}
      </Show>
    </span>
  );
}
