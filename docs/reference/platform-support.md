# Platform support

| Platform | Application discovery                                                              | Limits                                                                                                                                                                                 |
| -------- | ---------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| macOS    | `/Applications`, `~/Applications`, `/System/Applications`, and Finder              | Four directory levels. Does not enter app bundles, hidden directories, or directory symlinks. Localized display-name files are not read. App-bundle symlinks are resolved.             |
| Windows  | User and shared Start menu program folders and desktops, through known-folder APIs | Finds `.lnk`, `.exe`, and `.appref-ms` entries. Does not enumerate packaged apps without shortcuts or scan all of Program Files. Shortcut targets and their aliases are not extracted. |
| Linux    | GIO's installed desktop entries                                                    | GIO handles desktop visibility, localization, aliases, XDG precedence, launch arguments, and D-Bus activation. AppImages without desktop entries are not discovered.                   |

New applications appear after a refresh or restart. The file watcher applies only to file-search roots. Inaccessible application directories are skipped and counted in logs. The UI retains the old app index if a refresh fails.

The global shortcut backend supports Linux X11, not native Wayland. On Wayland, assign a compositor or desktop shortcut to run the TinyDash binary. The single-instance handler then requests focus for the existing window. Window placement and focus remain subject to compositor policy. This limit comes from the [global-hotkey platform support](https://docs.rs/global-hotkey/latest/global_hotkey/).

Some Linux desktops need a tray extension. If the tray cannot be created, TinyDash keeps a taskbar entry. Starting the binary again also requests the existing window. OS launch APIs confirm dispatch; an application can still fail after the OS accepts that request.

## Packages and verification

CI builds Apple silicon macOS apps, Windows x64 installers, and Ubuntu 24.04 x64 Debian packages. The Mac bundle declares macOS 12 as its minimum; CI does not test every older release. Intel Macs and Windows ARM are outside this build matrix.

Native automated checks currently cover Windows and Linux X11. macOS and Wayland need [desktop checks](../how-to/desktop-checks.md).
