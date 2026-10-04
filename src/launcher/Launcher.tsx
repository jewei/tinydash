import { createSignal, For, type JSX, Match, onCleanup, onMount, Show, Switch } from "solid-js";

import type { Action } from "../generated/Action";
import type { Category } from "../generated/Category";
import type { Platform } from "../generated/Platform";
import type { ResultAction } from "../generated/ResultAction";
import type { Settings } from "../generated/Settings";
import * as ipc from "../lib/ipc";
import { isComposing, modKey } from "../lib/keys";
import { applyTheme } from "../lib/theme";
import { ConfirmDialog } from "../ui/ConfirmDialog";
import { Glyph } from "../ui/Icon";
import { Keys } from "../ui/Keys";
import { ActionMenu, type MenuItem } from "./ActionMenu";
import { shortcutFor } from "./describe";
import { commandFor } from "./keymap";
import { PreviewPane } from "./PreviewPane";
import { optionId, ResultList } from "./ResultList";
import { CATEGORIES, createLauncher } from "./state";

/** Actions that belong to no result, listed in the actions menu. */
const GENERAL_ACTIONS: ResultAction[] = [
  { label: "Refresh Apps and Files", action: { type: "refresh" }, confirm: null },
  { label: "Settings", action: { type: "openSettings" }, confirm: null },
  { label: "Quit TinyDash", action: { type: "quit" }, confirm: null },
];

export function Launcher() {
  const launcher = createLauncher();
  const [settings, setSettings] = createSignal<Settings>();
  const [platform, setPlatform] = createSignal<Platform>();
  const [warnings, setWarnings] = createSignal<string[]>([]);
  const [menuOpen, setMenuOpen] = createSignal(false);
  let input!: HTMLInputElement;

  const focusInput = () => input.focus();

  const applySettings = (next: Settings) => {
    setSettings(next);
    applyTheme(next.theme);
  };

  onMount(() => {
    const listeners = [
      ipc.onLauncherShown(({ category }) => {
        setMenuOpen(false);
        launcher.reset(category);
        focusInput();
      }),
      ipc.onResultsStale(() => void launcher.refresh()),
      ipc.onSettingsChanged(applySettings),
    ];
    onCleanup(() => listeners.forEach((listener) => void listener.then((stop) => stop())));
    ipc
      .launcherInit()
      .then((init) => {
        applySettings(init.settings);
        setPlatform(init.platform);
        setWarnings(init.warnings);
      })
      .catch((error) => setWarnings([ipc.message(error)]));
    void launcher.refresh();
    focusInput();
  });

  /** Actions that belong to no result, such as opening Settings. */
  const runGeneral = (action: Action) => launcher.run({ label: "", action, confirm: null });

  const deleteSelected = () => {
    const position = launcher
      .selected()
      ?.actions.findIndex((entry) => entry.action.type === "deleteClip");
    if (position !== undefined && position >= 0) {
      launcher.activate(launcher.selectedIndex(), position);
    }
  };

  const onKeyDown = (event: KeyboardEvent) => {
    if (isComposing(event) || menuOpen() || launcher.pending()) return;
    if (clipboardOff() && event.key === "Enter") return enableClipboard();
    const command = commandFor(event, launcher.selectedIndex());
    if (!command) return;
    event.preventDefault();
    // Holding a key moves the selection; it never repeats an action.
    if (event.repeat && command.type !== "move") return;
    switch (command.type) {
      case "move":
        return launcher.move(command.by);
      case "run":
        return launcher.activate(command.index);
      case "runSecondary":
        return launcher.activate(launcher.selectedIndex(), 1);
      case "delete":
        return deleteSelected();
      case "menu":
        return setMenuOpen(true);
      case "category":
        return launcher.moveCategory(command.by);
      case "settings":
        return runGeneral({ type: "openSettings" });
      case "hide":
        // Escape first dismisses a startup warning, then hides.
        if (warnings().length && !launcher.actionError() && !launcher.searchError()) {
          return setWarnings((list) => list.slice(1));
        }
        return void ipc.hideLauncher();
    }
  };

  const menuItems = (): MenuItem[] => {
    const result = launcher.selected();
    const resultItems = (result?.actions ?? []).map((action) => ({
      label: action.label,
      keys: result && shortcutFor(result, action.action),
      run: () => launcher.run(action, result),
    }));
    const general = GENERAL_ACTIONS.map((action) => ({
      label: action.label,
      keys: action.action.type === "openSettings" ? [modKey(), ","] : undefined,
      run: () => launcher.run(action),
    }));
    return [...resultItems, ...general];
  };

  const clipboardOff = () =>
    launcher.category() === "clipboard" && settings()?.clipboardHistoryEnabled === false;

  const enableClipboard = () => {
    const current = settings();
    if (!current) return;
    ipc
      .updateSettings({ ...current, clipboardHistoryEnabled: true })
      .then(applySettings)
      .catch((error) => setWarnings([ipc.message(error)]));
  };

  const notice = () => launcher.actionError() ?? launcher.searchError() ?? warnings()[0];

  return (
    <div class="launcher" data-platform={platform()} onKeyDown={onKeyDown}>
      <header class="search">
        <Glyph name="search" size={20} />
        <input
          ref={input}
          class="search-input"
          role="combobox"
          aria-expanded={launcher.results().length > 0}
          aria-controls={launcher.results().length ? "results" : undefined}
          aria-activedescendant={
            launcher.results().length ? optionId(launcher.selectedIndex()) : undefined
          }
          placeholder="Search apps, files, clipboard, and more"
          value={launcher.query()}
          onInput={(event) => launcher.setQuery(event.currentTarget.value)}
          autocomplete="off"
          autocorrect="off"
          autocapitalize="off"
          spellcheck={false}
        />
      </header>

      <nav class="tabs" role="tablist" aria-label="Categories">
        <For each={CATEGORIES}>
          {(category) => (
            <button
              type="button"
              role="tab"
              class="tab"
              aria-selected={launcher.category() === category.id}
              tabIndex={-1}
              onMouseDown={(event) => event.preventDefault()}
              onClick={() => launcher.setCategory(category.id)}
            >
              {category.label}
            </button>
          )}
        </For>
        <span class="tabs-hint">
          <Keys keys={["Tab"]} /> next
        </span>
      </nav>

      <Show when={notice()}>
        {(text) => (
          <div class="notice" role="alert">
            <Glyph name="warning" size={16} />
            <span>{text()}</span>
            <Show when={!launcher.actionError() && !launcher.searchError()}>
              <button
                type="button"
                class="link"
                onClick={() => {
                  setWarnings((list) => list.slice(1));
                  focusInput();
                }}
              >
                Dismiss <Keys keys={["esc"]} />
              </button>
            </Show>
          </div>
        )}
      </Show>

      <main class="body">
        <Show
          when={!clipboardOff()}
          fallback={
            <Empty title="Clipboard history is off">
              <p>TinyDash saves text you copy, on this computer only. Passwords are skipped.</p>
              <button type="button" class="button primary" onClick={enableClipboard}>
                Turn On Clipboard History
              </button>
            </Empty>
          }
        >
          <Show
            when={launcher.results().length > 0}
            fallback={
              <EmptyResults
                query={launcher.query()}
                category={launcher.category()}
                onSettings={() => runGeneral({ type: "openSettings" })}
              />
            }
          >
            <ResultList
              results={launcher.results()}
              selectedIndex={launcher.selectedIndex()}
              onSelect={launcher.select}
              onRun={(index) => {
                launcher.select(index);
                launcher.activate(index);
              }}
            />
            <PreviewPane
              result={launcher.selected()}
              revision={launcher.revision()}
              disabled={launcher.running()}
              onRun={(action) => launcher.run(action, launcher.selected())}
            />
          </Show>
        </Show>
      </main>

      <footer class="footer">
        <Show when={launcher.selected()?.actions[0]}>
          {(action) => (
            <span class="footer-primary">
              {action().label} <Keys keys={["↩"]} />
            </span>
          )}
        </Show>
        <button
          type="button"
          class="footer-actions"
          onMouseDown={(event) => event.preventDefault()}
          onClick={() => setMenuOpen(true)}
        >
          Actions <Keys keys={[modKey(), "K"]} />
        </button>
      </footer>

      <Show when={menuOpen()}>
        <ActionMenu
          items={menuItems()}
          onClose={() => {
            setMenuOpen(false);
            focusInput();
          }}
        />
      </Show>

      <Show when={launcher.pending()}>
        {(pending) => (
          <ConfirmDialog
            message={pending().action.confirm ?? ""}
            confirmLabel={pending().action.label}
            onConfirm={() => {
              launcher.confirm();
              focusInput();
            }}
            onCancel={() => {
              launcher.cancel();
              focusInput();
            }}
          />
        )}
      </Show>
    </div>
  );
}

