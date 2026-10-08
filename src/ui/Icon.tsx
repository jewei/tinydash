import { createSignal, type JSX } from "solid-js";

import type { Icon } from "../generated/Icon";
import type { Symbol } from "../generated/Symbol";
import { iconUrl } from "../lib/ipc";

/** Glyphs drawn on a 24-unit grid with round 1.75-unit strokes. */
const GLYPHS: Record<
  | Symbol
  | "search"
  | "pin"
  | "warning"
  | "download"
  | "disk"
  | "clipboard"
  | "copy"
  | "check"
  | "close"
  | "back",
  () => JSX.Element
> = {
  app: () => (
    <>
      <rect x="4" y="4" width="7" height="7" rx="2" />
      <rect x="13" y="4" width="7" height="7" rx="2" />
      <rect x="4" y="13" width="7" height="7" rx="2" />
      <rect x="13" y="13" width="7" height="7" rx="2" />
    </>
  ),
  file: () => <path d="M7 3h7l5 5v13H7zM14 3v5h5" />,
  folder: () => (
    <path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z" />
  ),
  text: () => <path d="M5 6h14M5 10h14M5 14h10M5 18h7" />,
  image: () => (
    <>
      <rect x="3" y="5" width="18" height="14" rx="2" />
      <circle cx="9" cy="10" r="1.5" />
      <path d="M21 16l-5-5-8 8" />
    </>
  ),
  files: () => (
    <>
      <rect x="8" y="3" width="12" height="15" rx="2" />
      <path d="M5 7v12a2 2 0 0 0 2 2h9" />
    </>
  ),
  snippet: () => (
    <path d="M8 4C6 4 5 5 5 7v2c0 1.5-1 3-2 3 1 0 2 1.5 2 3v2c0 2 1 3 3 3M16 4c2 0 3 1 3 3v2c0 1.5 1 3 2 3-1 0-2 1.5-2 3v2c0 2-1 3-3 3" />
  ),
  link: () => (
    <path d="M10 14a4 4 0 0 0 5.7 0l3-3a4 4 0 0 0-5.7-5.7l-1 1M14 10a4 4 0 0 0-5.7 0l-3 3a4 4 0 0 0 5.7 5.7l1-1" />
  ),
  calculator: () => (
    <>
      <rect x="5" y="3" width="14" height="18" rx="2" />
      <path d="M8 7h8M8.5 12h.01M12 12h.01M15.5 12h.01M8.5 16h.01M12 16h.01M15.5 16h.01" />
    </>
  ),
  clock: () => (
    <>
      <circle cx="12" cy="12" r="9" />
      <path d="M12 7v5l3 2" />
    </>
  ),
  key: () => (
    <>
      <circle cx="8" cy="15" r="4" />
      <path d="M11 12l9-9M17 6l3 3M15 8l2 2" />
    </>
  ),
  globe: () => (
    <>
      <circle cx="12" cy="12" r="9" />
      <path d="M3 12h18M12 3c2.5 2.5 3.5 5.5 3.5 9s-1 6.5-3.5 9c-2.5-2.5-3.5-5.5-3.5-9s1-6.5 3.5-9z" />
    </>
  ),
  lock: () => (
    <>
      <rect x="5" y="11" width="14" height="10" rx="2" />
      <path d="M8 11V8a4 4 0 0 1 8 0v3" />
    </>
  ),
  moon: () => <path d="M20 14.5A8 8 0 1 1 9.5 4a6.5 6.5 0 0 0 10.5 10.5z" />,
  restart: () => <path d="M4 12a8 8 0 1 0 2.3-5.7L4 8.5M4 4v4.5h4.5" />,
  power: () => <path d="M12 3v9M6.3 6.3a8 8 0 1 0 11.4 0" />,
  logOut: () => <path d="M15 4h3a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2h-3M10 16l-4-4 4-4M6 12h10" />,
  trash: () => <path d="M4 7h16M9 7V4h6v3M6 7l1 13h10l1-13" />,
  settings: () => (
    <>
      <path d="M4 6h9M17 6h3M4 12h3M11 12h9M4 18h11M19 18h1" />
      <circle cx="15" cy="6" r="2" />
      <circle cx="9" cy="12" r="2" />
      <circle cx="17" cy="18" r="2" />
    </>
  ),
  quit: () => (
    <>
      <circle cx="12" cy="12" r="9" />
      <path d="M9 9l6 6M15 9l-6 6" />
    </>
  ),
  search: () => (
    <>
      <circle cx="11" cy="11" r="7" />
      <path d="M20 20l-4-4" />
    </>
  ),
  pin: () => <path d="M12 16v5M8 3h8l-1 6 3 3v2H6v-2l3-3z" />,
  warning: () => <path d="M12 4l9 16H3zM12 10v4M12 17h.01" />,
  download: () => <path d="M12 4v11M7 10l5 5 5-5M5 20h14" />,
  clipboard: () => (
    <>
      <rect x="7" y="3" width="10" height="4" rx="1" />
      <path d="M8 5H6a2 2 0 0 0-2 2v12a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7a2 2 0 0 0-2-2h-2" />
    </>
  ),
  copy: () => (
    <>
      <rect x="9" y="9" width="12" height="12" rx="2" />
      <path d="M5 15V5a2 2 0 0 1 2-2h10" />
    </>
  ),
  close: () => <path d="M6 6l12 12M18 6L6 18" />,
  back: () => <path d="M15 6l-6 6 6 6" />,
  check: () => <path d="M5 12l5 5L20 7" />,
  disk: () => (
    <>
      <rect x="3" y="12" width="18" height="8" rx="2" />
      <path d="M5 12l2.5-7h9L19 12M17 16h.01" />
    </>
  ),
};

export type GlyphName = keyof typeof GLYPHS;

export function Glyph(props: { name: GlyphName; size?: number }) {
  return (
    <svg
      class="glyph"
      width={props.size ?? 18}
      height={props.size ?? 18}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      stroke-width="1.75"
      stroke-linecap="round"
      stroke-linejoin="round"
      aria-hidden="true"
    >
      {GLYPHS[props.name]()}
    </svg>
  );
}

/** A result's icon: the system icon for a file, an emoji, a color swatch, or a glyph. */
export function ResultIcon(props: { icon: Icon; size: number; fallback: GlyphName }) {
  // Remember which path failed, so the next result's icon still loads when
  // this component is reused.
  const [brokenPath, setBrokenPath] = createSignal<string>();
  const content = () => {
    const icon = props.icon;
    if (icon.type === "file" && icon.path !== brokenPath()) {
      return (
        <img
          src={iconUrl(icon.path, props.size * window.devicePixelRatio)}
          alt=""
          decoding="async"
          onError={() => setBrokenPath(icon.path)}
        />
      );
    }
    if (icon.type === "emoji") {
      return (
        <span class="emoji" style={{ "font-size": `${Math.round(props.size * 0.8)}px` }}>
          {icon.glyph}
        </span>
      );
    }
    if (icon.type === "color") {
      return <span class="color-swatch" style={{ background: icon.hex }} />;
    }
    const name = icon.type === "symbol" ? icon.name : props.fallback;
    return <Glyph name={name} size={Math.round(props.size * 0.6)} />;
  };
  return (
    <span class="result-icon" style={{ width: `${props.size}px`, height: `${props.size}px` }}>
      {content()}
    </span>
  );
}
