---
name: formal-verify-clipboard
description: "Model-check TinyDash's clipboard history (the monitor, Clear, and history on/off) with TLA+. Use before you change internal/engine/monitor.go, internal/engine/clipboard.go, or internal/engine/settings.go, or how Engine.Run orders changes, and whenever a change could let a cleared or switched-off clipboard entry come back."
---

# Formal verification: clipboard history

## The promise

Nothing that the clipboard monitor read before a Clear or a switch-off of clipboard history is saved after it. The harm if it breaks: the user clears the history, for example after copying a password, and the cleared text comes back.

## What is verified, and how far

- Model: `formal/ClipboardHistory.tla`. Rule: `NothingStaleIsSaved` (each saved entry was read in the same history generation in which it was saved). Also `TypeOK`, which includes `epoch = gen`.
- `formal/ClipboardHistory.cfg` models the code: up to 3 copies and 3 user requests (Clear, history off, history on), in every interleaving with the monitor's steps and the writer. It passes: about 94,000 distinct states.
- `formal/ClipboardHistoryBroken.cfg` sets `EpochFirst = FALSE`: the monitor reads the clipboard before the epoch. It must fail with `NothingStaleIsSaved`. This shows the check can catch the bug.
- `Monitor.Tick` reads no content while history is off; the model lets the monitor read then too. The model allows more than the code does, so a pass still holds for the code.
- Left out: the content of clips, image and file kinds, the history limit, deletes of single entries, pins, the store's SQL, and the OS clipboard itself. The bound covers 3 copies and 3 requests, not more.

## Coverage record

Each part of the model is one step of this code:

| Model                                                           | Code                                                                                                                                           |
| --------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------- |
| `MonitorStarts`, `MonitorFinishes`, `MonitorQueuesSave`, `seen` | `Monitor.Tick` in `internal/engine/monitor.go`: counter check, `ClipEpoch()`, `read()`, `SaveClip`                                             |
| `EpochFirst = TRUE`                                             | the order of `epoch := m.engine.ClipEpoch()` before `content, ok := m.read()` in `Monitor.Tick`                                                |
| `queue`, `WriterApplies`                                        | `Engine.Change` and `Engine.Run` in `internal/engine/engine.go`: one writer, changes in order                                                  |
| `WriterApplies` "save"                                          | `Engine.SaveClip` in `internal/engine/clipboard.go`: the epoch and enabled check                                                               |
| `WriterApplies` "clear"                                         | `Engine.ClearClipboard`: bumps `clipEpoch`, then clears                                                                                        |
| `WriterApplies` "off" / "on"                                    | `Engine.SetClipboardHistory` and `Engine.UpdateSettings` (`internal/engine/settings.go`): each bumps `clipEpoch` only when turning history off |

Sync commit: the model was last confirmed to describe the code at the commit that added this skill (`git log -1 --format=%h -- .claude/skills/formal-verify-clipboard/SKILL.md`). Move it forward only after you confirm, file by file, that the table above still describes the code. To see what changed since: `git diff <sync commit> -- internal/engine/monitor.go internal/engine/clipboard.go internal/engine/engine.go internal/engine/settings.go`.

## Run it

```sh
bash scripts/formal-verify.sh
```

It needs Java 11 or later. The first run downloads TLC 1.7.4 into `~/.cache/tinydash` and checks its SHA-256. It takes under a minute. Then run the replay of the model's bug path against the real code:

```sh
go test -count=1 -race -run 'AClearDuringATickWins|ContentReadBefore' ./internal/engine/
```

## Read a result

Read every result with the `/statechart` skill: on a violation it gives the shortest path to the broken rule in plain steps; on a pass it draws the chart. Set `TLA2TOOLS` and `JAVA` if mise is not installed.

- `ClipboardHistory.cfg` fails: the design allows a stale save. Turn the shortest path into a test in `internal/engine/monitor_test.go`, one step per action, the way `TestAClearDuringATickWins` replays the known path. If the test passes on the real code, suspect the model before the code: a step in the model that the code does not take, or a missing guard.
- `ClipboardHistoryBroken.cfg` passes: the model no longer catches the bug it guards. Fix the model; never delete the broken config.
- Never weaken `NothingStaleIsSaved` or the bounds to make a check pass.

## When the code changes on purpose

1. Update the model so each step is still one step of the code, and update the coverage table.
2. Run `bash scripts/formal-verify.sh` and the replay tests.
3. When both pass and the table is true, commit; this skill's commit is the new sync point.
