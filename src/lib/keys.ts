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

// AltGraph is the right Alt key on layouts that have AltGr.
const MODIFIER_KEYS = new Set(["Shift", "Control", "Alt", "AltGraph", "Meta"]);

/**
 * Codes of the sign keys. Windows matches signs by key codes that differ
 * between layouts, so on Windows and Linux the recorder ignores them.
 */
const SIGN_CODES = new Set([
  "Comma",
  "Period",
  "Minus",
  "Equal",
  "Semicolon",
  "Quote",
  "Slash",
  "Backslash",
  "BracketLeft",
  "BracketRight",
  "Backquote",
  "IntlBackslash",
  "IntlRo",
  "IntlYen",
]);

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
  const key = mac ? keyByPosition(event.code) : keyByLayout(event);
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

/** macOS reads every key by its position. */
function keyByPosition(code: string): string {
  if (/^Key[A-Z]$/.test(code)) return code.slice(3);
  if (/^Digit\d$/.test(code)) return code.slice(5);
  return NAMED_KEYS[code] ?? code;
}

/**
 * Numpad keys that Windows and X11 register as numpad keys: global-hotkey
 * maps Numpad Enter and = to main keys on Windows and to nothing on X11.
 */
const NUMPAD_KEYS = /^Numpad(\d|Add|Subtract|Multiply|Divide|Decimal)$/;

/**
 * Windows and Linux read a letter by what the layout types (AZERTY's A key
 * is "KeyQ", its M key "Semicolon"), so take the typed Latin letter; a
 * letter of another script is read by its Latin key position. Digits and
 * the numpad keys in `NUMPAD_KEYS` are the same keys everywhere. Signs, on
 * whatever key, give `undefined`: each layout puts them elsewhere.
 */
function keyByLayout(event: KeyboardEvent): string | undefined {
  const code = event.code;
  if (/^Digit\d$/.test(code)) return code.slice(5);
  if (code.startsWith("Numpad")) return NUMPAD_KEYS.test(code) ? code : undefined;
  if (/^[a-z]$/i.test(event.key)) return event.key.toUpperCase();
  if (/^Key[A-Z]$/.test(code)) return /^\p{L}$/u.test(event.key) ? code.slice(3) : undefined;
  if (SIGN_CODES.has(code)) return undefined;
  return NAMED_KEYS[code] ?? code;
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
