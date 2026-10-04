import { createEffect, createSignal, Show } from "solid-js";
import Icon from "./Icon";
import { loadAppIcon } from "../app-icons";
import { observeIconDisplay } from "../icon-display";

export default function AppAvatar(props: {
  icon: string | null;
  active?: boolean;
}) {
  let element!: HTMLSpanElement;
  const [nativeSource, setNativeSource] = createSignal<{
    key: string;
    url?: string;
  }>();
  const [failedSource, setFailedSource] = createSignal<string>();
  createEffect(
    () => ({ key: props.icon, active: props.active }),
    ({ key, active }) => {
      setNativeSource(undefined);
      if (!key?.startsWith("app-icon:") || active === false) return;
      let identity: string | undefined;
      let releaseImage: (() => void) | undefined;
      const stopDisplay = observeIconDisplay(element, (visible, pixels) => {
        const next = visible ? `${key}/${pixels}` : undefined;
        if (next === identity) return;
        releaseImage?.();
        releaseImage = undefined;
        identity = next;
        setNativeSource(undefined);
        if (visible) {
          releaseImage = loadAppIcon(key, pixels, (url) => {
            // An entry exists even while loading. A warm hit supplies its URL
            // in this callback, before a fallback shape needs to mount.
            setNativeSource({ key, url });
          });
        }
      });
      return () => {
        stopDisplay();
        releaseImage?.();
      };
    },
  );
  const source = () => {
    const icon = props.icon;
    const loaded = nativeSource();
    const url = icon?.startsWith("data:image/png;base64,")
      ? icon
      : props.active !== false && loaded?.key === icon
        ? loaded?.url
        : undefined;
    return url !== failedSource() ? url : undefined;
  };
  return (
    <span
      ref={element}
      class={{ "app-avatar": true, "has-app-icon": !!source() }}
      aria-hidden="true"
    >
      <Show
        when={source()}
        fallback={
          <Show
            when={
              !props.icon?.startsWith("app-icon:") ||
              nativeSource()?.key === props.icon
            }
          >
            <Icon name="window" size={20} />
          </Show>
        }
      >
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
