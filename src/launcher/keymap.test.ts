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
    expect(commandFor(key("3", { ctrlKey: true }), false)).toEqual({ type: "runRow", index: 2 });
    expect(commandFor(key("Enter", { metaKey: true }), true)).toEqual({ type: "runSecondary" });
  });

  it("leaves typing alone", () => {
    expect(commandFor(key("k"))).toBeNull();
    expect(commandFor(key("3"))).toBeNull();
  });
});
