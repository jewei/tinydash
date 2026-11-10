# TinyDash (Go port) verification map

This folder is the maintained source for verifying what a user does in the TinyDash launcher panel. Read this index before you drive the app. Then use the matching feature file as the recipe.

## Baseline preconditions

- `V=.claude/skills/verify-tinydash/verify.sh`. Run from the repo root.
- `export TD_RUN=<unique name>`, so that concurrent runs do not share a box.
- `$V build`, then `$V launch`. The launch seeds `settings.json` (`fileSearchFolders: ["~/Documents"]`, all other keys default), three files under `~/Documents`, and the app "Verify Notes".
- `$V doctor` exits 0, with no `FAIL` line and no build `WARN`.
- The panel shows: `$V auto windows` prints `w1 [content] "TinyDash" focused`.
- Never drive a box that this run did not launch.

## Driving conventions

- Start every recipe from a freshly launched box unless its preconditions say otherwise. Recipes that change data (usage, calculations, clipboard, aliases) change the ranking of later recipes in the same box.
- Prefer result-row keys (`key=file:/root/…`) and labels (`[tab] "Clipboard"`) over refs. Refs change when an overlay opens or closes, so take them from the latest diff, or run `$V ref '<text>'`.
- Type with `$V auto type` and press keys with `$V auto key`, as the user does. Use `press` only for buttons and tabs that the user clicks.
- Press the global shortcut only through `$V show` and `$V hide`, and run other X-level actions (a copy by another app) only through `$V box`. They reach the container's display `:99`.
- Wait for state (`wait --text`, `wait --gone`, `--settle`), never a fixed sleep. The one exception is a timing window that the app has on purpose: wait 1 s after Turn On Clipboard History (Linux baseline read). Show and hide the panel with `$V show` and `$V hide`: they press the real shortcut, wait for the window, and `$V show` waits out `blurWithin` (400 ms) after any hide, by Enter, a digit, or the shortcut.

## Proof and skip reporting

- Capture the state before the action and the state after it: `$V evidence <feature>-<step>`.
- Save each side effect to the run folder with `tee`: the SQL rows, the X clipboard, or the marker file.
- Record the feature ID and the entry point with each artifact. The step names in the recipes do this.
- When a path is unreachable, report the command that you tried and the unmet precondition. Do not report it as verified through another path.

## Feature entry contract

Each feature file starts with an H1 title and one paragraph about the user-visible behavior. It then has exactly four H2 sections, in this order:

1. `Sub-features`: short IDs, one line each.
2. `How to get to it (user POV)`: every entry point.
3. `Driving it with verify.sh`: `Preconditions:`, then labeled bullets that pair each action with an exact command and an observable result.
4. `Gotchas`: traps that can waste or invalidate a run.

## Features

- [Search and run](./search-and-run.md): search on the All tab, select, and run an app with Enter or Mod+digit, with usage recorded.
- [Calculator answer](./calculator.md): type an expression, see the answer row, and copy it to the X clipboard and the calculations history.
- [Clipboard history](./clipboard-history.md): turn history on from the Clipboard tab, and see a copy made by another app appear and persist.
- [Global shortcut](./global-shortcut.md): `Control+Shift+Space` on the display shows and hides the panel and resets the query.
- [Alias](./alias.md): add an alias from the Actions menu, then type it to put the result first.
- [Clipboard cards](./clipboard-cards.md): turn cards on in Settings, copy a color or JSON, run the card's main action, and open the full JSON view.
- [Notepad](./notepad.md): turn the Notepad on, type in it with Ctrl+J, and see the note saved, kept, and retried after a failure.

## Known app failures

None. The two this skill found on 2026-11-09 (the shortcut that did not hide a focused panel on X11, and a Quit that only hid the panel) are fixed.

## Last full run

On 2026-11-09, at build `v0.2.0-822-g050947908c40`, all five journeys passed as written, each in a fresh box. The evidence is in `.local/verify/runs/`: `r2docs-search`, `r2docs-calc`, `r2docs-clip`, `r2docs-shortcut`, and `r2docs-alias`.
