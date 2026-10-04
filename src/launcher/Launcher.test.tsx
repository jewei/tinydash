import { fireEvent, render, screen, waitFor } from "@solidjs/testing-library";
import { emit } from "@tauri-apps/api/event";
import { describe, expect, it } from "vite-plus/test";

import type { SearchResult } from "../generated/SearchResult";
import { app, defaultSettings, fakeBackend, restart } from "../test/backend";
import { Launcher } from "./Launcher";

function setup(results: (query: string, category: string) => SearchResult[], extra = {}) {
  const backend = fakeBackend({
    search: (args) => results(String(args.query), String(args.category)),
    ...extra,
  });
  render(() => <Launcher />);
  const input = screen.getByRole("combobox");
  const press = (key: string, options: KeyboardEventInit = {}) =>
    fireEvent.keyDown(input, { key, ...options });
  return { backend, input, press };
}

describe("Launcher", () => {
  it("searches as the user types", async () => {
    const { backend, input } = setup((query) => (query === "saf" ? [app] : []));
    fireEvent.input(input, { target: { value: "saf" } });
    expect(await screen.findByRole("option", { name: /Safari/ })).toBeTruthy();
    expect(backend.called("search").at(-1)?.args).toEqual({ query: "saf", category: "all" });
  });

  it("runs the main action with its result for ranking", async () => {
    const { backend, press } = setup(() => [app]);
    await screen.findByRole("option", { name: /Safari/ });
    press("Enter");
    await waitFor(() => expect(backend.called("run_action")).toHaveLength(1));
    expect(backend.called("run_action")[0]?.args).toEqual({
      action: app.actions[0]?.action,
      resultId: app.id,
    });
  });

  it("runs the second action with Mod+Enter and does not count it as use", async () => {
    const { backend, press } = setup(() => [app]);
    await screen.findByRole("option", { name: /Safari/ });
    press("Enter", { ctrlKey: true });
    await waitFor(() => expect(backend.called("run_action")).toHaveLength(1));
    expect(backend.called("run_action")[0]?.args).toEqual({
      action: app.actions[1]?.action,
      resultId: null,
    });
  });

  it("asks before a destructive action, with Cancel focused", async () => {
    const { backend, press } = setup(() => [restart]);
    await screen.findByRole("option", { name: /Restart/ });
    press("Enter");
    const dialog = await screen.findByRole("alertdialog");
    expect(dialog.textContent).toContain("Restart the computer?");
    expect(document.activeElement?.textContent).toBe("Cancel");
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(backend.called("run_action")).toHaveLength(0);

    press("Enter");
    fireEvent.click(await screen.findByRole("button", { name: "Run" }));
    await waitFor(() => expect(backend.called("run_action")).toHaveLength(1));
  });

  it("moves through categories with Tab", async () => {
    const { backend, press } = setup(() => []);
    await waitFor(() => expect(backend.called("search")).toHaveLength(1));
    press("Tab");
    await waitFor(() =>
      expect(backend.called("search").at(-1)?.args).toEqual({ query: "", category: "apps" }),
    );
    expect(screen.getByRole("tab", { name: "Apps" }).getAttribute("aria-selected")).toBe("true");
  });

  it("opens the action menu with Mod+K", async () => {
    const { backend, press } = setup(() => [app]);
    await screen.findByRole("option", { name: /Safari/ });
    press("k", { ctrlKey: true });
    const filter = await screen.findByPlaceholderText("Search actions");
    fireEvent.input(filter, { target: { value: "show" } });
    fireEvent.keyDown(filter, { key: "Enter" });
    await waitFor(() => expect(backend.called("run_action")).toHaveLength(1));
    expect(backend.called("run_action")[0]?.args.action).toEqual(app.actions[1]?.action);
  });

  it("hides with Escape", async () => {
    const { backend, press } = setup(() => []);
    press("Escape");
    await waitFor(() => expect(backend.called("hide_launcher")).toHaveLength(1));
  });

  it("starts over when the launcher opens", async () => {
    const { backend, input } = setup(() => []);
    fireEvent.input(input, { target: { value: "old" } });
    await emit("launcher:shown", { category: "clipboard" });
    await waitFor(() =>
      expect(backend.called("search").at(-1)?.args).toEqual({ query: "", category: "clipboard" }),
    );
    expect((input as HTMLInputElement).value).toBe("");
  });

  it("keeps the selection when results refresh", async () => {
    const other = { ...app, id: "app:/Applications/Notes.app", title: "Notes" };
    const { press } = setup(() => [app, other]);
    await screen.findByRole("option", { name: /Notes/ });
    press("ArrowDown");
    await emit("results:stale", null);
    await waitFor(() =>
      expect(screen.getByRole("option", { name: /Notes/ }).getAttribute("aria-selected")).toBe(
        "true",
      ),
    );
  });

  it("offers to turn on clipboard history", async () => {
    const { backend } = setup(() => [], {
      launcher_init: () => ({
        settings: { ...defaultSettings, clipboardHistoryEnabled: false },
        platform: "macos",
        warnings: [],
      }),
    });
    fireEvent.click(await screen.findByRole("tab", { name: "Clipboard" }));
    fireEvent.click(await screen.findByRole("button", { name: "Turn On Clipboard History" }));
    await waitFor(() => expect(backend.called("update_settings")).toHaveLength(1));
    expect(backend.called("update_settings")[0]?.args).toMatchObject({
      settings: { clipboardHistoryEnabled: true },
    });
  });

  it("shows action errors", async () => {
    const { press } = setup(() => [app], {
      run_action: () => {
        throw "This app is no longer installed.";
      },
    });
    await screen.findByRole("option", { name: /Safari/ });
    press("Enter");
    expect((await screen.findByRole("alert")).textContent).toContain("no longer installed");
  });
});
