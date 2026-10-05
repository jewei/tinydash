import { fireEvent, render, screen, waitFor } from "@solidjs/testing-library";
import { describe, expect, it } from "vite-plus/test";

import type { LibraryItem } from "../generated/LibraryItem";
import { testSettings, fakeBackend } from "../test/backend";
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
      settings: { ...testSettings, hideOnBlur: false },
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
      settings: { ...testSettings, hideOnBlur: false, launchAtLogin: true },
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

  it("stops adding folders when the list is full", async () => {
    const folders = Array.from({ length: 50 }, (_, index) => `~/Folder${index}`);
    fakeBackend({ get_settings: () => ({ ...testSettings, fileSearchFolders: folders }) });
    render(() => <Settings />);
    fireEvent.click(await screen.findByRole("button", { name: "Files" }));
    expect(await screen.findByText(/The list is full \(50\)/)).toBeTruthy();
    const [addFolder, addSkipName] = screen.getAllByRole("button", { name: "Add" });
    expect((addFolder as HTMLButtonElement).disabled).toBe(true);
    expect((addSkipName as HTMLButtonElement).disabled).toBe(false);
    fireEvent.click(screen.getByRole("button", { name: "Remove ~/Folder0" }));
    expect(document.activeElement).toBe(screen.getByRole("textbox", { name: "Folder to add" }));
  });

  it("says when the snippets cannot be loaded", async () => {
    fakeBackend({
      library_items: () => {
        throw "The database is locked.";
      },
    });
    render(() => <Settings />);
    fireEvent.click(await screen.findByRole("button", { name: "Snippets" }));
    expect((await screen.findByRole("alert")).textContent).toContain("The database is locked.");
    expect(screen.queryByText(/No snippets yet/)).toBeNull();
  });

  it("says when the app details cannot be read", async () => {
    fakeBackend({
      about: () => {
        throw "No data folder.";
      },
    });
    render(() => <Settings />);
    fireEvent.click(await screen.findByRole("button", { name: "About" }));
    expect((await screen.findByRole("alert")).textContent).toContain("No data folder.");
  });

  it("stops recording and says why when the shortcut cannot be paused", async () => {
    fakeBackend({
      pause_shortcut: (args) => {
        if (args.paused) throw "Could not release the current shortcut.";
        return null;
      },
    });
    render(() => <Settings />);
    const recorder = await screen.findByRole("button", { name: /Launcher shortcut/ });
    fireEvent.click(recorder);
    expect((await screen.findByRole("alert")).textContent).toContain("Could not release");
    expect(recorder.textContent).not.toContain("Press keys");
  });

  it("says when a folder is already in the list", async () => {
    const backend = fakeBackend();
    render(() => <Settings />);
    fireEvent.click(await screen.findByRole("button", { name: "Files" }));
    const field = await screen.findByRole("textbox", { name: "Folder to add" });
    fireEvent.input(field, { target: { value: "~/Desktop" } });
    fireEvent.keyDown(field, { key: "Enter" });
    expect(screen.getByText("“~/Desktop” is already in the list.")).toBeTruthy();
    expect(backend.called("update_settings")).toHaveLength(0);
    fireEvent.click(screen.getByRole("button", { name: "Remove ~/Desktop" }));
    expect(screen.queryByText(/is already in the list/)).toBeNull();
  });

  it("describes each setting to screen readers", async () => {
    fakeBackend();
    render(() => <Settings />);
    fireEvent.click(await screen.findByRole("button", { name: "Clipboard" }));
    const row = await screen.findByRole("group", { name: "Save clipboard history" });
    const description = document.getElementById(row.getAttribute("aria-describedby") ?? "");
    expect(description?.textContent).toContain("unencrypted");
  });

  it("says when the shortcut cannot be turned back on", async () => {
    fakeBackend({
      pause_shortcut: (args) => {
        if (!args.paused) throw "Could not register Alt+Space.";
        return null;
      },
    });
    render(() => <Settings />);
    const recorder = await screen.findByRole("button", { name: /Launcher shortcut/ });
    fireEvent.click(recorder);
    fireEvent.keyDown(recorder, { key: " ", code: "Space", altKey: true });
    expect((await screen.findByRole("alert")).textContent).toContain(
      "Could not register Alt+Space",
    );
  });

  it("focuses the recorder on click and resumes only after the pause", async () => {
    const steps: string[] = [];
    let release!: () => void;
    const slow = new Promise<void>((resolve) => (release = resolve));
    fakeBackend({
      pause_shortcut: async (args) => {
        steps.push(args.paused ? "pause" : "resume");
        if (args.paused) {
          await slow;
          steps.push("paused");
        }
        return null;
      },
    });
    render(() => <Settings />);
    const recorder = await screen.findByRole("button", { name: /Launcher shortcut/ });
    fireEvent.click(recorder);
    expect(document.activeElement).toBe(recorder);
    fireEvent.keyDown(recorder, { key: "Escape" });
    release();
    await waitFor(() => expect(steps).toEqual(["pause", "paused", "resume"]));
  });

  it("records a new shortcut while the old one is paused", async () => {
    const backend = fakeBackend();
    render(() => <Settings />);
    const recorder = await screen.findByRole("button", { name: /Launcher shortcut/ });
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

  it("does not send a failed change again with the next one", async () => {
    let calls = 0;
    const backend = fakeBackend({
      update_settings: (args) => {
        calls += 1;
        if (calls === 1) throw "Could not change open at login.";
        return args.settings;
      },
    });
    render(() => <Settings />);
    fireEvent.click(await screen.findByRole("switch", { name: "Open at login" }));
    fireEvent.click(screen.getByRole("switch", { name: "Hide when another app is focused" }));
    await waitFor(() => expect(backend.called("update_settings")).toHaveLength(2));
    expect(backend.called("update_settings")[1]?.args).toEqual({
      settings: { ...testSettings, hideOnBlur: false },
    });
  });

  it("restores an emptied number instead of saving zero", async () => {
    const backend = fakeBackend();
    render(() => <Settings />);
    fireEvent.click(await screen.findByRole("button", { name: "Clipboard" }));
    const field = (await screen.findByRole("spinbutton", {
      name: "Entries to keep",
    })) as HTMLInputElement;
    fireEvent.input(field, { target: { value: "" } });
    fireEvent.blur(field);
    expect(field.value).toBe("200");
    fireEvent.input(field, { target: { value: "5000" } });
    fireEvent.blur(field);
    await waitFor(() => expect(backend.called("update_settings")).toHaveLength(1));
    expect(backend.called("update_settings")[0]?.args).toMatchObject({
      settings: { clipboardHistoryLimit: 1000 },
    });
  });

  it("saves a snippet once when Save is clicked twice", async () => {
    let release!: () => void;
    const slow = new Promise<void>((resolve) => (release = resolve));
    const backend = fakeBackend({
      save_library_item: async (args) => {
        await slow;
        return { ...(args.item as LibraryItem), id: 1 };
      },
    });
    render(() => <Settings />);
    fireEvent.click(await screen.findByRole("button", { name: "Snippets" }));
    fireEvent.input(await screen.findByLabelText("Name"), { target: { value: "Sig" } });
    fireEvent.input(screen.getByLabelText("Text"), { target: { value: "Regards" } });
    const save = screen.getByRole("button", { name: "Save Snippet" });
    fireEvent.click(save);
    fireEvent.click(save);
    release();
    await waitFor(() => expect(backend.called("library_items").length).toBeGreaterThan(1));
    expect(backend.called("save_library_item")).toHaveLength(1);
  });

  it("returns focus to Clear History after its dialog closes", async () => {
    fakeBackend();
    render(() => <Settings />);
    fireEvent.click(await screen.findByRole("button", { name: "Clipboard" }));
    const clear = await screen.findByRole("button", { name: "Clear History…" });
    fireEvent.click(clear);
    fireEvent.click(await screen.findByRole("button", { name: "Cancel" }));
    expect(document.activeElement).toBe(clear);
  });

  it("asks with the saved name and moves focus through the inline delete confirmation", async () => {
    const item: LibraryItem = { id: 3, kind: "snippet", name: "Sig", keyword: "", text: "Hi" };
    fakeBackend({ library_items: () => [item] });
    render(() => <Settings />);
    fireEvent.click(await screen.findByRole("button", { name: "Snippets" }));
    fireEvent.click(await screen.findByRole("button", { name: /Sig/ }));
    fireEvent.input(screen.getByRole("textbox", { name: "Name" }), { target: { value: "Other" } });
    fireEvent.click(await screen.findByRole("button", { name: "Delete" }));
    const question = screen.getByText("Delete “Sig”?");
    expect(document.activeElement?.textContent).toBe("Keep");
    expect(document.activeElement?.getAttribute("aria-describedby")).toBe(question.id);
    fireEvent.click(screen.getByRole("button", { name: "Keep" }));
    expect(document.activeElement?.textContent).toBe("Delete");
  });
});
