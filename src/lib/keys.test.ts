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

  it("records letters as the OS will match them", () => {
    // AZERTY: the key labeled A is in the QWERTY Q position.
    const azertyA = press("KeyQ", "a", { ctrlKey: true, shiftKey: true });
    expect(acceleratorFromEvent(azertyA, false)).toBe("Control+Shift+A");
    expect(acceleratorFromEvent(azertyA, true)).toBe("Control+Shift+Q");
    // A layout that types another script falls back to the key position.
    expect(acceleratorFromEvent(press("KeyQ", "й", { ctrlKey: true }), false)).toBe("Control+Q");
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
