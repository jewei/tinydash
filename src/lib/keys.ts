// Keyboard helpers shared by both windows.

export const IS_MAC = /Mac/.test(navigator.userAgent);

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

const MODIFIER_KEYS = new Set(["Shift", "Control", "Alt", "Meta"]);

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
export function acceleratorFromEvent(event: KeyboardEvent, mac = IS_MAC): string | null {
  if (MODIFIER_KEYS.has(event.key)) return null;
  const code = event.code;
  // The OS reads a letter by key position on macOS, but by the letter the
  // layout types on Windows and Linux (AZERTY's A key is "KeyQ"), so record
  // what this OS will match. Digits are the number-row keys everywhere.
  const typed = /^[a-z]$/i.test(event.key) ? event.key.toUpperCase() : undefined;
  const key = /^Key[A-Z]$/.test(code)
    ? mac
      ? code.slice(3)
      : (typed ?? code.slice(3))
    : /^Digit\d$/.test(code)
      ? code.slice(5)
      : (NAMED_KEYS[code] ?? code);
  if (!key) return null;
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
  Super: "Win",
  Command: "Win",
  Cmd: "Win",
  CommandOrControl: "Ctrl",
  CmdOrCtrl: "Ctrl",
  Up: "↑",
  Down: "↓",
  Left: "←",
  Right: "→",
};

/** Keys of an accelerator for display, for example `["⌃", "⇧", "Space"]`. */
export function displayKeys(accelerator: string, mac = IS_MAC): string[] {
  const names = mac ? MAC_SYMBOLS : OTHER_NAMES;
  return accelerator
    .split("+")
    .filter(Boolean)
    .map((part) => names[part] ?? part);
}

/** The app-shortcut modifier as shown to the user. */
export const modKey = (mac = IS_MAC) => (mac ? "⌘" : "Ctrl");
