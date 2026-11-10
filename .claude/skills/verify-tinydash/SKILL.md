---
name: verify-tinydash
description: Launch and drive the Go (mygo) port of TinyDash, the keyboard launcher, inside a Linux container on a virtual X display, through mygo's automation socket, and capture proof (window captures, element trees, tinydash.db rows, the X clipboard). Use to prove that a Go launcher change works in the real app (search and run, calculator, clipboard history, the global shortcut, aliases), never on the Mac's desktop.
---

# Verify TinyDash (Go port)

The surface is the launcher panel: one 880 × 560 window titled `TinyDash` with a search field, category tabs, a result list, and a footer with `Actions`. You drive the real app, built with `-tags e2e`, inside a container. Its input arrives in the app's content through mygo's automation plugin (`internal/app/automation_e2e.go`). X-level actions (the global shortcut, a copy by another app) go to the container's virtual display with `xdotool` and `xclip`.

## Safety rules

- Never run TinyDash on the Mac. Never send keys, clicks, or synthetic events to the macOS desktop (no osascript, cliclick, CGEvent). Never read, write, or clear the Mac's clipboard.
- The app runs only inside the container, with `--data-dir /scratch/data`. The container has no mount of the Mac's home or TinyDash data.
- `xdotool` and `xclip` are fine only through `verify.sh box`, which runs them inside the container on display `:99`.
- Drive only a box that you launched. Give each concurrent run its own `TD_RUN`.

## Helper

All steps go through one script. Run it from the repo root:

```sh
V=.claude/skills/verify-tinydash/verify.sh
export TD_RUN=proof-1          # names the box and the evidence folder; default "local"
$V build                       # Docker image + this run's linux binaries; prints BUILD (commit, sha256, arch)
$V launch                      # start the box, seed it, start TinyDash, wait until ready
$V doctor                      # read-only health check; exit 1 on a failure
$V auto <mygo-automation args> # e.g. $V auto snapshot -i ; $V auto key Enter --settle
$V ref '[tab] "Clipboard"'     # the ref (@eN) of the first snapshot line that holds the text
$V box '<shell command>'       # inside the box, e.g. $V box 'xclip -selection clipboard -o'
$V sql '<query>'               # read-only sqlite3 on /scratch/data/tinydash.db
$V evidence <name>             # <name>.png, <name>.tree.txt, <name>.windows.txt in the run folder
$V cleanup                     # copy app.log to the run folder, stop the box; evidence stays
```

`box.sh` runs inside the container (`start`, `doctor`); `verify.sh` calls it. `Dockerfile` defines the image `tinydash-verify` (Ubuntu 24.04, Xvfb, openbox, xdotool, xclip, GTK 3, libayatana-appindicator3-1, dbus-x11, sqlite3, fonts-dejavu-core).

The `mygo-automation` guide for the pinned mygo version: `$V auto skills`. Read it once if you need a command that this file does not show.

## Launch

1. `$V build`. It cross-builds `./cmd/tinydash` with `GOOS=linux GOARCH=<Docker's arch> CGO_ENABLED=0 -tags e2e`, and `mygo-automation` from this module, so the client has the mygo version that `go.mod` pins. The output goes to `.local/verify/builds/$TD_RUN/`, so build with the same `TD_RUN` as the launch. Run it again after each code change.
2. `$V launch`. It starts the container `tinydash-verify-$TD_RUN` and then, inside it:
   - It seeds the data. `/scratch/data/settings.json` is `{"fileSearchFolders": ["~/Documents"]}`, and all other settings keep their defaults (clipboard history is off, the shortcut is `Control+Shift+Space`, hideOnBlur is on). The files are `~/Documents/Quarterly Report.txt`, `~/Documents/grocery-list.md`, and `~/Documents/projects/roadmap-2027.md`. The app is `~/.local/share/applications/verify-notes.desktop` ("Verify Notes"). When it runs, it writes `/scratch/launched-verify-notes`.
   - It starts Xvfb `:99`, openbox, a D-Bus session, and then `MYGO_AUTOMATION=/run/td/automation.sock tinydash --data-dir /scratch/data`, without `--hidden`.
   - It waits (30 s maximum) until the socket answers, `windows` lists `TinyDash`, and the log has `Indexed applications` and `Indexed files`. Then it prints those log lines, for example `apps=1` and `files=4`. `tinydash.db` starts empty and migrates on the first start.
3. Ready output ends with `ready: tinydash-verify-<run>, evidence in .local/verify/runs/<run>` and the panel shows as `w1 [content] "TinyDash" focused`.

SQLite: mygo's SQLite plugin downloads `libmygo-sqlite3.so` on first use and checks its SHA-256. The box sets `XDG_CACHE_HOME=/cache`, mounted from `.local/verify/cache/`, so the download happens once and later runs reuse the checked file. The first launch needs network access to GitHub releases.

## Doctor

`$V doctor` is read-only. It checks:

- The container runs.
- The pid in `/scratch/app.pid` runs `/opt/td/tinydash --data-dir /scratch/data`.
- That binary's sha256 is the one in `BUILD`, so a rebuild after the launch shows up.
- The automation socket answers `windows`.
- The log has both `Indexed` lines.
- `BUILD`'s commit is this checkout's `git describe --dirty`. A mismatch is a `WARN`: build and launch again.

