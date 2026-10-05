import {
  createEffect,
  createMemo,
  createResource,
  createSignal,
  For,
  Match,
  onCleanup,
  onMount,
  Show,
  Switch,
} from "solid-js";

import type { About as AboutInfo } from "../generated/About";
import type { EmojiLanguage } from "../generated/EmojiLanguage";
import type { SearchEngine } from "../generated/SearchEngine";
import type { Settings as Values } from "../generated/Settings";
import type { Theme } from "../generated/Theme";
import * as ipc from "../lib/ipc";
import { IS_MAC } from "../lib/keys";
import { applyTheme } from "../lib/theme";
import { ConfirmDialog } from "../ui/ConfirmDialog";
import { ListEditor, NumberField, Row, Select, Toggle } from "./controls";
import { Library } from "./Library";
import { ShortcutRecorder } from "./ShortcutRecorder";

const SECTIONS = [
  { id: "general", label: "General" },
  { id: "clipboard", label: "Clipboard" },
  { id: "files", label: "Files" },
  { id: "search", label: "Search" },
  { id: "snippets", label: "Snippets" },
  { id: "quicklinks", label: "Quicklinks" },
  { id: "about", label: "About" },
] as const;

type Section = (typeof SECTIONS)[number]["id"];

const THEMES: ReadonlyArray<{ value: Theme; label: string }> = [
  { value: "system", label: "Match System" },
  { value: "light", label: "Light" },
  { value: "dark", label: "Dark" },
];

const ENGINES: ReadonlyArray<{ value: SearchEngine; label: string }> = [
  { value: "google", label: "Google" },
  { value: "duckDuckGo", label: "DuckDuckGo" },
  { value: "bing", label: "Bing" },
  { value: "brave", label: "Brave" },
];

const SKIN_TONES = [
  "👋 Default",
  "👋🏻 Light",
  "👋🏼 Medium-light",
  "👋🏽 Medium",
  "👋🏾 Medium-dark",
  "👋🏿 Dark",
].map((label, value) => ({ value, label }));

const LANGUAGES: ReadonlyArray<{ value: EmojiLanguage; label: string }> = [
  { value: "zh", label: "Chinese (Simplified)" },
  { value: "ms", label: "Malay" },
  { value: "es", label: "Spanish" },
];

