# Alias

The user gives a result a one-word alias from the Actions menu. Typing the alias puts that result first. The alias is stored in `tinydash.db` and survives a restart.

## Sub-features

- `alias-menu`: `Ctrl+K` on a selected result lists `Add Alias…` among its actions, and the menu filter narrows the list.
- `alias-dialog`: `Add Alias…` opens the `Alias for <title>` dialog with a focused field.
- `alias-save`: `Save Alias` stores the alias (an `aliases` row) and closes the dialog.
- `alias-match`: typing the alias puts the result first, selected.
- `alias-invalid`: a value with a space shows `An alias is one word of up to 32 characters.`
- `alias-change`: the action then reads `Change Alias “<alias>”…`.

## How to get to it (user POV)

- Search for a result, keep it selected, press `Ctrl+K` (or click `Actions`), and choose `Add Alias…`.

## Driving it with verify.sh

Preconditions:

- A freshly launched box. `$V doctor` passes, and `TinyDash` is `focused`.
- `$V sql 'select count(*) from aliases'` prints `0`.

- **Find the file.** Run `$V auto type "roadmap" --settle`. The diff adds `[listItem] "roadmap-2027.md" key=file:/root/Documents/projects/roadmap-2027.md`, `checked`.
- **Open the menu.** Run `$V auto key CmdOrCtrl+K --settle`. The diff adds `[dialog] "Actions"` with `[textField] "Search actions" focused` and the items `Open`, `Show in Files`, `Copy Path`, `Pin`, `Add Alias…`, `Set Hotkey…`, `Hide from Results`, then the general actions. Run `$V evidence alias-0-menu`.
- **Choose Add Alias.** Run `$V auto type "alias" --settle`. `Add Alias…` is the only item and is `checked`. Run `$V auto key Enter --settle`. The diff adds `[dialog] "Alias for roadmap-2027.md"` and its `[textField]`, which is `focused`.
- **Save.** Run `$V auto type "rmap" --settle`. Keep the dialog's ref: `D=$($V ref '[dialog] "Alias for')`. Then run `$V auto press "$($V ref '[button] "Save Alias"')" --settle`. The buttons turn `disabled` while it saves. `$V auto wait --gone "$D" --timeout 5s` prints `ok` when the dialog closes.
- **Side effect: stored.** Run `$V sql 'select id, alias from aliases' | tee .local/verify/runs/$TD_RUN/alias-1-rows.sql.txt`. It prints `file:/root/Documents/projects/roadmap-2027.md|rmap`.
- **Use the alias.** Empty the field: `$V auto fill "$($V ref '[textField] "Search"')" "" --settle`. Then run `$V auto type "rmap" --settle`. The first `listItem` is `"roadmap-2027.md"`, `checked`, above the emoji matches and the web fallback. Run `$V evidence alias-2-match`.

## Gotchas

- Every ref goes stale when the menu or the dialog opens or closes, the search field's ref too. Use `$V ref` again after each overlay.
- After the dialog closes, the search field keeps `roadmap`, and `type` appends to it (`roadmaprmap`). Empty it first.
- Enter in the menu filter runs the `checked` item. Filter until the item you want is the only one, or the first one.
- `Pin` is a one-step action in the same menu. Prove it with `select id, position from pins` and the `Pinned` icon label in the row.
