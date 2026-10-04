# System commands

Select **System** to see the available commands. Search by name or alias: `reboot` finds Restart, `suspend` finds Sleep, and `preferences` finds Open system settings. The same commands appear in nonempty All searches.

In All, system commands appear before clipboard matches. Type at least two characters from the start of a command name or alias to put system commands before apps and files too. For example, `sl` and `sle` show Sleep first, `re` and `res` show Restart first, and `sh` and `shu` show Shut down first. Complete names and aliases also take priority. These commands remain visible even when 30 app paths match the query. Other queries keep apps and files ahead of system commands. The result limit remains 30.

The command set includes Lock screen, Sleep, Restart, Shut down, Open system settings, Toggle system appearance, Empty Trash, Log out, Show desktop, and Toggle mute. Windows calls its trash action **Empty Recycle Bin**. Both trash names work as search aliases. Other aliases include `dark mode`, `light mode`, `sign out`, `desktop`, `mute`, and `unmute`.

Sleep, Restart, Shut down, Empty Trash, and Log out require confirmation. Cancel receives focus. Enter on Cancel or Escape closes the dialog without sending a command. Select the named confirmation button to continue. This rule also applies to the numbered result shortcuts and actions menu. Rust rejects these requests without explicit confirmation. The dialog keeps the selected command if background results change.

Empty Trash permanently deletes all trashed items for the current user, including items on connected drives. It cannot be undone. The OS may show another dialog or refuse to delete some items. Log out ends the current session without forcing applications to close.

On macOS, Empty Trash first checks whether Finder reports any trashed items. An empty Trash finishes without a deletion request. Script errors distinguish a canceled operation from denied Automation access. Only a permission denial shows the Automation settings advice. A failed command stays in its confirmation dialog, with one error message and an option to retry or cancel.

Toggle mute changes **system sound output**. It does not change microphone mute or a meeting application's mute button. Toggle system appearance changes the OS preference. Apps with their own theme settings can keep their current appearance.

| Action                    | macOS                                                                                                                                    | Windows                                                                                                        | Linux                                                                                                                                                                                                                                              |
| ------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Toggle system appearance  | System Events switches light/dark mode. Requires Automation access when macOS asks.                                                      | Sets the current user's Windows and app light/dark preferences, then notifies applications.                    | GNOME uses its `color-scheme` setting. KDE Plasma reads the portal appearance and applies Breeze Light or Breeze Dark with `plasma-apply-colorscheme`. This replaces a custom KDE color scheme. Other desktops report an unsupported-action error. |
| Empty Trash               | Finder empties the Trash. Requires Automation access when macOS asks.                                                                    | The native Recycle Bin API empties the current user's bins on all drives.                                      | GIO deletes top-level `trash:///` items through the desktop trash service. Requires a working GIO trash backend, usually GVfs. It does not delete guessed filesystem paths.                                                                        |
| Log out                   | Requests normal logout with a system Apple event.                                                                                        | Requests normal logout of the current interactive session. No shutdown privilege or forced-close flag is used. | Uses GNOME, KDE Plasma, or Xfce session logout. Other desktops report an unsupported-action error. No forced `login1` session termination is used.                                                                                                 |
| Lock screen               | Uses the legacy Apple helper when installed; otherwise sends Control-Command-Q. The shortcut requires Accessibility access.              | Uses `LockWorkStation`.                                                                                        | Uses GNOME, freedesktop, Cinnamon, or MATE screen-lock services. These can work on X11 or Wayland when the desktop provides one.                                                                                                                   |
| Show desktop              | Sends Apple's Fn-F11 shortcut. Requires Accessibility access and an enabled Show Desktop shortcut. Running it again can restore windows. | Sends Windows+D. Running it again can restore windows.                                                         | KDE uses KWin on X11 or Wayland. Other X11 desktops require `wmctrl` and window-manager support. Other Wayland desktops report an unsupported-action error.                                                                                        |
| Toggle mute               | Toggles the default sound output mute setting. Some external devices do not support software mute.                                       | Sends the system volume-mute key.                                                                              | Uses PipeWire/WirePlumber `wpctl`, or PulseAudio-compatible `pactl`, for the default output. Requires a running audio service and the matching tool.                                                                                               |
| Sleep, Restart, Shut down | Native Apple events request the normal OS transition.                                                                                    | Native Win32 APIs temporarily enable the shutdown privilege and restore it after the call.                     | The system `login1` D-Bus service applies session policy and inhibitors.                                                                                                                                                                           |
| Open system settings      | Opens System Settings or System Preferences.                                                                                             | Opens `ms-settings:`.                                                                                          | Opens the settings app for GNOME, KDE, Xfce, Cinnamon, MATE, LXQt, or COSMIC.                                                                                                                                                                      |

