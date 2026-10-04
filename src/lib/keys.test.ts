import { describe, expect, it } from "vite-plus/test";

import { acceleratorFromEvent, displayKeys } from "./keys";

const press = (code: string, key: string, options: KeyboardEventInit = {}) =>
  new KeyboardEvent("keydown", { code, key, ...options });

describe("acceleratorFromEvent", () => {
  it("builds Tauri accelerators", () => {
    expect(acceleratorFromEvent(press("Space", " ", { ctrlKey: true, shiftKey: true }))).toBe(
      "Control+Shift+Space",
    );
    expect(acceleratorFromEvent(press("KeyK", "k", { metaKey: true }))).toBe("Super+K");
    expect(acceleratorFromEvent(press("Digit1", "1", { altKey: true }))).toBe("Alt+1");
    expect(acceleratorFromEvent(press("ArrowUp", "ArrowUp", { ctrlKey: true }))).toBe("Control+Up");
  });

  it("allows function keys and Shift+Space alone", () => {
    expect(acceleratorFromEvent(press("F5", "F5"))).toBe("F5");
    expect(acceleratorFromEvent(press("Space", " ", { shiftKey: true }))).toBe("Shift+Space");
  });

  it("rejects plain typing and lone modifiers", () => {
    expect(acceleratorFromEvent(press("KeyA", "a"))).toBeNull();
    expect(acceleratorFromEvent(press("KeyA", "A", { shiftKey: true }))).toBeNull();
    expect(acceleratorFromEvent(press("ShiftLeft", "Shift", { shiftKey: true }))).toBeNull();
  });
});

describe("displayKeys", () => {
  it("uses symbols on macOS and names elsewhere", () => {
    expect(displayKeys("Control+Shift+Space", true)).toEqual(["⌃", "⇧", "Space"]);
    expect(displayKeys("CommandOrControl+K", false)).toEqual(["Ctrl", "K"]);
  });
});
