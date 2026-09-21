# Command line

| Command                    | Behavior                                        |
| -------------------------- | ----------------------------------------------- |
| `tinydash`                 | Open the launcher, or show the existing process |
| `tinydash --settings`      | Open Settings                                   |
| `tinydash --background`    | Start without showing the launcher              |
| `tinydash --mode CATEGORY` | Open a category with an empty query             |
| `tinydash --help`          | Print supported arguments                       |

`CATEGORY` accepts `all`, `apps`, `files`, `clipboard`, `calculator`, `system`, `emoji`, `password`, `timezone`, `url`, and `web`. The UI calls `timezone` Datetime.

Start at login uses `--background`. Category shortcuts use the same empty-query behavior as `--mode`. On Wayland, assign a compositor shortcut to one of these commands.
