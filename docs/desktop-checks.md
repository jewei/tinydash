# Desktop checks

Use a Windows or Linux desktop session in a virtual machine or on a physical computer. Check Linux X11 and Wayland in separate sessions. Use the macOS application for the same checks on a Mac.

## Get the build

Open the GitHub Actions **Checks** run for the commit under test. Download the `TinyDash-<OS>-<architecture>` artifact. Check `build.txt` to confirm the commit and CPU architecture. Extract the artifact before starting the app.

Use the [installation guide](install.md) to verify checksums and install the DMG, Windows setup executable, or Ubuntu 24.04 Debian package. These are unsigned development builds. Windows needs the WebView2 Runtime. The Linux package needs compatible GTK 3, WebKitGTK 4.1, AppIndicator, libxdo, and OpenSSL libraries. Bun and Rust are not needed to run the build.

Quit any older TinyDash process through its tray menu before opening the new build. Reopening the window of an existing process does not load the new code.

Start the installed app from Applications on macOS, the Start menu on Windows, or the application menu on Linux. For standalone checks, use the inner Mac ZIP, the Windows executable without `-setup` in its name, or the Linux tar archive.

CI checks installation, same-version replacement, and removal. On a physical desktop, also check replacement while the app is running, the Windows WebView2 download on a clean system, and the OS response to an unsigned build. Confirm that removal preserves saved settings and history unless you explicitly request their deletion.

## Check the launcher

1. Start TinyDash. Confirm that the window appears and typing immediately enters text in the search field.
2. Search for an installed app by name. Search again with an abbreviation and part of its path. Confirm that the expected app appears.
3. Use the arrow keys. Confirm that selection moves and wraps at both ends. Press Enter and confirm that the selected app starts.
4. Press Control + Shift + Space while another app has focus. Confirm that TinyDash appears and immediately accepts text. Confirm that Siri, Spotlight, or another system panel does not open. Repeat ten times.
5. Press Escape. Confirm that the window hides and can reopen through the shortcut.
6. Click another app. Confirm that TinyDash hides with the default `hideOnBlur` setting.
7. Use the tray menu to open the launcher and refresh the app list. Confirm that the tray can quit TinyDash.
8. Start TinyDash again while it is running. Confirm that the existing window appears and a second launcher process does not remain running.
9. Use Command/Ctrl + Enter on an app result. Confirm that the OS file manager shows its location.
10. Set `clearQueryOnOpen` to `false` in `settings.json`, restart, and reopen after a search in Emoji mode. Confirm that the query and mode remain and the query text is selected. Restore the setting after the check. Confirm that reopening then clears the query and returns to All mode.
11. Drag the handle above the search field. Confirm that the window moves and typing still enters text in the search field. Select text with the mouse and change the search mode. Confirm that these controls do not move the window. Hide and reopen the launcher. Confirm that it keeps a position that fits on the screen.
12. Move the launcher to another display, then disconnect that display or reduce its resolution. Reopen the launcher. Confirm that it moves inside the remaining screen's work area. Check displays with different scale factors. On Wayland, check placement through the compositor because TinyDash cannot set absolute positions there.
13. Move the selection with the arrow keys while refreshing the file or application index. Confirm that the refresh keeps the latest selection if that result still exists. Enter a new query and confirm that it selects the first result.
14. Type a query, then immediately press Escape. Add a file in a configured folder while TinyDash is hidden. Reopen TinyDash and confirm that the new file is searchable. Repeat after hiding through the global shortcut and after opening a result.
15. Press Tab in the search field. Confirm that the full category list opens and All stays selected. Press Tab again to select Apps. Type a query and confirm that the list closes, typing stays in the search field, and Apps stays selected. Press Tab again and confirm that Files is selected. Use Shift + Tab to move back. Check that selection wraps between the first and last categories. Press Escape once to close the list and again to hide the launcher.

