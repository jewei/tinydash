import { fireEvent, render } from "@solidjs/testing-library";
import { mockConvertFileSrc } from "@tauri-apps/api/mocks";
import { createSignal } from "solid-js";
import { describe, expect, it } from "vite-plus/test";

import type { Icon } from "../generated/Icon";
import { ResultIcon } from "./Icon";

describe("ResultIcon", () => {
  it("loads the next icon after one fails", () => {
    mockConvertFileSrc("macos");
    const [icon, setIcon] = createSignal<Icon>({ type: "file", path: "/gone.txt" });
    const { container } = render(() => <ResultIcon icon={icon()} size={32} fallback="file" />);
    fireEvent.error(container.querySelector("img")!);
    expect(container.querySelector("img")).toBeNull();
    setIcon({ type: "file", path: "/Applications/Safari.app" });
    expect(container.querySelector("img")?.getAttribute("src")).toContain("Safari.app");
  });

  it("draws a typed color as a swatch", () => {
    const { container } = render(() => (
      <ResultIcon icon={{ type: "color", hex: "#2F6F5E" }} size={32} fallback="image" />
    ));
    const swatch = container.querySelector<HTMLElement>(".color-swatch");
    expect(swatch?.style.background).toBe("rgb(47, 111, 94)");
  });
});
