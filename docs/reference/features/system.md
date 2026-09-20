# System commands

Select **System** to see the available commands. Search by name or alias: `reboot` finds Restart, `suspend` finds Sleep, and `preferences` finds Open system settings. The same commands appear in nonempty All searches.

Sleep, Restart, and Shut down require confirmation. Cancel receives focus. Enter on Cancel or Escape closes the dialog without sending a command. Select the named confirmation button to continue. This rule also applies to the numbered result shortcuts and actions menu. Rust rejects power requests without explicit confirmation. The dialog keeps the selected command if background results change.

| Platform | Implementation and limits                                                                                                                                                                                                                                                                                                                                                                           |
| -------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| macOS    | Native Apple events request sleep, restart, and shutdown from the system process. Settings opens the installed System Settings or System Preferences app. Lock screen is offered only when Apple's legacy `CGSession` helper exists. If it is absent, use the operating system shortcut. TinyDash does not use private lock APIs or request Accessibility access.                                   |
| Windows  | Native Win32 APIs request lock, sleep, restart, and shutdown. Power commands temporarily enable the process's shutdown privilege and restore it after the call. Settings uses the `ms-settings:` URI. Permissions and session policy can deny a request.                                                                                                                                            |
| Linux    | Power commands use the system `login1` D-Bus service and respect session policy and inhibitors. Lock uses an existing GNOME, freedesktop, Cinnamon, or MATE screen-lock service. These services can work on X11 or Wayland; a compositor without one is unsupported. Settings opens a known desktop entry for GNOME, KDE, Xfce, Cinnamon, MATE, LXQt, or COSMIC. Missing services produce an error. |

Commands run on the existing background action worker. They add no polling or startup service queries. TinyDash does not force applications to quit or bypass OS policy. An accepted request does not guarantee completion: the OS, another application, or unsaved work can prevent the transition. Only accepted requests update usage history. Empty trash and volume controls are not part of this command set.

The Mac integration adds the small `objc2-core-services` binding with only the required Apple event modules. Windows and Linux reuse their existing dependencies. Platform APIs follow [Apple's loginwindow lifecycle](https://developer.apple.com/library/archive/documentation/MacOSX/Conceptual/BPSystemStartup/Chapters/Lifecycle.html), [Win32 ExitWindowsEx](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-exitwindowsex), and [systemd's login1 interface](https://github.com/systemd/systemd/blob/main/man/org.freedesktop.login1.xml).

## Verification

Find Restart, open its confirmation dialog, and cancel it. Confirm the dialog closes and no power action runs.

Tests: [tests/launcher.spec.ts](../../../tests/launcher.spec.ts), [tests/native/smoke.ts](../../../tests/native/smoke.ts).

Automated checks cancel power operations. Lock, sleep, restart, and shutdown transitions require a disposable desktop session.

Use the [verification procedure](../../how-to/verify.md) and [desktop checks](../../how-to/desktop-checks.md).
