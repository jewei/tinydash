import { fireEvent, render, screen, waitFor, within } from "@solidjs/testing-library";
import { describe, expect, it } from "vite-plus/test";

import type { LibraryItem } from "../generated/LibraryItem";
import { testSettings, fakeBackend } from "../test/backend";
import { Settings } from "./Settings";

/** A fake library that lists what it saved, as the backend does. */
function fakeLibrary() {
  const items: LibraryItem[] = [];
  return {
    list: () => items,
    save: (item: LibraryItem, id: number) => {
      const saved = { ...item, id };
      items.push(saved);
      return saved;
    },
  };
}

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

  it("says when the app details cannot be read, and tries again on opening About", async () => {
    let fail = true;
    const backend = fakeBackend({
      about: () => {
        if (fail) throw "No data folder.";
        return { version: "0.2.0", settingsFolder: "/d", dataFolder: "/d", richClipboard: true };
      },
    });
    render(() => <Settings />);
    fireEvent.click(await screen.findByRole("button", { name: "About" }));
    expect((await screen.findByRole("alert")).textContent).toContain("No data folder.");
    expect(screen.queryByText(/Settings and saved data/)).toBeNull();
    fail = false;
    fireEvent.click(screen.getByRole("button", { name: "General" }));
    fireEvent.click(screen.getByRole("button", { name: "About" }));
    expect(await screen.findByText(/TinyDash 0\.2\.0/)).toBeTruthy();
    expect(screen.queryByRole("alert")).toBeNull();
    expect(backend.called("about")).toHaveLength(2);
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

  it.each([
    ["/data", "/data", ["Settings and saved data: /data"]],
    ["/config", "/local", ["Settings: /config", "Saved data: /local"]],
  ])("shows where settings (%s) and data (%s) are", async (settingsFolder, dataFolder, lines) => {
    fakeBackend({
      about: () => ({ version: "0.2.0", settingsFolder, dataFolder, richClipboard: true }),
    });
    render(() => <Settings />);
    fireEvent.click(await screen.findByRole("button", { name: "About" }));
    for (const line of lines) {
      expect(await screen.findByText((_, node) => node?.textContent === line)).toBeTruthy();
    }
  });

  it("cancels recording on a second click, and runs each request after the last", async () => {
    const steps: string[] = [];
    let release!: () => void;
    const slow = new Promise<void>((resolve) => (release = resolve));
    fakeBackend({
      pause_shortcut: async (args) => {
        steps.push(args.paused ? "pause" : "resume");
        if (args.paused && steps.length === 1) await slow;
        return null;
      },
    });
    render(() => <Settings />);
    const recorder = await screen.findByRole("button", { name: /Launcher shortcut/ });
    fireEvent.click(recorder);
    // The press keeps focus, so WebKit cannot blur and restart the recorder.
    expect(fireEvent.mouseDown(recorder)).toBe(false);
    fireEvent.click(recorder);
    expect(recorder.textContent).not.toContain("Press keys");
    fireEvent.click(recorder);
    release();
    await waitFor(() => expect(steps).toEqual(["pause", "resume", "pause"]));
    expect(recorder.textContent).toContain("Press keys");
  });

  it("offers image and file history only where the OS can save them", async () => {
    fakeBackend({
      about: () => ({
        version: "0.2.0",
        settingsFolder: "/c",
        dataFolder: "/d",
        richClipboard: false,
      }),
    });
    render(() => <Settings />);
    fireEvent.click(await screen.findByRole("button", { name: "Clipboard" }));
    await screen.findByRole("switch", { name: "Save clipboard history" });
    await waitFor(() => expect(screen.queryByRole("switch", { name: "Save images" })).toBeNull());
    expect(screen.queryByRole("switch", { name: "Save copied files" })).toBeNull();
  });

  it("leaves name and keyword limits to the backend, which counts characters", async () => {
    fakeBackend();
    render(() => <Settings />);
    fireEvent.click(await screen.findByRole("button", { name: "Snippets" }));
    for (const name of ["Name", /Keyword/]) {
      expect((await screen.findByRole("textbox", { name })).hasAttribute("maxlength")).toBe(false);
    }
  });

  it("keeps a refused folder in the field and says why next to it", async () => {
    fakeBackend({
      update_settings: () => {
        throw "“Projects” is not a full folder path.";
      },
    });
    render(() => <Settings />);
    fireEvent.click(await screen.findByRole("button", { name: "Files" }));
    const field = (await screen.findByRole("textbox", {
      name: "Folder to add",
    })) as HTMLInputElement;
    fireEvent.input(field, { target: { value: "Projects" } });
    fireEvent.keyDown(field, { key: "Enter" });
    const editor = within(field.closest(".list-editor") as HTMLElement);
    await waitFor(() =>
      expect(editor.getByRole("status").textContent).toContain("not a full folder path"),
    );
    expect(field.value).toBe("Projects");
  });

  it("applies each folder change to the saved list, so a refused entry is not sent again", async () => {
    let refuse!: () => void;
    const refused = new Promise<void>((resolve) => (refuse = resolve));
    const backend = fakeBackend({
      update_settings: async (args) => {
        const folders = (args.settings as typeof testSettings).fileSearchFolders;
        if (folders.includes("Projects")) {
          await refused;
          throw "“Projects” is not a full folder path.";
        }
        return args.settings;
      },
    });
    render(() => <Settings />);
    fireEvent.click(await screen.findByRole("button", { name: "Files" }));
    const field = await screen.findByRole("textbox", { name: "Folder to add" });
    fireEvent.input(field, { target: { value: "Projects" } });
    fireEvent.keyDown(field, { key: "Enter" });
    fireEvent.click(await screen.findByRole("button", { name: "Remove ~/Desktop" }));
    refuse();
    const editor = within(field.closest(".list-editor") as HTMLElement);
    await waitFor(() =>
      expect(editor.getByRole("status").textContent).toContain("not a full folder path"),
    );
    await waitFor(() => expect(backend.called("update_settings")).toHaveLength(2));
    expect(backend.called("update_settings")[1]?.args).toMatchObject({
      settings: { fileSearchFolders: ["~/Documents", "~/Downloads"] },
    });
  });

  it("shows a save error even when the list is full", async () => {
    const folders = Array.from({ length: 50 }, (_, index) => `~/Folder${index}`);
    fakeBackend({
      get_settings: () => ({ ...testSettings, fileSearchFolders: folders }),
      update_settings: () => {
        throw "“Projects” is not a full folder path.";
      },
    });
    render(() => <Settings />);
    fireEvent.click(await screen.findByRole("button", { name: "Files" }));
    fireEvent.click(await screen.findByRole("button", { name: "Remove ~/Folder3" }));
    const field = screen.getByRole("textbox", { name: "Folder to add" });
    const editor = within(field.closest(".list-editor") as HTMLElement);
    await waitFor(() =>
      expect(editor.getByRole("status").textContent).toContain("not a full folder path"),
    );
  });

  it("never sends an emoji language again after its save failed", async () => {
    let refuse!: () => void;
    const refused = new Promise<void>((resolve) => (refuse = resolve));
    const backend = fakeBackend({
      update_settings: async (args) => {
        const settings = args.settings as typeof testSettings;
        if (backend.called("update_settings").length === 1) {
          await refused;
          throw "Could not save settings.";
        }
        return settings;
      },
    });
    render(() => <Settings />);
    fireEvent.click(await screen.findByRole("button", { name: "Search" }));
    fireEvent.click(await screen.findByRole("switch", { name: "Chinese (Simplified)" }));
    fireEvent.click(screen.getByRole("switch", { name: "Malay" }));
    refuse();
    await waitFor(() => expect(backend.called("update_settings")).toHaveLength(2));
    expect(backend.called("update_settings")[1]?.args).toMatchObject({
      settings: { emojiLanguages: ["ms"] },
    });
  });

  it("saves a folder added during a failing save on its own", async () => {
    let refuse!: () => void;
    const refused = new Promise<void>((resolve) => (refuse = resolve));
    const backend = fakeBackend({
      update_settings: async (args) => {
        const settings = args.settings as typeof testSettings;
        if (settings.fileSearchFolders.includes("Projects")) {
          await refused;
          throw "“Projects” is not a full folder path.";
        }
        return settings;
      },
    });
    render(() => <Settings />);
    fireEvent.click(await screen.findByRole("button", { name: "Files" }));
    const field = (await screen.findByRole("textbox", {
      name: "Folder to add",
    })) as HTMLInputElement;
    fireEvent.input(field, { target: { value: "Projects" } });
    fireEvent.keyDown(field, { key: "Enter" });
    // Leaving the section and coming back must not change what is sent.
    fireEvent.click(screen.getByRole("button", { name: "General" }));
    fireEvent.click(screen.getByRole("button", { name: "Files" }));
    const again = (await screen.findByRole("textbox", {
      name: "Folder to add",
    })) as HTMLInputElement;
    fireEvent.input(again, { target: { value: "~/Work" } });
    fireEvent.keyDown(again, { key: "Enter" });
    refuse();
    await waitFor(() => expect(backend.called("update_settings")).toHaveLength(2));
    expect(backend.called("update_settings")[1]?.args).toMatchObject({
      settings: { fileSearchFolders: [...testSettings.fileSearchFolders, "~/Work"] },
    });
  });

  it("keeps typing that happens while a snippet saves", async () => {
    let release!: () => void;
    const slow = new Promise<void>((resolve) => (release = resolve));
    const library = fakeLibrary();
    fakeBackend({
      library_items: library.list,
      save_library_item: async (args) => {
        await slow;
        return library.save(args.item as LibraryItem, 9);
      },
    });
    render(() => <Settings />);
    fireEvent.click(await screen.findByRole("button", { name: "Snippets" }));
    const name = (await screen.findByRole("textbox", { name: "Name" })) as HTMLInputElement;
    const text = screen.getByRole("textbox", { name: "Text" }) as HTMLTextAreaElement;
    fireEvent.input(name, { target: { value: "Greeting" } });
    fireEvent.input(text, { target: { value: "Hello" } });
    fireEvent.click(screen.getByRole("button", { name: "Save Snippet" }));
    fireEvent.input(text, { target: { value: "Hello world" } });
    release();
    await screen.findByRole("button", { name: "Delete" });
    expect(text.value).toBe("Hello world");
  });

  it("leaves a new snippet opened during a save without the saved one's ID", async () => {
    let release!: () => void;
    const slow = new Promise<void>((resolve) => (release = resolve));
    const library = fakeLibrary();
    fakeBackend({
      library_items: library.list,
      save_library_item: async (args) => {
        await slow;
        return library.save(args.item as LibraryItem, 9);
      },
    });
    render(() => <Settings />);
    fireEvent.click(await screen.findByRole("button", { name: "Snippets" }));
    fireEvent.input(await screen.findByRole("textbox", { name: "Name" }), {
      target: { value: "Greeting" },
    });
    fireEvent.input(screen.getByRole("textbox", { name: "Text" }), { target: { value: "Hi" } });
    fireEvent.click(screen.getByRole("button", { name: "Save Snippet" }));
    fireEvent.click(screen.getByRole("button", { name: "New Snippet" }));
    release();
    await new Promise((resolve) => setTimeout(resolve, 10));
    expect(screen.queryByRole("button", { name: "Delete" })).toBeNull();
  });

  it("leaves an item opened during a delete as it is", async () => {
    let release!: () => void;
    const slow = new Promise<void>((resolve) => (release = resolve));
    const items: LibraryItem[] = [
      { id: 1, kind: "snippet", name: "Alpha", keyword: "", text: "A" },
      { id: 2, kind: "snippet", name: "Beta", keyword: "", text: "B" },
    ];
    fakeBackend({ library_items: () => items, delete_library_item: () => slow });
    render(() => <Settings />);
    fireEvent.click(await screen.findByRole("button", { name: "Snippets" }));
    fireEvent.click(await screen.findByRole("button", { name: /Alpha/ }));
    fireEvent.click(screen.getByRole("button", { name: "Delete" }));
    fireEvent.click(screen.getAllByRole("button", { name: "Delete" }).at(-1) as HTMLElement);
    fireEvent.click(screen.getByRole("button", { name: /Beta/ }));
    const text = screen.getByRole("textbox", { name: "Text" }) as HTMLTextAreaElement;
    fireEvent.input(text, { target: { value: "B edited" } });
    release();
    await new Promise((resolve) => setTimeout(resolve, 10));
    expect(text.value).toBe("B edited");
  });

  it("shows a failed save's error only with the item it came from", async () => {
    let refuse!: () => void;
    const refused = new Promise<void>((resolve) => (refuse = resolve));
    fakeBackend({
      library_items: () => [],
      save_library_item: async () => {
        await refused;
        throw "The keyword is in use.";
      },
    });
    render(() => <Settings />);
    fireEvent.click(await screen.findByRole("button", { name: "Snippets" }));
    fireEvent.input(await screen.findByRole("textbox", { name: "Name" }), {
      target: { value: "Greeting" },
    });
    fireEvent.input(screen.getByRole("textbox", { name: "Text" }), { target: { value: "Hi" } });
    fireEvent.click(screen.getByRole("button", { name: "Save Snippet" }));
    fireEvent.click(screen.getByRole("button", { name: "New Snippet" }));
    refuse();
    await new Promise((resolve) => setTimeout(resolve, 10));
    expect(screen.queryByText("The keyword is in use.")).toBeNull();
  });

  it("clears an old save error when a later save works", async () => {
    let calls = 0;
    let release!: () => void;
    const slow = new Promise<void>((resolve) => (release = resolve));
    const library = fakeLibrary();
    fakeBackend({
      library_items: library.list,
      save_library_item: async (args) => {
        calls += 1;
        if (calls === 1) throw "The keyword is in use.";
        await slow;
        return library.save(args.item as LibraryItem, 4);
      },
    });
    render(() => <Settings />);
    fireEvent.click(await screen.findByRole("button", { name: "Snippets" }));
    fireEvent.input(await screen.findByRole("textbox", { name: "Name" }), {
      target: { value: "Greeting" },
    });
    const text = screen.getByRole("textbox", { name: "Text" });
    fireEvent.input(text, { target: { value: "Hi" } });
    fireEvent.click(screen.getByRole("button", { name: "Save Snippet" }));
    await screen.findByText("The keyword is in use.");
    const saveButton = screen.getByRole("button", { name: "Save Snippet" }) as HTMLButtonElement;
    await waitFor(() => expect(saveButton.disabled).toBe(false));
    fireEvent.click(saveButton);
    fireEvent.input(text, { target: { value: "Hi there" } });
    release();
    await screen.findByRole("button", { name: "Delete" });
    expect(screen.queryByText("The keyword is in use.")).toBeNull();
  });

  it("keeps an item reopened during its delete as a new one", async () => {
    let release!: () => void;
    const slow = new Promise<void>((resolve) => (release = resolve));
    let items: LibraryItem[] = [{ id: 1, kind: "snippet", name: "Alpha", keyword: "", text: "A" }];
    const backend = fakeBackend({
      library_items: () => items,
      delete_library_item: async () => {
        await slow;
        items = [];
      },
      save_library_item: (args) => ({ ...(args.item as LibraryItem), id: 2 }),
    });
    render(() => <Settings />);
    fireEvent.click(await screen.findByRole("button", { name: "Snippets" }));
    fireEvent.click(await screen.findByRole("button", { name: /Alpha/ }));
    fireEvent.click(screen.getByRole("button", { name: "Delete" }));
    fireEvent.click(screen.getAllByRole("button", { name: "Delete" }).at(-1) as HTMLElement);
    fireEvent.click(screen.getByRole("button", { name: /Alpha/ }));
    release();
    await waitFor(() => expect(screen.queryByRole("button", { name: "Delete" })).toBeNull());
    expect((screen.getByRole("textbox", { name: "Text" }) as HTMLTextAreaElement).value).toBe("A");
    fireEvent.click(screen.getByRole("button", { name: "Save Snippet" }));
    await waitFor(() => expect(backend.called("save_library_item")).toHaveLength(1));
    expect(backend.called("save_library_item")[0]?.args).toMatchObject({ item: { id: null } });
  });

  it("rereads the list after a failed save, so a deleted item can be added again", async () => {
    let items: LibraryItem[] = [{ id: 1, kind: "snippet", name: "Alpha", keyword: "", text: "A" }];
    const backend = fakeBackend({
      library_items: () => items,
      save_library_item: (args) => {
        const item = args.item as LibraryItem;
        if (item.id !== null) {
          // Another window deleted it meanwhile.
          items = [];
          throw "This item was deleted, so it was not saved. Save again to add it as a new item.";
        }
        return { ...item, id: 2 };
      },
    });
    render(() => <Settings />);
    fireEvent.click(await screen.findByRole("button", { name: "Snippets" }));
    fireEvent.click(await screen.findByRole("button", { name: /Alpha/ }));
    fireEvent.click(screen.getByRole("button", { name: "Save Snippet" }));
    await screen.findByText(/This item was deleted/);
    await waitFor(() => expect(screen.queryByRole("button", { name: "Delete" })).toBeNull());
    const saveButton = screen.getByRole("button", { name: "Save Snippet" }) as HTMLButtonElement;
    await waitFor(() => expect(saveButton.disabled).toBe(false));
    fireEvent.click(saveButton);
    await waitFor(() => expect(backend.called("save_library_item")).toHaveLength(2));
    expect(backend.called("save_library_item")[1]?.args).toMatchObject({ item: { id: null } });
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
