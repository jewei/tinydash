import { fireEvent, render, screen, waitFor } from "@solidjs/testing-library";
import { describe, expect, it, vi } from "vite-plus/test";

import { fakeBackend } from "../test/backend";
import { Settings } from "./Settings";

// jsdom's user agent names no Mac, and Up Next shows only on macOS.
vi.mock("../lib/keys", async (original) => ({
  ...(await original<typeof import("../lib/keys")>()),
  IS_MAC: true,
}));

describe("Settings on macOS", () => {
  it("turns on Up Next", async () => {
    const backend = fakeBackend();
    render(() => <Settings />);
    fireEvent.click(await screen.findByRole("button", { name: "Widgets" }));
    fireEvent.click(await screen.findByRole("switch", { name: "Up Next" }));
    await waitFor(() => expect(backend.called("update_settings")).toHaveLength(1));
    expect(backend.called("update_settings")[0]?.args).toEqual({
      changes: { showUpNext: true },
    });
  });
});
