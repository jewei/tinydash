# Configure search and shortcuts

## Change the global shortcut

1. Open Actions, then Settings. You can also use Command + comma on macOS or Ctrl + comma on Windows and Linux.
2. Select Shortcut, then Record new.
3. Press a key combination and select Save changes.

TinyDash keeps the previous shortcut if registration or saving fails. On Wayland, configure a desktop shortcut to `tinydash` instead.

## Choose visible categories

1. Open Settings, then Categories.
2. Select the categories to show. Keep at least one selected.
3. Select Save changes.

Hidden categories still contribute matching results to All. Category shortcuts need a visible category and a unique key combination.

## Add an application alias

Open Settings, then Search. Select an application and enter its aliases. Save the changes. The same section can hide an application from search.

## Add a custom web search

In Settings, Search, add a keyword and an HTTP or HTTPS template with exactly one `{query}` placeholder. For example, use `https://example.com/search?q={query}`. Check the preview, then save. Type the keyword and search text in the launcher.

## Choose file search folders

In Settings, File search, add the folders to scan and save. TinyDash starts a new scan. Use smaller folders if the index reaches its limit. See [file search](../reference/features/files.md) for exclusions and limits.

## Export and import settings

Open Settings, then Privacy. Save or discard unfinished changes before export. Choose Export settings and save the JSON file.

To import, choose Import settings and select the file. Read the preview before applying it. Save changes to persist the imported settings. Canceling the preview keeps the current settings.

See [settings reference](../reference/settings.md) for defaults and manual configuration.
