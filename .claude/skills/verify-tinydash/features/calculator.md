# Calculator answer

The user types an arithmetic expression in the panel and sees the answer as the first row. Enter copies the answer to the clipboard, hides the panel, and saves the expression in the calculations history. Typing `=` lists that history.

## Sub-features

- `calc-answer`: `12 * 8` shows a row titled `96` with the subtitle `= 12 * 8` and the kind `Calculation`.
- `calc-copy`: Enter (`Copy Result`) puts `96` on the clipboard and hides the panel.
- `calc-history-row`: the copy adds or updates a `calculations` row (`expression`, `result`, `used_at`).
- `calc-history-list`: typing `=` lists the saved calculations.

## How to get to it (user POV)

- Open the panel on the All tab and type an expression.
- Press Enter, or open `Actions` (`Ctrl+K`) and choose `Copy Result`.

## Driving it with verify.sh

Preconditions:

- A freshly launched box. `$V doctor` passes, and `TinyDash` is `focused`.
- `$V sql 'select count(*) from calculations'` prints `0`.

- **Type the expression.** Run `$V auto type "12 * 8" --settle`. The diff adds `[listItem] "96" key="calc:12 * 8"` and a `checked` `"96"` row with `= 12 * 8` and `Calculation`. The footer reads `Copy Result`.
- **Capture before copying.** Run `$V evidence calc-1-answer`.
- **Copy with Enter.** Run `$V auto key Enter --settle`. Then `$V auto windows` says `hidden`.
- **Side effect: X clipboard.** Run `$V box 'xclip -selection clipboard -o' | tee .local/verify/runs/$TD_RUN/calc-2-clipboard.txt`. It prints `96`.
- **Side effect: history row.** Run `$V sql 'select expression, result from calculations' | tee .local/verify/runs/$TD_RUN/calc-2-history.sql.txt`. It prints `12 * 8|96`.
- **Second view.** Show the panel with `$V box 'xdotool key ctrl+shift+space'`, then run `$V auto type "=" --settle`. A row for `12 * 8` with the result `96` is listed. Run `$V evidence calc-3-history`.

## Gotchas

- The panel owns the X clipboard selection after a copy. `xclip -o` reads it from TinyDash while the app runs. After cleanup the copy is gone, so save it to the run folder first.
- The `key=` of a calc row holds spaces. Quote it twice: `key='"calc:12 * 8"'`.
- Currency answers (`100 usd to eur`) need rates. The seed keeps them off, so those rows offer `Turn On Currency Rates`, and turning them on downloads from the network. Leave currency out of this journey.
- With clipboard history on, this copy also appears in the Clipboard tab. Run the calculator journey first, or in a fresh box, when the clipboard journey counts rows.
