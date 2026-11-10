# Global shortcut

From any app, the user presses `Control+Shift+Space` to show the launcher, and presses it again to hide it. Each show resets the query, chooses the All tab, and puts the panel a third of the way down the display with the pointer.

## Sub-features

- `shortcut-hide`: the shortcut hides a visible panel.
- `shortcut-show`: the shortcut shows a hidden panel, focused, with an empty search field.
- `shortcut-reset`: a show clears the query that was typed before the hide.
- `shortcut-blur`: with hideOnBlur on (the default), the panel hides when another window takes the focus.

## How to get to it (user POV)

- Press `Control+Shift+Space` anywhere. The shortcut is `settings.json` `"shortcut"`, in Tauri syntax.

## Driving it with verify.sh

Preconditions:

- A freshly launched box. `$V doctor` passes, and `$V auto windows` prints `TinyDash` `focused`.

- **Type something first.** Run `$V auto type "roadmap" --settle`. The search field value is `roadmap`. Run `$V evidence shortcut-0-typed`.
- **Hide.** First check that `$V auto windows` prints `TinyDash` `focused`: `$V hide` presses no key on a hidden panel, so only a shown panel proves the shortcut. Run `$V hide | tee .local/verify/runs/$TD_RUN/shortcut-1-hidden.txt`. It presses the real shortcut on the display and waits until the panel is `hidden`. If it prints `already hidden: no key pressed`, the check did not run: start again from a shown panel. As a second view, `$V box 'xdotool search --onlyvisible --name "^TinyDash$"'` prints nothing.
- **Show.** Run `$V show`. It waits out the blur window, presses the real shortcut, and waits until the panel is `focused`. Then `$V box 'xdotool search --onlyvisible --name "^TinyDash$"'` prints a window ID.
- **Query reset.** Run `$V auto snapshot -i`. `[textField] "Search" focused` has no `value=`, and `[tab] "All"` is `checked`. Run `$V evidence shortcut-2-shown`.

## Gotchas

- **Fixed failure (found 2026-11-09).** On X11 the key grab takes focus from the panel, so blur hid it before the shortcut's toggle ran, and the toggle showed it again. The app now treats a press within 400 ms of a blur-hide as the cause of that blur (`blurWasThisPress` in `internal/app/app.go`). If `shortcut-hide` fails again, check that first.
- Show with `$V show`, never with a raw `xdotool key`. A press within `blurWithin` (400 ms) of a hide's blur is taken as the cause of that blur and does nothing, so the panel stays hidden. A user does not press that fast; a script does. `$V show` waits 0.5 s first for this reason.
- The shortcut is an X11 key grab. xdotool's XTEST events reach it on display `:99` only through `$V box`. Never send it on the Mac.
- `mygo-automation key` sends keys into the app's content. It cannot press a global shortcut. Use xdotool for this journey.
- No window manager means no focus, and with hideOnBlur on the panel hides as soon as it shows. The image runs openbox for this reason. If the panel hides at launch, check `$V box 'cat /scratch/wm.log'`.
- The capture of a hidden window still renders its content. Prove "hidden" with `windows`, not with a PNG.
