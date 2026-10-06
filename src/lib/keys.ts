// Keyboard helpers shared by both windows.

import type { Platform } from "../generated/Platform";

export const IS_MAC = /Mac/.test(navigator.userAgent);

const PLATFORM: Platform = IS_MAC
  ? "macos"
  : /Windows/.test(navigator.userAgent)
    ? "windows"
    : "linux";

/**
 * Command on macOS, Control elsewhere: the key that app shortcuts use. No
 * shortcut uses Alt, and on Windows AltGr arrives as Control+Alt, so a
 * press with Alt types a character instead.
 */
export const hasMod = (event: KeyboardEvent, mac = IS_MAC) =>
  !event.altKey && (mac ? event.metaKey && !event.ctrlKey : event.ctrlKey && !event.metaKey);

/**
 * The character a shortcut means: the typed one, or on a layout that types
 * another script, the letter on the physical key (Control+K gives "л" on a
 * Russian layout).
 */
export function shortcutKey(event: KeyboardEvent): string {
  const key = event.key.toLowerCase();
  if (key.length !== 1 || /^[\x20-\x7e]$/.test(key)) return key;
  const letter = /^Key([A-Z])$/.exec(event.code)?.[1];
  if (letter) return letter.toLowerCase();
  return event.code === "Comma" ? "," : key;
}

/** Keys pressed while an input method composes text belong to the IME. */
export const isComposing = (event: KeyboardEvent) => event.isComposing || event.keyCode === 229;

// AltGraph is the right Alt key on layouts that have AltGr.
const MODIFIER_KEYS = new Set(["Shift", "Control", "Alt", "AltGraph", "Meta"]);

const NAMED_KEYS: Record<string, string> = {
  ArrowUp: "Up",
  ArrowDown: "Down",
  ArrowLeft: "Left",
  ArrowRight: "Right",
};

/**
 * A global shortcut in Tauri accelerator syntax, such as `Control+Shift+Space`,
 * or `null` while only modifiers are held or the combination is not allowed.
 * A shortcut needs Control, Alt, or Command/Super, except function keys and
 * Shift+Space, which do not interfere with typing.
 */
export function acceleratorFromEvent(event: KeyboardEvent, platform = PLATFORM): string | null {
  if (MODIFIER_KEYS.has(event.key)) return null;
  const key = platform === "macos" ? keyByPosition(event.code) : keyByLayout(event);
  if (!key) return null;
  if (platform === "windows" && blocksAltGr(event, key)) return null;
  const functionKey = /^F\d{1,2}$/.test(key);
  const strongModifier = event.ctrlKey || event.altKey || event.metaKey;
  if (!strongModifier && !functionKey && !(event.shiftKey && key === "Space")) return null;
  const modifiers = [
    event.ctrlKey && "Control",
    event.altKey && "Alt",
    event.shiftKey && "Shift",
    event.metaKey && "Super",
  ].filter((modifier): modifier is string => Boolean(modifier));
  return [...modifiers, key].join("+");
}

/**
 * Windows treats Control+Alt as AltGr, so a shortcut on a key with an AltGr
 * character (German Control+Alt+7 types "{") would block typing it. Without
 * Shift, such a key types something other than its own letter or digit; a
 * letter of a script such as Cyrillic or Greek is the layout's plain letter,
 * not an AltGr one (but "µ", whose script is Common, is AltGr+M in German). With Shift, Windows has almost no AltGr characters to block.
 */
function blocksAltGr(event: KeyboardEvent, key: string): boolean {
  if (!event.ctrlKey || !event.altKey || event.shiftKey || key.length !== 1) return false;
  const typed = event.key;
  return (
    typed.toUpperCase() !== key && !/^(?!\p{Script=Latin}|\p{Script=Common})\p{L}$/u.test(typed)
  );
}

/** macOS reads every key by its position. */
function keyByPosition(code: string): string {
  if (/^Key[A-Z]$/.test(code)) return code.slice(3);
  if (/^Digit\d$/.test(code)) return code.slice(5);
  return NAMED_KEYS[code] ?? code;
}

/**
 * Keys that Windows and X11 register as themselves on every layout. Sign
 * keys are not here: Windows finds them by codes that differ between
 * layouts. Nor are numpad Enter and =, Num Lock, or the play and pause
 * keys, which global-hotkey registers as other keys (Num Lock as F1 on
 * X11, Media Pause as Pause on Windows) or not at all. Pause is not here
 * either: Windows turns Control+Pause into Break, so it would never fire.
 * Volume and track keys work but control playback, so they are left out.
 */
const FIXED_KEYS =
  /^(F([1-9]|1\d|2[0-4])|Space|Enter|Tab|Backspace|Delete|Insert|Home|End|PageUp|PageDown|Arrow(Up|Down|Left|Right)|PrintScreen|ScrollLock|Numpad(\d|Add|Subtract|Multiply|Divide|Decimal))$/;

/**
 * Windows and Linux read a letter by what the layout types (AZERTY's A key
 * is "KeyQ", its M key "Semicolon"), so take the typed Latin letter; a
 * letter of another script is read by its Latin key position. Digits and
 * `FIXED_KEYS` are read by code. Any other key, such as a sign on a letter
 * key, gives `undefined`.
 */
function keyByLayout(event: KeyboardEvent): string | undefined {
  const code = event.code;
  if (/^Digit\d$/.test(code)) return code.slice(5);
  if (/^[a-z]$/i.test(event.key)) return event.key.toUpperCase();
  if (/^Key[A-Z]$/.test(code)) return /^\p{L}$/u.test(event.key) ? code.slice(3) : undefined;
  // With Num Lock off, a numpad digit types End or Delete, and Windows
  // sends that key instead, so the shortcut would not fire.
  if (code.startsWith("Numpad") && event.key.length !== 1) return undefined;
  return FIXED_KEYS.test(code) ? (NAMED_KEYS[code] ?? code) : undefined;
}

const MAC_SYMBOLS: Record<string, string> = {
  Control: "⌃",
  Ctrl: "⌃",
  Alt: "⌥",
  Option: "⌥",
  Shift: "⇧",
  Super: "⌘",
  Command: "⌘",
  Cmd: "⌘",
  CommandOrControl: "⌘",
  CmdOrCtrl: "⌘",
  Up: "↑",
  Down: "↓",
  Left: "←",
  Right: "→",
  Enter: "↩",
  Backspace: "⌫",
};

const OTHER_NAMES: Record<string, string> = {
  Control: "Ctrl",
  CommandOrControl: "Ctrl",
  CmdOrCtrl: "Ctrl",
  Up: "↑",
  Down: "↓",
  Left: "←",
  Right: "→",
};

const SUPER_NAMES = new Set(["Super", "Command", "Cmd"]);

/** Keys of an accelerator for display, for example `["⌃", "⇧", "Space"]`. */
export function displayKeys(accelerator: string, platform = PLATFORM): string[] {
  const names = platform === "macos" ? MAC_SYMBOLS : OTHER_NAMES;
  // Windows calls the Super key Win; Linux desktops call it Super.
  const superName = platform === "windows" ? "Win" : "Super";
  return accelerator
    .split("+")
    .filter(Boolean)
    .map((part) =>
      platform !== "macos" && SUPER_NAMES.has(part) ? superName : (names[part] ?? part),
    );
}

/** The app-shortcut modifier as shown to the user. */
export const modKey = (mac = IS_MAC) => (mac ? "⌘" : "Ctrl");