The global shortcut uses Control on all platforms. For shortcuts shown as Command/Ctrl, use Ctrl on Windows and Linux, and Command on macOS. If a shortcut conflicts with another application, change it in `settings.json` and repeat the check. On macOS 27, Command + Shift + Space opens Siri Visual Intelligence and must not be used as TinyDash's default.

## Check calculations and emoji

1. In All mode, enter `12 * 8`. Confirm that `96` is the first result. Press Enter, then paste into a text editor. Confirm that the pasted text is `96`.
2. Select Calculator mode. Check `sqrt(144)` → `12`, `5 ft to cm` → `152.4 cm`, and `32 C to F` → `89.6 °F`. Confirm that unit conversion also works with the network disconnected.
3. Enter `12 +` in Calculator mode. Confirm that an error appears and no copy action is available. Enter `12 + 1` and confirm that the error clears.
4. In All mode, enter `:rocket`. Confirm that 🚀 is the first result. Press Enter, then paste into a text editor. Confirm that the complete emoji is pasted.
5. Search for `coffee`, `laugh`, and `heart` in Emoji mode. Check the results and the copy action. Use Command/Ctrl + K to confirm that the actions menu offers Copy and no app-location action.
6. Change between Apps, Emoji, and Calculator modes while a query is present. Confirm that results follow the selected mode. Reopen TinyDash after each copy and confirm that the input accepts text.

7. While online, enter `100 USD to MYR` in Calculator mode. Confirm that the result includes the ECB rate date. Use Command/Ctrl + R to refresh and confirm that typing remains responsive.
8. After a successful refresh, disconnect the network and restart TinyDash. Confirm that currency conversion still works from SQLite. Refresh again and confirm that the error does not remove the saved result. Arithmetic and units must also work without any saved rates.
9. Set `currencyRatesEnabled` to `false` and restart. Confirm that saved rates remain usable and a manual refresh reports the disabled setting. Restore the setting after the check.

Clipboard access and emoji fonts can differ across desktop sessions; record any failure with the session details. Automatic paste is not implemented.

## Check usage ranking

1. Launch an app from a lower position in the list. Reopen TinyDash and clear the query. Confirm that the app moves higher in the list.
2. Copy an emoji. Reopen TinyDash and enter `:`. Confirm that the copied emoji moves higher in the emoji list.
3. Quit TinyDash fully, then start it again. Confirm that both ranking changes remain. Reopening a hidden window is not a process restart.
4. Search for an exact app name after using another app several times. Confirm that the exact match remains above weak matches.
5. Set `clearQueryOnOpen` to `false`, restart, and launch an app from a query with several matches. Reopen the launcher. Confirm that it keeps the query and refreshes the order. Restore the setting after the check.

The existing usage history can affect the order. Use a separate test profile for repeatable checks. Usage records do not contain queries or calculation text.

## Check clipboard history

Use test text for these checks. History stores text without encryption. These checks delete saved history, so use a separate test profile.

1. Copy a short text with several lines, spaces, and an emoji from a text editor. Open TinyDash and select Clipboard. Confirm that the entry appears within two seconds and the preview preserves its text.
2. Copy the same text twice. Confirm that only one entry exists. Copy different text, then the first text again. Confirm that the first entry moves to the top.
3. Search for a word from the second line. Confirm that both Clipboard mode and All mode find the entry. Confirm that Apps mode does not return clipboard text.
4. Copy another value in the editor. Select the older entry in TinyDash and press Enter. Paste into the editor. Confirm that the full historical text is restored.
5. Delete that entry with Command/Ctrl + Backspace. Confirm that the launcher stays open. Reopen it and confirm that the unchanged clipboard does not restore the deleted entry during this session.
6. Select Clear history. Confirm that Cancel receives focus. Cancel first and confirm that entries remain. Then confirm the clear action and check that all entries disappear. Confirm that the current system clipboard is unchanged.
7. Capture new test text, quit TinyDash fully, then start it again. Confirm that saved entries remain. Startup also captures the current system clipboard.
8. Set `clipboardHistoryLimit` to 2 and restart. Copy three distinct values at least two seconds apart. Confirm that only the newest two remain. Restore the setting to 100.
9. Set `clipboardHistoryEnabled` to `false` and restart. Copy new text and confirm that no new entry appears. Confirm that existing entries can still be copied and cleared. Restore the setting when finished.
10. Copy an image, whitespace-only text, and text larger than 16 KiB. Confirm that none becomes an entry. Test clipboard capture while the launcher is hidden and after reopening it.

