# Notepad

The user turns the Notepad widget on in Settings > Widgets. Beside an empty All search, the widget pane then shows one scratch note. `Ctrl+J` moves the keyboard to it, and `Ctrl+J`, `Escape`, `Tab`, or `Shift+Tab` moves it back to the search field. The note saves after a pause in typing, when the user leaves it, and when a search starts or the launcher hides. It is the row `id = 1` of the `note` table in `tinydash.db`.

## Sub-features

- `notepad-switch`: the `[switch] "Notepad"` in Settings > Widgets is off by default and writes `"showNotepad": true`.
- `notepad-focus`: `Ctrl+J` focuses `[textField] "Notepad" multiline`; `Escape` focuses `[textField] "Search"` again and does not hide the panel.
- `notepad-save`: typing shows `[status] "<n> words · Saving…"`, then `"<n> words · Saved"`, and the `note` row holds the text. Enter in the note breaks the line and runs no result.
- `notepad-kept`: the text is still there after a search and after a hide and show.
- `notepad-failed`: a save that fails shows `Not saved` and an `[alert]` with the reason; the text stays, and the next save after the pane comes back succeeds.
- `notepad-unreadable`: a note that cannot be read shows `Could not read the note. …` in its card, and the clocks and disk space still show.

## How to get to it (user POV)

- Settings (`Ctrl+,` in the launcher) > Widgets > Notepad. Then an empty All search, and `Ctrl+J` or a click in the note.

## Driving it with verify.sh

Preconditions:

- A freshly launched box. `$V doctor` passes, and `TinyDash` is `focused`.

- **Turn it on.** Run `$V auto key CmdOrCtrl+, --settle`, then `$V auto window w3` (`"TinyDash Settings"`). Press `[button] "Widgets"`, then `[switch] "Notepad"`. `$V box 'grep showNotepad /scratch/data/settings.json'` prints `"showNotepad": true`. Run `$V show`, then `$V auto window w1`.
- **Type.** Run `$V auto key CmdOrCtrl+J --settle`: the note is `focused`. Run `$V auto type "Buy milk" --settle`, `$V auto key Enter --settle`, and `$V auto type "Call Sam" --settle`. Then `$V auto wait --text "4 words · Saved" --timeout 5s` prints `ok`. Run `$V evidence notepad-1-saved`.
- **Side effect: stored.** `$V sql 'select id, text from note' | tee .local/verify/runs/$TD_RUN/notepad-1-rows.sql.txt` prints `1|Buy milk` and `Call Sam`.
- **Leave and search.** Run `$V auto key Escape --settle`: `[textField] "Search"` is `focused`, and `$V auto windows` still shows `TinyDash` `focused`. Type a query, empty the field, then `$V hide` and `$V show`: the note has the same value.
- **A failed save.** Run `$V box 'sqlite3 /scratch/data/tinydash.db "ALTER TABLE note RENAME TO note_moved"'`. Type in the note, and wait for `Not saved`. Leave the note and type a query. Rename the table back, and empty the field: `$V auto wait --text "Saved" --timeout 5s` prints `ok`, and the row holds the new text.
- **An unreadable note.** Rename the table away again, then `$V hide` and `$V show`. The `[group] "Notepad"` holds `Could not read the note. …`, beside `Clocks` and `Disk space`.

## Gotchas

- The Settings window takes the focus. Pick `w1` again with `$V auto window w1` after `$V show`, or every command goes to Settings.
- `CmdOrCtrl+Comma` is not a key name for `mygo-automation`; use `CmdOrCtrl+,`.
- The note's text shows in the tree only as the field's `value=`, not as `[text]` lines.
- A save ends on a goroutine. After an action that saves, wait for the status text, not for `--settle`.
