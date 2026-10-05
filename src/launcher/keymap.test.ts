import { describe, expect, it } from "vite-plus/test";

import { commandFor } from "./keymap";

const key = (key: string, options: KeyboardEventInit = {}) =>
  new KeyboardEvent("keydown", { key, ...options });

describe("commandFor", () => {
  it("maps navigation keys", () => {
    expect(commandFor(key("ArrowDown"))).toEqual({ type: "move", by: 1 });
    expect(commandFor(key("Tab", { shiftKey: true }))).toEqual({ type: "category", by: -1 });
    expect(commandFor(key("Enter"))).toEqual({ type: "run" });
    expect(commandFor(key("Escape"))).toEqual({ type: "hide" });
  });

  it("uses Command on macOS and Control elsewhere", () => {
    expect(commandFor(key("k", { metaKey: true }), true)).toEqual({ type: "menu" });
    expect(commandFor(key("k", { ctrlKey: true }), true)).toBeNull();
    expect(commandFor(key("k", { ctrlKey: true }), false)).toEqual({ type: "menu" });
    expect(commandFor(key("3", { ctrlKey: true, code: "Digit3" }), false)).toEqual({
      type: "runRow",
      index: 2,
    });
    // Russian: Control+K gives "л"; the physical key still means K.
    expect(commandFor(key("л", { ctrlKey: true, code: "KeyK" }), false)).toEqual({
      type: "menu",
    });
    // Windows AltGr is Control+Alt: AltGr+7 types "{" on a German layout.
    expect(commandFor(key("{", { ctrlKey: true, altKey: true, code: "Digit7" }), false)).toBe(null);
    // AZERTY: the 1 key gives "&" without Shift.
    expect(commandFor(key("&", { metaKey: true, code: "Digit1" }), true)).toEqual({
      type: "runRow",
      index: 0,
    });
    expect(commandFor(key("Enter", { metaKey: true }), true)).toEqual({ type: "runSecondary" });
  });

  it("leaves typing alone", () => {
    expect(commandFor(key("k"))).toBeNull();
    expect(commandFor(key("3"))).toBeNull();
  });
});
