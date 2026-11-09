# Clipboard history

Clipboard history is off for a new user. On the Clipboard tab the user turns it on with one button. From then on, TinyDash saves the text that any app copies, newest first, and keeps it in `tinydash.db`. On Linux it keeps text only.

## Sub-features

- `clip-off-state`: the Clipboard tab shows `Clipboard history is off` and `Turn On Clipboard History` while history is off.
- `clip-enable`: the button turns history on and saves `"clipboardHistoryEnabled": true` in `settings.json`.
- `clip-capture`: text that another app copies appears as a row (`Text · Just now`) within about a second.
- `clip-persist`: each capture is a `clipboard` row (`kind`, `title`, `content`).
- `clip-copy-back`: Enter on a row copies it to the clipboard again.
- `clip-clear`: the footer `Clear History…` asks for confirmation, then deletes all entries except pinned ones.

## How to get to it (user POV)

- Open the panel and choose the `Clipboard` tab: click it, or press Tab until it is chosen.
- Copy text in any app.

## Driving it with verify.sh

Preconditions:

- A freshly launched box (history off). `$V doctor` passes, and `TinyDash` is `focused`.
- `$V sql 'select count(*) from clipboard'` prints `0`.

- **Open the tab.** Run `$V auto press "$($V ref '[tab] "Clipboard"')" --settle`. The diff shows `[tab] "Clipboard" checked`, `[heading] "Clipboard history is off"`, and `[button] "Turn On Clipboard History"`. Run `$V evidence clip-0-off`.
- **Turn it on.** Run `$V auto press "$($V ref '"Turn On Clipboard History"')"`. Then run `$V box 'grep clipboardHistoryEnabled /scratch/data/settings.json' | tee .local/verify/runs/$TD_RUN/clip-1-settings.txt`. It prints `"clipboardHistoryEnabled": true,`.
- **Copy in another app.** Run `$V box 'printf "verify clip 7f3a" | xclip -selection clipboard -i'`. xclip is another X client, as a text editor would be.
- **See it captured.** Run `$V auto wait --text "verify clip 7f3a" --timeout 5s`. It prints `ok`. Then run `$V auto snapshot` and `$V evidence clip-2-captured`. The list shows `[listItem] "verify clip 7f3a" key=clip:<n>` with `Text · Just now`.
- **Side effect: stored.** Run `$V sql 'select kind, title, content from clipboard' | tee .local/verify/runs/$TD_RUN/clip-2-rows.sql.txt`. The output has `text|verify clip 7f3a|verify clip 7f3a`.
- **Copy back.** Select the row (`$V auto key Down --settle` until it is `checked`), then run `$V auto key Enter --settle`. `$V box 'xclip -selection clipboard -o'` prints the row's text.

## Gotchas

- The search text filters the Clipboard tab too. With a leftover query, a capture is stored but the tab shows `No results`. Empty the field first.
- The press on `Turn On Clipboard History` returns `(no changes)` with `--settle`: the setting is saved on a goroutine, and the view draws afterwards. Wait for the captured text, not for the diff.
- When history turns on, the text that is already on the clipboard (for example `96` from the calculator journey) is captured too. Count rows from a fresh box.
- The selection stays on the row it was on when a new capture arrives (it is kept by ID). The newest row is first, but it is not always `checked`.
- The monitor polls every 500 ms and compares the text. Two copies of the same text make one row.
- On Linux, images and copied files are never kept (`RichClipboard = false`). Do not try to prove them here.
- Never use the Mac's `pbcopy`/`pbpaste`. Only `xclip` inside the box.