export function Settings() {
  // `saved` is what the backend holds; `changes` are edits on their way.
  // The form shows saved values with pending changes on top.
  const [saved, setSaved] = createSignal<Values>();
  const [changes, setChanges] = createSignal<Partial<Values>[]>([]);
  const values = createMemo(() => {
    const base = saved();
    return base && changes().reduce<Values>((all, change) => ({ ...all, ...change }), base);
  });
  const [section, setSection] = createSignal<Section>("general");
  const [error, setError] = createSignal<string>();
  // Read once: About shows it, and the Clipboard section hides what this
  // OS cannot save.
  const [aboutError, setAboutError] = createSignal<string>();
  const [about] = createResource(() =>
    ipc.about().catch((failure: unknown) => {
      setAboutError(ipc.message(failure));
      return undefined;
    }),
  );
  const [confirmClear, setConfirmClear] = createSignal(false);
  let clearButton: HTMLButtonElement | undefined;
  const closeClear = () => {
    setConfirmClear(false);
    clearButton?.focus();
  };

  createEffect(() => {
    const current = values();
    if (current) applyTheme(current.theme);
  });

  onMount(() => {
    const stop = ipc.onSettingsChanged(setSaved);
    onCleanup(() => void stop.then((unlisten) => unlisten()));
    ipc.getSettings().then(setSaved, (failure) => setError(ipc.message(failure)));
  });

  // Saves run one at a time. Each applies its change on top of the latest
  // saved settings, so a change that failed is never sent again.
  let saving = Promise.resolve();
  const save = (change: Partial<Values>) => {
    setChanges((list) => [...list, change]);
    saving = saving.then(async () => {
      const base = saved();
      try {
        if (base) setSaved(await ipc.updateSettings({ ...base, ...change }));
        setError(undefined);
      } catch (failure) {
        setError(ipc.message(failure));
      } finally {
        setChanges((list) => list.filter((pending) => pending !== change));
      }
    });
    return saving;
  };

  const clearHistory = () =>
    ipc.runAction({ type: "clearClipboard" }).catch((failure) => setError(ipc.message(failure)));

  return (
    <div class="settings">
      <nav class="sidebar" aria-label="Settings sections">
        <For each={SECTIONS}>
          {(entry) => (
            <button
              type="button"
              class="section-link"
              aria-current={section() === entry.id ? "page" : undefined}
              onClick={() => setSection(entry.id)}
            >
              {entry.label}
            </button>
          )}
        </For>
      </nav>

      <main class="content">
        <h1>{SECTIONS.find((entry) => entry.id === section())?.label}</h1>
        <Show when={error()}>
          <p class="error" role="alert">
            {error()}
          </p>
        </Show>
        <Show when={values()}>
          {(settings) => (
            <Switch>
              <Match when={section() === "general"}>
                <Row label="Launcher shortcut" description="Shows or hides TinyDash from any app.">
                  <ShortcutRecorder
                    value={settings().shortcut}
                    onChange={(shortcut) => save({ shortcut })}
                    onError={setError}
                  />
                </Row>
                <Row label="Appearance">
                  <Select
                    label="Appearance"
                    value={settings().theme}
                    options={THEMES}
                    onChange={(theme) => void save({ theme })}
                  />
                </Row>
                <Row label="Hide when another app is focused">
                  <Toggle
                    label="Hide when another app is focused"
                    checked={settings().hideOnBlur}
                    onChange={(hideOnBlur) => void save({ hideOnBlur })}
                  />
                </Row>
                <Row label="Open at login" description="Starts hidden, ready for the shortcut.">
                  <Toggle
                    label="Open at login"
                    checked={settings().launchAtLogin}
                    onChange={(launchAtLogin) => void save({ launchAtLogin })}
                  />
                </Row>
                <Row label={IS_MAC ? "Show menu bar icon" : "Show tray icon"}>
                  <Toggle
                    label={IS_MAC ? "Show menu bar icon" : "Show tray icon"}
                    checked={settings().showTrayIcon}
                    onChange={(showTrayIcon) => void save({ showTrayIcon })}
                  />
                </Row>
              </Match>

              <Match when={section() === "clipboard"}>
                <Row
                  label="Save clipboard history"
                  description="Kept on this computer, unencrypted. Copies marked secret, such as passwords, are skipped."
                >
                  <Toggle
                    label="Save clipboard history"
                    checked={settings().clipboardHistoryEnabled}
                    onChange={(clipboardHistoryEnabled) => void save({ clipboardHistoryEnabled })}
                  />
                </Row>
                {/* 1000 and 32 match CLIPBOARD_LIMIT_MAX and MAX_IMAGES in the Rust code. */}
                <Row label="Entries to keep" description="Pinned entries do not count.">
                  <NumberField
                    label="Entries to keep"
                    value={settings().clipboardHistoryLimit}
                    min={1}
                    max={1000}
                    onChange={(clipboardHistoryLimit) => void save({ clipboardHistoryLimit })}
                  />
                </Row>
                <Show when={about()?.richClipboard !== false}>
                  <Row
                    label="Save images"
                    description="Up to 32 images, not counting pins. Large images are skipped."
                  >
                    <Toggle
                      label="Save images"
                      checked={settings().clipboardCaptureImages}
                      onChange={(clipboardCaptureImages) => void save({ clipboardCaptureImages })}
                    />
                  </Row>
                  <Row
                    label="Save copied files"
                    description="Saves references to files, not their contents."
                  >
                    <Toggle
                      label="Save copied files"
                      checked={settings().clipboardCaptureFiles}
                      onChange={(clipboardCaptureFiles) => void save({ clipboardCaptureFiles })}
                    />
                  </Row>
                </Show>
                <Row label="Clear history" description="Deletes every entry except pinned ones.">
                  <button
                    ref={clearButton}
                    type="button"
                    class="button"
                    onClick={() => setConfirmClear(true)}
                  >
                    Clear History…
                  </button>
                </Row>
              </Match>

              <Match when={section() === "files"}>
                <p class="section-intro">
                  TinyDash searches the names of files and folders here. It never reads file
                  contents. Hidden files are skipped.
                </p>
                {/* 50 matches FOLDER_LIST_MAX in the Rust code. */}
                <h2>Folders</h2>
                <ListEditor
                  label="Folder to add"
                  max={50}
                  placeholder="~/Projects"
                  items={settings().fileSearchFolders}
                  onChange={(fileSearchFolders) => void save({ fileSearchFolders })}
                />
                <h2>Skip folders named</h2>
                <ListEditor
                  label="Folder name to skip"
                  max={50}
                  placeholder="node_modules"
                  items={settings().fileSearchExcludedDirs}
                  onChange={(fileSearchExcludedDirs) => void save({ fileSearchExcludedDirs })}
                />
              </Match>

              <Match when={section() === "search"}>
                <Row
                  label="Web search"
                  description="Used for the web search at the end of All results."
                >
                  <Select
                    label="Web search"
                    value={settings().searchEngine}
                    options={ENGINES}
                    onChange={(searchEngine) => void save({ searchEngine })}
                  />
                </Row>
                <Row
                  label="Currency rates"
                  description="Downloads daily ECB rates. Your amounts stay on this computer."
                >
                  <Toggle
                    label="Currency rates"
                    checked={settings().currencyRatesEnabled}
                    onChange={(currencyRatesEnabled) => void save({ currencyRatesEnabled })}
                  />
                </Row>
                <Row label="Emoji skin tone">
                  <Select
                    label="Emoji skin tone"
                    value={settings().emojiSkinTone}
                    options={SKIN_TONES}
                    onChange={(emojiSkinTone) => void save({ emojiSkinTone })}
                  />
                </Row>
                <h2>Emoji keywords</h2>
                <p class="section-intro">
                  English names and :shortcodes: always work. Add keywords in:
                </p>
                <For each={LANGUAGES}>
                  {(language) => (
                    <Row label={language.label}>
                      <Toggle
                        label={language.label}
                        checked={settings().emojiLanguages.includes(language.value)}
                        onChange={(on) => {
                          const others = settings().emojiLanguages.filter(
                            (value) => value !== language.value,
                          );
                          void save({ emojiLanguages: on ? [...others, language.value] : others });
                        }}
                      />
                    </Row>
                  )}
                </For>
              </Match>

              <Match when={section() === "snippets"}>
                <Library kind="snippet" />
              </Match>

              <Match when={section() === "quicklinks"}>
                <Library kind="quicklink" />
              </Match>

              <Match when={section() === "about"}>
                <About about={about()} error={aboutError()} />
              </Match>
            </Switch>
          )}
        </Show>
      </main>

      <Show when={confirmClear()}>
        <ConfirmDialog
          message="Delete all clipboard history except pinned entries?"
          confirmLabel="Clear History"
          onConfirm={() => {
            closeClear();
            void clearHistory();
          }}
          onCancel={closeClear}
        />
      </Show>
    </div>
  );
}

