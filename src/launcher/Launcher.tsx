import { createSignal, For, type JSX, Match, onCleanup, onMount, Show, Switch } from "solid-js";

import type { Action } from "../generated/Action";
import type { Category } from "../generated/Category";
import type { Platform } from "../generated/Platform";
import type { ResultAction } from "../generated/ResultAction";
import type { Settings } from "../generated/Settings";
import * as ipc from "../lib/ipc";
import { isComposing, modKey, shortcutKey } from "../lib/keys";
import { CATEGORY_LABELS, OPTIONAL_TABS } from "../lib/categories";
import { applyTheme } from "../lib/theme";
import { ConfirmDialog } from "../ui/ConfirmDialog";
import { Glyph } from "../ui/Icon";
import { Keys } from "../ui/Keys";
import { ActionMenu, type MenuItem } from "./ActionMenu";
import { shortcutFor } from "./describe";
import { commandFor } from "./keymap";
import { PreviewPane } from "./PreviewPane";
import { optionId, ResultList } from "./ResultList";
import { createLauncher } from "./state";
import { hasWidgets, inNote, type PaneControls, WidgetPane } from "./WidgetPane";

/** Keys that change the query: characters (AltGr included), deletion, paste. */
function editsQuery(event: KeyboardEvent) {
  const mod = event.metaKey || (event.ctrlKey && !event.altKey);
  if (event.key.length === 1) return !mod || shortcutKey(event) === "v";
  return event.key === "Backspace" || event.key === "Delete";
}

/**
 * Actions that belong to no result, listed in the actions menu. Refresh also
 * downloads exchange rates when they are on, so its label says so.
 */
const generalActions = (rates: boolean): ResultAction[] => [
  {
    label: rates ? "Refresh Apps, Files, and Rates" : "Refresh Apps and Files",
    action: { type: "refresh" },
    confirm: null,
  },
  { label: "Center Launcher", action: { type: "centerLauncher" }, confirm: null },
  { label: "Settings", action: { type: "openSettings" }, confirm: null },
  { label: "Quit TinyDash", action: { type: "quit" }, confirm: null },
];

/** The System command's action, with its words (`features/system.rs`). */
const clearHistory: ResultAction = {
  label: "Clear Clipboard History",
  action: { type: "clearClipboard" },
  confirm: "Delete all clipboard history except pinned entries?",
};

