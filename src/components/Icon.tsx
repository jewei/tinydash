import { Switch, Match } from "solid-js";

type IconName =
  | "search"
  | "return"
  | "folder"
  | "refresh"
  | "quit"
  | "apps"
  | "close"
  | "calculator"
  | "copy";

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
      <Switch>
        <Match when={props.name === "calculator"}>
          <rect x="5" y="2" width="14" height="20" rx="2" />
          <path d="M8 6h8M8 11h1m3 0h1m3 0h.1M8 15h1m3 0h1m3 0h.1M8 19h1m3 0h1m3 0h.1" />
        </Match>
        <Match when={props.name === "copy"}>
          <rect x="8" y="7" width="12" height="14" rx="2" />
          <path d="M16 7V3H4v14h4" />
        </Match>
        <Match when={props.name === "search"}>
          <circle cx="10.5" cy="10.5" r="6.8" />
          <path d="m16 16 4.5 4.5" />
        </Match>
        <Match when={props.name === "return"}>
          <path d="M20 5v8a3 3 0 0 1-3 3H4m4-4-4 4 4 4" />
        </Match>
        <Match when={props.name === "folder"}>
          <path d="M3 7V5a1 1 0 0 1 1-1h5l3 3h8a1 1 0 0 1 1 1v11a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1V7Z" />
        </Match>
        <Match when={props.name === "refresh"}>
          <path d="M20 10a8 8 0 1 0-1 7M20 4v6h-6" />
        </Match>
        <Match when={props.name === "quit"}>
          <path d="M12 3v9m-5-7a8 8 0 1 0 10 0" />
        </Match>
        <Match when={props.name === "apps"}>
          <rect x="3" y="3" width="7" height="7" rx="2" />
          <rect x="14" y="3" width="7" height="7" rx="2" />
          <rect x="3" y="14" width="7" height="7" rx="2" />
          <rect x="14" y="14" width="7" height="7" rx="2" />
        </Match>
        <Match when={props.name === "close"}>
          <path d="m6 6 12 12M6 18 18 6" />
        </Match>
      </Switch>
    </svg>
  );
}
