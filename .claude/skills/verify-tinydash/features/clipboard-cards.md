# Clipboard cards

Clipboard cards are off for a new user. The user turns them on in Settings > Widgets. Then, each time the panel shows on an empty All search, the widget pane shows a card for the clipboard text when it is a color, a Unix time, or JSON. Mod+Shift+Enter runs the card's main action: it copies a color as hex or a time as local time, or it opens the full JSON view.

## Sub-features

- `cards-enable`: the `Clipboard cards` switch saves `"showClipboardCards": true` in `settings.json`.
- `cards-color`: a copied `rgb(47 111 94)` shows `[group] "Color on the clipboard"` with `from RGB` and `#2F6F5E`.
- `cards-main`: Mod+Shift+Enter on a color card copies `#2F6F5E` and hides the panel.
- `cards-json-view`: on a JSON card, Mod+Shift+Enter turns the pane into `[dialog] "Clipboard JSON"`. Escape turns it back, and the panel stays.
- `cards-view-hide`: the view closes when the panel hides, so the next show has `[region] "Widgets"`.
- `cards-copy-pretty`: `Copy Pretty` puts the indented JSON on the clipboard.
- `cards-dismiss`: `Dismiss` removes the card until the clipboard holds new text.

## How to get to it (user POV)

- Turn it on: Mod+, opens Settings, then `Widgets`, then the `Clipboard cards` switch.
- Copy a color, a Unix time, or JSON in any app, then show the panel.

## Driving it with verify.sh

Preconditions:

- A freshly launched box. `$V doctor` passes, and `TinyDash` is `focused`.

- **Turn it on.** Run `$V auto key "CmdOrCtrl+," --settle`. `$V auto windows` shows `w3 [content] "TinyDash Settings" focused`. Run `$V auto snapshot -i --window w3`, press `[button] "Widgets"`, then press `[switch] "Clipboard cards"`. Run `$V box 'grep showClipboardCards /scratch/data/settings.json' | tee .local/verify/runs/$TD_RUN/cards-1-settings.txt`. It prints `"showClipboardCards": true`.
- **Copy a color.** Run `$V box 'printf "rgb(47 111 94)" | xclip -selection clipboard -i'`, then `$V show`. Run `$V auto wait --text "from RGB" --timeout 5s --window w1`, then `$V evidence cards-2-color`.
- **Main action.** Run `$V auto key "CmdOrCtrl+Shift+Enter" --window w1`. Then `$V box 'xclip -selection clipboard -o' | tee .local/verify/runs/$TD_RUN/cards-3-hex.clipboard.txt` prints `#2F6F5E`, and `w1` is `hidden`.
- **Copy JSON.** Run `$V box 'printf "{\"id\":\"evt_8f\",\"n\":1.50}" | xclip -selection clipboard -i'`, then `$V show` and `$V auto wait --text "View Full JSON" --timeout 5s --window w1`.
- **Full view.** Run `$V auto key "CmdOrCtrl+Shift+Enter" --settle --window w1`. The diff shows `+ @e18 [dialog] "Clipboard JSON"`. Run `$V evidence cards-4-json-view`. Run `$V auto key Escape --settle --window w1`. The diff shows `+ @e18 [region] "Widgets"`, and `w1` stays `focused`.
- **Closes on hide.** Open the view again, then run `$V hide` and `$V show`. `$V auto snapshot --window w1` has `[region] "Widgets"` and no `[dialog]`.
- **Copy Pretty.** Press `[button] "Copy Pretty"` (ref from `$V auto snapshot -i --window w1`). `$V box 'xclip -selection clipboard -o'` prints the JSON over several indented lines.
- **Dismiss.** Show the panel, press `[button] "Dismiss"` with `--settle`. The diff drops `[group] "JSON on the clipboard"`.

## Gotchas

- The accelerator for Mod+, is `CmdOrCtrl+,`. `CmdOrCtrl+Comma` fails as an unknown key.
- Grep the snapshot with the brackets: `[dialog] "Clipboard JSON"`. The text `dialog "Clipboard JSON"` never matches.
- A copy from the card replaces the clipboard, so the next show has a card for that copy.
- The card reads the clipboard as the panel shows. Copy first, then `$V show`.
- xclip offers one format, so it cannot add the secret markers. A secret copy gets no card through `Clipboard.Text`; its unit tests prove that, not this journey.
