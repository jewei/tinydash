# Keyboard shortcuts

The default global shortcut is **Control + Shift + Space**. It shows or hides the launcher on macOS, Windows, and X11 Linux.

| Control                  | Action                                                          |
| ------------------------ | --------------------------------------------------------------- |
| Arrow Up / Arrow Down    | Move through results                                            |
| Enter                    | Run the selected result's primary action                        |
| Command/Ctrl + Enter     | Reveal a result when supported                                  |
| Escape                   | Hide the launcher; close the actions menu first when it is open |
| Tab / Shift+Tab          | Move to the next or previous visible category                   |
| Option/Alt + Left/Right  | Move between categories                                         |
| Command/Ctrl + K         | Open and filter the actions menu                                |
| Command/Ctrl + 1 to 9    | Run the primary action for a visible result                     |
| Command/Ctrl + Backspace | Delete the selected entry when supported                        |
| Command/Ctrl + Comma     | Open Settings                                                   |
| Command/Ctrl + R         | Refresh the current data source                                 |
| Command/Ctrl + Q         | Quit TinyDash                                                   |

The application uses Command on macOS and Control on Windows or Linux for the Command/Ctrl controls. Emoji mode supports arrow navigation in a grid. The category bar also accepts mouse clicks.

Use [configuration](../how-to/configure.md) to change global or category shortcuts.

Window placement shortcuts are off by default. In **Settings → Shortcut → Window placement shortcuts**, select a preset and save, or record each action separately. The standard preset uses Control + Option on macOS and Control + Alt on Windows/X11. The second preset adds Shift to each binding.

| Preset key  | Window action                       |
| ----------- | ----------------------------------- |
| Left arrow  | Left half                           |
| Right arrow | Right half                          |
| Up arrow    | Maximize to the work area           |
| C           | Center at the current size          |
| Down arrow  | Restore the saved position and size |

These shortcuts act on the active app window without opening the launcher. macOS requires Accessibility access. Linux requires X11 and the window tools described in [native utilities](features/utilities.md#windows). Wayland is unsupported. Restore applies to moves made by these shortcuts or the corresponding search commands; history lasts for the current session, up to 16 windows and 30 minutes since each window's last placement.
