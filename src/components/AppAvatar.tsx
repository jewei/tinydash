import { createEffect, createSignal, onCleanup, onMount, Show } from "solid-js";
import Icon from "./Icon";
import { loadAppIcon } from "../app-icons";

export default function AppAvatar(props: {
  icon: string | null;
  active?: boolean;
}) {
  let element!: HTMLSpanElement;
  const [nativeSource, setNativeSource] = createSignal<string>();
  const [inView, setInView] = createSignal(false);
  const [pixels, setPixels] = createSignal(72);
  const [failedSource, setFailedSource] = createSignal<string>();
  onMount(() => {
    let scale: MediaQueryList;
    const resize = () => {
      const required = Math.ceil(
        element.getBoundingClientRect().width * window.devicePixelRatio,
      );
      setPixels(
        [16, 24, 32, 36, 48, 64, 72, 96, 108, 128, 144, 192, 256].find(
          (size) => size >= required,
        ) ?? 256,
      );
      scale?.removeEventListener("change", resize);
      scale = matchMedia(`(resolution: ${window.devicePixelRatio}dppx)`);
      scale.addEventListener("change", resize);
    };
    const sizes = new ResizeObserver(resize);
    sizes.observe(element);
    const visible = new IntersectionObserver(([entry]) =>
      setInView(entry.isIntersecting),
    );
    visible.observe(element);
    resize();
    onCleanup(() => {
      sizes.disconnect();
      visible.disconnect();
      scale.removeEventListener("change", resize);
    });
  });
  createEffect(() => {
    const key = props.icon;
    setNativeSource(undefined);
    if (key?.startsWith("app-icon:") && props.active !== false && inView()) {
      onCleanup(loadAppIcon(key, pixels(), setNativeSource));
    }
  });
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
