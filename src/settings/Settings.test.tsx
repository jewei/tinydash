import { fireEvent, render, screen, waitFor } from "@solidjs/testing-library";
import { describe, expect, it } from "vite-plus/test";

import type { LibraryItem } from "../generated/LibraryItem";
import { defaultSettings, fakeBackend } from "../test/backend";
import { Settings } from "./Settings";

describe("Settings", () => {
  it("saves a change at once", async () => {
    const backend = fakeBackend();
    render(() => <Settings />);
    fireEvent.click(
      await screen.findByRole("switch", { name: "Hide when another app is focused" }),
    );
    await waitFor(() => expect(backend.called("update_settings")).toHaveLength(1));
    expect(backend.called("update_settings")[0]?.args).toEqual({
      settings: { ...defaultSettings, hideOnBlur: false },
    });
  });

  it("keeps quick changes in order", async () => {
    const backend = fakeBackend();
    render(() => <Settings />);
    fireEvent.click(
      await screen.findByRole("switch", { name: "Hide when another app is focused" }),
    );
    fireEvent.click(screen.getByRole("switch", { name: "Open at login" }));
    await waitFor(() => expect(backend.called("update_settings")).toHaveLength(2));
    expect(backend.called("update_settings")[1]?.args).toEqual({
      settings: { ...defaultSettings, hideOnBlur: false, launchAtLogin: true },
    });
  });

  it("shows why a change failed and restores the saved value", async () => {
    fakeBackend({
      update_settings: () => {
        throw "Could not register Control+K.";
      },
    });
    render(() => <Settings />);
    const toggle = await screen.findByRole("switch", { name: "Open at login" });
    fireEvent.click(toggle);
    expect((await screen.findByRole("alert")).textContent).toContain("Could not register");
    await waitFor(() => expect((toggle as HTMLInputElement).checked).toBe(false));
  });

  it("records a new shortcut while the old one is paused", async () => {
    const backend = fakeBackend();
    render(() => <Settings />);
    const recorder = await screen.findByRole("button", { name: "Launcher shortcut" });
    fireEvent.click(recorder);
    fireEvent.keyDown(recorder, { key: " ", code: "Space", altKey: true });
    await waitFor(() => expect(backend.called("update_settings")).toHaveLength(1));
    expect(backend.called("update_settings")[0]?.args).toMatchObject({
      settings: { shortcut: "Alt+Space" },
    });
    await waitFor(() =>
      expect(backend.called("pause_shortcut").map((call) => call.args.paused)).toEqual([
        true,
        false,
      ]),
    );
  });

  it("creates a snippet", async () => {
    const saved: LibraryItem[] = [];
    const backend = fakeBackend({
      library_items: () => saved,
      save_library_item: (args) => {
        const item = { ...(args.item as LibraryItem), id: 7 };
        saved.push(item);
        return item;
      },
    });
    render(() => <Settings />);
    fireEvent.click(await screen.findByRole("button", { name: "Snippets" }));
    fireEvent.input(await screen.findByLabelText("Name"), { target: { value: "Signature" } });
    fireEvent.input(screen.getByLabelText("Text"), { target: { value: "Regards, {date}" } });
    fireEvent.click(screen.getByRole("button", { name: "Save Snippet" }));
    await waitFor(() => expect(backend.called("save_library_item")).toHaveLength(1));
    expect(backend.called("save_library_item")[0]?.args.item).toMatchObject({
      id: null,
      kind: "snippet",
      name: "Signature",
      text: "Regards, {date}",
    });
    expect(await screen.findByRole("button", { name: /Signature/ })).toBeTruthy();
  });
});
