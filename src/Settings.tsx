import {
  createEffect,
  createMemo,
  createSignal,
  For,
  onCleanup,
  onMount,
  Show,
} from "solid-js";
import { isTauri } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { backend, type SettingsInfo, type SettingsValues } from "./bridge";
import {
  appearances,
  readAppearance,
  saveAppearance,
  watchAppearance,
} from "./appearance";
import ConfirmDialog from "./components/ConfirmDialog";
import Icon from "./components/Icon";
import {
  categories,
  defaultCategories,
  normalizeCategories,
} from "./categories";
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
type FolderMode = "default" | "custom" | "off";
const lines = (text: string) => [
  ...new Set(
    text
      .split(/\r?\n/)
      .map((line) => line.trim())
      .filter(Boolean),
  ),
];
const folderModeFor = (value: SettingsValues): FolderMode =>
  value.fileSearchRoots === null
    ? "default"
    : value.fileSearchRoots.length
      ? "custom"
      : "off";

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
  const [appearance, setAppearance] = createSignal(readAppearance());
  const [folderMode, setFolderMode] = createSignal<FolderMode>("default");
  const [foldersText, setFoldersText] = createSignal("");
  const [excludedText, setExcludedText] = createSignal("");
  const [saving, setSaving] = createSignal(false);
  const [recording, setRecording] = createSignal(false);
  const [preparing, setPreparing] = createSignal(false);
  const [clearOpen, setClearOpen] = createSignal(false);
  const [clearing, setClearing] = createSignal(false);
  const [error, setError] = createSignal<string>();
  const [dialogError, setDialogError] = createSignal<string>();
  const [status, setStatus] = createSignal("");
  let disposed = false;
  let recordingSequence = 0;
  let recorder!: HTMLButtonElement;
  let content!: HTMLDivElement;
  const stops: UnlistenFn[] = [];
  const value = () => draft()!;
  const currentSection = () => sections.find((item) => item.id === section())!;
  const dirty = createMemo(
    () =>
      !!draft() &&
      (JSON.stringify(draft()) !== JSON.stringify(saved()) ||
        folderMode() !== folderModeFor(saved()!)),
  );
  const modifier = () => (info()?.platform === "macos" ? "⌘" : "Ctrl");
  const shortcutKeys = () =>
    (draft()?.shortcut ?? "").split("+").map((key) => {
      if (key === "Super" || key === "Command" || key === "Meta")
        return info()?.platform === "macos" ? "Command" : "Windows";
      if (key === "Alt") return info()?.platform === "macos" ? "Option" : "Alt";
      if (key === "CommandOrControl")
        return info()?.platform === "macos" ? "Command" : "Control";
      return key.replace(/^(Key|Digit)/, "");
    });

  createEffect(() => {
    document.documentElement.dataset.appearance = appearance();
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
      setSaved({
        ...response.settings,
        visibleCategories: normalizeCategories(
          response.settings.visibleCategories,
        ),
      });
      resetDraft(response.settings);
    } catch (reason) {
      if (!disposed) setError(String(reason));
    }
  }

  async function stopRecording() {
    recordingSequence += 1;
    if (!recording() && !preparing()) return;
    setRecording(false);
    setPreparing(true);
    try {
      await backend.recordShortcut(false);
    } catch (reason) {
      if (!disposed) setError(String(reason));
    } finally {
      if (!disposed) setPreparing(false);
    }
  }
  async function startRecording() {
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
      recorder.focus();
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
      setSaved(settings);
      resetDraft(settings);
      setStatus("Changes saved.");
    } catch (reason) {
      if (!disposed) setError(String(reason));
    } finally {
      if (!disposed) setSaving(false);
    }
  }
  async function clearHistory() {
    if (clearing()) return;
    setClearing(true);
    setDialogError(undefined);
    try {
      await backend.clearClipboard();
      setClearOpen(false);
      setStatus("Clipboard history cleared.");
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
        (!event.ctrlKey && !event.altKey && !event.metaKey) ||
        event.getModifierState("AltGraph")
      ) {
        setError(
          "Hold Control, Option / Alt, or Command / Windows, then press one key.",
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
      field("shortcut", keys.join("+"));
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
  onMount(() => {
    document.title = "TinyDash Settings";
    document.addEventListener("keydown", onKey, true);
    window.addEventListener("blur", onBlur);
    void load();
    void watchAppearance(setAppearance).then((stop) =>
      disposed ? stop() : stops.push(stop),
    );
    if (desktop)
      void listen<string>("shortcut-error", (event) =>
        setError(event.payload),
      ).then((stop) => (disposed ? stop() : stops.push(stop)));
  });
  onCleanup(() => {
    void stopRecording();
    disposed = true;
    stops.forEach((stop) => stop());
    document.removeEventListener("keydown", onKey, true);
    window.removeEventListener("blur", onBlur);
  });

  return (
    <main class="settings-shell" aria-label="TinyDash settings">
      <aside class="settings-sidebar">
        <div class="settings-brand">
          <span class="tiny-mark" aria-hidden="true">
            <i />
            <i />
          </span>
          TinyDash
        </div>
        <nav aria-label="Settings sections">
          <For each={sections}>
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
        noValidate
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
                    class="shortcut-recorder"
                    classList={{ "is-recording": recording() }}
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
                      ? "On Wayland, set a desktop shortcut that starts TinyDash."
                      : recording()
                        ? "Hold a modifier and press one key. Press Escape to cancel."
                        : "Click Record new, then press the keys you want to use."}
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
                    label="Clear the search each time"
                    hint="Turn this off to keep your last search selected."
                    checked={value().clearQueryOnOpen}
                    onChange={(next) => field("clearQueryOnOpen", next)}
                  />
                </div>
              </Show>
              <Show when={section() === "appearance"}>
                <div class="settings-group">
                  <h2>Launcher style</h2>
                  <div
                    class="settings-appearances"
                    role="radiogroup"
                    aria-label="Launcher style"
                  >
                    <For each={appearances}>
                      {(item) => (
                        <label
                          class="style-option"
                          classList={{
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
                          <span class="settings-hint">
                            {item.id === "light"
                              ? "Cream and peach"
                              : item.id === "dark"
                                ? "Charcoal and olive"
                                : "A smaller result list"}
                          </span>
                        </label>
                      )}
                    </For>
                  </div>
                  <p class="settings-note">
                    Appearance changes apply at once in both windows.
                  </p>
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
                            class="category-choice"
                            classList={{
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
                <Toggle
                  label="Save clipboard history"
                  hint="Save copied text so you can find and copy it again."
                  checked={value().clipboardHistoryEnabled}
                  onChange={(next) => field("clipboardHistoryEnabled", next)}
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
                  <h2>Saved text</h2>
                  <p>
                    Turning history off stops new entries. It keeps the entries
                    you already saved.
                  </p>
                  <button
                    type="button"
                    class="settings-button"
                    onClick={() => {
                      setDialogError(undefined);
                      setClearOpen(true);
                    }}
                  >
                    Clear clipboard history
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
              </Show>
              <Show when={section() === "about"}>
                <div class="settings-about">
                  <span class="tiny-mark" aria-hidden="true">
                    <i />
                    <i />
                  </span>
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
            <p role="status" classList={{ "has-changes": dirty() }}>
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
          title="Clear clipboard history?"
          description="This deletes all saved text entries, including pinned entries. The current system clipboard stays available."
          confirmLabel="Clear history"
          busyLabel="Clearing..."
          busy={clearing()}
          error={dialogError()}
          onClose={() => setClearOpen(false)}
          onConfirm={() => void clearHistory()}
        />
      </Show>
    </main>
  );
}
