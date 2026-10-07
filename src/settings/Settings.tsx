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
import { TabsEditor } from "./TabsEditor";

const SECTIONS = [
  { id: "general", label: "General" },
  { id: "widgets", label: "Widgets" },
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

/**
 * Fields to set, or for a list, a function of the current values: it runs
 * on the saved values when it is sent, so it never carries another edit
 * that is still saving or that failed.
 */
type Change = Partial<Values> | ((current: Values) => Partial<Values>);

const fieldsOf = (values: Values, change: Change): Partial<Values> =>
  typeof change === "function" ? change(values) : change;

const apply = (values: Values, change: Change): Values => ({
  ...values,
  ...fieldsOf(values, change),
});

export function Settings() {
  // `saved` is what the backend holds; `changes` are edits on their way.
  // The form shows saved values with pending changes on top.
  const [saved, setSaved] = createSignal<Values>();
  const [changes, setChanges] = createSignal<Change[]>([]);
  const values = createMemo(() => {
    const base = saved();
    return base && changes().reduce(apply, base);
  });
  const [section, setSection] = createSignal<Section>("general");
  const [error, setError] = createSignal<string>();
  // Read when Settings opens, and again on opening About after a failure:
  // About shows it, and the Clipboard section hides what this OS cannot save.
  const [aboutError, setAboutError] = createSignal<string>();
  const [about, { refetch: readAbout }] = createResource(() =>
    ipc.about().catch((failure: unknown) => {
      setAboutError(ipc.message(failure));
      return undefined;
    }),
  );
  const updates = createUpdateCheck(() => about());
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
  // saved settings, so a change that failed is never sent again. A save
  // gives back its error, or nothing when it worked.
  let saving = Promise.resolve();
  const save = (change: Change) => {
    setChanges((list) => [...list, change]);
    const done = saving.then(async () => {
      const base = saved();
      try {
        if (base) setSaved(await ipc.updateSettings(fieldsOf(base, change)));
        setError(undefined);
        return undefined;
      } catch (failure) {
        const message = ipc.message(failure);
        setError(message);
        return message;
      } finally {
        setChanges((list) => list.filter((pending) => pending !== change));
      }
    });
    saving = done.then(() => {});
    return done;
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
              onClick={() => {
                setSection(entry.id);
                // A failed read may have been brief: try again on opening About.
                if (entry.id === "about" && aboutError()) {
                  setAboutError(undefined);
                  void readAbout();
                }
              }}
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
                <Show when={about()?.selfUpdate}>
                  <Row
                    label="Check for updates"
                    description="Looks for a new version when the launcher opens, at most every six hours. Nothing installs until you choose."
                  >
                    <Toggle
                      label="Check for updates"
                      checked={settings().checkForUpdates}
                      onChange={(checkForUpdates) => void save({ checkForUpdates })}
                    />
                  </Row>
                </Show>
                <Row label={IS_MAC ? "Show menu bar icon" : "Show tray icon"}>
                  <Toggle
                    label={IS_MAC ? "Show menu bar icon" : "Show tray icon"}
                    checked={settings().showTrayIcon}
                    onChange={(showTrayIcon) => void save({ showTrayIcon })}
                  />
                </Row>
                <Row
                  label="Launcher position"
                  description={
                    settings().launcherPosition
                      ? "Opens where you last dragged it."
                      : "Opens in the center of the screen. Drag the tab bar or the footer to move it."
                  }
                >
                  <button
                    type="button"
                    class="button"
                    disabled={!settings().launcherPosition}
                    onClick={() => void save({ launcherPosition: null })}
                  >
                    Center
                  </button>
                </Row>
                <h2>Tabs</h2>
                <p class="section-intro">
                  Choose which tabs show after All, and in what order. Turn off “in All” to keep a
                  tab's results out of All, such as emoji; its tab still finds them. A tab is never
                  both hidden and out of All: turning one off turns the other on.
                </p>
                <TabsEditor
                  tabs={settings().tabs}
                  onChange={(edit) => save((current) => ({ tabs: edit(current.tabs) }))}
                />
              </Match>

              <Match when={section() === "widgets"}>
                <p class="section-intro">
                  Widgets show next to the results of an empty All search.
                </p>
                <Row label="Clocks" description="Local time and up to three cities.">
                  <Toggle
                    label="Clocks"
                    checked={settings().showClocks}
                    onChange={(showClocks) => void save({ showClocks })}
                  />
                </Row>
                <Row label="Disk space" description="Free space on the disk of your home folder.">
                  <Toggle
                    label="Disk space"
                    checked={settings().showDiskSpace}
                    onChange={(showDiskSpace) => void save({ showDiskSpace })}
                  />
                </Row>
                <Row label="Notepad" description="One scratch note, saved on this computer.">
                  <Toggle
                    label="Notepad"
                    checked={settings().showNotepad}
                    onChange={(showNotepad) => void save({ showNotepad })}
                  />
                </Row>
                {/* 3 matches MAX_CLOCK_CITIES in the Rust code. */}
                <h2>Clock cities</h2>
                <ListEditor
                  label="City to add"
                  max={3}
                  placeholder="Tokyo, London, or Europe/Paris"
                  items={settings().clockCities}
                  onChange={(edit) =>
                    save((current) => ({ clockCities: edit(current.clockCities) }))
                  }
                />
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
                  onChange={(edit) =>
                    save((current) => ({ fileSearchFolders: edit(current.fileSearchFolders) }))
                  }
                />
                <h2>Skip folders named</h2>
                <ListEditor
                  label="Folder name to skip"
                  max={50}
                  placeholder="node_modules"
                  items={settings().fileSearchExcludedDirs}
                  onChange={(edit) =>
                    save((current) => ({
                      fileSearchExcludedDirs: edit(current.fileSearchExcludedDirs),
                    }))
                  }
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
                        onChange={(on) =>
                          void save((current) => {
                            const others = current.emojiLanguages.filter(
                              (value) => value !== language.value,
                            );
                            return { emojiLanguages: on ? [...others, language.value] : others };
                          })
                        }
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
                <About about={about()} error={aboutError()} updates={updates} />
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

/**
 * Check for a newer version on request, and install it. The state lives in
 * the Settings window, so leaving About and coming back keeps an install
 * that is running and its result.
 */
function createUpdateCheck(about: () => AboutInfo | undefined) {
  const [doing, setDoing] = createSignal<"check" | "install">();
  const [found, setFound] = createSignal<string | null>();
  const [result, setResult] = createSignal<string>();
  // An offer found before Settings opened, as the launcher shows it. Only
  // until this window learns more from a check or an event.
  createEffect(() => {
    const pending = about()?.pendingUpdate;
    if (pending && found() === undefined) {
      setFound(pending);
      setResult(`TinyDash ${pending} is available.`);
    }
  });
  // A background check may find a newer version, or find that the offer
  // is gone. A "latest" message stays: the check that wrote it sends this
  // event too.
  const stop = ipc.onUpdateChanged((version) => {
    if (version) {
      setFound(version);
      setResult(`TinyDash ${version} is available.`);
    } else if (found()) {
      setFound(null);
      setResult(undefined);
    }
  });
  onCleanup(() => void stop.then((unlisten) => unlisten()));
  const run = (job: "check" | "install", work: () => Promise<void>) => {
    if (doing()) return;
    setDoing(job);
    setResult(undefined);
    work()
      .catch((failure) => setResult(ipc.message(failure)))
      .finally(() => setDoing(undefined));
  };
  return {
    doing,
    found,
    result,
    check: (current: string) =>
      run("check", async () => {
        const version = await ipc.checkForUpdate();
        setFound(version);
        setResult(
          version
            ? `TinyDash ${version} is available.`
            : `TinyDash ${current} is the latest version.`,
        );
      }),
    // On success the app restarts, so only a failure comes back.
    install: () => run("install", () => ipc.installUpdate()),
  };
}

function UpdateCheck(props: { version: string; updates: ReturnType<typeof createUpdateCheck> }) {
  const updates = () => props.updates;
  return (
    <div class="update-check">
      <button
        type="button"
        class="button"
        disabled={updates().doing() !== undefined}
        onClick={() => updates().check(props.version)}
      >
        {updates().doing() === "check" ? "Checking…" : "Check for Updates"}
      </button>
      <Show when={updates().found()}>
        <button
          type="button"
          class="button primary"
          disabled={updates().doing() !== undefined}
          onClick={() => updates().install()}
        >
          {updates().doing() === "install" ? "Installing…" : "Install and Restart"}
        </button>
      </Show>
      <p class="list-status" role="status">
        {updates().result() ?? ""}
      </p>
    </div>
  );
}

function About(props: {
  about: AboutInfo | undefined;
  error: string | undefined;
  updates: ReturnType<typeof createUpdateCheck>;
}) {
  return (
    <div class="about">
      <Show when={props.error}>
        {(text) => (
          <p class="error" role="alert">
            Could not read the app details: {text()}
          </p>
        )}
      </Show>
      {/* Only what was read: no empty version or folders after a failure. */}
      <Show when={props.about}>
        {(about) => (
          <>
            <p>
              <strong>TinyDash {about().version}</strong> — a small, keyboard-first launcher.
            </p>
            <Show
              when={about().selfUpdate}
              fallback={
                <p>
                  New versions: <code>github.com/jewei/tinydash/releases</code>
                </p>
              }
            >
              <UpdateCheck version={about().version} updates={props.updates} />
            </Show>
            <Show
              when={about().settingsFolder !== about().dataFolder}
              fallback={
                <p>
                  Settings and saved data: <code>{about().dataFolder}</code>
                </p>
              }
            >
              <p>
                Settings: <code>{about().settingsFolder}</code>
              </p>
              <p>
                Saved data: <code>{about().dataFolder}</code>
              </p>
            </Show>
          </>
        )}
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