Commands run on the existing background action worker. Search does not query desktop services, start helpers, or request permissions. Missing services, tools, and permissions produce an error. TinyDash does not force applications to quit or bypass OS policy. An accepted request does not guarantee completion. The OS, another application, or unsaved work can prevent a transition. Only accepted requests update usage history.

On macOS, allow TinyDash under **System Settings > Privacy & Security > Automation** for System Events and Finder when prompted. Lock screen and Show desktop can request **Accessibility** access when run. No permission prompt occurs while searching. OS shortcuts can be changed or disabled by the user.

Platform references: [Apple keyboard shortcuts](https://support.apple.com/en-my/102650), [Apple's loginwindow lifecycle](https://developer.apple.com/library/archive/documentation/MacOSX/Conceptual/BPSystemStartup/Chapters/Lifecycle.html), [Windows appearance settings](https://learn.microsoft.com/en-us/windows/apps/develop/settings/settings-common), [SHEmptyRecycleBin](https://learn.microsoft.com/en-us/windows/win32/api/shellapi/nf-shellapi-shemptyrecyclebinw), [ExitWindowsEx](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-exitwindowsex), [GNOME session logout](https://gnome.pages.gitlab.gnome.org/gnome-session/re04.html), [KWin's desktop interface](https://github.com/KDE/kwin/blob/master/src/org.kde.KWin.xml), [WirePlumber controls](https://pipewire.pages.freedesktop.org/wireplumber/daemon/getting_started.html).

## Verification

Run this browser recipe from the repository root. It retains successful traces in a unique evidence directory:

```sh
bun run verify:browser tests/launcher.spec.ts --grep "system|confirmation"
```

For affected backend behavior:

```sh
bun run test:rust -- providers::system
bun run test:rust -- all_search
bun run test:rust -- platform::macos::tests
```

Use System and All. Open a power confirmation through Enter, a numbered shortcut, and Actions when those paths change. Confirm Cancel has focus. Cancel or press Escape and check that no operation is sent. Check that Rust rejects unconfirmed power, trash, and logout requests. Test Empty Trash and Log out with Cancel and with a mocked OS failure. Never confirm these actions on a personal desktop during verification.

On macOS, the Rust tests execute harmless AppleScript errors through the real script runner. They check cancellation, denied Automation access, and other errors separately. The empty-Trash test replaces the item count and deletion with safe fixtures, then executes the command's script. Browser tests check that Finder cancellation appears only in the dialog and that retry works. These tests do not prove deletion of real Trash contents.

In All, search `sl`, `sle`, `sleep`, `re`, `res`, `sh`, and `shu`. Confirm that the matching power command appears before apps, files, and clipboard entries. Rust tests cover full result lists. Browser tests check the displayed order and canceled power confirmations with mocked results.

Search `dar`, `em`, `log`, `loc`, `des`, and `mu` in All and System. Check the matching result, description, and icon. Toggle mute twice in a controlled desktop session and verify that the default output mute state changes and returns to its original state. Check that the microphone state stays unchanged. Toggle appearance twice and restore the prior setting, including any automatic schedule. Verify Show desktop with a disposable application window. Check missing permissions and services without granting access automatically.

Run `bun run verify:native` on Windows and Linux X11 for cancellation and backend rejection. Use desktop checks for macOS and opening OS settings. Actual trash deletion, logout, lock, sleep, restart, and shutdown need an explicitly selected disposable session and are not part of automated verification.

Find Restart, open its confirmation dialog, and cancel it. Confirm the dialog closes and no power action runs.

Tests: [tests/launcher.spec.ts](../../../tests/launcher.spec.ts), [tests/native/smoke.ts](../../../tests/native/smoke.ts).

Automated checks cancel power, trash, and logout operations. Actual deletion and session transitions require a disposable desktop session with synthetic trash contents.

Use the [verification procedure](../../how-to/verify.md) and [desktop checks](../../how-to/desktop-checks.md).