function About(props: { about: AboutInfo | undefined; error: string | undefined }) {
  const about = () => props.about;
  return (
    <div class="about">
      <Show when={props.error}>
        {(text) => (
          <p class="error" role="alert">
            Could not read the app details: {text()}
          </p>
        )}
      </Show>
      <p>
        <strong>TinyDash {about()?.version}</strong> — a small, keyboard-first launcher.
      </p>
      <Show
        when={about()?.settingsFolder !== about()?.dataFolder}
        fallback={
          <p>
            Settings and saved data: <code>{about()?.dataFolder}</code>
          </p>
        }
      >
        <p>
          Settings: <code>{about()?.settingsFolder}</code>
        </p>
        <p>
          Saved data: <code>{about()?.dataFolder}</code>
        </p>
      </Show>
      <p>
        Source and license: <code>github.com/jewei/tinydash</code> (MIT)
      </p>
      <h2>Credits</h2>
      <ul>
        <li>Figtree and Caprasimo fonts, SIL Open Font License 1.1</li>
        <li>EFF Large Wordlist by the Electronic Frontier Foundation, CC BY 4.0</li>
        <li>Emoji keywords from Unicode CLDR, Unicode License V3</li>
        <li>Exchange rates from the European Central Bank through Frankfurter</li>
      </ul>
    </div>
  );
}
