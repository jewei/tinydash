---
name: formal-verify-clipboard
description: "Model-check TinyDash's clipboard history (the monitor, Clear, and history on/off) with TLA+. Use before you change internal/engine/monitor.go, internal/engine/clipboard.go, or internal/engine/settings.go, or how Engine.Run orders changes, and whenever a change could let a cleared clipboard entry come back or save content that should stay unsaved."
---

# Formal verification: clipboard history

## The promise

A copy that the user cleared never comes back, nothing that the monitor read before a Clear or a switch-off is saved after it, and what was on the clipboard when TinyDash started is never saved. The harm if it breaks: the user clears the history, for example after copying a password, and the cleared text comes back.

## What is verified, and how far

- Model: `formal/ClipboardHistory.tla`. Rules:
  - `NothingStaleIsSaved`: each saved entry was read in the same history generation in which it was saved.
  - `ClearedNeverReturns`: no copy that was in the history at a Clear is saved again.
  - `StartupNeverSaved`: the content at startup is never saved.
  - `TypeOK`, which includes `epoch = gen`.
- Each config covers up to 3 copies and 3 user requests (Clear, history off, history on), in every interleaving with the monitor's steps and the writer. The configs that model the code must pass:
  - `ClipboardHistory.counted.cfg`: macOS and Windows, which have a change counter (about 62,000 states).
  - `ClipboardHistory.linux.cfg`: Linux, where the monitor compares content (about 138,000 states).
- The `.broken` configs model known bugs and must fail. They show the check can catch them:
  - `counted.broken` (`EpochFirst = FALSE`: the clipboard read before the epoch) fails `NothingStaleIsSaved`.
  - `counted.unfixed.broken` (`Fixed = FALSE`: no counter recheck after the read) fails `ClearedNeverReturns`.
  - `linux.broken` (`Fixed = FALSE`: a switch-off reset the seen content) fails `ClearedNeverReturns`.
- Not a rule: "nothing copied while history was off is saved". With a counter, a copy made in the last half second before history comes back on can be saved; the Rust app polled the same counter and had the same race.
- Left out: the content of clips, image and file kinds, the history limit, deletes of single entries, pins, the store's SQL, and the OS clipboard itself. A content read and the counter recheck are one step in the model; in the code they are two calls, and a copy between them only makes the recheck drop the read. The bound covers 3 copies and 3 requests, not more.

## Coverage record

Each part of the model is one step of this code:

| Model | Code |
| --- | --- |
| `MonitorStarts`: `seen`, `monOffs`, `rebase`, the `enabled` check | `Monitor.Tick` in `internal/engine/monitor.go`, up to `m.read()`: the counter check, `historyOffs`, `ClipEpoch()`, and the history-off branch |
| `MonitorFinishes`: the second read, the counter recheck, `monFirst`, `seenCopy` | `Monitor.Tick` from `m.read()` to the `first` check: the recheck `after != n`, `seenHash` |
| `MonitorQueuesSave` | `m.engine.SaveClip(epoch, content, now)` |
| `EpochFirst = TRUE` | `epoch := m.engine.ClipEpoch()` before `content, ok := m.read()` |
| `queue`, `WriterApplies` | `Engine.Change` and `Engine.Run` in `internal/engine/engine.go`: one writer, changes in order |
| `WriterApplies` "save" | `Engine.SaveClip` in `internal/engine/clipboard.go`: the epoch and enabled check |
| `WriterApplies` "clear" | `Engine.ClearClipboard` in `internal/engine/clipboard.go`: bumps `clipEpoch`, then clears |
| `WriterApplies` "off" / "on" | `Engine.UpdateSettings` in `internal/engine/settings.go`: turning history off bumps `historyOffs` and `clipEpoch` |

Sync commit: the model was last confirmed to describe the code at the commit that last changed this skill (`git log -1 --format=%h -- .claude/skills/formal-verify-clipboard/SKILL.md`). Move it forward only after you confirm, file by file, that the table above still describes the code. To see what changed since: `git diff <sync commit> -- internal/engine/monitor.go internal/engine/clipboard.go internal/engine/engine.go internal/engine/settings.go`.

## Run it

```sh
bash scripts/formal-verify.sh
```

It needs Java 11 or later. The first run downloads TLC 1.7.4 into `~/.cache/tinydash` and checks its SHA-256; set `TLA2TOOLS` to use another copy, and `JAVA` for another Java. It takes about a minute. Then run the replays of the model's paths against the real code:

```sh
go test -count=1 -race -run 'AClearDuringATickWins|ContentReadBefore|DuringATickIsSavedOnce|WhileHistoryIsOff|ClearedCopy|ShortOffPeriod|RightAfterAClear' ./internal/engine/
```

## Read a result

Read every result with the `/statechart` skill: on a violation it gives the shortest path to the broken rule in plain steps; on a pass it draws the chart.

- A config that models the code fails: the design allows the bug. Turn the shortest path into a test in `internal/engine/monitor_test.go`, one step per action, run it for both `counted: true` and `counted: false` where it applies, the way `TestACopyDuringATickIsSavedOnceByTheNextTick` replays a path the model found. If the test passes on the real code, suspect the model before the code: a step in the model that the code does not take, or a missing guard.
- A `.broken` config passes: the model no longer catches the bug it guards. Fix the model; never delete a broken config.
- Never weaken a rule or the bounds to make a check pass.

## When the code changes on purpose

1. Update the model so each step is still one step of the code, and update the coverage table.
2. Run `bash scripts/formal-verify.sh` and the replay tests.
3. When both pass and the table is true, commit; this skill's commit is the new sync point.
