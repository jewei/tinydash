import { hasMod, IS_MAC, shortcutKey } from "../lib/keys";

/** What a key press in the launcher means. The view decides what to do. */
type Command =
  | { type: "move"; by: 1 | -1 }
  | { type: "run" }
  | { type: "runRow"; index: number }
  | { type: "runSecondary" }
  | { type: "delete" }
  | { type: "menu" }
  | { type: "category"; by: 1 | -1 }
  | { type: "settings" }
  | { type: "hide" };

/**
 * Shortcuts, with Mod as Command on macOS and Control elsewhere:
 * ↑/↓ move, Enter runs, Mod+Enter runs the second action, Mod+K opens
 * actions, Mod+Backspace deletes, Mod+1–9 runs that row, Tab and Shift+Tab
 * change category, Mod+, opens Settings, and Escape hides.
 */
export function commandFor(event: KeyboardEvent, mac = IS_MAC): Command | null {
  const mod = hasMod(event, mac);
  switch (event.key) {
    case "ArrowDown":
      return { type: "move", by: 1 };
    case "ArrowUp":
      return { type: "move", by: -1 };
    case "Enter":
      return mod ? { type: "runSecondary" } : { type: "run" };
    case "Tab":
      return { type: "category", by: event.shiftKey ? -1 : 1 };
    case "Escape":
      return { type: "hide" };
  }
  if (!mod) return null;
  if (shortcutKey(event) === "k") return { type: "menu" };
  if (event.key === "Backspace") return { type: "delete" };
  if (shortcutKey(event) === ",") return { type: "settings" };
  // The physical digit key: on layouts such as AZERTY, `key` is "&" for 1.
  const digit = /^(?:Digit|Numpad)([1-9])$/.exec(event.code)?.[1];
  if (digit) return { type: "runRow", index: Number(digit) - 1 };
  return null;
}
