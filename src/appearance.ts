export const appearances = [
  { id: "light", label: "Light", description: "Cream and peach" },
  { id: "dark", label: "Dark", description: "Charcoal and olive" },
  { id: "sage", label: "Sage", description: "Soft green and forest" },
  { id: "rose", label: "Rose", description: "Pale pink and plum" },
  { id: "ink", label: "Ink", description: "Black and off-white" },
] as const;

export type Appearance = (typeof appearances)[number]["id"];

const storageKey = "tinydash.appearance";
const compactKey = "tinydash.compact";
const systemGlassKey = "tinydash.followSystemGlass";

export function isAppearance(value: unknown): value is Appearance {
  return appearances.some((appearance) => appearance.id === value);
}

export function readAppearance(): Appearance {
  try {
    const value = localStorage.getItem(storageKey);
    if (isAppearance(value)) return value;
    if (value === "mint") return "dark";
    if (value === "paper") return "light";
    if (value === "compact" || value === "graphite") {
      if (localStorage.getItem(compactKey) === null)
        localStorage.setItem(compactKey, "true");
      return "light";
    }
  } catch {
    // Appearance still works when the webview cannot access local storage.
  }
  return "light";
}

export function readCompact(): boolean {
  try {
    const value = localStorage.getItem(compactKey);
    if (value !== null) return value === "true";
    return ["compact", "graphite"].includes(
      localStorage.getItem(storageKey) ?? "",
    );
  } catch {
    return false;
  }
}

export function saveCompact(value: boolean) {
  try {
    localStorage.setItem(compactKey, String(value));
  } catch {
    // Keep the selection for this session if storage is unavailable.
  }
  if (isTauri()) void emit("compact-changed", value).catch(() => {});
}

export function readFollowSystemGlass(): boolean {
  try {
    return localStorage.getItem(systemGlassKey) !== "false";
  } catch {
    return true;
  }
}

export function saveFollowSystemGlass(value: boolean) {
  try {
    localStorage.setItem(systemGlassKey, String(value));
  } catch {
    // Keep the selection for this session if storage is unavailable.
  }
  if (isTauri()) void emit("system-glass-changed", value).catch(() => {});
}

export function saveAppearance(value: Appearance) {
  try {
    localStorage.setItem(storageKey, value);
  } catch {
    // Keep the selection for this session if storage is unavailable.
  }
  if (isTauri()) void emit("appearance-changed", value).catch(() => {});
}

export async function watchAppearance(
  update: (value: Appearance) => void,
  updateCompact: (value: boolean) => void,
  updateSystemGlass: (value: boolean) => void,
) {
  const onStorage = (event: StorageEvent) => {
    if (event.key === storageKey) update(readAppearance());
    if (event.key === compactKey) updateCompact(readCompact());
    if (event.key === systemGlassKey)
      updateSystemGlass(readFollowSystemGlass());
  };
  window.addEventListener("storage", onStorage);
  const stops: (() => void)[] = [];
  try {
    if (isTauri()) {
      stops.push(
        await listen("appearance-changed", (event) => {
          if (isAppearance(event.payload)) update(event.payload);
        }),
      );
      stops.push(
        await listen("compact-changed", (event) => {
          if (typeof event.payload === "boolean") updateCompact(event.payload);
        }),
      );
      stops.push(
        await listen("system-glass-changed", (event) => {
          if (typeof event.payload === "boolean")
            updateSystemGlass(event.payload);
        }),
      );
    }
  } catch {
    // Storage events still keep windows in sync if the event API is unavailable.
  }
  return () => {
    stops.forEach((stop) => stop());
    window.removeEventListener("storage", onStorage);
  };
}
import { isTauri } from "@tauri-apps/api/core";
import { emit, listen } from "@tauri-apps/api/event";
