import { createEffect, createSignal, Show } from "solid-js";
import Icon from "./Icon";
import { loadAppIcon } from "../app-icons";
import { observeIconDisplay } from "../icon-display";

export default function AppAvatar(props: {
  icon: string | null;
  active?: boolean;
}) {
  let element!: HTMLSpanElement;
  const [nativeSource, setNativeSource] = createSignal<string>();
  const [inView, setInView] = createSignal(false);
  const [pixels, setPixels] = createSignal(72);
  const [failedSource, setFailedSource] = createSignal<string>();
  createEffect(
    () => ({ icon: props.icon, active: props.active }),
    ({ icon, active }) => {
      setInView(false);
      if (!icon?.startsWith("app-icon:") || active === false) return;
      return observeIconDisplay(element, (visible, required) => {
        setPixels(required);
        setInView(visible);
      });
    },
  );
  createEffect(
    () => ({
      key: props.icon,
      active: props.active,
      visible: inView(),
      pixels: pixels(),
    }),
    ({ key, active, visible, pixels }) => {
      setNativeSource(undefined);
      if (key?.startsWith("app-icon:") && active !== false && visible) {
        return loadAppIcon(key, pixels, setNativeSource);
      }
    },
  );
  const source = () =>
    (nativeSource() ??
      (props.icon?.startsWith("data:image/png;base64,")
        ? props.icon
        : undefined)) !== failedSource()
      ? (nativeSource() ??
        (props.icon?.startsWith("data:image/png;base64,")
          ? props.icon
          : undefined))
      : undefined;
  return (
    <span
      ref={element}
      class={{ "app-avatar": true, "has-app-icon": !!source() }}
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
