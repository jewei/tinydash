import {
  createEffect,
  createMemo,
  createSignal,
  For,
  onCleanup,
  onSettled,
  Show,
} from "solid-js";
import { isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { createNativeSubscriptions } from "./nativeSubscriptions";
import {
  folderModeFor,
  mergeSettingsDraft,
  type FolderMode,
} from "./settingsDraft";
import {
  backend,
  type SearchMode,
  type SettingsImport,
  type SettingsInfo,
  type SettingsValues,
  type UpdateStatus,
} from "./bridge";
import {
  appearances,
  isAppearance,
  readAppearance,
  readCompact,
  readFollowSystemGlass,
  saveAppearance,
  saveCompact,
  saveFollowSystemGlass,
  watchAppearance,
} from "./appearance";
import ConfirmDialog from "./components/ConfirmDialog";
import ItemPreferences from "./components/ItemPreferences";
import Icon from "./components/Icon";
import {
  AppPreferences,
  WebSearchPreferences,
} from "./components/SearchPreferences";
import {
  categories,
  defaultCategories,
  normalizeCategories,
} from "./categories";
import appIconUrl from "../app-icon.svg";
import "./styles/settings.css";

const sections = [
  {
    id: "shortcut",
    label: "Shortcut",
    description: "Open TinyDash from anywhere.",
  },
  {
    id: "appearance",
    label: "Appearance",
    description: "Choose how your launcher looks.",
  },
  {
    id: "search",
    label: "Search",
    description: "Choose app names and web searches.",
  },
  {
    id: "categories",
    label: "Categories",
    description: "Choose the categories shown in the launcher.",
  },
  {
    id: "clipboard",
    label: "Clipboard history",
    description: "Keep the text you want to use again.",
  },
  {
    id: "files",
    label: "File search",
    description: "Choose which folders TinyDash searches.",
  },
  {
    id: "currency",
    label: "Currency",
    description: "Control updates to saved exchange rates.",
  },
  {
    id: "privacy",
    label: "Privacy",
    description: "See what TinyDash stores on this computer.",
  },
  {
    id: "about",
    label: "About",
    description: "A few keys to your apps and everyday tasks.",
  },
] as const;
type Section = (typeof sections)[number]["id"];
type ShortcutTarget = "global" | SearchMode | `item:${string}`;
const sectionKeywords: Record<Section, string> = {
  shortcut: "keyboard hotkey startup login blur reset menu bar",
  appearance: "theme dark light compact glass transparency",
  search: "aliases applications hidden shortcuts commands web keywords",
  categories: "tabs visible hide providers",
  clipboard:
    "history copy paste images files exclusions privacy retention days",
  files: "index folders roots ignore patterns hidden watch preview",
  currency: "exchange rates offline updates",
  privacy: "backup import export recovery data storage",
  about: "version update license",
};
const lines = (text: string) => [
  ...new Set(
    text
      .split(/\r?\n/)
      .map((line) => line.trim())
      .filter(Boolean),
  ),
];
function Toggle(props: {
  label: string;
  hint: string;
  checked: boolean;
  onChange: (value: boolean) => void;
}) {
  return (
    <label class="settings-row settings-toggle-row">
      <span>
        <strong>{props.label}</strong>
        <span class="settings-hint">{props.hint}</span>
      </span>
      <input
        type="checkbox"
        role="switch"
        aria-label={props.label}
        checked={props.checked}
        onChange={(event) => props.onChange(event.currentTarget.checked)}
      />
    </label>
  );
}

export default function Settings() {
  const desktop = isTauri();
  const [info, setInfo] = createSignal<SettingsInfo>();
  const [draft, setDraft] = createSignal<SettingsValues>();
  const [saved, setSaved] = createSignal<SettingsValues>();
  const [section, setSection] = createSignal<Section>("shortcut");
  const [settingsQuery, setSettingsQuery] = createSignal("");
  const matchingSections = createMemo(() => {
    const words = settingsQuery()
      .toLocaleLowerCase()
      .trim()
      .split(/\s+/)
      .filter(Boolean);
    return sections.filter((item) =>
      words.every((word) =>
        `${item.label} ${item.description} ${sectionKeywords[item.id]}`
          .toLocaleLowerCase()
          .includes(word),
      ),
    );
  });
  const [appearance, setAppearance] = createSignal(readAppearance());
  const [savedAppearance, setSavedAppearance] = createSignal(readAppearance());
  const [compact, setCompact] = createSignal(readCompact());
  const [savedCompact, setSavedCompact] = createSignal(readCompact());
  const [followSystemGlass, setFollowSystemGlass] = createSignal(
    readFollowSystemGlass(),
  );
  const [savedSystemGlass, setSavedSystemGlass] = createSignal(
    readFollowSystemGlass(),
  );
  const [folderMode, setFolderMode] = createSignal<FolderMode>("default");
  const [foldersText, setFoldersText] = createSignal("");
  const [excludedText, setExcludedText] = createSignal("");
  const [saving, setSaving] = createSignal(false);
  const [recording, setRecording] = createSignal(false);
  const [preparing, setPreparing] = createSignal(false);
  const [recordingTarget, setRecordingTarget] =
    createSignal<ShortcutTarget | null>(null);
  const [clearOpen, setClearOpen] = createSignal(false);
  const [clearKeepPinned, setClearKeepPinned] = createSignal(true);
  const [clearing, setClearing] = createSignal(false);
  const [error, setError] = createSignal<string>();
  const [dialogError, setDialogError] = createSignal<string>();
  const [status, setStatus] = createSignal("");
  const [pendingImport, setPendingImport] = createSignal<SettingsImport>();
  const [importing, setImporting] = createSignal(false);
  const [updateBusy, setUpdateBusy] = createSignal(false);
  const [updateStatus, setUpdateStatus] = createSignal<UpdateStatus>();
  let disposed = false;
  let recordingSequence = 0;
  let recorder!: HTMLButtonElement;
  let content!: HTMLDivElement;
  const subscriptions = createNativeSubscriptions(listen);
  const value = () => draft()!;
  const currentSection = () => sections.find((item) => item.id === section())!;
  const dirty = createMemo(
    () =>
      !!draft() &&
      (JSON.stringify(draft()) !== JSON.stringify(saved()) ||
        folderMode() !== folderModeFor(saved()!) ||
        appearance() !== savedAppearance() ||
        compact() !== savedCompact() ||
        followSystemGlass() !== savedSystemGlass()),
  );
  const modifier = () => (info()?.platform === "macos" ? "⌘" : "Ctrl");
  const shortcutKeys = () => shortcutKeysFor("global");
  function shortcutValue(target: ShortcutTarget): string {
    if (target === "global") return draft()?.shortcut ?? "";
    return (
      draft()?.categoryShortcuts.find((binding) => binding.mode === target)
        ?.shortcut ?? ""
    );
  }
  function shortcutKeysFor(target: ShortcutTarget) {
    return shortcutValue(target)
      .split("+")
      .filter(Boolean)
      .map((key) => {
        if (key === "Super" || key === "Command" || key === "Meta")
          return info()?.platform === "macos" ? "Command" : "Windows";
        if (key === "Alt")
          return info()?.platform === "macos" ? "Option" : "Alt";
        if (key === "CommandOrControl")
          return info()?.platform === "macos" ? "Command" : "Control";
        return key.replace(/^(Key|Digit)/, "");
      });
  }

  createEffect(appearance, (value) => {
    document.documentElement.dataset.appearance = value;
  });

  function resetDraft(settings: SettingsValues) {
    setDraft({
      ...structuredClone(settings),
      visibleCategories: normalizeCategories(settings.visibleCategories),
    });
    setFolderMode(folderModeFor(settings));
    setFoldersText(settings.fileSearchRoots?.join("\n") ?? "");
    setExcludedText(settings.fileSearchExcludedDirs.join("\n"));
  }

  function receiveSettings(settings: SettingsValues) {
    const merged = mergeSettingsDraft(saved(), draft(), settings, folderMode());
    setInfo((info) => (info ? { ...info, settings: merged.saved } : info));
    setSaved(merged.saved);
    setDraft(merged.draft);
    if (merged.updateFolders) {
      setFolderMode(folderModeFor(merged.draft));
      setFoldersText(merged.draft.fileSearchRoots?.join("\n") ?? "");
    }
    if (merged.updateExcluded) {
      setExcludedText(merged.draft.fileSearchExcludedDirs.join("\n"));
    }
  }
  function setShortcut(target: ShortcutTarget, shortcut: string) {
    if (target === "global") {
      field("shortcut", shortcut);
      return;
    }
    if (target.startsWith("item:")) {
      const id = target.slice(5);
      const item = value().itemPreferences[id] ?? {
        aliases: [],
        shortcut: "",
        hidden: false,
        disabled: false,
      };
      field("itemPreferences", {
        ...value().itemPreferences,
        [id]: { ...item, shortcut },
      });
      return;
    }
    const bindings = value().categoryShortcuts.filter(
      (binding) => binding.mode !== target,
    );
    field("categoryShortcuts", [
      ...bindings,
      { mode: target as SearchMode, shortcut },
    ]);
  }
  function field<K extends keyof SettingsValues>(
    key: K,
    next: SettingsValues[K],
  ) {
    setDraft((previous) =>
      previous ? { ...previous, [key]: next } : previous,
    );
    setError(undefined);
    setStatus("");
  }
  async function load() {
    setError(undefined);
    if (!desktop) {
      setError("Open the desktop app to change settings.");
      return;
    }
    try {
      const response = await backend.settings();
      if (disposed) return;
      setInfo(response);
      receiveSettings(response.settings);
    } catch (reason) {
      if (!disposed) setError(String(reason));
    }
  }

  async function stopRecording() {
    recordingSequence += 1;
    if (!recording() && !preparing()) return;
    setRecording(false);
    setRecordingTarget(null);
    setPreparing(true);
    try {
      await backend.recordShortcut(false);
    } catch (reason) {
      if (!disposed) setError(String(reason));
    } finally {
      if (!disposed) setPreparing(false);
    }
  }
  async function startRecording(target: ShortcutTarget = "global") {
    if (recording()) {
      await stopRecording();
      return;
    }
    const request = ++recordingSequence;
    setError(undefined);
    setPreparing(true);
    try {
      await backend.recordShortcut(true);
      if (disposed || request !== recordingSequence) {
        await backend.recordShortcut(false);
        return;
      }
      setRecording(true);
      setRecordingTarget(target);
      recorder?.focus();
    } catch (reason) {
      if (!disposed) setError(String(reason));
    } finally {
      if (!disposed && request === recordingSequence) setPreparing(false);
    }
  }
  function navigate(next: Section) {
    void stopRecording();
    setSection(next);
    content?.scrollTo(0, 0);
  }
  function validate(): boolean {
    const settings = value();
    if (!settings.visibleCategories.length) {
      setSection("categories");
      setError("Select at least one category.");
      return false;
    }
    if (
      !Number.isInteger(settings.clipboardHistoryLimit) ||
      settings.clipboardHistoryLimit < 1 ||
      settings.clipboardHistoryLimit > 500
    ) {
      setSection("clipboard");
      setError("Set the history limit to a whole number from 1 to 500.");
      return false;
    }
    if (
      !Number.isInteger(settings.fileSearchLimit) ||
      settings.fileSearchLimit < 1 ||
      settings.fileSearchLimit > 100000
    ) {
      setSection("files");
      setError("Set the file limit to a whole number from 1 to 100,000.");
      return false;
    }
    if (folderMode() === "custom" && !settings.fileSearchRoots?.length) {
      setSection("files");
      setError("Add at least one search folder, or turn file search off.");
      return false;
    }
    return true;
  }
  async function save() {
    if (!dirty() || saving() || recording() || preparing() || !validate())
      return;
    setSaving(true);
    setError(undefined);
    setStatus("");
    try {
      const settings = await backend.saveSettings(value());
      if (disposed) return;
      if (appearance() !== savedAppearance()) {
        saveAppearance(appearance());
        setSavedAppearance(appearance());
      }
      if (compact() !== savedCompact()) {
        saveCompact(compact());
        setSavedCompact(compact());
      }
      if (followSystemGlass() !== savedSystemGlass()) {
        saveFollowSystemGlass(followSystemGlass());
        setSavedSystemGlass(followSystemGlass());
      }
      setSaved(settings);
      resetDraft(settings);
      setStatus("Changes saved.");
    } catch (reason) {
      if (!disposed) setError(String(reason));
    } finally {
      if (!disposed) setSaving(false);
    }
  }
  async function previewImport() {
    if (importing()) return;
    setImporting(true);
    setError(undefined);
    try {
      const imported = await backend.importSettings();
      if (imported) setPendingImport(imported);
      else setStatus("No settings file was selected.");
    } catch (reason) {
      setError(String(reason));
    } finally {
      setImporting(false);
    }
  }
  function applyImport() {
    const imported = pendingImport();
    if (!imported) return;
    resetDraft(imported.settings);
    if (isAppearance(imported.appearance)) setAppearance(imported.appearance);
    else if (imported.appearance === "compact") {
      setAppearance("light");
      setCompact(true);
    }
    if (typeof imported.compact === "boolean") setCompact(imported.compact);
    if (typeof imported.followSystemGlass === "boolean")
      setFollowSystemGlass(imported.followSystemGlass);
    setPendingImport(undefined);
    setStatus("Import preview applied. Save changes to keep it.");
  }
  async function exportCurrentSettings() {
    setError(undefined);
    try {
      const exported = await backend.exportSettings(
        savedAppearance(),
        savedCompact(),
        savedSystemGlass(),
      );
      setStatus(exported ? "Saved settings exported." : "Export canceled.");
    } catch (reason) {
      setError(String(reason));
    }
  }
  async function revealBackup() {
    try {
      await backend.revealBackup();
      setStatus("Backup folder opened.");
    } catch (reason) {
      setError(String(reason));
    }
  }
  async function checkUpdate() {
    if (updateBusy()) return;
    setUpdateBusy(true);
    setError(undefined);
    try {
      setUpdateStatus(await backend.checkUpdate());
    } catch (reason) {
      setError(String(reason));
    } finally {
      setUpdateBusy(false);
    }
  }
  async function installUpdate() {
    if (updateBusy()) return;
    setUpdateBusy(true);
    setError(undefined);
    try {
      await backend.installUpdate();
      setUpdateStatus(undefined);
      setStatus("Update installed. Restart TinyDash when ready.");
    } catch (reason) {
      setError(String(reason));
    } finally {
      setUpdateBusy(false);
    }
  }
  async function clearHistory(keepPinned = false) {
    if (clearing()) return;
    setClearing(true);
    setDialogError(undefined);
    try {
      await backend.clearClipboard(keepPinned);
      setClearOpen(false);
      setStatus(
        keepPinned
          ? "Unpinned clipboard entries cleared."
          : "Clipboard history cleared.",
      );
    } catch (reason) {
      setDialogError(String(reason));
    } finally {
      setClearing(false);
    }
  }
  function onKey(event: KeyboardEvent) {
    if (event.isComposing || event.keyCode === 229 || clearOpen()) return;
    if (recording()) {
      if (event.key === "Tab") {
        void stopRecording();
        return;
      }
      event.preventDefault();
      event.stopPropagation();
      if (event.key === "Escape") {
        void stopRecording();
        return;
      }
      if (
        ["Control", "Alt", "Meta", "Shift"].includes(event.key) ||
        event.repeat
      )
        return;
      if (
        (!event.ctrlKey &&
          !event.altKey &&
          !event.metaKey &&
          !(event.shiftKey && event.code === "Space")) ||
        event.getModifierState("AltGraph")
      ) {
        setError(
          "Hold Control, Option / Alt, or Command / Windows and press one key. You can also use Shift+Space.",
        );
        return;
      }
      if (
        !/^(Key[A-Z]|Digit[0-9]|F([1-9]|1[0-9]|2[0-4])|Space|Enter|Backspace|Delete|Insert|Home|End|PageUp|PageDown|Arrow(Up|Down|Left|Right)|Comma|Period|Slash|Semicolon|Quote|Backquote|Minus|Equal|BracketLeft|BracketRight|Backslash|Numpad[0-9])$/.test(
          event.code,
        )
      ) {
        setError("This key is not supported. Try a letter, number, or Space.");
        return;
      }
      const keys = [
        event.ctrlKey && "Control",
        event.altKey && "Alt",
        event.shiftKey && "Shift",
        event.metaKey && "Super",
        event.code,
      ].filter(Boolean);
      setShortcut(recordingTarget() ?? "global", keys.join("+"));
      void stopRecording();
      return;
    }
    const command =
      info()?.platform === "macos" ? event.metaKey : event.ctrlKey;
    if (command && event.key.toLowerCase() === "s") {
      event.preventDefault();
      void save();
    }
  }
  const onBlur = () => {
    void stopRecording();
  };
  onSettled(() => {
    document.title = "TinyDash Settings";
    document.addEventListener("keydown", onKey, true);
    window.addEventListener("blur", onBlur);
    if (desktop) {
      void subscriptions
        .register<SettingsValues>("settings-changed", receiveSettings)
        .then((active) => {
          if (active) return load();
        })
        .catch((reason) => {
          if (!disposed) setError(String(reason));
        });
    } else {
      void load();
    }
    void subscriptions
      .own(
        watchAppearance(
          subscriptions.guard((value) => {
            setAppearance(value);
            setSavedAppearance(value);
          }),
          subscriptions.guard((value) => {
            setCompact(value);
            setSavedCompact(value);
          }),
          subscriptions.guard((value) => {
            setFollowSystemGlass(value);
            setSavedSystemGlass(value);
          }),
        ),
      )
      .catch((reason) => {
        if (!disposed) setError(String(reason));
      });
    if (desktop)
      void subscriptions
        .register<string>("shortcut-error", setError)
        .catch((reason) => {
          if (!disposed) setError(String(reason));
        });
  });
  onCleanup(() => {
    void stopRecording();
    disposed = true;
    subscriptions.dispose();
    document.removeEventListener("keydown", onKey, true);
    window.removeEventListener("blur", onBlur);
  });

  return (
    <main class="settings-shell" aria-label="TinyDash settings">
      <aside class="settings-sidebar">
        <div class="settings-brand">
          <img
            class="tiny-mark"
            src={appIconUrl}
            alt=""
            width="32"
            height="32"
          />
          TinyDash
        </div>
        <label class="settings-field-label">
          Search settings
          <input
            type="search"
            aria-label="Search settings"
            value={settingsQuery()}
            onInput={(event) => setSettingsQuery(event.currentTarget.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter" && matchingSections()[0]) {
                event.preventDefault();
                navigate(matchingSections()[0].id);
              }
            }}
          />
        </label>
        <Show when={!matchingSections().length}>
          <p role="status">No matching settings.</p>
        </Show>
        <nav aria-label="Settings sections">
          <For each={matchingSections()}>
            {(item) => (
              <button
                type="button"
                aria-current={section() === item.id ? "page" : undefined}
                onClick={() => navigate(item.id)}
              >
                {item.label}
              </button>
            )}
          </For>
        </nav>
        <label class="settings-mobile-nav">
          <select
            aria-label="Settings section"
            value={section()}
            onChange={(event) => navigate(event.currentTarget.value as Section)}
          >
            <For each={sections}>
              {(item) => <option value={item.id}>{item.label}</option>}
            </For>
          </select>
        </label>
        <p class="settings-sidebar-note">Open Settings with {modifier()} ,</p>
      </aside>
      <form
        class="settings-main"
        novalidate
        onSubmit={(event) => {
          event.preventDefault();
          void save();
        }}
      >
        <div class="settings-scroll" ref={content}>
          <header class="settings-heading">
            <span class="settings-eyebrow">Settings</span>
            <h1>{currentSection().label}</h1>
            <p>{currentSection().description}</p>
          </header>
          <Show
            when={info() && draft()}
            fallback={
              <div class="settings-loading" role="status">
                {error() ?? "Loading settings..."}
                <Show when={error() && desktop}>
                  <button
                    class="settings-button"
                    type="button"
                    onClick={() => void load()}
                  >
                    Try again
                  </button>
                </Show>
              </div>
            }
          >
            <fieldset class="settings-fields" disabled={saving()}>
              <Show when={section() === "shortcut"}>
                <div class="settings-group">
                  <h2>Launch shortcut</h2>
                  <div
                    class={{
                      "shortcut-recorder": true,
                      "is-recording": recording(),
                    }}
                  >
                    <div class="shortcut-keys" aria-live="polite">
                      <Show
                        when={!recording()}
                        fallback={<span>Press your new shortcut...</span>}
                      >
                        <For each={shortcutKeys()}>
                          {(key) => <kbd>{key}</kbd>}
                        </For>
                      </Show>
                    </div>
                    <button
                      ref={recorder}
                      class="settings-button"
                      type="button"
                      disabled={preparing() || !info()?.shortcutsAvailable}
                      onClick={() => void startRecording()}
                    >
                      {preparing()
                        ? "Please wait..."
                        : recording()
                          ? "Cancel"
                          : "Record new"}
                    </button>
                  </div>
                  <p class="settings-note">
                    {!info()?.shortcutsAvailable
                      ? "Global shortcuts are unavailable. On Wayland, set a desktop shortcut that starts TinyDash."
                      : recording()
                        ? "Hold a modifier and press one key. Press Escape to cancel."
                        : "Click Record new, then press the keys you want to use. Shift+Space is also supported."}
                  </p>
                  <button
                    type="button"
                    class="settings-link"
                    disabled={
                      recording() ||
                      preparing() ||
                      value().shortcut === info()?.defaults.shortcut
                    }
                    onClick={() => field("shortcut", info()!.defaults.shortcut)}
                  >
                    Use default shortcut
                  </button>
                </div>
                <div class="settings-group">
                  <h2>When the launcher opens</h2>
                  <Toggle
                    label="Hide when another app takes focus"
                    hint="Settings stays open while you use other apps."
                    checked={value().hideOnBlur}
                    onChange={(next) => field("hideOnBlur", next)}
                  />
                  <Toggle
                    label="Reset search and category on open"
                    hint="Clear the query and return to the first visible category. Turn this off to keep your query and category."
                    checked={value().clearQueryOnOpen}
                    onChange={(next) => field("clearQueryOnOpen", next)}
                  />
                </div>
                <Show when={info()?.platform === "macos"}>
                  <Toggle
                    label="Show menu bar icon"
                    hint="The global shortcut remains available when the icon is hidden."
                    checked={value().showMenuBarIcon}
                    onChange={(next) => field("showMenuBarIcon", next)}
                  />
                </Show>
                <div class="settings-group">
                  <h2>Category shortcuts</h2>
                  <p>
                    Give a visible category its own shortcut. Each shortcut must
                    be unique. Category shortcuts open an empty search.
                  </p>
                  <Show when={!info()?.shortcutsAvailable}>
                    <p class="settings-hint">
                      On Wayland, set a desktop shortcut to run{" "}
                      <code>tinydash --mode clipboard</code>. Replace clipboard
                      with the category name.
                    </p>
                  </Show>
                  <For
                    each={categories.filter(
                      (category) => category.id !== "all",
                    )}
                  >
                    {(category) => {
                      const target = category.id as SearchMode;
                      const binding = () =>
                        value().categoryShortcuts.find(
                          (item) => item.mode === target,
                        );
                      return (
                        <div class="shortcut-category-row">
                          <span>
                            <strong>{category.label}</strong>
                            <span class="settings-hint">
                              {binding()
                                ? shortcutKeysFor(target).join(" + ")
                                : "No shortcut"}
                            </span>
                          </span>
                          <button
                            type="button"
                            class="settings-button"
                            disabled={
                              preparing() ||
                              !info()?.shortcutsAvailable ||
                              !value().visibleCategories.includes(target)
                            }
                            onClick={() => void startRecording(target)}
                          >
                            {recordingTarget() === target
                              ? "Cancel"
                              : binding()
                                ? "Change"
                                : "Record"}
                          </button>
                          <Show when={binding()}>
                            <button
                              type="button"
                              class="settings-button"
                              disabled={recording() || preparing()}
                              aria-label={`Remove ${category.label} shortcut`}
                              onClick={() =>
                                field(
                                  "categoryShortcuts",
                                  value().categoryShortcuts.filter(
                                    (item) => item.mode !== target,
                                  ),
                                )
                              }
                            >
                              Remove
                            </button>
                          </Show>
                        </div>
                      );
                    }}
                  </For>
                </div>
                <Toggle
                  label="Start TinyDash when you sign in"
                  hint="Open one launcher process after you sign in."
                  checked={value().startAtLogin}
                  onChange={(next) => field("startAtLogin", next)}
                />
              </Show>
              <Show when={section() === "appearance"}>
                <div class="settings-group">
                  <h2>Theme</h2>
                  <div
                    class="settings-appearances"
                    role="radiogroup"
                    aria-label="Theme"
                  >
                    <For each={appearances}>
                      {(item) => (
                        <label
                          class={{
                            "style-option": true,
                            "is-selected": appearance() === item.id,
                          }}
                        >
                          <input
                            type="radio"
                            name="appearance"
                            value={item.id}
                            checked={appearance() === item.id}
                            onChange={() => {
                              setAppearance(item.id);
                              saveAppearance(item.id);
                              setSavedAppearance(item.id);
                              setStatus("Appearance saved.");
                            }}
                          />
                          <span
                            class={`style-preview preview-${item.id}`}
                            aria-hidden="true"
                          >
                            <span class="style-search" />
                            <span class="style-results">
                              <i />
                              <i />
                              <i />
                            </span>
                            <span class="style-detail" />
                          </span>
                          <span class="style-label">
                            {item.label}
                            <span aria-hidden="true">
                              {appearance() === item.id ? "✓" : ""}
                            </span>
                          </span>
                          <span class="settings-hint">{item.description}</span>
                        </label>
                      )}
                    </For>
                  </div>
                  <p class="settings-note">
                    Theme changes apply at once in both windows.
                  </p>
                </div>
                <Toggle
                  label="Compact layout"
                  hint="Use shorter rows and a single result column with any theme."
                  checked={compact()}
                  onChange={(next) => {
                    setCompact(next);
                    saveCompact(next);
                    setSavedCompact(next);
                    setStatus("Layout saved.");
                  }}
                />
                <Show when={info()?.platform === "macos"}>
                  <Toggle
                    label="Follow macOS Liquid Glass"
                    hint="Use the macOS glass settings. Turn off for a solid theme background."
                    checked={followSystemGlass()}
                    onChange={(next) => {
                      setFollowSystemGlass(next);
                      saveFollowSystemGlass(next);
                      setSavedSystemGlass(next);
                      setStatus("Liquid Glass preference saved.");
                    }}
                  />
                  <div class="settings-group">
                    <h2>Liquid Glass</h2>
                    <p class="settings-note">
                      When enabled on macOS 26 or later, the launcher uses
                      native Liquid Glass. On macOS 27, adjust Liquid Glass in
                      System Settings, Appearance. macOS controls the blur and
                      transparency and follows your accessibility settings.
                      Earlier versions use a solid background.
                    </p>
                  </div>
                </Show>
              </Show>
              <Show when={section() === "search"}>
                <ItemPreferences
                  value={value().itemPreferences}
                  onChange={(next) => field("itemPreferences", next)}
                  onRecord={(id) => void startRecording(`item:${id}`)}
                  recording={recordingTarget()}
                />
                <div class="settings-group">
                  <h2>Application names</h2>
                  <AppPreferences
                    value={value().appPreferences}
                    onChange={(next) => field("appPreferences", next)}
                  />
                </div>
                <div class="settings-group settings-separated">
                  <h2>Web searches</h2>
                  <WebSearchPreferences
                    value={value().webSearches}
                    onChange={(next) => field("webSearches", next)}
                  />
                </div>
              </Show>
              <Show when={section() === "categories"}>
                <div class="settings-group">
                  <p>
                    Tab selects the next visible category. Shift+Tab selects the
                    previous one.
                  </p>
                  <fieldset
                    class="category-choices"
                    aria-describedby="category-selection-note"
                  >
                    <legend>Visible categories</legend>
                    <div class="category-choice-grid">
                      <For each={categories}>
                        {(category) => (
                          <label
                            class={{
                              "category-choice": true,
                              "is-selected": value().visibleCategories.includes(
                                category.id,
                              ),
                            }}
                          >
                            <input
                              type="checkbox"
                              name="visibleCategories"
                              value={category.id}
                              checked={value().visibleCategories.includes(
                                category.id,
                              )}
                              disabled={
                                value().visibleCategories.length === 1 &&
                                value().visibleCategories.includes(category.id)
                              }
                              onChange={(event) => {
                                const selected = new Set(
                                  value().visibleCategories,
                                );
                                if (event.currentTarget.checked)
                                  selected.add(category.id);
                                else selected.delete(category.id);
                                if (selected.size)
                                  field(
                                    "visibleCategories",
                                    defaultCategories.filter((id) =>
                                      selected.has(id),
                                    ),
                                  );
                              }}
                            />
                            <span>{category.label}</span>
                          </label>
                        )}
                      </For>
                    </div>
                  </fieldset>
                  <p class="settings-note" id="category-selection-note">
                    Keep at least one category selected. Hiding a category does
                    not remove its results from All.
                  </p>
                  <button
                    class="settings-link"
                    type="button"
                    disabled={
                      value().visibleCategories.length === categories.length
                    }
                    onClick={() =>
                      field("visibleCategories", [...defaultCategories])
                    }
                  >
                    Show all categories
                  </button>
                </div>
              </Show>
              <Show when={section() === "clipboard"}>
                <Show when={!value().clipboardHistoryDecided}>
                  <p class="settings-callout" role="status">
                    Choose whether TinyDash saves new clipboard text. You can
                    change this later.
                  </p>
                </Show>
                <Toggle
                  label="Save clipboard history"
                  hint="Save copied text so you can find and copy it again."
                  checked={value().clipboardHistoryEnabled}
                  onChange={(next) => {
                    field("clipboardHistoryEnabled", next);
                    field("clipboardHistoryDecided", true);
                  }}
                />
                <label class="settings-row">
                  <span>
                    <strong>History limit</strong>
                    <span class="settings-hint">
                      Keep 1 to 500 unpinned entries. Pinned entries stay saved.
                    </span>
                  </span>
                  <input
                    class="settings-number"
                    aria-label="History limit"
                    type="number"
                    min="1"
                    max="500"
                    step="1"
                    value={value().clipboardHistoryLimit}
                    onInput={(event) =>
                      field(
                        "clipboardHistoryLimit",
                        Number(event.currentTarget.value),
                      )
                    }
                  />
                </label>
                <div class="settings-group settings-separated">
                  <label class="settings-field-label">
                    Retention in days
                    <input
                      type="number"
                      min="0"
                      max="3650"
                      aria-label="Clipboard retention days"
                      value={value().clipboardRetentionDays}
                      onInput={(event) =>
                        field(
                          "clipboardRetentionDays",
                          Number(event.currentTarget.value),
                        )
                      }
                    />
                  </label>
                  <p>
                    0 keeps entries until the count limit. Pinned text stays
                    saved.
                  </p>
                  <label class="settings-field-label">
                    Excluded applications
                    <textarea
                      aria-label="Excluded clipboard applications"
                      rows={3}
                      placeholder="Application name or bundle ID, one per line"
                      value={value().clipboardExcludedApps.join("\n")}
                      onInput={(event) =>
                        field(
                          "clipboardExcludedApps",
                          lines(event.currentTarget.value),
                        )
                      }
                    />
                  </label>
                  <p>
                    Exclusions match exact application names or identifiers. If
                    the source cannot be identified, capture stops while any
                    exclusions are configured. Linux cannot attribute sources
                    here. Secret-marked content is always skipped.
                  </p>
                  <Toggle
                    label="Capture clipboard images"
                    hint="Opt in to bounded local PNG history (currently macOS only). Requires clipboard history to be enabled."
                    checked={value().clipboardCaptureImages}
                    onChange={(next) => field("clipboardCaptureImages", next)}
                  />
                  <Toggle
                    label="Capture copied files"
                    hint="Keep references, not file backups (currently macOS only). Requires clipboard history to be enabled."
                    checked={value().clipboardCaptureFiles}
                    onChange={(next) => field("clipboardCaptureFiles", next)}
                  />
                  <h2>Saved history</h2>
                  <p>
                    Turning history off stops new entries. It keeps the entries
                    you already saved.
                  </p>
                  <button
                    type="button"
                    class="settings-button"
                    onClick={() => {
                      setDialogError(undefined);
                      setClearKeepPinned(false);
                      setClearOpen(true);
                    }}
                  >
                    Clear clipboard history
                  </button>
                  <button
                    type="button"
                    class="settings-button settings-danger-button"
                    onClick={() => {
                      setDialogError(undefined);
                      setClearKeepPinned(true);
                      setClearOpen(true);
                    }}
                  >
                    Clear unpinned entries
                  </button>
                </div>
              </Show>
              <Show when={section() === "files"}>
                <div class="settings-group">
                  <label class="settings-field-label" for="folder-mode">
                    Search folders
                  </label>
                  <select
                    id="folder-mode"
                    value={folderMode()}
                    onChange={(event) => {
                      const next = event.currentTarget.value as FolderMode;
                      setFolderMode(next);
                      field(
                        "fileSearchRoots",
                        next === "default"
                          ? null
                          : next === "off"
                            ? []
                            : lines(foldersText()),
                      );
                    }}
                  >
                    <option value="default">
                      Desktop, Documents, and Downloads
                    </option>
                    <option value="custom">Choose folders</option>
                    <option value="off">File search off</option>
                  </select>
                  <Show when={folderMode() === "custom"}>
                    <label class="settings-field-label" for="search-folders">
                      Folder paths
                    </label>
                    <textarea
                      id="search-folders"
                      rows="4"
                      spellcheck={false}
                      placeholder={"~/Documents\n~/Projects"}
                      value={foldersText()}
                      onInput={(event) => {
                        setFoldersText(event.currentTarget.value);
                        field(
                          "fileSearchRoots",
                          lines(event.currentTarget.value),
                        );
                      }}
                    />
                    <p class="settings-note">
                      Enter one full path per line. Use ~ for your home folder.
                    </p>
                  </Show>
                </div>
                <Show when={folderMode() !== "off"}>
                  <Toggle
                    label="Include hidden files"
                    hint="Search dotfiles and hidden folders. Symbolic links are still not followed."
                    checked={value().fileSearchIncludeHidden}
                    onChange={(next) => field("fileSearchIncludeHidden", next)}
                  />
                  <label class="settings-field-label">
                    Ignore patterns
                    <textarea
                      aria-label="File ignore patterns"
                      rows={3}
                      placeholder={"*.log\nbuild/**"}
                      value={value().fileSearchIgnorePatterns.join("\n")}
                      onInput={(event) =>
                        field(
                          "fileSearchIgnorePatterns",
                          lines(event.currentTarget.value),
                        )
                      }
                    />
                  </label>
                  <Toggle
                    label="Update files automatically"
                    hint="Watch these folders for changes."
                    checked={value().fileWatchEnabled}
                    onChange={(next) => field("fileWatchEnabled", next)}
                  />
                  <label class="settings-row">
                    <span>
                      <strong>File limit</strong>
                      <span class="settings-hint">
                        Index up to 100,000 files.
                      </span>
                    </span>
                    <input
                      class="settings-number"
                      aria-label="File limit"
                      type="number"
                      min="1"
                      max="100000"
                      step="1"
                      value={value().fileSearchLimit}
                      onInput={(event) =>
                        field(
                          "fileSearchLimit",
                          Number(event.currentTarget.value),
                        )
                      }
                    />
                  </label>
                  <div class="settings-group settings-separated">
                    <label class="settings-field-label" for="excluded-folders">
                      Excluded folder names
                    </label>
                    <textarea
                      id="excluded-folders"
                      rows="3"
                      spellcheck={false}
                      value={excludedText()}
                      onInput={(event) => {
                        setExcludedText(event.currentTarget.value);
                        field(
                          "fileSearchExcludedDirs",
                          lines(event.currentTarget.value),
                        );
                      }}
                    />
                    <p class="settings-note">
                      Enter one folder name per line, such as node_modules.
                      Names must match exactly.
                    </p>
                  </div>
                </Show>
              </Show>
              <Show when={section() === "currency"}>
                <Toggle
                  label="Keep currency rates up to date"
                  hint="Check for rates when TinyDash opens. Saved rates stay fresh for 24 hours."
                  checked={value().currencyRatesEnabled}
                  onChange={(next) => field("currencyRatesEnabled", next)}
                />
                <div class="settings-group settings-separated">
                  <h2>Available offline</h2>
                  <p>
                    TinyDash uses European Central Bank rates from Frankfurter.
                    Saved rates remain available when updates are off.
                  </p>
                  <p>
                    Calculations and unit conversions do not need a network
                    connection.
                  </p>
                </div>
              </Show>
              <Show when={section() === "privacy"}>
                <div class="settings-group">
                  <h2>Stored on this computer</h2>
                  <p>
                    TinyDash stores your settings, app usage counts, pins, and
                    saved clipboard text locally. Tool pins save their input.
                    TinyDash searches file names and paths.
                  </p>
                  <p>
                    Generated passwords stay in memory. TinyDash does not add
                    them to clipboard history when you copy them. Web searches
                    open in your browser only when you select an Open action.
                  </p>
                  <p>
                    Currency updates request exchange rates from Frankfurter.
                    Your search text and clipboard history are not part of that
                    request.
                  </p>
                </div>
                <div class="settings-path">
                  <h2>Settings file</h2>
                  <code>{info()?.configPath}</code>
                  <button
                    class="settings-button"
                    type="button"
                    onClick={() =>
                      void backend
                        .revealSettings()
                        .catch((reason) => setError(String(reason)))
                    }
                  >
                    <Icon name="folder" size={16} />
                    Show settings file
                  </button>
                </div>
                <div class="settings-path">
                  <h2>History and usage data</h2>
                  <code>{info()?.dataPath}</code>
                  <button
                    class="settings-button"
                    type="button"
                    onClick={() =>
                      void backend
                        .revealSettings(true)
                        .catch((reason) => setError(String(reason)))
                    }
                  >
                    <Icon name="folder" size={16} />
                    Show data file
                  </button>
                </div>
                <div class="settings-group settings-separated">
                  <h2>Recovery backup</h2>
                  <p>
                    TinyDash keeps a recovery backup before a data migration. It
                    can contain saved clipboard text. Later changes are not in
                    that backup.
                  </p>
                  <details class="settings-hint">
                    <summary>How to restore a backup</summary>
                    <p>
                      Quit TinyDash first. Copy the current settings, database,
                      recovery.tar, and any database files ending in -wal, -shm,
                      or -journal to a separate folder. Keep these copies.
                      Extract the archive into a new folder. Use an app version
                      that supports sourceSchema in manifest.json. Move the
                      current database and its companion files out of the data
                      folder, then copy the archive's database and settings to
                      the locations above. Start TinyDash and check your data.
                      If recovery fails, quit and restore the copies you kept.
                    </p>
                  </details>
                  <div class="settings-inline-actions">
                    <button
                      class="settings-button"
                      type="button"
                      onClick={() => void revealBackup()}
                    >
                      Show recovery backup
                    </button>
                  </div>
                  <div class="settings-inline-actions">
                    <button
                      class="settings-button"
                      type="button"
                      disabled={dirty() || saving()}
                      onClick={() => void exportCurrentSettings()}
                    >
                      Export settings
                    </button>
                    <button
                      class="settings-button"
                      type="button"
                      disabled={importing()}
                      onClick={() => void previewImport()}
                    >
                      {importing() ? "Reading import..." : "Import settings"}
                    </button>
                  </div>
                  <Show when={pendingImport()}>
                    {(preview) => (
                      <div
                        class="settings-import-preview"
                        role="dialog"
                        aria-label="Import preview"
                      >
                        <strong>Import preview</strong>
                        <p>
                          Review the imported settings. Apply them to the draft,
                          then select Save changes.
                        </p>
                        <p class="settings-hint">
                          Clipboard capture:{" "}
                          {preview().settings.clipboardHistoryEnabled
                            ? "on"
                            : "off"}
                          . Start at login:{" "}
                          {preview().settings.startAtLogin ? "on" : "off"}.
                          Theme: {preview().appearance ?? "keep current"}.
                          Compact layout:{" "}
                          {preview().compact == null
                            ? "keep current"
                            : preview().compact
                              ? "on"
                              : "off"}
                          . Follow macOS Liquid Glass:{" "}
                          {preview().followSystemGlass == null
                            ? "keep current"
                            : preview().followSystemGlass
                              ? "on"
                              : "off"}
                          .
                        </p>
                        <details open>
                          <summary>Settings to import</summary>
                          <pre
                            class="settings-preview"
                            aria-label="Imported settings"
                          >
                            {JSON.stringify(preview().settings, null, 2)}
                          </pre>
                        </details>
                        <Show when={preview().ignoredKeys.length}>
                          <p class="settings-hint">
                            Ignored fields: {preview().ignoredKeys.join(", ")}
                          </p>
                        </Show>
                        <div class="settings-inline-actions">
                          <button
                            class="settings-button"
                            type="button"
                            onClick={() => setPendingImport(undefined)}
                          >
                            Cancel
                          </button>
                          <button
                            class="settings-button settings-save"
                            type="button"
                            onClick={applyImport}
                          >
                            Apply import
                          </button>
                        </div>
                      </div>
                    )}
                  </Show>
                </div>
              </Show>
              <Show when={section() === "about"}>
                <div class="settings-about">
                  <img
                    class="tiny-mark"
                    src={appIconUrl}
                    alt=""
                    width="48"
                    height="48"
                  />
                  <div>
                    <h2>TinyDash</h2>
                    <p>Version {info()?.version}</p>
                  </div>
                </div>
                <dl class="settings-shortcuts">
                  <div>
                    <dt>Next / previous category</dt>
                    <dd>
                      <kbd>Tab</kbd>
                      <kbd>Shift Tab</kbd>
                    </dd>
                  </div>
                  <div>
                    <dt>Move through results</dt>
                    <dd>
                      <kbd>↑</kbd>
                      <kbd>↓</kbd>
                    </dd>
                  </div>
                  <div>
                    <dt>Open the selected item</dt>
                    <dd>
                      <kbd>↵</kbd>
                    </dd>
                  </div>
                  <div>
                    <dt>Open Actions</dt>
                    <dd>
                      <kbd>{modifier()} K</kbd>
                    </dd>
                  </div>
                  <div>
                    <dt>Open Settings</dt>
                    <dd>
                      <kbd>{modifier()} ,</kbd>
                    </dd>
                  </div>
                  <div>
                    <dt>Save settings</dt>
                    <dd>
                      <kbd>{modifier()} S</kbd>
                    </dd>
                  </div>
                </dl>
                <div class="settings-group settings-separated">
                  <h2>Passphrase word list</h2>
                  <p>
                    The EFF Large Wordlist is by Joseph Bonneau and the
                    Electronic Frontier Foundation. Used unchanged under
                    Creative Commons Attribution 4.0.
                  </p>
                  <p class="settings-credit">
                    eff.org/dice
                    <br />
                    creativecommons.org/licenses/by/4.0/
                  </p>
                </div>
                <div class="settings-group settings-separated">
                  <h2>Updates</h2>
                  <p>
                    Check for a signed update. TinyDash installs it only after
                    you choose to continue.
                  </p>
                  <div class="settings-inline-actions">
                    <button
                      class="settings-button"
                      type="button"
                      disabled={updateBusy()}
                      onClick={() => void checkUpdate()}
                    >
                      {updateBusy() ? "Checking..." : "Check for updates"}
                    </button>
                    <Show when={updateStatus()?.available}>
                      <button
                        class="settings-button settings-save"
                        type="button"
                        disabled={updateBusy()}
                        onClick={() => void installUpdate()}
                      >
                        Install {updateStatus()!.version}
                      </button>
                    </Show>
                  </div>
                  <Show when={updateStatus()}>
                    <p class="settings-note" role="status">
                      {updateStatus()!.message}
                    </p>
                    <Show when={updateStatus()?.notes}>
                      <p class="settings-note">{updateStatus()!.notes}</p>
                    </Show>
                  </Show>
                </div>
              </Show>
            </fieldset>
          </Show>
        </div>
        <footer class="settings-footer">
          <Show when={error() && info()}>
            <p class="settings-error" role="alert">
              {error()}
            </p>
          </Show>
          <div class="settings-save-row">
            <p role="status" class={{ "has-changes": dirty() }}>
              {saving()
                ? "Saving changes..."
                : dirty()
                  ? "Unsaved changes"
                  : status() || "All changes saved"}
            </p>
            <div>
              <Show when={dirty()}>
                <button
                  type="button"
                  class="settings-button"
                  disabled={saving() || recording() || preparing()}
                  onClick={() => {
                    resetDraft(saved()!);
                    setAppearance(savedAppearance());
                    setCompact(savedCompact());
                    setFollowSystemGlass(savedSystemGlass());
                    setError(undefined);
                    setStatus("Changes discarded.");
                  }}
                >
                  Discard
                </button>
              </Show>
              <button
                class="settings-button settings-save"
                type="submit"
                disabled={!dirty() || saving() || recording() || preparing()}
              >
                {saving() ? "Saving..." : "Save changes"}
              </button>
            </div>
          </div>
        </footer>
      </form>
      <Show when={clearOpen()}>
        <ConfirmDialog
          title={
            clearKeepPinned()
              ? "Clear unpinned entries?"
              : "Clear clipboard history?"
          }
          description={
            clearKeepPinned()
              ? "This deletes unpinned text and all saved images and file references. Pinned text stays available. The system clipboard does not change."
              : "This deletes all saved text, images, and file references, including pinned text. The system clipboard does not change."
          }
          confirmLabel={clearKeepPinned() ? "Clear unpinned" : "Clear history"}
          busyLabel="Clearing..."
          busy={clearing()}
          error={dialogError()}
          onClose={() => setClearOpen(false)}
          onConfirm={() => void clearHistory(clearKeepPinned())}
        />
      </Show>
    </main>
  );
}
