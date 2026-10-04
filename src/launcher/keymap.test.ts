import { describe, expect, it } from "vite-plus/test";

import { commandFor } from "./keymap";

const key = (key: string, options: KeyboardEventInit = {}) =>
  new KeyboardEvent("keydown", { key, ...options });

describe("commandFor", () => {
  it("maps navigation keys", () => {
    expect(commandFor(key("ArrowDown"), 0)).toEqual({ type: "move", by: 1 });
    expect(commandFor(key("Tab", { shiftKey: true }), 0)).toEqual({ type: "category", by: -1 });
    expect(commandFor(key("Enter"), 4)).toEqual({ type: "run", index: 4 });
    expect(commandFor(key("Escape"), 0)).toEqual({ type: "hide" });
  });

  it("uses Command on macOS and Control elsewhere", () => {
    expect(commandFor(key("k", { metaKey: true }), 0, true)).toEqual({ type: "menu" });
    expect(commandFor(key("k", { ctrlKey: true }), 0, true)).toBeNull();
    expect(commandFor(key("k", { ctrlKey: true }), 0, false)).toEqual({ type: "menu" });
    expect(commandFor(key("3", { ctrlKey: true }), 0, false)).toEqual({ type: "run", index: 2 });
    expect(commandFor(key("Enter", { metaKey: true }), 0, true)).toEqual({ type: "runSecondary" });
  });

  it("leaves typing alone", () => {
    expect(commandFor(key("k"), 0)).toBeNull();
    expect(commandFor(key("3"), 0)).toBeNull();
  });
});
