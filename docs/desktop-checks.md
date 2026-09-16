# Desktop checks

Use a Windows or Linux desktop session in a virtual machine or on a physical computer. Check Linux X11 and Wayland in separate sessions. Use the macOS application for the same checks on a Mac.

## Get the build

Open the GitHub Actions **Checks** run for the commit under test. Download the `TinyDash-<OS>-<architecture>` artifact. Check `build.txt` to confirm the commit and CPU architecture. Extract the artifact before starting the app.

These are unsigned development builds. Windows needs the WebView2 Runtime. The Linux binary is built on Ubuntu 24.04 and needs compatible GTK 3, WebKitGTK 4.1, AppIndicator, and libxdo libraries. Bun and Rust are not needed to run the build.

On macOS, extract the inner ZIP and open `TinyDash.app`. On Windows, run the `.exe`. On Linux, extract the inner archive and run `./tinydash` from a terminal.

## Check the launcher

1. Start TinyDash. Confirm that the window appears and typing immediately enters text in the search field.
2. Search for an installed app by name. Search again with an abbreviation and part of its path. Confirm that the expected app appears.
3. Use the arrow keys. Confirm that selection moves and wraps at both ends. Press Enter and confirm that the selected app starts.
4. Press Command/Ctrl + Shift + Space while another app has focus. Confirm that TinyDash appears and immediately accepts text. Repeat ten times.
5. Press Escape. Confirm that the window hides and can reopen through the shortcut.
6. Click another app. Confirm that TinyDash hides with the default `hideOnBlur` setting.
7. Use the tray menu to open the launcher and refresh the app list. Confirm that the tray can quit TinyDash.
8. Start TinyDash again while it is running. Confirm that the existing window appears and a second launcher process does not remain running.
9. Use Command/Ctrl + Enter on a result. Confirm that the OS file manager shows its location.
10. Set `clearQueryOnOpen` to `false` in `settings.json`, restart, and reopen after a search. Confirm that the query remains and its text is selected. Restore the setting after the check.

Use Ctrl on Windows and Linux. Use Command on macOS. If a shortcut conflicts with another application, change it in `settings.json` and repeat the check.

## Linux session differences

- On X11, test the registered global shortcut directly.
- On Wayland, TinyDash displays a shortcut warning. Assign a desktop or compositor shortcut to the TinyDash executable, then test opening the existing window and input focus. Direct global shortcut registration is not supported in this version.
- Record the desktop environment and whether a tray extension is installed. If the tray is unavailable, test access through the taskbar or by starting the executable again.

The native CI tests use a virtual X11 display. They do not establish Wayland support, reliable global shortcuts, or correct focus behavior on every desktop.

## Record the result

Include the following information in the issue or pull request:

```text
Commit:
OS version and CPU architecture:
Desktop environment and X11/Wayland session, if applicable:
Checks passed:
Checks failed or not run:
Steps to reproduce a failure:
Screenshot or log:
```

Mark checks that were not run as pending. A successful build alone does not verify desktop behavior.