It also prints any `WARN`/`ERROR` lines from the app. Run doctor first whenever a step acts oddly.

## Drive

- **Handles.** Elements have no app-set keys except the result rows. The List gives each row its result ID as its key: `key=app:/root/.local/share/applications/verify-notes.desktop`, `key=file:/root/Documents/projects/roadmap-2027.md`, `key='"calc:12 * 8"'`, `key=clip:2`. Everything else is a label: `[textField] "Search"`, `[tab] "All"` … `"System"`, `[list] "Results"`, `[button] "Actions"`, `[dialog] "Actions"` with `[textField] "Search actions"`, `[dialog] "Alias for <title>"` with `[button] "Save Alias"`, and `[button] "Turn On Clipboard History"`. Get the ref of a label with `$V ref '<line text>'`, or read it from the last diff.
- **Each result shows as two `listItem` lines.** The first has the key. The second is the row itself, and it is `checked` while it is selected. Enter runs the `checked` row.
- **Type as the user does.** The search field has focus when the panel shows. Use `$V auto type "<text>" --settle`, then `$V auto key Enter --settle` (the main action), `CmdOrCtrl+Enter` (the second action), `CmdOrCtrl+K` (the actions menu), `Tab` (the next tab), and `Escape`. `fill <ref> ""` empties a field.
- **Wait for state, not time.** Use `$V auto wait --text "<text>" --timeout 5s`, `wait --gone <ref>`, or `--settle` on an action. Work that the app does on a goroutine, such as turning on clipboard history or the capture tick, draws after `--settle` returns. Wait for its text.
- **An action that hides the panel** sometimes prints `warning: the window did not settle in 10s`: a hidden window draws no more frames. Check the result with `$V auto windows` and the side effect, not with the diff.
- **Restarting the app inside a box** is not supported. Clean up and launch again. A second instance quits at once because of the single-instance lock.
- **Show and hide** the panel with `$V show` and `$V hide`. They press the real shortcut on the virtual display and wait until `$V auto windows` shows `focused` or `hidden`. Do not press the shortcut with a raw `xdotool key`: a press within `blurWithin` (400 ms) of a hide by Enter or a digit counts as the cause of that blur, so the panel stays hidden. `$V show` waits out that window. Each does nothing when the panel is already in its state; to start a fresh query on a shown panel, empty the field instead.

## Evidence

- Evidence goes to `.local/verify/runs/$TD_RUN/` (git-ignored). It holds `BUILD` (the build under test), `$V evidence <step>` captures, the saved side effects, and `app.log` after cleanup.
- Save side effects next to the captures: `$V sql '…' | tee .local/verify/runs/$TD_RUN/<step>.sql.txt` and `$V box 'xclip -selection clipboard -o' | tee .local/verify/runs/$TD_RUN/<step>.clipboard.txt`.
- Proof standards:
  - Drive the real user path: keys and text into the panel, the real global shortcut, a copy made by another X client. Do not call internal setters or test-only endpoints.
  - Capture the action and the state that results from it, not only the final screen. Take a capture before Enter and the side effect after it.
  - A visible change is not enough. Check the side effect too: a `tinydash.db` row, the X clipboard, or the marker file of a launched app.
  - The only fake is the seeded `.desktop` app, and launching it is the real `mygo.Shell.LaunchApp` path. Currency rates stay off, so the run downloads nothing but the SQLite library.

## Cleanup

`$V cleanup` copies `/scratch/app.log` to the run folder and stops the container by its name `tinydash-verify-$TD_RUN`. The app ends on SIGTERM, and the end of the container ends every process in it anyway. The container was started with `--rm --init`, so its data dir, home, and socket go with it. It never kills by process name. The evidence in `.local/verify/runs/$TD_RUN/`, the binaries in `.local/verify/builds/$TD_RUN/`, and the SQLite cache stay. Run cleanup after every failed attempt too. Remove old run and build folders by hand when you no longer need them.

## Isolation and limits

- Each `TD_RUN` has its own binaries (`.local/verify/builds/<run>/`, mounted read-only at `/opt/td`), container, socket, data dir, and automation session (`MYGO_AUTOMATION_SESSION=$TD_RUN`), so runs side by side do not share state, and a build for one run never replaces the app of another. Only the Docker image and the SQLite library cache (`.local/verify/cache/`) are shared. `launch` refuses a `TD_RUN` whose container exists.
- This is Linux with X11 and openbox, a stand-in for the user's macOS. The Linux run cannot check these macOS-only behaviors: the nonactivating NSPanel and its focus, Carbon hot keys, Spotlight results, rich clipboard (images and files; Linux keeps text only), password-app detection, and tray template icons. Prove those on a Mac by hand, not with this skill.
- Without a window manager the panel never gets the focus, and with hideOnBlur on it hides as soon as it shows. The image therefore runs openbox. Do not remove it.
- Context menus and tray menus cannot be driven through automation. Use the `Actions` menu (`CmdOrCtrl+K`), which is drawn inside the panel.

## Feature map

`features/README.md` indexes the journeys. Read it before a run and keep it current when launcher behavior changes.
