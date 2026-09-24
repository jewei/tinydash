# Settings

Open **Actions → Settings**, press **Command+,** on macOS or **Ctrl+,** on Windows and Linux, or select **Settings** from the tray menu. The separate window opens on Shortcut. Click **Record new**, press a key combination, then click **Save changes**. Shortcut changes apply at once. If registration or saving fails, TinyDash keeps the previous shortcut.

Shift + Space is supported for the launcher and category shortcuts. Other shortcuts need Control, Option/Alt, or Command/Windows. An input method or another app can already use the combination. Choose another binding if it interferes with typing. Native Wayland uses a desktop shortcut instead.

Settings includes Shortcut, Appearance, Categories, Search, Clipboard history, File search, Currency, Privacy, and About. Save changes to apply visible categories, window behaviour, clipboard limits, file folders, file watching, and currency updates while TinyDash runs. Appearance applies immediately and stays in sync between windows. Closing Settings keeps an unfinished form. Use Discard to restore saved values.

Appearance provides Light, Dark, Sage, Rose, and Ink themes. Use Compact layout with any theme. These choices persist between sessions and are included in settings exports. An older Compact appearance becomes Light with Compact enabled. On macOS, Follow macOS Liquid Glass is on by default. Turn it off in Appearance for a solid theme background. The choice saves immediately and is included in settings exports. When it is on, macOS 26 or later uses native Liquid Glass. macOS controls the glass blur and transparency. On macOS 27, adjust the Liquid Glass slider in System Settings, Appearance. Reduce transparency also applies. Other platforms and earlier macOS versions use solid theme backgrounds.

The Search section adds app aliases, hidden apps, and custom web search templates with a URL preview. Shortcut settings include direct category keys and start at login. Category shortcuts and `tinydash --mode clipboard` open an empty category search. Start at login uses `--background` and keeps the window hidden. Privacy settings can export saved settings, preview an import before saving, and show the recovery archive. About includes an explicit update check. Release builds need an updater key and HTTPS feed; Ubuntu updates use a new `.deb` package. See [configuration](../how-to/configure.md), [data recovery](../how-to/recover-data.md), and [release preparation](../how-to/release.md).

You can also open Settings directly with `tinydash --settings`. TinyDash writes `settings.json` in its application configuration directory on first launch. Manual file edits still require a restart. The settings screen preserves unknown JSON fields and refuses to overwrite a damaged file.

| Platform | Default location                                                                                           |
| -------- | ---------------------------------------------------------------------------------------------------------- |
| macOS    | `~/Library/Application Support/dev.tinydash.launcher/settings.json`                                        |
| Windows  | `%APPDATA%\dev.tinydash.launcher\settings.json`                                                            |
| Linux    | `$XDG_CONFIG_HOME/dev.tinydash.launcher/settings.json`, or `~/.config/dev.tinydash.launcher/settings.json` |

This partial example leaves clipboard capture off. Fresh installations ask for consent before capture. Omitted fields use defaults.

```json
{
  "clearQueryOnOpen": true,
  "hideOnBlur": true,
  "shortcut": "Control+Shift+Space",
  "clipboardHistoryEnabled": false,
  "clipboardHistoryLimit": 100,
  "fileSearchRoots": null,
  "fileSearchLimit": 50000,
  "fileSearchExcludedDirs": ["node_modules", "target"],
  "fileWatchEnabled": true,
  "currencyRatesEnabled": true
}
```

Set `clearQueryOnOpen` to `false` to keep the previous query and search mode. TinyDash selects that text when the window opens. With the default setting, it clears the query and selects the first visible category, which is All by default. Set `hideOnBlur` to `false` to keep the window visible when another app receives focus. Use the settings screen to apply shortcut changes at once. An invalid settings file is left unchanged; the app uses defaults and displays a warning.

On macOS, a saved `CommandOrControl+Shift+Space` value from earlier TinyDash versions now uses `Control+Shift+Space`. This compatibility rule keeps the settings file intact, including other user settings. Other custom shortcuts remain unchanged. New settings files contain `Control+Shift+Space`.

Set `clipboardHistoryLimit` to a value between 1 and 500. TinyDash applies the limit when saved in Settings, on restart, and after each capture. Missing settings use their defaults. The settings file remains the editable startup configuration. Usage and clipboard data are stored separately in SQLite.

Set `fileSearchRoots` to `null` for the default folders, `[]` to disable file scanning, or an array of absolute paths. A leading `~` refers to your home folder on all three platforms. For example, `["~/Documents", "~/Projects"]` scans those two folders; `["~"]` scans your home folder. Windows paths in JSON need escaped backslashes, such as `"C:\\Users\\Alex\\Documents"`. `fileSearchLimit` is restricted to 1 through 100,000. `fileSearchExcludedDirs` contains exact folder names, not patterns. Changes saved in Settings start a new scan and update the file watcher. Manual JSON edits take effect after a process restart.