macOS and Windows read their clipboard change counters once per second. Copies made within the same interval can be missed. Linux uses GTK clipboard events. On Wayland, the compositor can limit access while TinyDash lacks focus; record which changes appear only after opening the launcher.

## Check file search

1. Put a text file in Documents. Give it a name with spaces and Unicode characters. Restart TinyDash, select Files, and wait for the scan to finish.
2. Search by filename, abbreviation, and part of the full path. Confirm that All mode also finds the file. Confirm that Apps mode does not return it.
3. Search for text that occurs only inside the file. Confirm that the file does not appear.
4. Press Enter on the file. Confirm that its default application opens it. Reopen TinyDash and use Command/Ctrl + Enter to show it in the file manager.
5. Add and rename a second file. Confirm that the results update without a manual refresh. Create a new subfolder and immediately add a file inside it. Confirm that the file appears.
6. Delete the first file outside TinyDash. Confirm that its result disappears automatically. If an old result is opened before the scan finishes, confirm that the app reports that the file is unavailable.
7. Rename a configured root folder, then recreate it and add a file. Confirm that the replacement root updates. Use Refresh files to confirm that manual recovery remains available. Set `fileWatchEnabled` to `false` and restart to check manual-only operation, then restore the setting.
8. Configure a temporary folder in `fileSearchRoots`. Add hidden files, an excluded `node_modules` folder, and symbolic links. Restart and confirm that the scanner excludes them. Test a folder without read permission and confirm that the app stays usable and shows a warning.
9. Set `fileSearchLimit` to 2 in a folder with three files. Restart and confirm that the UI reports an incomplete scan. Set `fileSearchRoots` to `[]`, restart, and confirm that Files mode is empty. Restore the settings when finished.

File access depends on OS permissions. On macOS, record any Files and Folders permission prompt or denial. Test Windows redirected Known Folders and Linux XDG user directories if available. Files open through their OS association; a missing or broken association can prevent the target application from opening.

## Check system commands

1. Select System. Confirm that the supported commands appear. Search `reboot`, `suspend`, and `preferences`. Confirm that the aliases find Restart, Sleep, and Open system settings. Check the same queries in All mode and confirm that Apps mode excludes system commands.
2. Select Restart and press Enter. Confirm that the dialog names Restart and Cancel has focus. Press Enter again. Confirm that the dialog closes and the computer remains running.
3. Select Shut down with its numbered shortcut. Press Escape in the dialog. Confirm that the launcher stays open and no command runs. Repeat through the actions menu.
4. Open a power confirmation dialog, hide the launcher by clicking another app, then reopen it. Confirm that the old confirmation is gone.
5. Run Open system settings. Confirm that the correct settings app opens. Reopen TinyDash and confirm that the command's usage ranking has changed.
6. In a disposable desktop session, save work and test Lock screen if offered. Unlock and confirm that TinyDash can open again. On macOS without the legacy `CGSession` helper, the command is omitted; use Control + Command + Q.
7. In a disposable desktop session, save work and confirm Sleep, Restart, and Shut down separately. Check the OS transition. Do not run these checks on a hosted CI runner or a computer with active work.
8. On Linux, test the desktop's policy denial or a missing service. Confirm that TinyDash reports the failure and remains usable. Check X11 and Wayland separately. A standalone compositor must provide a supported screen-lock service for Lock screen to work.

Automated tests cover command search, confirmation, cancellation, and backend rejection of requests without confirmation. They do not prove that a power transition completes. Mark transitions that were not run as pending. An OS can accept a request and then cancel it because of unsaved work or session policy.

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