function Empty(props: { title: string; children?: JSX.Element }) {
  return (
    <div class="empty">
      <h2>{props.title}</h2>
      {props.children}
    </div>
  );
}

/** What an empty list says, by category, before and after typing. */
function EmptyResults(props: { query: string; category: Category; onSettings: () => void }) {
  return (
    <Switch>
      <Match when={props.query.trim()}>
        <Empty title="No results">
          <p>Nothing matches “{props.query.trim()}”.</p>
        </Empty>
      </Match>
      <Match when={props.category === "clipboard"}>
        <Empty title="Nothing copied yet">
          <p>Text you copy appears here, newest first.</p>
        </Empty>
      </Match>
      <Match when={props.category === "snippets"}>
        <Empty title="No snippets yet">
          <p>Save text you type often, or links you open with a keyword.</p>
          <button type="button" class="button primary" onClick={() => props.onSettings()}>
            Add in Settings
          </button>
        </Empty>
      </Match>
      <Match when={props.category === "files"}>
        <Empty title="Search your files">
          <p>Type part of a file or folder name. Choose the folders in Settings.</p>
        </Empty>
      </Match>
      <Match when={true}>
        <Empty title="What do you need?">
          <ul class="tips">
            <li>Type an app, file, snippet, or emoji name</li>
            <li>
              Calculate <code>12 * 8</code>, <code>5 ft to cm</code>, <code>100 usd to eur</code>
            </li>
            <li>
              Ask <code>time in tokyo</code> or <code>next friday + 2 weeks</code>
            </li>
            <li>
              Generate <code>password</code>, or search the web with <code>g rust</code>
            </li>
          </ul>
        </Empty>
      </Match>
    </Switch>
  );
}