export function Launcher() {
  const launcher = createLauncher();
  const [settings, setSettings] = createSignal<Settings>();
  const [platform, setPlatform] = createSignal<Platform>();
  const [warnings, setWarnings] = createSignal<string[]>([]);
  // The open actions menu keeps the items it showed when it opened, so a
  // refresh or another tab cannot change what Enter runs in it.
  const [menu, setMenu] = createSignal<MenuItem[]>();
  const [enableError, setEnableError] = createSignal<string>();
  // A newer version that is ready to install, and how its install went.
  const [update, setUpdate] = createSignal<string>();
  const [updateError, setUpdateError] = createSignal<string>();
  const [installing, setInstalling] = createSignal(false);
  // Not while it installs, also from a menu opened earlier: hiding the bar
  // then would hide a failure.
  const hideUpdate = () => {
    if (!installing()) setUpdate(undefined);
  };
  const installUpdate = () => {
    if (installing()) return;
    setInstalling(true);
    setUpdateError(undefined);
    // On success the app restarts, so only a failure comes back.
    ipc
      .installUpdate()
      .catch((error) => setUpdateError(ipc.message(error)))
      .finally(() => setInstalling(false));
  };
  let input!: HTMLInputElement;
  // Set while the widget pane is on screen.
  let pane: PaneControls | undefined;

  const focusInput = () => input.focus();

  const applySettings = (next: Settings) => {
    // A newer save happened, so an old failure to turn on history is stale.
    setEnableError(undefined);
    setSettings(next);
    applyTheme(next.theme);
  };

  onMount(() => {
    const listeners = [
      ipc.onLauncherShown(({ category }) => {
        setMenu(undefined);
        setEnableError(undefined);
        launcher.reset(category);
        focusInput();
      }),
      ipc.onResultsStale(() => void launcher.refresh()),
      ipc.onSettingsChanged(applySettings),
      ipc.onUpdateChanged((version) => {
        setUpdateError(undefined);
        setUpdate(version ?? undefined);
      }),
    ];
    // Listen on the window: a click on a row or the preview moves focus to
    // <body>, and the shortcuts must keep working.
    window.addEventListener("keydown", onKeyDown);
    // An Enter that waits for results must not run after the user left.
    window.addEventListener("blur", launcher.cancelQueued);
    onCleanup(() => {
      window.removeEventListener("keydown", onKeyDown);
      window.removeEventListener("blur", launcher.cancelQueued);
      listeners.forEach((listener) => void listener.then((stop) => stop()));
    });
    ipc
      .launcherInit()
      .then((init) => {
        applySettings(init.settings);
        setPlatform(init.platform);
        setWarnings(init.warnings);
        if (init.update) setUpdate(init.update);
        if (init.category) launcher.setCategory(init.category);
      })
      .catch((error) => setWarnings([ipc.message(error)]));
    void launcher.refresh();
    focusInput();
  });

  /** Actions that belong to no result, such as opening Settings. */
  const runGeneral = (action: Action) => launcher.run({ label: "", action, confirm: null });

  // While the Clipboard tab says history is off, it shows no results, even
  // before its search returns; keys and hints must not act on hidden ones.
  const results = () => (clipboardOff() ? [] : launcher.results());
  const selected = () => (clipboardOff() ? undefined : launcher.selected());
  const canClearHistory = () => launcher.category() === "clipboard" && !clipboardOff();

  // All, then the tabs that Settings shows (every tab until it loads). A
  // hidden tab opened on purpose (`--mode clipboard`) shows while it is
  // open, so the view has a name.
  const tabs = (): Category[] => {
    const chosen = settings()
      ?.tabs.filter((tab) => tab.shown)
      .map((tab) => tab.category);
    const shown: Category[] = ["all", ...(chosen ?? OPTIONAL_TABS)];
    return shown.includes(launcher.category()) ? shown : [...shown, launcher.category()];
  };

  // Deleting cannot be undone, so it never waits for newer results: it acts
  // only on the highlighted entry the user can see.
  const deleteAction = () =>
    selected()?.actions.find((entry) => entry.action.type === "deleteClip");
  const canDelete = () => launcher.isCurrent() && deleteAction() !== undefined;
  const deleteSelected = () => {
    const action = deleteAction();
    if (action && canDelete()) launcher.run(action, selected());
  };

  const onKeyDown = (event: KeyboardEvent) => {
    if (event.defaultPrevented || isComposing(event) || menu() || launcher.pending()) return;
    if (clipboardOff() && event.key === "Enter") {
      // A focused Turn On button would also click on Enter, and on each repeat.
      event.preventDefault();
      if (!event.repeat) enableClipboard();
      return;
    }
    const command = commandFor(event);
    // Keys in the note type there; only Mod+J leaves it, as Escape does.
    if (inNote(event.target)) {
      if (command?.type !== "note") return;
      event.preventDefault();
      return focusInput();
    }
    if (!command) {
      // Typing, deleting, and pasting go to the search field, even after a
      // click moved focus. Other keys (Mod+C on selected preview text, a
      // lone modifier) leave focus where it is.
      if (document.activeElement !== input && editsQuery(event)) focusInput();
      return;
    }
    // Mod+Backspace deletes a clipboard entry; anywhere else it edits the
    // text, in the search field even after a click moved focus.
    if (command.type === "delete" && !canDelete()) {
      if (document.activeElement !== input) focusInput();
      return;
    }
    event.preventDefault();
    // Holding a key moves the selection; it never repeats an action.
    if (event.repeat && command.type !== "move") return;
    switch (command.type) {
      case "move":
        return launcher.move(command.by);
      case "run":
        return launcher.activate("selected");
      case "runRow":
        if (!clipboardOff()) launcher.activate(command.index);
        return;
      case "runSecondary":
        return launcher.activate("selected", 1);
      case "delete":
        return deleteSelected();
      case "menu":
        return openMenu();
      case "category":
        return launcher.moveCategory(command.by, tabs());
      case "settings":
        return runGeneral({ type: "openSettings" });
      case "note":
        if (!(showsWidgets() && pane?.focusNote())) focusInput();
        return;
      case "hide":
        // Escape first dismisses a startup warning, then hides.
        if (warnings().length && !launcher.actionError() && !launcher.searchError()) {
          return setWarnings((list) => list.slice(1));
        }
        launcher.cancelQueued();
        return void ipc.hideLauncher();
    }
  };

  const menuItems = (): MenuItem[] => {
    const result = selected();
    const resultItems = (result?.actions ?? []).map((action) => ({
      label: action.label,
      keys: result && shortcutFor(result, action.action),
      run: () => launcher.run(action, result),
    }));
    // On the Clipboard tab, clearing it is one key away, not a trip to System.
    const general = [
      ...(canClearHistory() ? [clearHistory] : []),
      ...generalActions(settings()?.currencyRatesEnabled ?? false),
    ].map((action) => ({
      label: action.label,
      keys: action.action.type === "openSettings" ? [modKey(), ","] : undefined,
      run: () => launcher.run(action),
    }));
    // The update bar's buttons take a click; the menu takes the keyboard.
    const version = update();
    const install = version
      ? [
          { label: `Install TinyDash ${version} and Restart`, keys: undefined, run: installUpdate },
          ...(installing()
            ? []
            : [{ label: "Hide Update Notice", keys: undefined, run: hideUpdate }]),
        ]
      : [];
    return [...resultItems, ...install, ...general];
  };

  // Only results of the current input: rows on screen during a search may
  // be gone when it returns, and the menu would keep acting on them.
  const openMenu = () => {
    if (launcher.isCurrent()) setMenu(menuItems());
  };
  const closeMenu = () => {
    setMenu(undefined);
    focusInput();
  };

  // The widget pane takes the preview's place while All has no query.
  const showsWidgets = () => {
    const current = settings();
    return (
      launcher.category() === "all" &&
      launcher.query() === "" &&
      current !== undefined &&
      hasWidgets(current)
    );
  };

  const clipboardOff = () =>
    launcher.category() === "clipboard" && settings()?.clipboardHistoryEnabled === false;

  const enableClipboard = () => {
    setEnableError(undefined);
    ipc
      .updateSettings({ clipboardHistoryEnabled: true })
      .then(applySettings)
      .catch((error) => setEnableError(ipc.message(error)));
  };

  const notice = () => launcher.actionError() ?? launcher.searchError() ?? warnings()[0];

  // The window has no title bar, so the empty parts of the tab bar and the
  // footer move it. Cancelled, so focus stays in the search field.
  const dragFromEmptySpace = (event: MouseEvent) => {
    if (event.button !== 0 || (event.target as Element).closest("button")) return;
    event.preventDefault();
    // A desktop may refuse a drag (some Wayland compositors); nothing to undo.
    ipc.dragLauncher().catch(() => undefined);
  };

  return (
    <div class="launcher" data-platform={platform()}>
      <header class="search">
        <Glyph name="search" size={20} />
        <input
          ref={input}
          class="search-input"
          role="combobox"
          aria-expanded={results().length > 0}
          aria-controls={results().length ? "results" : undefined}
          aria-activedescendant={results().length ? optionId(launcher.selectedIndex()) : undefined}
          placeholder="Search apps, files, clipboard, and more"
          value={launcher.query()}
          onInput={(event) => launcher.setQuery(event.currentTarget.value)}
          autocomplete="off"
          autocorrect="off"
          autocapitalize="off"
          spellcheck={false}
        />
      </header>

      <nav class="tabs" aria-label="Categories" onMouseDown={dragFromEmptySpace}>
        {/* The hint stays outside the tab list, which may hold only tabs. */}
        <div class="tab-list" role="tablist" aria-label="Categories">
          <For each={tabs()}>
            {(category) => (
              <button
                type="button"
                role="tab"
                class="tab"
                aria-selected={launcher.category() === category}
                tabIndex={-1}
                onMouseDown={(event) => event.preventDefault()}
                onClick={() => {
                  if (menu()) closeMenu();
                  launcher.setCategory(category);
                }}
              >
                {CATEGORY_LABELS[category]}
              </button>
            )}
          </For>
        </div>
        <Show when={tabs().length > 1}>
          <span class="tabs-hint">
            <Keys keys={["Tab"]} /> next
          </span>
        </Show>
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

      <Show when={update()}>
        {(version) => (
          <div class="update-offer" role="status">
            <Glyph name="download" size={16} />
            <span>{updateError() ?? `TinyDash ${version()} is available.`}</span>
            <button
              type="button"
              class="link"
              disabled={installing()}
              onMouseDown={(event) => event.preventDefault()}
              onClick={installUpdate}
            >
              {installing() ? "Installing…" : "Install and Restart"}
            </button>
            <button
              type="button"
              class="link"
              disabled={installing()}
              onMouseDown={(event) => event.preventDefault()}
              onClick={hideUpdate}
            >
              Later
            </button>
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
              <Show when={enableError()}>
                {(text) => (
                  <p class="empty-error" role="alert">
                    {text()}
                  </p>
                )}
              </Show>
            </Empty>
          }
        >
          <Show
            when={launcher.results().length > 0}
            fallback={
              // After a failed search the notice explains; "No results" would not be true.
              <Show when={!launcher.searchError()}>
                <EmptyResults
                  query={launcher.query()}
                  category={launcher.category()}
                  beside={showsWidgets()}
                  onSettings={() => runGeneral({ type: "openSettings" })}
                />
              </Show>
            }
          >
            <ResultList
              results={launcher.results()}
              selectedIndex={launcher.selectedIndex()}
              onSelect={launcher.select}
              onRun={(index) => {
                // A click runs the row the user sees, whatever is still loading.
                const result = launcher.results()[index];
                const action = result?.actions[0];
                launcher.select(index);
                if (result && action) launcher.run(action, result);
              }}
            />
            <Show when={!showsWidgets()}>
              <PreviewPane
                result={launcher.selected()}
                revision={launcher.revision()}
                disabled={launcher.running()}
                onRun={(action) => launcher.run(action, launcher.selected())}
              />
            </Show>
          </Show>
          <Show when={showsWidgets()}>
            <WidgetPane
              controls={(controls) => {
                pane = controls;
                onCleanup(() => (pane = undefined));
              }}
              onLeave={focusInput}
            />
          </Show>
        </Show>
      </main>

      <footer class="footer" onMouseDown={dragFromEmptySpace}>
        <Show when={selected()?.actions[0]}>
          {(action) => (
            <span class="footer-primary">
              {action().label} <Keys keys={["↩"]} />
            </span>
          )}
        </Show>
        <Show when={canClearHistory()}>
          <button
            type="button"
            class="footer-actions footer-clear"
            onMouseDown={(event) => event.preventDefault()}
            onClick={() => launcher.run(clearHistory)}
          >
            Clear History…
          </button>
        </Show>
        <button
          type="button"
          class="footer-actions"
          onMouseDown={(event) => event.preventDefault()}
          onClick={() => (menu() ? closeMenu() : openMenu())}
        >
          Actions <Keys keys={[modKey(), "K"]} />
        </button>
      </footer>

      <Show when={menu()}>{(items) => <ActionMenu items={items()} onClose={closeMenu} />}</Show>

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

/** `beside` keeps it in the left column, next to the widget pane. */
function Empty(props: { title: string; beside?: boolean; children?: JSX.Element }) {
  return (
    <div class="empty" classList={{ beside: props.beside }}>
      <h2>{props.title}</h2>
      {props.children}
    </div>
  );
}

/** What an empty list says, by category, before and after typing. */
function EmptyResults(props: {
  query: string;
  category: Category;
  beside: boolean;
  onSettings: () => void;
}) {
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
        <Empty title="What do you need?" beside={props.beside}>
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
