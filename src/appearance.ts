export const appearances = [
  { id: "light", label: "Light", description: "Warm and open" },
  { id: "dark", label: "Dark", description: "Soft and quiet" },
  { id: "compact", label: "Compact", description: "Small and focused" },
] as const;

export type Appearance = (typeof appearances)[number]["id"];

const storageKey = "tinydash.appearance";

export function isAppearance(value: unknown): value is Appearance {
  return appearances.some((appearance) => appearance.id === value);
}

export function readAppearance(): Appearance {
  try {
    const value = localStorage.getItem(storageKey);
    if (isAppearance(value)) return value;
    if (value === "mint") return "dark";
    if (value === "paper") return "light";
    if (value === "graphite") return "compact";
  } catch {
    // Appearance still works when the webview cannot access local storage.
  }
  return "light";
}

export function saveAppearance(value: Appearance) {
  try {
    localStorage.setItem(storageKey, value);
  } catch {
    // Keep the selection for this session if storage is unavailable.
  }
  if (isTauri()) void emit("appearance-changed", value).catch(() => {});
}

export async function watchAppearance(update: (value: Appearance) => void) {
  const onStorage = (event: StorageEvent) => {
    if (event.key === storageKey) update(readAppearance());
  };
  window.addEventListener("storage", onStorage);
  let stop: (() => void) | undefined;
  try {
    if (isTauri())
      stop = await listen("appearance-changed", (event) => {
        if (isAppearance(event.payload)) update(event.payload);
      });
  } catch {
    // Storage events still keep windows in sync if the event API is unavailable.
  }
  return () => {
    stop?.();
    window.removeEventListener("storage", onStorage);
  };
}
import { isTauri } from "@tauri-apps/api/core";
import { emit, listen } from "@tauri-apps/api/event";
