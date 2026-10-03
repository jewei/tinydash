# Native utilities

Open **Actions → Utilities**, or search All/System for **Quit a process**, **Color picker**, **Keep awake**, **Media controls**, or **Window management**. Each opens the relevant tab. Searching these command names performs no OS enumeration. Panels release their timers when closed.

## Processes

Refresh an on-demand snapshot of the current user's processes, capped at 4096. System/root-owned processes, PID 0/1, TinyDash itself, and executable names containing `tinydash` are excluded. Filter by name or PID.

Quit sends SIGTERM on Unix or requests the Windows main window to close; neither guarantees graceful saving. Force kill loses unsaved work. Both require confirmation, with Cancel initially focused. Rust issues a random, single-use, 30-second token bound to the process identity and action, then checks identity again before execution.

Linux uses pidfds and requires Linux 5.3 or later. Windows uses the current SID/session, skips inaccessible processes, and pins the process handle during its final start-time check; enumeration requires Windows PowerShell/CIM. macOS checks owner, name, and microsecond start time before signaling; a small final PID-reuse race remains because this path has no pidfd equivalent. No operation requests elevation.

### Quit from app results

App results also offer **Quit** and **Force Quit** in Actions. They share the process confirmation rules above. Matching uses the catalog path, not the app's display name, and occurs only after selecting an action. Multiple matching accessible processes require selection in Utilities. TinyDash and protected processes remain excluded. Cancel closes the dialog without a termination request. An expired or failed confirmation requires another app check and a new confirmation.

- macOS matches the canonical bundle URL of a running application. Quit uses `NSRunningApplication.terminate`, which permits the app's save dialogs. Force Quit uses `forceTerminate`. Native process identity is checked before either request.
- Windows supports executable results and `.lnk` shortcuts whose target is an executable with no arguments. Matching requires the exact executable path and a main window in the current user's session. Deployment shortcuts, shared launchers, and inaccessible processes require Utilities.
- Linux resolves a desktop entry's executable and matches `/proc/<pid>/exe`. Entries with arguments other than standard desktop field codes, known shared launchers, and ambiguous multi-process apps require Utilities. Quit sends SIGTERM; it does not guarantee a save dialog. Force Quit sends SIGKILL. The existing pidfd checks require Linux 5.3 or later.

These actions do not elevate permissions, launch a stopped app, terminate an entire process tree, or update launch frequency. The success message reports a request, because an app can take time to exit or refuse normal closure.

## Colors

Enter short/long hex, RGB(A), or HSL(A), then choose **Convert color**. RGB channels may be numeric or percentages; HSL saturation/lightness use percentages. Invalid, out-of-range, and nonfinite values are rejected. **Copy HEX**, **Copy RGB**, and **Copy HSL** reparse the displayed result's original input in Rust before writing the clipboard. Editing an unconverted draft does not change the copied result. Named colors and other CSS color spaces are not supported.

Converted values have separate HEX, RGB, and HSL labels below the color preview. The values share one row in wide windows and stack in narrow windows. Utility tabs scroll horizontally when they do not fit.

On macOS, **Pick from screen** uses `NSColorSampler` on the main thread and converts to 8-bit sRGB. Click a pixel or press Escape. Only one native session is allowed; the IPC wait is bounded to 120 seconds. macOS provides no cancellation API here: a timed-out sampler must still be dismissed with Escape. Other platforms use the WebView EyeDropper API only when available and directly invoked by a click. Linux WebKitGTK usually lacks it; enter a color instead.

## Keep awake

Start a 1–480 minute idle-sleep inhibitor, or Stop it explicitly. Replacing the timer releases the previous assertion. A single owner thread releases the assertion/descriptor on expiry, cancellation, channel disconnection, and application exit. It creates no child process and does not persist active state.

macOS uses IOKit; Windows uses a same-thread execution-state assertion; Linux holds a logind inhibitor file descriptor through GIO and requires appropriate session policy. It does not promise to keep the display lit or override lid behavior. Linux's `sleep:idle` inhibition may also block explicit sleep, depending on policy.

Closing the panel leaves the timer running. The launcher header shows its end time and a Stop button. Status is queried while active and once on mount, not continuously when idle. The countdown uses wall time for display; expiry uses a monotonic deadline.

## Media

Play/pause, next, previous, volume up/down, and mute target the OS/tool-selected media session. macOS checks event-posting access and sends system media keys; Windows sends media keys. Linux requires `playerctl` for MPRIS playback and `wpctl` for volume. Missing tools, permissions, and OS failures are reported. Successful dispatch alone does not prove a player changed state.

