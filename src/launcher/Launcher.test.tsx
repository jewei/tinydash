import { fireEvent, render, screen, waitFor, within } from "@solidjs/testing-library";
import { emit } from "@tauri-apps/api/event";
import { describe, expect, it, vi } from "vite-plus/test";

import type { SearchResult } from "../generated/SearchResult";
import { app, testSettings, fakeBackend, restart } from "../test/backend";
import { Launcher } from "./Launcher";

const clip: SearchResult = {
  ...app,
  id: "clip:1",
  kind: "clipboard",
  actions: [
    { label: "Copy", action: { type: "copyClip", id: 1 }, confirm: null },
    { label: "Pin", action: { type: "pin", id: "clip:1" }, confirm: null },
    { label: "Delete", action: { type: "deleteClip", id: 1 }, confirm: null },
  ],
};

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

  it("keeps only tabs in the tab list", async () => {
    setup(() => []);
    const tablist = await screen.findByRole("tablist", { name: "Categories" });
    for (const child of tablist.children) expect(child.getAttribute("role")).toBe("tab");
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

  it("shows the tabs that Settings shows, in its order", async () => {
    const { backend, press } = setup(() => [], {
      launcher_init: () => ({
        settings: {
          ...testSettings,
          tabs: (["emoji", "apps", "files", "clipboard", "snippets", "system"] as const).map(
            (category) => ({
              category,
              shown: category === "emoji" || category === "apps",
              inAll: true,
            }),
          ),
        },
        platform: "macos",
        warnings: [],
      }),
    });
    const tablist = await screen.findByRole("tablist", { name: "Categories" });
    await waitFor(() =>
      expect([...tablist.children].map((tab) => tab.textContent)).toEqual(["All", "Emoji", "Apps"]),
    );
    for (const category of ["emoji", "apps", "all"]) {
      press("Tab");
      await waitFor(() =>
        expect(backend.called("search").at(-1)?.args).toEqual({ query: "", category }),
      );
    }
    // A hidden tab opened on purpose shows while it is open.
    await emit("launcher:shown", { category: "clipboard" });
    const clipboard = await screen.findByRole("tab", { name: "Clipboard" });
    expect(clipboard.getAttribute("aria-selected")).toBe("true");
  });

  it("does nothing on Tab when only All shows", async () => {
    const { backend, press } = setup(() => [], {
      launcher_init: () => ({
        settings: {
          ...testSettings,
          tabs: testSettings.tabs.map((tab) => ({ ...tab, shown: false })),
        },
        platform: "macos",
        warnings: [],
      }),
    });
    await waitFor(() => expect(screen.queryByText("next")).toBeNull());
    await waitFor(() => expect(backend.called("search").length).toBeGreaterThan(0));
    const searches = backend.called("search").length;
    press("Tab");
    await new Promise((resolve) => setTimeout(resolve, 10));
    expect(backend.called("search")).toHaveLength(searches);
  });

  it("offers a found update, installs it once on request, and says why an install failed", async () => {
    let fail!: () => void;
    const failed = new Promise<void>((resolve) => (fail = resolve));
    const { backend, input, press } = setup(() => [app], {
      launcher_init: () => ({
        settings: testSettings,
        platform: "macos",
        warnings: [],
        category: null,
        update: "0.2.2",
      }),
      install_update: async () => {
        await failed;
        throw "Could not install TinyDash 0.2.2: offline.";
      },
    });
    expect(await screen.findByText("TinyDash 0.2.2 is available.")).toBeTruthy();
    await screen.findByRole("option", { name: /Safari/ });
    // Its buttons keep focus in the search field.
    const install = screen.getByRole("button", { name: "Install and Restart" });
    expect(fireEvent.mouseDown(install)).toBe(false);
    expect(fireEvent.mouseDown(screen.getByRole("button", { name: "Later" }))).toBe(false);
    fireEvent.click(install);
    expect(await screen.findByRole("button", { name: "Installing…" })).toBeTruthy();
    // A second request while it runs does nothing.
    press("k", { ctrlKey: true });
    fireEvent.click(
      await screen.findByRole("option", { name: /Install TinyDash 0.2.2 and Restart/ }),
    );
    fail();
    expect(await screen.findByText("Could not install TinyDash 0.2.2: offline.")).toBeTruthy();
    expect(backend.called("install_update")).toHaveLength(1);
    expect(document.activeElement).toBe(input);

    // The keyboard hides it through the actions menu.
    press("k", { ctrlKey: true });
    fireEvent.click(await screen.findByRole("option", { name: /Hide Update Notice/ }));
    await waitFor(() => expect(screen.queryByText(/is available|Could not install/)).toBeNull());
    // A later check shows a newer one, and a check that finds none clears it.
    await emit("update:changed", "0.2.3");
    expect(await screen.findByText("TinyDash 0.2.3 is available.")).toBeTruthy();
    await emit("update:changed", null);
    await waitFor(() => expect(screen.queryByText(/is available/)).toBeNull());
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

  it("opens in the category of a show that came before the page loaded", async () => {
    const backend = fakeBackend({
      launcher_init: () => ({
        settings: testSettings,
        platform: "macos",
        warnings: [],
        category: "emoji",
      }),
    });
    render(() => <Launcher />);
    await waitFor(() =>
      expect(screen.getByRole("tab", { name: "Emoji" }).getAttribute("aria-selected")).toBe("true"),
    );
    expect(backend.called("search").at(-1)?.args).toEqual({ query: "", category: "emoji" });
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

  it("selects a row when the pointer moves, not when the list scrolls under it", async () => {
    const other = { ...app, id: "app:/Applications/Notes.app", title: "Notes" };
    setup(() => [app, other]);
    const notes = await screen.findByRole("option", { name: /Notes/ });
    fireEvent.mouseMove(notes, { movementX: 0, movementY: 0 });
    expect(notes.getAttribute("aria-selected")).toBe("false");
    fireEvent.mouseMove(notes, { movementX: 2, movementY: 0 });
    expect(notes.getAttribute("aria-selected")).toBe("true");
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
        settings: { ...testSettings, clipboardHistoryEnabled: false },
        platform: "macos",
        warnings: [],
      }),
    });
    fireEvent.click(await screen.findByRole("tab", { name: "Clipboard" }));
    fireEvent.click(await screen.findByRole("button", { name: "Turn On Clipboard History" }));
    await waitFor(() => expect(backend.called("update_settings")).toHaveLength(1));
    expect(backend.called("update_settings")[0]?.args).toEqual({
      changes: { clipboardHistoryEnabled: true },
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

  it("waits for the newest search before Enter runs a result", async () => {
    let release!: () => void;
    const slow = new Promise<void>((resolve) => (release = resolve));
    const notes = { ...app, id: "app:/Applications/Notes.app", title: "Notes" };
    const backend = fakeBackend({
      search: async (args) => {
        if (args.query === "notes") await slow;
        return args.query === "notes" ? [notes] : [app];
      },
    });
    render(() => <Launcher />);
    const input = screen.getByRole("combobox");
    await screen.findByRole("option", { name: /Safari/ });
    fireEvent.input(input, { target: { value: "notes" } });
    fireEvent.keyDown(input, { key: "Enter" });
    expect(backend.called("run_action")).toHaveLength(0);
    release();
    await waitFor(() => expect(backend.called("run_action")).toHaveLength(1));
    expect(backend.called("run_action")[0]?.args.resultId).toBe(notes.id);
  });

  it("never repeats an action while a key is held", async () => {
    const { backend, press } = setup(() => [app]);
    await screen.findByRole("option", { name: /Safari/ });
    press("Enter", { repeat: true });
    press("Enter", { ctrlKey: true, repeat: true });
    await new Promise((resolve) => setTimeout(resolve, 10));
    expect(backend.called("run_action")).toHaveLength(0);
  });

  it("keeps the position when the selected entry disappears", async () => {
    const clip = (id: number) => ({ ...app, id: `clip:${id}`, title: `Entry ${id}` });
    let entries = [clip(3), clip(2), clip(1)];
    const { press } = setup(() => entries);
    await screen.findByRole("option", { name: /Entry 3/ });
    press("ArrowDown");
    entries = [clip(3), clip(1)];
    await emit("results:stale", null);
    await waitFor(() =>
      expect(screen.getByRole("option", { name: /Entry 1/ }).getAttribute("aria-selected")).toBe(
        "true",
      ),
    );
  });

  it("says when no action matches, outside the list of options", async () => {
    const { press } = setup(() => [app]);
    await screen.findByRole("option", { name: /Safari/ });
    press("k", { ctrlKey: true });
    const filter = await screen.findByRole("combobox", { name: "Search actions" });
    fireEvent.input(filter, { target: { value: "zzz" } });
    const empty = await screen.findByRole("status");
    expect(empty.textContent).toBe("No matching actions");
    const menu = screen.getByRole("dialog", { name: "Actions" });
    expect(within(menu).getByRole("listbox").children).toHaveLength(0);
    // A click on the text keeps focus in the filter, which keeps the menu open;
    // a click in the filter still places the cursor.
    expect(fireEvent.mouseDown(empty)).toBe(false);
    expect(fireEvent.mouseDown(filter)).toBe(true);
  });

  it("moves the window from the empty parts of the tab bar and footer", async () => {
    const { backend, input } = setup(() => [app]);
    await screen.findByRole("option", { name: /Safari/ });
    const tabs = screen.getByRole("navigation", { name: "Categories" });
    // Cancelled, so focus stays in the search field.
    expect(fireEvent.mouseDown(tabs)).toBe(false);
    fireEvent.mouseDown(screen.getByRole("contentinfo"));
    await waitFor(() => expect(backend.called("drag_launcher")).toHaveLength(2));
    // Buttons there keep their own job, and only the main button drags.
    fireEvent.mouseDown(screen.getByRole("tab", { name: "Apps" }));
    fireEvent.mouseDown(screen.getByRole("button", { name: /Actions/ }));
    fireEvent.mouseDown(tabs, { button: 2 });
    await new Promise((resolve) => setTimeout(resolve, 10));
    expect(backend.called("drag_launcher")).toHaveLength(2);
    expect(document.activeElement).toBe(input);
  });

  it("centers the launcher from the actions menu", async () => {
    const { backend, press } = setup(() => [app]);
    await screen.findByRole("option", { name: /Safari/ });
    press("k", { ctrlKey: true });
    const filter = await screen.findByPlaceholderText("Search actions");
    fireEvent.input(filter, { target: { value: "center" } });
    fireEvent.keyDown(filter, { key: "Enter" });
    await waitFor(() =>
      expect(backend.called("run_action")[0]?.args).toEqual({
        action: { type: "centerLauncher" },
        resultId: null,
      }),
    );
  });

  it("keeps the action menu as it opened, and closes it on a tab click", async () => {
    let results = [app];
    const { press } = setup(() => results);
    await screen.findByRole("option", { name: /Safari/ });
    press("k", { ctrlKey: true });
    const menu = await screen.findByRole("dialog", { name: "Actions" });
    results = [{ ...clip, title: "Entry" }];
    await emit("results:stale", null);
    await screen.findByRole("option", { name: /Entry/ });
    expect(within(menu).getByRole("option", { name: /Show in Finder/ })).toBeTruthy();
    expect(within(menu).queryByRole("option", { name: /Delete/ })).toBeNull();
    fireEvent.click(screen.getByRole("tab", { name: "Clipboard" }));
    expect(screen.queryByRole("dialog", { name: "Actions" })).toBeNull();
    expect(document.activeElement).toBe(screen.getByRole("combobox"));
  });

  it("opens the action menu only for the results of what was typed", async () => {
    let release!: () => void;
    const slow = new Promise<void>((resolve) => (release = resolve));
    fakeBackend({
      search: async (args) => {
        if (args.query === "x") await slow;
        return [args.query === "x" ? { ...clip, title: "Bravo" } : { ...clip, title: "Alpha" }];
      },
    });
    render(() => <Launcher />);
    const input = screen.getByRole("combobox");
    await screen.findByRole("option", { name: /Alpha/ });
    fireEvent.input(input, { target: { value: "x" } });
    fireEvent.keyDown(input, { key: "k", ctrlKey: true });
    expect(screen.queryByRole("dialog", { name: "Actions" })).toBeNull();
    release();
    await screen.findByRole("option", { name: /Bravo/ });
    fireEvent.click(screen.getByRole("button", { name: /^Actions/ }));
    const menu = await screen.findByRole("dialog", { name: "Actions" });
    expect(within(menu).getByRole("listbox", { name: "Actions" })).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: /^Actions/ }));
    expect(screen.queryByRole("dialog", { name: "Actions" })).toBeNull();
  });

  it("keeps the action menu open while Mod+K is held", async () => {
    const { press } = setup(() => [app]);
    await screen.findByRole("option", { name: /Safari/ });
    press("k", { ctrlKey: true });
    const filter = await screen.findByRole("combobox", { name: "Search actions" });
    fireEvent.keyDown(filter, { key: "k", ctrlKey: true, repeat: true });
    expect(screen.getByRole("dialog", { name: "Actions" })).toBeTruthy();
    fireEvent.keyDown(filter, { key: "k", ctrlKey: true });
    expect(screen.queryByRole("dialog", { name: "Actions" })).toBeNull();
  });

  it("scrolls the selected row into view when new results arrive", async () => {
    const scroll = vi.spyOn(Element.prototype, "scrollIntoView");
    try {
      const { input } = setup((query) => [{ ...app, title: query || "Safari" }]);
      await screen.findByRole("option", { name: /Safari/ });
      scroll.mockClear();
      fireEvent.input(input, { target: { value: "x" } });
      await screen.findByRole("option", { name: /^x/ });
      await waitFor(() =>
        expect(scroll.mock.contexts.map((row) => (row as Element).id)).toContain("result-0"),
      );
    } finally {
      scroll.mockRestore();
    }
  });

  it("reads Control+K and Control+V on a layout that types another script", async () => {
    const { input, press } = setup(() => [app]);
    await screen.findByRole("option", { name: /Safari/ });
    press("л", { ctrlKey: true, code: "KeyK" });
    const filter = await screen.findByRole("combobox", { name: "Search actions" });
    fireEvent.keyDown(filter, { key: "л", ctrlKey: true, code: "KeyK" });
    expect(screen.queryByRole("dialog", { name: "Actions" })).toBeNull();
    (document.activeElement as HTMLElement).blur();
    fireEvent.keyDown(document.body, { key: "м", ctrlKey: true, code: "KeyV" });
    expect(document.activeElement).toBe(input);
  });

  it("returns focus to the search field for Mod+Backspace with nothing to delete", async () => {
    const { input } = setup(() => [app]);
    await screen.findByRole("option", { name: /Safari/ });
    (document.activeElement as HTMLElement).blur();
    const event = { key: "Backspace", code: "Backspace", ctrlKey: true };
    // Not cancelled, so the field deletes the word as usual.
    expect(fireEvent.keyDown(document.body, event)).toBe(true);
    expect(document.activeElement).toBe(input);
  });

  it("names exchange rates in Refresh only when they are on", async () => {
    const { press } = setup(() => [app]);
    await screen.findByRole("option", { name: /Safari/ });
    press("k", { ctrlKey: true });
    expect(
      await screen.findByRole("option", { name: "Refresh Apps, Files, and Rates" }),
    ).toBeTruthy();
    const filter = screen.getByRole("combobox", { name: "Search actions" });
    fireEvent.keyDown(filter, { key: "Escape" });
    await emit("settings:changed", { ...testSettings, currencyRatesEnabled: false });
    press("k", { ctrlKey: true });
    expect(await screen.findByRole("option", { name: "Refresh Apps and Files" })).toBeTruthy();
  });

  it("forgets a failed history turn-on when the launcher opens again", async () => {
    const { press } = setup(() => [], {
      launcher_init: () => ({
        settings: { ...testSettings, clipboardHistoryEnabled: false },
        platform: "macos",
        warnings: [],
      }),
      update_settings: () => {
        throw "Could not save settings.";
      },
    });
    fireEvent.click(await screen.findByRole("tab", { name: "Clipboard" }));
    await screen.findByText("Clipboard history is off");
    press("Enter");
    await screen.findByText("Could not save settings.");
    await emit("launcher:shown", { category: "clipboard" });
    await waitFor(() => expect(screen.queryByText("Could not save settings.")).toBeNull());
  });

  it("closes the action menu when focus leaves it", async () => {
    const { input, press } = setup(() => [app]);
    await screen.findByRole("option", { name: /Safari/ });
    press("k", { ctrlKey: true });
    const filter = await screen.findByRole("combobox", { name: "Search actions" });
    fireEvent.focusOut(filter, { relatedTarget: input });
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Actions" })).toBeNull());
  });

  it("runs the row selected in the new results, not an old position", async () => {
    let release!: () => void;
    const slow = new Promise<void>((resolve) => (release = resolve));
    const rows = (prefix: string) =>
      [1, 2, 3].map((n) => ({ ...app, id: `app:/${prefix}${n}`, title: `${prefix} ${n}` }));
    const backend = fakeBackend({
      search: async (args) => {
        if (args.query === "b") await slow;
        return rows(args.query === "b" ? "Beta" : "Alpha");
      },
    });
    render(() => <Launcher />);
    const input = screen.getByRole("combobox");
    await screen.findByRole("option", { name: /Alpha 1/ });
    fireEvent.keyDown(input, { key: "ArrowDown" });
    fireEvent.keyDown(input, { key: "ArrowDown" });
    fireEvent.input(input, { target: { value: "b" } });
    fireEvent.keyDown(input, { key: "Enter" });
    release();
    await waitFor(() => expect(backend.called("run_action")).toHaveLength(1));
    expect(backend.called("run_action")[0]?.args.resultId).toBe("app:/Beta1");
  });

  it("deletes only an entry the user can see selected", async () => {
    let release!: () => void;
    const slow = new Promise<void>((resolve) => (release = resolve));
    const backend = fakeBackend({
      search: async (args) => {
        if (args.query === "x") await slow;
        return [clip];
      },
    });
    render(() => <Launcher />);
    const input = screen.getByRole("combobox");
    await screen.findByRole("option", { name: /Safari/ });
    fireEvent.input(input, { target: { value: "x" } });
    fireEvent.keyDown(input, { key: "Backspace", ctrlKey: true });
    release();
    await new Promise((resolve) => setTimeout(resolve, 10));
    expect(backend.called("run_action")).toHaveLength(0);
    fireEvent.keyDown(input, { key: "Backspace", ctrlKey: true });
    await waitFor(() => expect(backend.called("run_action")).toHaveLength(1));
  });

  it("shows Mod+Backspace as the key for deleting a clipboard entry", async () => {
    const { press } = setup(() => [clip]);
    await screen.findByRole("option", { name: /Safari/ });
    press("k", { ctrlKey: true });
    const remove = await screen.findByRole("option", { name: /Delete/ });
    expect(remove.textContent).toContain("⌫");
    expect(remove.textContent).not.toContain("↩");
  });

  it("keeps shortcuts working after focus leaves the search field", async () => {
    const { backend } = setup(() => [app]);
    await screen.findByRole("option", { name: /Safari/ });
    (document.activeElement as HTMLElement).blur();
    fireEvent.keyDown(document.body, { key: "Escape" });
    await waitFor(() => expect(backend.called("hide_launcher")).toHaveLength(1));
  });

  it("dismisses a startup warning with Escape before hiding", async () => {
    const { backend, press } = setup(() => [], {
      launcher_init: () => ({
        settings: testSettings,
        platform: "macos",
        warnings: ["Could not register Control+Space."],
      }),
    });
    expect((await screen.findByRole("alert")).textContent).toContain("Could not register");
    press("Escape");
    await waitFor(() => expect(screen.queryByRole("alert")).toBeNull());
    expect(backend.called("hide_launcher")).toHaveLength(0);
  });

  it("turns on clipboard history with Enter in its empty state", async () => {
    const { backend, press } = setup(() => [], {
      launcher_init: () => ({
        settings: { ...testSettings, clipboardHistoryEnabled: false },
        platform: "macos",
        warnings: [],
      }),
    });
    fireEvent.click(await screen.findByRole("tab", { name: "Clipboard" }));
    const button = await screen.findByRole("button", { name: "Turn On Clipboard History" });
    // Cancelled, so a focused button does not click and save again.
    expect(fireEvent.keyDown(button, { key: "Enter" })).toBe(false);
    press("Enter", { repeat: true });
    press("Enter", { repeat: true });
    await waitFor(() => expect(backend.called("update_settings")).toHaveLength(1));
    await new Promise((resolve) => setTimeout(resolve, 10));
    expect(backend.called("update_settings")).toHaveLength(1);
  });

  it("shows no results of another tab while clipboard history is off", async () => {
    fakeBackend({
      launcher_init: () => ({
        settings: { ...testSettings, clipboardHistoryEnabled: false },
        platform: "macos",
        warnings: [],
      }),
      // The Clipboard search never returns, so only the All results exist.
      search: (args) => (args.category === "clipboard" ? new Promise(() => {}) : [app]),
    });
    render(() => <Launcher />);
    const input = screen.getByRole("combobox");
    await screen.findByRole("option", { name: /Safari/ });
    fireEvent.click(screen.getByRole("tab", { name: "Clipboard" }));
    await screen.findByText("Clipboard history is off");
    expect(input.getAttribute("aria-activedescendant")).toBeNull();
    expect(screen.queryByText("Open")).toBeNull();
    // The Clipboard search has not returned, so the menu waits for it.
    fireEvent.keyDown(input, { key: "k", ctrlKey: true });
    expect(screen.queryByRole("dialog", { name: "Actions" })).toBeNull();
  });

  it("shows once why clipboard history did not turn on, apart from warnings", async () => {
    const { backend, press } = setup(() => [], {
      launcher_init: () => ({
        settings: { ...testSettings, clipboardHistoryEnabled: false },
        platform: "macos",
        warnings: ["Could not register Control+Shift+Space."],
      }),
      update_settings: () => {
        throw "Could not save settings.";
      },
    });
    fireEvent.click(await screen.findByRole("tab", { name: "Clipboard" }));
    await screen.findByText("Clipboard history is off");
    for (let attempt = 1; attempt <= 3; attempt += 1) {
      press("Enter");
      await waitFor(() => expect(backend.called("update_settings")).toHaveLength(attempt));
    }
    // Let every failed attempt answer before checking.
    await new Promise((resolve) => setTimeout(resolve, 10));
    expect(screen.getAllByRole("alert")).toHaveLength(2);
    expect(screen.getAllByText("Could not save settings.")).toHaveLength(1);
    press("Escape");
    expect(screen.queryByText(/Could not register/)).toBeNull();
    await emit("settings:changed", { ...testSettings, clipboardHistoryEnabled: false });
    await waitFor(() => expect(screen.queryByText("Could not save settings.")).toBeNull());
    press("Escape");
    await waitFor(() => expect(backend.called("hide_launcher")).toHaveLength(1));
  });

  it("runs no hidden clipboard row with Mod+1 after history turns off", async () => {
    const { backend, press } = setup((_query, category) =>
      category === "clipboard" ? [clip] : [],
    );
    fireEvent.click(await screen.findByRole("tab", { name: "Clipboard" }));
    await screen.findByRole("option", { name: /Safari/ });
    press("1", { code: "Digit1", ctrlKey: true });
    await waitFor(() => expect(backend.called("run_action")).toHaveLength(1));
    await emit("settings:changed", { ...testSettings, clipboardHistoryEnabled: false });
    await screen.findByText("Clipboard history is off");
    press("1", { code: "Digit1", ctrlKey: true });
    await new Promise((resolve) => setTimeout(resolve, 10));
    expect(backend.called("run_action")).toHaveLength(1);
  });

  it("cancels a confirmation when the backdrop is clicked", async () => {
    const { backend, press } = setup(() => [restart]);
    await screen.findByRole("option", { name: /Restart/ });
    press("Enter");
    const dialog = await screen.findByRole("alertdialog");
    fireEvent.click(dialog.parentElement as HTMLElement);
    await waitFor(() => expect(screen.queryByRole("alertdialog")).toBeNull());
    expect(backend.called("run_action")).toHaveLength(0);
  });

  it("clears the history from the Clipboard tab, after asking", async () => {
    const { backend, press } = setup((_query, category) =>
      category === "clipboard" ? [clip] : [app],
    );
    await screen.findByRole("option", { name: /Safari/ });
    expect(screen.queryByRole("button", { name: /Clear History/ })).toBeNull();
    fireEvent.click(screen.getByRole("tab", { name: "Clipboard" }));
    await waitFor(() =>
      expect(backend.called("search").at(-1)?.args).toMatchObject({ category: "clipboard" }),
    );
    fireEvent.click(await screen.findByRole("button", { name: /Clear History/ }));
    const dialog = await screen.findByRole("alertdialog");
    expect(dialog.textContent).toContain("Delete all clipboard history except pinned entries?");
    fireEvent.click(within(dialog).getByRole("button", { name: "Clear Clipboard History" }));
    await waitFor(() =>
      expect(backend.called("run_action")[0]?.args).toEqual({
        action: { type: "clearClipboard" },
        resultId: null,
      }),
    );
    // Mod+K lists it there too.
    press("k", { ctrlKey: true });
    expect(await screen.findByRole("option", { name: /Clear Clipboard History/ })).toBeTruthy();
  });

  it("reloads the preview when results refresh", async () => {
    const snippet = { ...app, id: "snippet:1", kind: "snippet" as const, title: "Sig" };
    let text = "Regards";
    fakeBackend({
      search: () => [snippet],
      preview: () => ({ type: "text", text }),
    });
    render(() => <Launcher />);
    expect(await screen.findByText("Regards")).toBeTruthy();
    text = "Cheers";
    await emit("results:stale", null);
    expect(await screen.findByText("Cheers")).toBeTruthy();
  });

  it("shows a copied text once, without repeating its first line as a title", async () => {
    const files = { ...clip, id: "clip:2", title: "one.txt and 1 more" };
    fakeBackend({
      search: () => [{ ...clip, title: "repos" }, files],
      preview: (args) =>
        args.id === "clip:1"
          ? { type: "text", text: "repos" }
          : { type: "files", paths: ["/a/one.txt", "/a/two.txt"] },
    });
    render(() => <Launcher />);
    const details = await screen.findByRole("complementary", { name: "Details" });
    await waitFor(() => expect(within(details).getByText("repos").tagName).toBe("PRE"));
    expect(within(details).queryByRole("heading")).toBeNull();
    // Other clips keep their title, which says what the preview does not.
    fireEvent.keyDown(screen.getByRole("combobox"), { key: "ArrowDown" });
    expect(
      await within(details).findByRole("heading", { name: "one.txt and 1 more" }),
    ).toBeTruthy();
  });

  it.each([
    [999_999, "1.0 MB"],
    [9_960, "10 KB"],
    [9_940, "9.9 KB"],
    [1, "1 byte"],
  ])("shows %i bytes as %s", async (size, text) => {
    const file = { ...app, id: "file:/notes.txt", kind: "file" as const, title: "notes.txt" };
    fakeBackend({
      search: () => [file],
      preview: () => ({ type: "file", path: "/notes.txt", size, modified: null, isDir: false }),
    });
    render(() => <Launcher />);
    expect(await screen.findByText(text)).toBeTruthy();
  });

  it("sends typing back to the search field after a click", async () => {
    const { input } = setup(() => [app]);
    await screen.findByRole("option", { name: /Safari/ });
    (document.activeElement as HTMLElement).blur();
    fireEvent.keyDown(document.body, { key: "x" });
    expect(document.activeElement).toBe(input);
  });

  it("leaves Mod+Backspace to the text field when nothing can be deleted", async () => {
    const { input } = setup(() => [app]);
    await screen.findByRole("option", { name: /Safari/ });
    const event = new KeyboardEvent("keydown", {
      key: "Backspace",
      ctrlKey: true,
      bubbles: true,
      cancelable: true,
    });
    input.dispatchEvent(event);
    expect(event.defaultPrevented).toBe(false);
  });

  it("keeps a confirmation modal after focus leaves it", async () => {
    const { backend, press } = setup(() => [restart]);
    await screen.findByRole("option", { name: /Restart/ });
    press("Enter");
    await screen.findByRole("alertdialog");
    (document.activeElement as HTMLElement).blur();
    fireEvent.keyDown(document.body, { key: "Escape" });
    await waitFor(() => expect(screen.queryByRole("alertdialog")).toBeNull());
    expect(backend.called("run_action")).toHaveLength(0);
    expect(backend.called("hide_launcher")).toHaveLength(0);
  });

  it("opens and closes the action menu with Caps Lock on", async () => {
    const { press } = setup(() => [app]);
    await screen.findByRole("option", { name: /Safari/ });
    press("K", { ctrlKey: true });
    const filter = await screen.findByRole("combobox", { name: "Search actions" });
    fireEvent.keyDown(filter, { key: "K", ctrlKey: true });
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Actions" })).toBeNull());
  });

  it("runs the row that was clicked", async () => {
    const notes = { ...app, id: "app:/Applications/Notes.app", title: "Notes" };
    const { backend } = setup(() => [app, notes]);
    fireEvent.click(await screen.findByRole("option", { name: /Notes/ }));
    await waitFor(() => expect(backend.called("run_action")).toHaveLength(1));
    expect(backend.called("run_action")[0]?.args.resultId).toBe(notes.id);
  });

  it("ignores Enter before and after a search fails", async () => {
    let fail!: () => void;
    const failing = new Promise<void>((_, reject) => (fail = () => reject("Search failed.")));
    const backend = fakeBackend({
      search: async (args) => {
        if (args.query === "x") await failing;
        return [app];
      },
    });
    render(() => <Launcher />);
    const input = screen.getByRole("combobox");
    await screen.findByRole("option", { name: /Safari/ });
    fireEvent.input(input, { target: { value: "x" } });
    fireEvent.keyDown(input, { key: "Enter" });
    fail();
    await screen.findByRole("alert");
    expect(screen.queryByText("No results")).toBeNull();
    fireEvent.keyDown(input, { key: "Enter" });
    fireEvent.input(input, { target: { value: "y" } });
    await waitFor(() => expect(screen.queryByRole("alert")).toBeNull());
    expect(backend.called("run_action")).toHaveLength(0);
  });

  it.each([
    ["the launcher hides", (input: HTMLElement) => fireEvent.keyDown(input, { key: "Escape" })],
    ["the window loses focus", () => fireEvent.blur(window)],
  ])("drops a queued Enter when %s", async (_, leave) => {
    let release!: () => void;
    const slow = new Promise<void>((resolve) => (release = resolve));
    const backend = fakeBackend({
      search: async (args) => {
        if (args.query === "y") await slow;
        return [app];
      },
    });
    render(() => <Launcher />);
    const input = screen.getByRole("combobox");
    await screen.findByRole("option", { name: /Safari/ });
    fireEvent.input(input, { target: { value: "y" } });
    fireEvent.keyDown(input, { key: "Enter" });
    leave(input);
    release();
    await new Promise((resolve) => setTimeout(resolve, 10));
    expect(backend.called("run_action")).toHaveLength(0);
  });

  it.each([
    [
      "the query changes",
      (input: HTMLElement) => fireEvent.input(input, { target: { value: "ter" } }),
    ],
    ["the tab changes", (input: HTMLElement) => fireEvent.keyDown(input, { key: "Tab" })],
  ])("drops a queued Enter when %s", async (_, change) => {
    let release!: () => void;
    const slow = new Promise<void>((resolve) => (release = resolve));
    const backend = fakeBackend({
      search: async (args) => {
        if (args.query === "term") await slow;
        return [app];
      },
    });
    render(() => <Launcher />);
    const input = screen.getByRole("combobox");
    await screen.findByRole("option", { name: /Safari/ });
    fireEvent.input(input, { target: { value: "term" } });
    fireEvent.keyDown(input, { key: "Enter" });
    change(input);
    release();
    await waitFor(() => expect(backend.called("search").length).toBeGreaterThanOrEqual(3));
    await new Promise((resolve) => setTimeout(resolve, 10));
    expect(backend.called("run_action")).toHaveLength(0);
  });

  it("leaves focus alone for copy shortcuts and lone modifiers", async () => {
    const { input } = setup(() => [app]);
    await screen.findByRole("option", { name: /Safari/ });
    (document.activeElement as HTMLElement).blur();
    fireEvent.keyDown(document.body, { key: "Meta", metaKey: true });
    fireEvent.keyDown(document.body, { key: "c", ctrlKey: true });
    fireEvent.keyDown(document.body, { key: "c", metaKey: true });
    expect(document.activeElement).not.toBe(input);
    fireEvent.keyDown(document.body, { key: "Backspace" });
    expect(document.activeElement).toBe(input);
  });

  it("does not answer a confirmation with a held Enter", async () => {
    const { backend, press } = setup(() => [restart]);
    await screen.findByRole("option", { name: /Restart/ });
    press("Enter");
    await screen.findByRole("alertdialog");
    // A browser turns an unblocked Enter on the focused button into a click.
    const held = new KeyboardEvent("keydown", {
      key: "Enter",
      repeat: true,
      bubbles: true,
      cancelable: true,
    });
    document.activeElement?.dispatchEvent(held);
    expect(held.defaultPrevented).toBe(true);
    expect(backend.called("run_action")).toHaveLength(0);
  });
});
