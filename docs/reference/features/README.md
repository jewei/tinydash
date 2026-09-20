# Features and verification map

This is the maintained feature catalog. Each page states supported behavior, entry points, expected results, tests, and proof limits. Update the relevant page when behavior or its verification changes.

| Feature                                        | Current checks                               |
| ---------------------------------------------- | -------------------------------------------- |
| [Launcher and navigation](launcher.md)         | Native and browser                           |
| [Application search](apps.md)                  | Native and browser                           |
| [File search](files.md)                        | Native and browser                           |
| [Clipboard history](clipboard.md)              | Native and browser                           |
| [Calculator and currency](calculator.md)       | Native and browser                           |
| [Emoji search](emoji.md)                       | Native and browser                           |
| [System commands](system.md)                   | Native and browser                           |
| [Password generation](passwords.md)            | Browser and Rust; desktop checks where noted |
| [Dates and time zones](datetime.md)            | Browser and Rust; desktop checks where noted |
| [URL cleaning](urls.md)                        | Browser and Rust; desktop checks where noted |
| [Web search](web.md)                           | Browser and Rust; desktop checks where noted |
| [Pins](pins.md)                                | Browser and Rust; desktop checks where noted |
| [Appearance and categories](appearance.md)     | Browser and Rust; desktop checks where noted |
| [Settings, recovery, and updates](settings.md) | Browser and Rust; desktop checks where noted |

Start with launcher navigation, application launch, calculator copying, clipboard history, and file watching when checking a desktop build. Other features remain part of the full catalog.

Browser tests use a mock backend. Rust tests exercise backend logic. Native tests use the installed application on Windows and Linux X11. A pass at one level does not prove behavior at another level. Follow [verification](../../how-to/verify.md).