## Windows

Opening the Windows tab resolves the application/window captured before TinyDash activated, then reads geometry on demand in a background worker. It does not operate on the launcher. **Capture window in 3 seconds** is an alternative: focus the target during that interval.

Left half, right half, maximize, center, and restore operate on that captured target with identity revalidation. Restore restores geometry, not maximized/minimized state.

### Direct placement commands and presets

Search All/System for **Move window left**, **Move window right**, **Maximize window**, **Center window**, or **Restore window**. These commands act on the application captured before the launcher opened and dismiss the launcher after success. They can be pinned and use the existing item aliases, visibility, disable, and shortcut settings.

In **Settings → Shortcut → Window placement shortcuts**, apply the Control + Option/Alt preset or its Shift variant. Left/Right place the window in the corresponding half, Up maximizes, C centers, and Down restores. Each binding can be recorded or removed separately. Save applies the draft only after shortcut validation and OS registration succeed. Window shortcuts are off by default. They use the same item preferences as the Search settings; there is no separate preset state to keep in sync.

Global placement shortcuts capture the foreground app/window identity before background work starts. They do not change the saved paste target or open the launcher on success. Native work checks the process identity and verifies that the captured window still has focus before requesting a move. Errors open the launcher with a message. Repeated presses during a move are ignored rather than queued. On macOS, the exact Accessibility window is captured when the worker starts; a switch to another window in the same app before that capture can select the newer window.

Direct commands and shortcuts share a bounded restore history. The first placement saves the window's geometry; later placements keep that original geometry. Center uses the current size and current display. Restore consumes the saved entry after success. History retains at most 16 recently used windows, and entries expire 30 minutes after their last placement when the next command runs. It is cleared at exit and is never written to disk. The Utilities panel still restores its separately captured geometry. Restore does not restore maximized/minimized state.

- macOS requires Accessibility and retains the AX window element. Placement uses the target display's current visible work area, excluding the Dock and menu bar. Screen frames are read on the main thread and converted to Accessibility's global logical-point coordinates, including displays above/left of the primary display. No Retina pixel scaling is applied.
- Windows uses the monitor work area and revalidates HWND, PID, and process start identity.
- Linux requires X11, EWMH, `xdotool`, `xprop`, and `wmctrl`; placement uses desktop work area rather than per-monitor work areas.
- Wayland explicitly reports unsupported. Window-manager constraints may adjust placement.

## Verification

For window presets, check both draft presets, per-action recording/removal, clear, discard, save, restart, export/import, and editing the same binding through Search settings. Check conflicts with launch/category/item shortcuts, OS registration failure, disabled commands, and Wayland controls. Confirm that failure leaves the old settings and registrations intact.

In controlled desktop sessions, focus a disposable app and exercise all five shortcuts. Confirm external geometry, preserved focus, and no launcher on success. Place window A twice, place window B, then restore each and check its original geometry. Check Restore before any placement, history expiry/eviction, closed windows, rapid presses, focus changes, and permission failures. Repeat through search results and pins. On macOS, check two windows in one app and multiple displays. Browser IPC cannot prove these effects.

```sh
bun run verify:browser tests/utilities.spec.ts tests/roadmap.spec.ts
bun run test:rust -- launcher::utilities
```

Component tests cover process confirmation/cancellation, identity payloads, conversion/copy errors, sampler cancellation, duration validation, status/stop, inactive polling cleanup, media/window dispatch, and permission errors. The roadmap suite also opens the panel through the real launcher UI. Browser IPC is mocked. Rust tests cover parsing, confirmation expiry/replay/consent, protected PIDs, process snapshots, geometry, and effect guards. Test builds reject utility desktop effects; `TINYDASH_DISABLE_UTILITY_EFFECTS=1` also disables them in an instrumented desktop build.

The Windows/Linux X11 native suite opens **Color picker** from a search result, converts a color in Rust, checks the exact HEX value in the OS clipboard, and closes the panel to check search focus. It also checks the complete System command catalog.

Other native proof requires controlled macOS, Windows, and Linux X11 sessions, plus Wayland rejection. Use disposable processes/windows and short timers. Verify termination, stale-identity rejection, timer expiry/replacement/quit cleanup, actual sampled color, actual media state, window geometry, and restore. Test missing permissions/tools. Never kill personal applications or rearrange personal windows during automation. Follow [verification](../../how-to/verify.md) and retain the tested build identity separately from mocked browser evidence.
