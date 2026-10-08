import { render, screen } from "@solidjs/testing-library";
import { emit } from "@tauri-apps/api/event";
import { describe, expect, it } from "vite-plus/test";

import { fakeBackend } from "../test/backend";
import { Hud } from "./Hud";

describe("Hud", () => {
  it("shows the message Rust sends", async () => {
    fakeBackend();
    render(() => <Hud />);
    await emit("hud:show", "Copied");
    expect((await screen.findByRole("status")).textContent).toBe("Copied");
  });
});
