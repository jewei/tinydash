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
    expect(acceleratorFromEvent(azertyA, "windows")).toBe("Control+Shift+A");
    expect(acceleratorFromEvent(azertyA, "macos")).toBe("Control+Shift+Q");
    // AZERTY: M is in the QWERTY ";" position, and "," in the M position.
    const azertyM = press("Semicolon", "m", { ctrlKey: true, shiftKey: true });
    expect(acceleratorFromEvent(azertyM, "windows")).toBe("Control+Shift+M");
    expect(acceleratorFromEvent(azertyM, "macos")).toBe("Control+Shift+Semicolon");
    // Signs, even on letter keys (AZERTY ",", Dvorak "'"), are not recorded there.
    expect(acceleratorFromEvent(press("KeyM", ",", { ctrlKey: true }), "windows")).toBeNull();
    expect(acceleratorFromEvent(press("KeyQ", "'", { ctrlKey: true }), "windows")).toBeNull();
    // Russian: "." is on the "Slash" key.
    expect(acceleratorFromEvent(press("Slash", ".", { ctrlKey: true }), "windows")).toBeNull();
    // German: Shift+, types ";", which Windows would find on another key.
    const germanShiftComma = press("Comma", ";", { ctrlKey: true, shiftKey: true });
    expect(acceleratorFromEvent(germanShiftComma, "windows")).toBeNull();
    expect(acceleratorFromEvent(germanShiftComma, "macos")).toBe("Control+Shift+Comma");
    // Signs whose Windows key differs by layout are not recorded there.
    expect(acceleratorFromEvent(press("Semicolon", ";", { ctrlKey: true }), "windows")).toBeNull();
    // Numpad keys stay numpad keys.
    const numpadMinus = press("NumpadSubtract", "-", { ctrlKey: true });
    expect(acceleratorFromEvent(numpadMinus, "windows")).toBe("Control+NumpadSubtract");
    // Windows would register Numpad Enter as the main Enter key.
    const numpadEnter = press("NumpadEnter", "Enter", { ctrlKey: true });
    expect(acceleratorFromEvent(numpadEnter, "windows")).toBeNull();
    expect(acceleratorFromEvent(press("Home", "Home", { ctrlKey: true }), "windows")).toBe(
      "Control+Home",
    );
    // X11 would register Num Lock as F1, and Windows Media Pause as Pause.
    expect(
      acceleratorFromEvent(press("NumLock", "NumLock", { ctrlKey: true }), "windows"),
    ).toBeNull();
    const mediaPause = press("MediaPause", "MediaPause", { ctrlKey: true });
    expect(acceleratorFromEvent(mediaPause, "windows")).toBeNull();
    // Windows types AltGr characters with Control+Alt.
    const germanBrace = press("Digit7", "{", { ctrlKey: true, altKey: true });
    expect(acceleratorFromEvent(germanBrace, "windows")).toBeNull();
    expect(
      acceleratorFromEvent(press("KeyS", "ś", { ctrlKey: true, altKey: true }), "windows"),
    ).toBeNull();
    const plain = press("KeyK", "k", { ctrlKey: true, altKey: true });
    expect(acceleratorFromEvent(plain, "windows")).toBe("Control+Alt+K");
    // No AltGr to block: Shift held, a Russian letter, or Linux.
    const shifted = press("Digit1", "!", { ctrlKey: true, altKey: true, shiftKey: true });
    expect(acceleratorFromEvent(shifted, "windows")).toBe("Control+Alt+Shift+1");
    const russian = press("KeyK", "л", { ctrlKey: true, altKey: true });
    expect(acceleratorFromEvent(russian, "windows")).toBe("Control+Alt+K");
    expect(acceleratorFromEvent(germanBrace, "linux")).toBe("Control+Alt+7");
    // With Num Lock off, numpad 1 types End.
    const numpadEnd = press("Numpad1", "End", { ctrlKey: true });
    expect(acceleratorFromEvent(numpadEnd, "windows")).toBeNull();
    // Windows turns Control+Pause into Break.
    expect(acceleratorFromEvent(press("Pause", "Pause", { ctrlKey: true }), "windows")).toBeNull();
    expect(acceleratorFromEvent(press("F24", "F24"), "windows")).toBe("F24");
    // A layout that types another script falls back to the key position.
    expect(acceleratorFromEvent(press("KeyQ", "й", { ctrlKey: true }), "windows")).toBe(
      "Control+Q",
    );
  });

  it("allows function keys and Shift+Space alone", () => {
    expect(acceleratorFromEvent(press("F5", "F5"))).toBe("F5");
    expect(acceleratorFromEvent(press("Space", " ", { shiftKey: true }))).toBe("Shift+Space");
  });

  it("rejects plain typing and lone modifiers", () => {
    expect(acceleratorFromEvent(press("KeyA", "a"))).toBeNull();
    expect(acceleratorFromEvent(press("KeyA", "A", { shiftKey: true }))).toBeNull();
    expect(acceleratorFromEvent(press("ShiftLeft", "Shift", { shiftKey: true }))).toBeNull();
    // AltGr held first: Windows reports Control and Alt with the AltGraph key.
    const altGr = press("AltRight", "AltGraph", { ctrlKey: true, altKey: true });
    expect(acceleratorFromEvent(altGr)).toBeNull();
  });
});

describe("displayKeys", () => {
  it("uses symbols on macOS and names elsewhere", () => {
    expect(displayKeys("Control+Shift+Space", "macos")).toEqual(["⌃", "⇧", "Space"]);
    expect(displayKeys("CommandOrControl+K", "windows")).toEqual(["Ctrl", "K"]);
    expect(displayKeys("Super+Space", "windows")).toEqual(["Win", "Space"]);
    expect(displayKeys("Super+Space", "linux")).toEqual(["Super", "Space"]);
  });
});
