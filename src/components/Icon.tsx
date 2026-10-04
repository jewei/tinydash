import type { JSX } from "solid-js";

export type IconName =
  | "search"
  | "return"
  | "folder"
  | "file"
  | "refresh"
  | "quit"
  | "apps"
  | "emoji"
  | "window"
  | "close"
  | "calculator"
  | "clipboard"
  | "delete"
  | "system"
  | "lock"
  | "sleep"
  | "appearance"
  | "volume"
  | "logout"
  | "desktop"
  | "share"
  | "copy"
  | "clock"
  | "globe"
  | "link"
  | "pin"
  | "arrow"
  | "chevron";

const shapes: Record<IconName, () => JSX.Element> = {
  share: () => (
    <>
      <path d="M12 15V3m-4 4 4-4 4 4M7 10H4v11h16V10h-3" />
    </>
  ),
  emoji: () => (
    <>
      <circle cx="12" cy="12" r="9" />
      <path d="M8 14a4 4 0 0 0 8 0M8 9h.01M16 9h.01" />
    </>
  ),
  arrow: () => (
    <>
      <path d="M4 12h15m-5-5 5 5-5 5" />
    </>
  ),
  pin: () => (
    <>
      <path d="m16 3 5 5-3 1-4 4v4l-3-3-5 5m5-5-4-3h4l4-4 1-4Z" />
    </>
  ),
  chevron: () => (
    <>
      <path d="m8 10 4 4 4-4" />
    </>
  ),
  clock: () => (
    <>
      <circle cx="12" cy="12" r="9" />
      <path d="M12 6v6l4 2" />
    </>
  ),
  globe: () => (
    <>
      <circle cx="12" cy="12" r="9" />
      <path d="M3 12h18M12 3c5 5 5 13 0 18-5-5-5-13 0-18Z" />
    </>
  ),
  link: () => (
    <>
      <path d="m10 13 4-4m-5 6-2 2a4 4 0 0 1-6-6l4-4a4 4 0 0 1 6 0m2 2 2-2a4 4 0 0 1 6 6l-4 4a4 4 0 0 1-6 0" />
    </>
  ),
  window: () => (
    <>
      <rect x="3" y="3" width="18" height="18" rx="3" />
      <path d="M3 8h18M7 5.5h.01" />
    </>
  ),
  system: () => (
    <>
      <path d="M3 6h9m4 0h5M3 12h3m4 0h11M3 18h11m4 0h3" />
      <circle cx="14" cy="6" r="2" />
      <circle cx="8" cy="12" r="2" />
      <circle cx="16" cy="18" r="2" />
    </>
  ),
  lock: () => (
    <>
      <rect x="5" y="10" width="14" height="11" rx="2" />
      <path d="M8 10V7a4 4 0 0 1 8 0v3m-4 5v2" />
    </>
  ),
  sleep: () => (
    <>
      <path d="M20 15a9 9 0 0 1-11-11 9 9 0 1 0 11 11Z" />
    </>
  ),
  appearance: () => (
    <>
      <circle cx="12" cy="12" r="9" />
      <path d="M12 3v18a9 9 0 0 0 0-18Z" fill="currentColor" />
    </>
  ),
  volume: () => (
    <>
      <path d="M11 4 5 9H2v6h3l6 5V4Zm5 5 6 6m0-6-6 6" />
    </>
  ),
  logout: () => (
    <>
      <path d="M10 3H4v18h6m-2-9h14m-5-5 5 5-5 5" />
    </>
  ),
  desktop: () => (
    <>
      <rect x="2" y="3" width="20" height="14" rx="2" />
      <path d="M8 21h8m-4-4v4" />
    </>
  ),
  file: () => (
    <>
      <path d="M14 2H5v20h14V7l-5-5Zm0 0v5h5M8 12h8M8 16h6" />
    </>
  ),
  clipboard: () => (
    <>
      <rect x="7" y="2" width="10" height="5" rx="2" />
      <path d="M7 4H4v18h16V4h-3M8 12h8M8 16h6" />
    </>
  ),
  delete: () => (
    <>
      <path d="M3 6h18M9 6V3h6v3M5 6l1 15h12l1-15M10 10v7m4-7v7" />
    </>
  ),
  calculator: () => (
    <>
      <rect x="5" y="2" width="14" height="20" rx="2" />
      <path d="M8 6h8M8 11h1m3 0h1m3 0h.1M8 15h1m3 0h1m3 0h.1M8 19h1m3 0h1m3 0h.1" />
    </>
  ),
  copy: () => (
    <>
      <rect x="8" y="7" width="12" height="14" rx="2" />
      <path d="M16 7V3H4v14h4" />
    </>
  ),
  search: () => (
    <>
      <circle cx="10.5" cy="10.5" r="6.8" />
      <path d="m16 16 4.5 4.5" />
    </>
  ),
  return: () => (
    <>
      <path d="M20 5v8a3 3 0 0 1-3 3H4m4-4-4 4 4 4" />
    </>
  ),
  folder: () => (
    <>
      <path d="M3 7V5a1 1 0 0 1 1-1h5l3 3h8a1 1 0 0 1 1 1v11a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1V7Z" />
    </>
  ),
  refresh: () => (
    <>
      <path d="M20 10a8 8 0 1 0-1 7M20 4v6h-6" />
    </>
  ),
  quit: () => (
    <>
      <path d="M12 3v9m-5-7a8 8 0 1 0 10 0" />
    </>
  ),
  apps: () => (
    <>
      <rect x="3" y="3" width="7" height="7" rx="2" />
      <rect x="14" y="3" width="7" height="7" rx="2" />
      <rect x="3" y="14" width="7" height="7" rx="2" />
      <rect x="14" y="14" width="7" height="7" rx="2" />
    </>
  ),
  close: () => (
    <>
      <path d="m6 6 12 12M6 18 18 6" />
    </>
  ),
};

export default function Icon(props: { name: IconName; size?: number }) {
  return (
    <svg
      width={props.size ?? 18}
      height={props.size ?? 18}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      stroke-width="1.65"
      stroke-linecap="round"
      stroke-linejoin="round"
      aria-hidden="true"
    >
      {shapes[props.name]?.()}
    </svg>
  );
}
