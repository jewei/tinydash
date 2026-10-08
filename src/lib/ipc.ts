// The only module that talks to the Rust backend. Each function wraps one
// command in src-tauri/src/commands.rs; event names match events.rs. Types
// in ../generated are written by ts-rs from the Rust types, so a change on
// either side shows up as a type error here.
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import type { About } from "../generated/About";
import type { Action } from "../generated/Action";
import type { Category } from "../generated/Category";
import type { HiddenResult } from "../generated/HiddenResult";
import type { LauncherInit } from "../generated/LauncherInit";
import type { LauncherShown } from "../generated/LauncherShown";
import type { LibraryItem } from "../generated/LibraryItem";
import type { Preview } from "../generated/Preview";
import type { SearchResult } from "../generated/SearchResult";
import type { Settings } from "../generated/Settings";
import type { Widgets } from "../generated/Widgets";

export const launcherInit = () => invoke<LauncherInit>("launcher_init");

export const search = (query: string, category: Category) =>
  invoke<SearchResult[]>("search", { query, category });

/** More Files tab results from Spotlight (macOS), after the index's own. */
export const spotlightFiles = (query: string) =>
  invoke<SearchResult[]>("spotlight_files", { query });

/** Pass `resultId` only with a result's primary action; it feeds usage ranking. */
export const runAction = (action: Action, resultId?: string) =>
  invoke<void>("run_action", { action, resultId: resultId ?? null });

export const preview = (id: string) => invoke<Preview | null>("preview", { id });

export const hideLauncher = () => invoke<void>("hide_launcher");

/** Moves the launcher with the pointer; call on a mouse-down. */
export const dragLauncher = () => invoke<void>("drag_launcher");

export const getSettings = () => invoke<Settings>("get_settings");

/**
 * Saves only the given fields, on top of what the backend holds. Resolves to
 * the saved, normalized settings; rejects with the reason when nothing was saved.
 */
export const updateSettings = (changes: Partial<Settings>) =>
  invoke<Settings>("update_settings", { changes });

export const pauseShortcut = (paused: boolean) => invoke<void>("pause_shortcut", { paused });

export const libraryItems = () => invoke<LibraryItem[]>("library_items");

export const saveLibraryItem = (item: LibraryItem) =>
  invoke<LibraryItem>("save_library_item", { item });

export const deleteLibraryItem = (id: number) => invoke<void>("delete_library_item", { id });

export const about = () => invoke<About>("about");

/** Results the user hid, by name. */
export const hiddenResults = () => invoke<HiddenResult[]>("hidden_results");

export const unhideResult = (id: string) => invoke<void>("unhide_result", { id });

/** Resolves to the newer version, or `null` when this one is the latest. */
export const checkForUpdate = () => invoke<string | null>("check_for_update");

/** Installs the version the latest check found; the app then restarts. */
export const installUpdate = () => invoke<void>("install_update");

/** What the widget pane shows now; a widget that is off is `null`. */
export const widgets = () => invoke<Widgets>("widgets");

/** Save the scratch note; rejects with the reason when it was not saved. */
export const saveNote = (text: string) => invoke<void>("save_note", { text });

/** A check finished: the newer version, or `null` when this one is the latest. */
export const onUpdateChanged = (handler: (version: string | null) => void) =>
  listen<string | null>("update:changed", (event) => handler(event.payload));

export const onLauncherShown = (handler: (event: LauncherShown) => void) =>
  listen<LauncherShown>("launcher:shown", (event) => handler(event.payload));

export const onResultsStale = (handler: () => void) => listen("results:stale", () => handler());

/** A widget changed on its own, such as the focus timer at the end of a phase. */
export const onWidgetsChanged = (handler: () => void) => listen("widgets:changed", () => handler());

/** The text for the message window, such as "Copied". */
export const onHudMessage = (handler: (message: string) => void) =>
  listen<string>("hud:show", (event) => handler(event.payload));

export const onSettingsChanged = (handler: (settings: Settings) => void) =>
  listen<Settings>("settings:changed", (event) => handler(event.payload));

/** URL of a file's system icon at `pixels` square. */
export const iconUrl = (path: string, pixels: number) =>
  `${convertFileSrc(path, "icon")}?size=${Math.round(pixels)}`;

/** URL of a saved clipboard image. */
export const clipboardImageUrl = (id: number) => convertFileSrc(String(id), "clip");

/** Command errors arrive as strings; anything else is unexpected. */
export const message = (error: unknown) =>
  typeof error === "string" ? error : error instanceof Error ? error.message : String(error);
