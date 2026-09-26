import {
  For,
  Show,
  createEffect,
  createSignal,
  onCleanup,
  onMount,
} from "solid-js";
import {
  awakeChangedEvent,
  utilities,
  type AwakeStatus,
  type MediaAction,
  type ProcessConfirmation,
  type UtilityCapabilities,
  type UtilityColor,
  type UtilityProcess,
  type UtilityTab,
  type WindowAction,
} from "../utilities-bridge";
import "../styles/utilities.css";

const tabs: { id: UtilityTab; label: string }[] = [
  { id: "processes", label: "Processes" },
  { id: "colors", label: "Colors" },
  { id: "awake", label: "Keep awake" },
  { id: "media", label: "Media" },
  { id: "windows", label: "Windows" },
];
const stopped: AwakeStatus = {
  active: false,
  endsAt: null,
  remainingSeconds: 0,
};

/** Mount once in the launcher header to keep the timed assertion visible after the panel closes. */
export function UtilitiesAwakeIndicator() {
  const [status, setStatus] = createSignal(stopped);
  const [error, setError] = createSignal("");
  let alive = true;
  let timer: ReturnType<typeof setTimeout> | undefined;
  const update = (next: AwakeStatus) => {
    if (!alive) return;
    setStatus(next);
    clearTimeout(timer);
    if (next.active)
      timer = setTimeout(
        () =>
          void utilities
            .awakeStatus()
            .then(update)
            .catch((e: unknown) => setError(String(e))),
        Math.max(
          500,
          Math.min(30000, (next.endsAt ?? Date.now()) - Date.now() + 100),
        ),
      );
  };
  const listener = (event: Event) =>
    update((event as CustomEvent<AwakeStatus>).detail);
  onMount(() => {
    window.addEventListener(awakeChangedEvent, listener);
    void utilities
      .awakeStatus()
      .then(update)
      .catch((e: unknown) => setError(String(e)));
  });
  onCleanup(() => {
    alive = false;
    clearTimeout(timer);
    window.removeEventListener(awakeChangedEvent, listener);
  });
  return (
    <>
      <Show when={status().active}>
        <button
          class="utility-awake-indicator"
          title="Stop preventing idle sleep"
          onClick={() =>
            void utilities
              .setAwake(0)
              .then(update)
              .catch((e: unknown) => setError(String(e)))
          }
        >
          Awake until{" "}
          {new Date(status().endsAt ?? Date.now()).toLocaleTimeString([], {
            hour: "2-digit",
            minute: "2-digit",
          })}{" "}
          · Stop
        </button>
      </Show>
      <Show when={error()}>
        <span role="status">{error()}</span>
      </Show>
    </>
  );
}

