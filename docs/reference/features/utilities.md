# Native utilities

Open **Actions → Utilities**, or search All/System for **Quit a process**, **Color picker**, **Keep awake**, **Media controls**, or **Window management**. Each opens the relevant tab. Searching these command names performs no OS enumeration. Panels release their timers when closed.

## Processes

Refresh an on-demand snapshot of the current user's processes, capped at 4096. System/root-owned processes, PID 0/1, TinyDash itself, and executable names containing `tinydash` are excluded. Filter by name or PID.

Quit sends SIGTERM on Unix or requests the Windows main window to close; neither guarantees graceful saving. Force kill loses unsaved work. Both require confirmation, with Cancel initially focused. Rust issues a random, single-use, 30-second token bound to the process identity and action, then checks identity again before execution.

Linux uses pidfds and requires Linux 5.3 or later. Windows uses the current SID/session, skips inaccessible processes, and pins the process handle during its final start-time check; enumeration requires Windows PowerShell/CIM. macOS checks owner, name, and microsecond start time before signaling; a small final PID-reuse race remains because this path has no pidfd equivalent. No operation requests elevation.

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

- macOS requires Accessibility and retains the AX window element. Placement uses the target display's current visible work area, excluding the Dock and menu bar. Screen frames are read on the main thread and converted to Accessibility's global logical-point coordinates, including displays above/left of the primary display. No Retina pixel scaling is applied.
- Windows uses the monitor work area and revalidates HWND, PID, and process start identity.
- Linux requires X11, EWMH, `xdotool`, `xprop`, and `wmctrl`; placement uses desktop work area rather than per-monitor work areas.
- Wayland explicitly reports unsupported. Window-manager constraints may adjust placement.

## Verification

```sh
bun run verify:browser tests/utilities.spec.ts tests/roadmap.spec.ts
bun run test:rust -- launcher::utilities
```

Component tests cover process confirmation/cancellation, identity payloads, conversion/copy errors, sampler cancellation, duration validation, status/stop, inactive polling cleanup, media/window dispatch, and permission errors. The roadmap suite also opens the panel through the real launcher UI. Browser IPC is mocked. Rust tests cover parsing, confirmation expiry/replay/consent, protected PIDs, process snapshots, geometry, and effect guards. Test builds reject utility desktop effects; `TINYDASH_DISABLE_UTILITY_EFFECTS=1` also disables them in an instrumented desktop build.

The Windows/Linux X11 native suite opens **Color picker** from a search result, converts a color in Rust, checks the exact HEX value in the OS clipboard, and closes the panel to check search focus. It also checks the complete System command catalog.

Other native proof requires controlled macOS, Windows, and Linux X11 sessions, plus Wayland rejection. Use disposable processes/windows and short timers. Verify termination, stale-identity rejection, timer expiry/replacement/quit cleanup, actual sampled color, actual media state, window geometry, and restore. Test missing permissions/tools. Never kill personal applications or rearrange personal windows during automation. Follow [verification](../../how-to/verify.md) and retain the tested build identity separately from mocked browser evidence.
