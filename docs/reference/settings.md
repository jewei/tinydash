# Settings

Open **Actions → Settings**, press **Command+,** on macOS or **Ctrl+,** on Windows and Linux, or select **Settings** from the tray menu. The separate window opens on Shortcut. Click **Record new**, press a key combination, then click **Save changes**. Shortcut changes apply at once. If registration or saving fails, TinyDash keeps the previous shortcut.

Shift + Space is supported for the launcher and category shortcuts. Other shortcuts need Control, Option/Alt, or Command/Windows. An input method or another app can already use the combination. Choose another binding if it interferes with typing. Native Wayland uses a desktop shortcut instead.

**Shortcut → Window placement shortcuts** provides two optional presets: Control + Option/Alt, and Control + Option/Alt + Shift. Each uses Left/Right for half-screen placement, Up for maximize, C for center, and Down for restore. No window shortcuts are assigned by default. Apply a preset, or record individual bindings, then save. Clear window shortcuts removes only these five bindings. Presets replace their bindings and enable their commands while keeping aliases and search visibility.

Window bindings use the existing `itemPreferences` entries for `command:window-left`, `command:window-right`, `command:window-maximize`, `command:window-center`, and `command:window-restore`. They are included in settings exports and also appear under **Search → Item shortcuts and aliases**. Recording a window shortcut enables its command. Rust checks duplicates against launch, category, and other active item shortcuts before saving. If OS registration fails, the previous bindings and saved settings are retained. Some desktop-reserved keys cannot be detected by registration; change the binding if the desktop intercepts it.

Settings includes Shortcut, Appearance, Categories, Search, Clipboard history, File search, Currency, Privacy, and About. Save changes to apply visible categories, window behaviour, clipboard limits, file folders, file watching, and currency updates while TinyDash runs. Appearance applies immediately and stays in sync between windows. Closing Settings keeps an unfinished form. Use Discard to restore saved values.

Appearance provides Light, Dark, Sage, Rose, and Ink themes. Use Compact layout with any theme. These choices persist between sessions and are included in settings exports. An older Compact appearance becomes Light with Compact enabled. On macOS, Follow macOS Liquid Glass is on by default. Turn it off in Appearance for a solid theme background. The choice saves immediately and is included in settings exports. When it is on, macOS 26 or later uses native Liquid Glass. macOS controls the glass blur and transparency. On macOS 27, adjust the Liquid Glass slider in System Settings, Appearance. Reduce transparency also applies. Other platforms and earlier macOS versions use solid theme backgrounds.

On macOS, TinyDash stays out of the Dock. **Shortcut → Show menu bar icon** is off by default; enable it and save to access the menu bar menu. The icon changes without a restart. Its saved JSON key is `showMenuBarIcon` (default `false`). With it hidden, use the global shortcut or launch TinyDash again, then open Settings from Actions or Command + comma. Windows and Linux keep their tray icon.

Use **Search settings** to find a section by its label or related terms such as retention, patterns, or hotkey. Enter opens the first match and preserves unsaved edits. The Search section adds app aliases, hidden apps, and custom web search templates with a URL preview. **Item shortcuts and aliases** also configures apps, system commands, native views, and saved library items: aliases, one global shortcut, Hide, and Disable. Hidden items keep their shortcuts; disabled items reject execution. All active shortcuts must be unique. Shortcut settings include direct category keys and start at login. Category shortcuts and `tinydash --mode clipboard` open an empty category search. Start at login uses `--background` and keeps the window hidden. Privacy settings can export saved settings, preview an import before saving, and show the recovery archive. About includes an explicit update check. Release builds need an updater key and HTTPS feed; Ubuntu updates use a new `.deb` package. See [configuration](../how-to/configure.md), [data recovery](../how-to/recover-data.md), and [release preparation](../how-to/release.md).

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
  "emojiSkinTone": 0,
  "emojiLanguages": [],
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

`showSuggestions` defaults to `true`. Turn off **Show usage-based suggestions** in Search settings to remove the six-item usage list from empty All. Pins and ordinary search ranking remain available. This choice is included in settings exports.

**Search > Emoji** has a skin-tone preference and optional language keywords. `emojiSkinTone` is 0–5: default, light, medium-light, medium, medium-dark, or dark. `emojiLanguages` is a list of distinct supported codes: `zh` (Simplified Chinese), `ms` (Malay), and `es` (Spanish). The default is `[]`; English names and shortcodes are always enabled. These fields are included in settings import/export. See [emoji behavior](features/emoji.md).

On macOS, a saved `CommandOrControl+Shift+Space` value from earlier TinyDash versions now uses `Control+Shift+Space`. This compatibility rule keeps the settings file intact, including other user settings. Other custom shortcuts remain unchanged. New settings files contain `Control+Shift+Space`.

`clipboardDefaultAction` accepts `"copy"` (default) or `"paste"`. Change it in **Clipboard history → Default text action**. It controls primary activation of saved clipboard text in All and Clipboard; explicit Copy and Paste remain available. Images, file references, and other result types are unchanged.

Set `clipboardHistoryLimit` to a value between 1 and 500. TinyDash applies the limit when saved in Settings, on restart, and after each capture. Missing settings use their defaults. The settings file remains the editable startup configuration. Usage and clipboard data are stored separately in SQLite. `clipboardRetentionDays` is 0–3650 (0 means no age expiry; text pins remain exempt). `clipboardExcludedApps` lists exact names/identifiers. `clipboardCaptureImages` and `clipboardCaptureFiles` default off; rich capture currently requires macOS. See [clipboard history](features/clipboard.md) for source-attribution and storage limits.

Set `fileSearchRoots` to `null` for the default folders, `[]` to disable file scanning, or an array of absolute paths. A leading `~` refers to your home folder on all three platforms. For example, `["~/Documents", "~/Projects"]` scans those two folders; `["~"]` scans your home folder. Windows paths in JSON need escaped backslashes, such as `"C:\\Users\\Alex\\Documents"`. `fileSearchLimit` is restricted to 1 through 100,000. `fileSearchExcludedDirs` contains exact folder names, not patterns. `fileSearchIncludeHidden` defaults off. `fileSearchIgnorePatterns` accepts up to 64 basename/root-relative glob patterns, each at most 256 bytes, using `*`, `?`, and `**`; forward slashes are required, with no negation or character classes. Changes saved in Settings start a new scan and update the file watcher. Manual JSON edits take effect after a process restart.