export default function UtilitiesPanel(props: {
  onClose: () => void;
  initialTab?: string;
}) {
  const [tab, setTab] = createSignal<UtilityTab>(
    tabs.find((t) => t.id === props.initialTab)?.id ?? "processes",
  );
  const [capabilities, setCapabilities] = createSignal<UtilityCapabilities>();
  const [processes, setProcesses] = createSignal<UtilityProcess[]>([]);
  const [filter, setFilter] = createSignal("");
  const [confirmation, setConfirmation] = createSignal<ProcessConfirmation>();
  const [input, setInput] = createSignal("#6750A4");
  const [color, setColor] = createSignal<UtilityColor>();
  const [convertedInput, setConvertedInput] = createSignal("");
  const [awake, setAwake] = createSignal(stopped);
  const [minutes, setMinutes] = createSignal(30);
  const [now, setNow] = createSignal(Date.now());
  const [target, setTarget] = createSignal(
    "Previously captured window, if available",
  );
  const [error, setError] = createSignal("");
  const [message, setMessage] = createSignal("");
  const [busy, setBusy] = createSignal(false);
  let panel: HTMLElement | undefined;
  let cancel: HTMLButtonElement | undefined;
  let lastAction: HTMLButtonElement | undefined;
  let alive = true;
  let loadedProcesses = false;
  let loadedWindow = false;
  const abort = new AbortController();
  const work = async (task: () => Promise<void>) => {
    if (busy()) return;
    setBusy(true);
    setError("");
    setMessage("");
    try {
      await task();
    } catch (e) {
      if (alive) setError(String(e));
    } finally {
      if (alive) setBusy(false);
    }
  };
  const refresh = async () => {
    const rows = await utilities.processes();
    if (alive) setProcesses(rows);
  };
  const dismiss = () => {
    if (busy() && confirmation()) return;
    setConfirmation(undefined);
    setError("");
    void utilities.cancelProcess().catch(() => {});
    lastAction?.focus();
  };
  const close = () => {
    if (busy() && confirmation()) return;
    dismiss();
    props.onClose();
  };
  onMount(() => {
    panel?.focus();
    void utilities
      .capabilities()
      .then((v) => {
        if (alive) setCapabilities(v);
      })
      .catch((e: unknown) => {
        if (alive) setError(String(e));
      });
    void utilities
      .awakeStatus()
      .then((v) => {
        if (alive) setAwake(v);
      })
      .catch((e: unknown) => {
        if (alive) setError(String(e));
      });
  });
  createEffect(() => {
    if (tab() === "processes" && !loadedProcesses) {
      loadedProcesses = true;
      void work(refresh);
    }
    if (tab() === "windows" && !loadedWindow) {
      loadedWindow = true;
      void work(async () => {
        const name = await utilities.captureWindow(false);
        if (alive) setTarget(name);
      });
    }
  });
  createEffect(() => {
    if (confirmation() && !busy()) queueMicrotask(() => cancel?.focus());
  });
  createEffect(() => {
    if (!awake().active) return;
    const timer = setInterval(() => {
      setNow(Date.now());
      if (Date.now() >= (awake().endsAt ?? 0))
        void utilities
          .awakeStatus()
          .then((v) => {
            if (alive) setAwake(v);
          })
          .catch((e: unknown) => {
            if (alive) setError(String(e));
          });
    }, 1000);
    onCleanup(() => clearInterval(timer));
  });
  onCleanup(() => {
    alive = false;
    abort.abort();
    void utilities.cancelProcess().catch(() => {});
  });
  const chooseTab = (id: UtilityTab) => {
    dismiss();
    setTab(id);
    setMessage("");
  };
  const filtered = () =>
    processes().filter((p) =>
      `${p.name} ${p.pid}`.toLowerCase().includes(filter().toLowerCase()),
    );
  const media: { action: MediaAction; label: string }[] = [
    { action: "previous", label: "Previous track" },
    { action: "playPause", label: "Play / Pause" },
    { action: "next", label: "Next track" },
    { action: "volumeDown", label: "Volume down" },
    { action: "volumeUp", label: "Volume up" },
    { action: "mute", label: "Toggle mute" },
  ];
  const windows: { action: WindowAction; label: string }[] = [
    { action: "left", label: "Left half" },
    { action: "right", label: "Right half" },
    { action: "maximize", label: "Maximize" },
    { action: "center", label: "Center" },
    { action: "restore", label: "Restore captured bounds" },
  ];

  return (
    <section
      class="utilities-panel"
      ref={panel}
      tabIndex={-1}
      aria-label="Native utilities"
      onKeyDown={(event) => {
        event.stopPropagation();
        if (event.key === "Escape") {
          event.preventDefault();
          if (confirmation()) dismiss();
          else close();
        }
        if (event.key === "Tab" && confirmation()) {
          const buttons = panel?.querySelectorAll<HTMLButtonElement>(
            ".utility-confirm button:not(:disabled)",
          );
          if (buttons?.length) {
            const first = buttons[0];
            const last = buttons[buttons.length - 1];
            if (event.shiftKey && document.activeElement === first) {
              event.preventDefault();
              last.focus();
            } else if (!event.shiftKey && document.activeElement === last) {
              event.preventDefault();
              first.focus();
            }
          }
        }
      }}
    >
      <header>
        <h2>Utilities</h2>
        <button onClick={close} aria-label="Close utilities">
          Close
        </button>
      </header>
      <div inert={!!confirmation()}>
        <nav aria-label="Utility categories">
          <For each={tabs}>
            {(item) => (
              <button
                aria-pressed={tab() === item.id}
                disabled={busy()}
                onClick={() => chooseTab(item.id)}
              >
                {item.label}
              </button>
            )}
          </For>
        </nav>
        <div class="utility-content" aria-busy={busy()}>
          <Show when={tab() === "processes"}>
            <p>{capabilities()?.processes}</p>
            <div class="utility-toolbar">
              <input
                aria-label="Filter processes"
                placeholder="Filter by name or PID"
                value={filter()}
                onInput={(e) => setFilter(e.currentTarget.value)}
              />
              <button disabled={busy()} onClick={() => void work(refresh)}>
                Refresh processes
              </button>
            </div>
            <p>{filtered().length} processes · snapshots, not a live monitor</p>
            <ul class="utility-processes">
              <For each={filtered()}>
                {(process) => (
                  <li>
                    <div>
                      <strong>{process.name}</strong>
                      <small>PID {process.pid}</small>
                    </div>
                    <For each={[false, true]}>
                      {(force) => (
                        <button
                          disabled={busy()}
                          classList={{ "utility-danger": force }}
                          onClick={(e) => {
                            lastAction = e.currentTarget;
                            void work(async () => {
                              const result = await utilities.prepareProcess(
                                process,
                                force,
                              );
                              if (alive) setConfirmation(result);
                            });
                          }}
                        >
                          {force ? "Force kill" : "Quit"}
                        </button>
                      )}
                    </For>
                  </li>
                )}
              </For>
            </ul>
            <Show when={!busy() && !filtered().length}>
              <p>No matching user processes. Try refreshing.</p>
            </Show>
          </Show>
          <Show when={tab() === "colors"}>
            <p>
              Convert hex, RGB(A), or HSL(A). Named colors and other CSS color
              spaces are not supported.
            </p>
            <form
              onSubmit={(e) => {
                e.preventDefault();
                void work(async () => {
                  const original = input();
                  const result = await utilities.color(original);
                  if (alive) {
                    setColor(result);
                    setConvertedInput(original);
                  }
                });
              }}
            >
              <label>
                Color
                <input
                  value={input()}
                  onInput={(e) => setInput(e.currentTarget.value)}
                  maxlength={128}
                  placeholder="#RRGGBB or hsl(210 50% 50%)"
                />
              </label>
              <button disabled={busy()}>Convert color</button>
            </form>
            <button
              disabled={
                busy() ||
                !(
                  capabilities()?.nativeEyedropper ||
                  utilities.canSampleScreen()
                )
              }
              onClick={() =>
                void work(async () => {
                  try {
                    const result = await utilities.sampleScreen(
                      abort.signal,
                      capabilities()?.nativeEyedropper,
                    );
                    if (alive) {
                      setColor(result);
                      setConvertedInput(result.hex);
                      setInput(result.hex);
                    }
                  } catch (e) {
                    if (!(e instanceof DOMException && e.name === "AbortError"))
                      throw e;
                  }
                })
              }
            >
              Pick screen color
            </button>
            <p>{capabilities()?.eyedropper}</p>
            <Show when={color()}>
              {(value) => (
                <div class="utility-color-result">
                  <span
                    class="utility-swatch"
                    style={{ "background-color": value().hex }}
                    aria-label={`Color preview ${value().hex}`}
                  />
                  <div class="utility-toolbar">
                    <For each={["hex", "rgb", "hsl"] as const}>
                      {(format) => (
                        <button
                          disabled={busy()}
                          onClick={() =>
                            void work(async () => {
                              await utilities.copyColor(
                                convertedInput(),
                                format,
                              );
                              if (alive)
                                setMessage(
                                  `${format.toUpperCase()} copied to clipboard.`,
                                );
                            })
                          }
                        >
                          Copy {format.toUpperCase()}
                        </button>
                      )}
                    </For>
                  </div>
                  <For each={[value().hex, value().rgb, value().hsl]}>
                    {(text) => (
                      <label>
                        Converted color
                        <input
                          readonly
                          value={text}
                          onFocus={(e) => e.currentTarget.select()}
                        />
                      </label>
                    )}
                  </For>
                </div>
              )}
            </Show>
          </Show>
          <Show when={tab() === "awake"}>
            <p>{capabilities()?.awake}</p>
            <p class="utility-awake-state" role="status">
              {awake().active
                ? `Keeping awake · ${Math.max(0, Math.ceil(((awake().endsAt ?? now()) - now()) / 60000))} minutes remaining`
                : "Keep awake is off"}
            </p>
            <label>
              Duration in minutes
              <input
                type="number"
                min="1"
                max="480"
                value={minutes()}
                onInput={(e) => setMinutes(e.currentTarget.valueAsNumber)}
              />
            </label>
            <div class="utility-toolbar">
              <button
                disabled={
                  busy() ||
                  !Number.isInteger(minutes()) ||
                  minutes() < 1 ||
                  minutes() > 480
                }
                onClick={() =>
                  void work(async () => {
                    const status = await utilities.setAwake(minutes());
                    if (alive) {
                      setNow(Date.now());
                      setAwake(status);
                    }
                  })
                }
              >
                {awake().active ? "Restart timer" : "Start keep awake"}
              </button>
              <button
                disabled={busy() || !awake().active}
                onClick={() =>
                  void work(async () => {
                    const status = await utilities.setAwake(0);
                    if (alive) setAwake(status);
                  })
                }
              >
                Stop keep awake
              </button>
            </div>
            <p>
              Continues when this panel closes. Stops at expiry or when TinyDash
              quits. Maximum 8 hours.
            </p>
          </Show>
          <Show when={tab() === "media"}>
            <p>{capabilities()?.media}</p>
            <div class="utility-actions">
              <For each={media}>
                {(item) => (
                  <button
                    disabled={busy()}
                    onClick={() =>
                      void work(async () => {
                        await utilities.media(item.action);
                        if (alive)
                          setMessage(
                            "Media request sent; the player decides whether to handle it.",
                          );
                      })
                    }
                  >
                    {item.label}
                  </button>
                )}
              </For>
            </div>
          </Show>
          <Show when={tab() === "windows"}>
            <p>{capabilities()?.windows}</p>
            <p>Target: {target()}</p>
            <button
              disabled={busy()}
              onClick={() =>
                void work(async () => {
                  setMessage(
                    "Switch to the target window now. Capturing in 3 seconds…",
                  );
                  const name = await utilities.captureWindow(true);
                  if (alive) {
                    setTarget(name);
                    setMessage(
                      "Window captured. Return to TinyDash to arrange it.",
                    );
                  }
                })
              }
            >
              Capture window in 3 seconds
            </button>
            <div class="utility-actions">
              <For each={windows}>
                {(item) => (
                  <button
                    disabled={busy()}
                    onClick={() =>
                      void work(async () => {
                        await utilities.window(item.action);
                        if (alive) setMessage("Window placement requested.");
                      })
                    }
                  >
                    {item.label}
                  </button>
                )}
              </For>
            </div>
            <p>
              Restore returns the captured rectangle, not the original
              maximized/minimized state.
            </p>
          </Show>
        </div>
      </div>
      <Show when={message()}>
        <p role="status">{message()}</p>
      </Show>
      <Show when={error()}>
        <p class="utility-error" role="alert">
          {error()}
        </p>
      </Show>
      <Show when={confirmation()}>
        {(request) => (
          <div
            class="utility-confirm"
            role="alertdialog"
            aria-modal="true"
            aria-labelledby="utility-confirm-title"
            aria-describedby="utility-confirm-description"
          >
            <h3 id="utility-confirm-title">
              {request().force ? "Force kill" : "Quit"} {request().process.name}
              ?
            </h3>
            <p id="utility-confirm-description">
              PID {request().process.pid}.{" "}
              {request().force
                ? "Unsaved work will be lost."
                : "The process may exit without saving work."}{" "}
              Identity is checked again before execution. Confirmation expires
              after 30 seconds.
            </p>
            <div class="utility-toolbar">
              <button ref={cancel} disabled={busy()} onClick={dismiss}>
                Cancel
              </button>
              <button
                class="utility-danger"
                disabled={busy()}
                onClick={() =>
                  void work(async () => {
                    const token = request().token;
                    try {
                      await utilities.confirmProcess(token);
                      if (alive) {
                        setConfirmation(undefined);
                        setMessage(
                          "Termination requested. Refresh to check whether the process exited.",
                        );
                        await refresh();
                      }
                    } catch (e) {
                      if (alive) setConfirmation(undefined);
                      throw e;
                    }
                  })
                }
              >
                Confirm {request().force ? "force kill" : "quit"}
              </button>
            </div>
          </div>
        )}
      </Show>
    </section>
  );
}
