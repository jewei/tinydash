# Linux desktop

On Linux, the System tab offers a command only when this desktop can run it and its program is installed. Without the tray library, the app logs a warning that names the package to install. The launcher window's X11 class is `tinydash`, the same value as `StartupWMClass` in the desktop file.

## Sub-features

- `system-none`: with no session programs (the stock box), the System tab lists only TinyDash's own five commands, and All offers no OS command.
- `system-gnome`: on GNOME under X11, with `loginctl`, `systemctl`, and `gio` installed, the tab offers Lock Screen, Sleep, Restart, Shut Down, Empty Trash, and Open Trash.
- `system-missing`: Log Out, Turn Off Displays, Dark Mode, and System Settings stay hidden while their programs are missing. Restart and Shut Down fall back to `systemctl` when `gnome-session-quit` is missing.
- `system-run`: choosing a command runs its program with its arguments.
- `tray-missing`: without `libayatana-appindicator3`, the log warns with the `apt install` command.
- `window-class`: the launcher's `WM_CLASS` is `"tinydash", "tinydash"`.

## How to get to it (user POV)

- Click the `System` tab, or type a command's name, such as `lock screen`, on the All tab, and press Enter.
- The tray warning shows in the log at start.
- The window class is what a dock reads; the user sees it as the right icon.

## Driving it with verify.sh

Preconditions:

- `system-none` and `window-class`: a stock box, `$V launch` with no options.
- The others: `$V launch --env XDG_CURRENT_DESKTOP=GNOME --env XDG_SESSION_TYPE=x11 --stub loginctl --stub systemctl --stub gio --without-lib libayatana-appindicator3`. `$V doctor` prints `ok   stub …` for each stub and `ok   without libayatana-appindicator3`.

- **Stock tab.** Run `$V auto press "$($V ref '[tab] "System"')" --settle`, then `$V auto snapshot | grep key=system`. The keys are exactly `system:clear-clipboard`, `system:clear-calculations`, `system:plain-text`, `system:tinydash-settings`, `system:quit-tinydash`. Run `$V evidence linux-0-stock`.
- **Stock All.** Press `[tab] "All"`, then `$V auto type "lock" --settle`. No key starts with `system:lock`.
- **Window class.** Run `$V box 'for w in $(xdotool search --name "^TinyDash$"); do xprop -id $w WM_CLASS; done'`. It prints `"tinydash", "tinydash"`. A 16 × 16 unmapped window with the class `mygo-<pid>-1` is the tray icon (egoist/mygo#1835), not an app window.
- **GNOME tab.** In the box with options, open the System tab as above. The keys are `system:lock`, `system:sleep`, `system:restart`, `system:shut-down`, `system:empty-trash`, `system:open-trash`, then the five TinyDash commands. No `system:log-out`, `system:sleep-displays`, `system:toggle-dark-mode`, or `system:settings`. Run `$V evidence linux-1-gnome`.
- **Run Lock Screen.** Press `[tab] "All"`, run `$V auto type "lock screen" --settle`, and check that `key=system:lock` is `checked`. Run `$V auto key Enter --settle`; it may print `the window did not settle`, because the panel hides. Then `$V box 'i=0; until [ -s /scratch/stub-calls ] || [ $i -ge 50 ]; do i=$((i+1)); sleep 0.1; done; cat /scratch/stub-calls' | tee .local/verify/runs/$TD_RUN/linux-2-calls.txt` prints `loginctl lock-session`.
- **Tray warning.** `$V doctor` lists `WARN The tray icon needs libayatana-appindicator3, which is not installed. On Debian or Ubuntu, install it with sudo apt install libayatana-appindicator3-1. …`.

## Gotchas

- The app reads the desktop, PATH, and libraries once, at start. Change them with launch options in a new box, never inside a running one.
- The stock box sets no `XDG_CURRENT_DESKTOP`, so the desktop is unknown: Log Out is never offered there, even with stubs.
- A stub succeeds and does nothing else. It proves which program and arguments the app chose, not that the real program works.
- Only the stubs the options name exist. `gnome-session-quit`, `xset`, `gsettings`, and `gnome-control-center` are missing on purpose, so their commands stay hidden.
