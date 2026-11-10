# Search and run

On the All tab, the user types part of a name and sees ranked results from every source, with a web-search fallback last. Enter runs the selected result. For an app, Enter launches the app and hides the panel, and TinyDash records the use so that the app ranks higher next time.

## Sub-features

- `search-all-match`: typing on the All tab lists the matching app, files, and a `Search Google for “…”` fallback.
- `search-select`: the first row is selected (`checked`), and Down moves the selection.
- `search-run-enter`: Enter runs the selected row's main action (`Open`, shown in the footer).
- `search-run-digit`: `CmdOrCtrl+<n>` runs row n without moving the selection.
- `search-usage`: a run adds a `usage` row, and the app then shows on the empty All tab.
- `search-open-error`: a run that fails keeps the panel and shows the error as an alert, with no usage recorded.
- `search-empty`: a query with no match shows only the web fallback.

## How to get to it (user POV)

- Open the panel. It shows at launch, or with `Control+Shift+Space`. The All tab is chosen and the search field has focus.
- Type in the search field.
- Press Enter, or `Ctrl+1`…`Ctrl+9`, or click a row.

## Driving it with verify.sh

Preconditions:

- A freshly launched box. `$V doctor` passes, and the log says `apps=1` and `files=4`.
- `$V auto windows` shows `TinyDash` `focused`, and `$V box 'ls /scratch/launched-verify-notes'` fails (the app has not run).

- **Baseline.** Take the empty state. Run `$V auto snapshot -i` and `$V evidence search-0-empty`. `[tab] "All" checked` and `[textField] "Search" focused` show.
- **Type a query.** Run `$V auto type "verify notes" --settle`. The diff adds `[list] "Results"`, `[listItem] "Verify Notes" key=app:/root/.local/share/applications/verify-notes.desktop`, a second `"Verify Notes"` line marked `checked`, and `Search Google for “verify notes”`. The footer text changes to `Open`.
- **Capture before running.** Run `$V evidence search-1-results`. The PNG shows the highlighted `Verify Notes` row with `Application` and `Ctrl 1`.
- **Run with Enter.** Run `$V auto key Enter --settle`. Then run `$V auto windows`: the panel is `hidden`.
- **Side effect: launched.** Run `$V box 'cat /scratch/launched-verify-notes' | tee .local/verify/runs/$TD_RUN/search-2-launched.txt`. It prints `launched`.
- **Side effect: usage.** Run `$V sql 'select id, count from usage' | tee .local/verify/runs/$TD_RUN/search-2-usage.sql.txt`. The output has a row `app:/root/.local/share/applications/verify-notes.desktop|1`.
- **Second view.** Run `$V show`. Run `$V auto snapshot`: with an empty query, the All tab now lists `Verify Notes`. Run `$V evidence search-3-suggested`.
- **Run by digit.** Remove the marker with `$V box 'rm /scratch/launched-verify-notes'`. Empty the field with `$V auto fill "$($V ref '[textField] "Search"')" "" --settle`, then run `$V auto type "verify" --settle` and `$V auto key CmdOrCtrl+1 --settle`. `$V auto windows` says `hidden`, the marker says `launched` again, and `$V sql 'select id, count from usage'` shows `count` `2`.
- **Open error.** Run `$V show`, then type `$V auto type "grocery" --settle`, and press `$V auto key CmdOrCtrl+1 --settle`. The box has no handler for `.md` files, so the panel stays, and an `[alert]` reads `Could not open /root/Documents/grocery-list.md: No application is registered as handling this file. …`. No `usage` row is added for the file. Run `$V evidence search-4-file-open-error`.
- **No match.** The panel is still shown, with `grocery` in the field. Empty it with `$V auto fill "$($V ref '[textField] "Search"')" "" --settle`, then type `$V auto type "zzqx" --settle`. The only row is `Search Google for “zzqx”`. Do not press Enter: it opens a browser.

## Gotchas

- Each result shows as two `listItem` lines. Only the one with `key=` has the key, and only the inner one gets `checked`.
- The box has no browser and no file handler. A file's `Open` fails with an alert (see `search-open-error`). Do not press Enter on a web row: with no browser the result is not a proof of anything. Prove file and web opens on a real desktop.
- The box has no color emoji font, so emoji rows show a blank icon. That is the box, not the app.
- A recorded use changes the ranking and the empty-All suggestions. Start from a fresh box when a recipe needs the default order.
- `$V auto type` appends to the current text. After a dialog, the search field keeps its text; empty it first with `fill <ref> ""`. Get the ref with `$V ref '[textField] "Search"'`.
