import { render, screen } from "@solidjs/testing-library";
import { describe, expect, it, vi } from "vite-plus/test";

import { app, fakeBackend, noWidgets } from "../test/backend";
import { Launcher } from "./Launcher";

// jsdom's user agent names no Mac, and Up Next shows only on macOS.
vi.mock("../lib/keys", async (original) => ({
  ...(await original<typeof import("../lib/keys")>()),
  IS_MAC: true,
}));

describe("Launcher on macOS", () => {
  it("shows the widgets when Up Next is the only one on", async () => {
    const upNext = { items: [], problems: [], ask: null, changesAtMs: Date.now() + 3_600_000 };
    fakeBackend(
      {
        search: () => [app],
        widgets: () => ({
          clocks: null,
          upNext,
          disk: null,
          note: null,
          focus: null,
          weather: null,
          clipCard: null,
        }),
      },
      { ...noWidgets, showUpNext: true },
    );
    render(() => <Launcher />);
    expect(await screen.findByRole("region", { name: "Up Next" })).toBeTruthy();
    expect(screen.queryByRole("complementary", { name: "Details" })).toBeNull();
  });
});
