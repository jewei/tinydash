import { createSignal, onCleanup, onSettled, Show } from "solid-js";
import { listen } from "@tauri-apps/api/event";
import {
  backend,
  type PasteQueueAction,
  type PasteQueueStatus,
} from "../bridge";
import { createNativeSubscriptions } from "../nativeSubscriptions";

export default function PasteQueueIndicator(props: {
  desktop: boolean;
  onDismiss: () => void;
}) {
  const [status, setStatus] = createSignal<PasteQueueStatus>();
  const [busy, setBusy] = createSignal(false);
  const [error, setError] = createSignal<string>();
  const subscriptions = createNativeSubscriptions(listen);
  let disposed = false;
  let revision = 0;

  async function refresh() {
    const current = ++revision;
    try {
      const value = await backend.pasteQueue("status", []);
      if (!disposed && current === revision) setStatus(value);
    } catch {
      // A paste owns the queue until dispatch settles. Its event updates us.
    }
  }

  async function act(action: PasteQueueAction) {
    if (busy()) return;
    setBusy(true);
    setError(undefined);
    ++revision;
    try {
      const value = await backend.pasteQueue(action, []);
      if (!disposed) {
        setStatus(value);
        if (action === "cancel") props.onDismiss();
      }
    } catch (reason) {
      if (!disposed) setError(String(reason));
      await refresh();
    } finally {
      if (!disposed) setBusy(false);
    }
  }

  onSettled(() => {
    if (!props.desktop) return;
    void (async () => {
      try {
        await Promise.all([
          subscriptions.register<PasteQueueStatus>(
            "paste-queue-changed",
            (value) => {
              ++revision;
              setStatus(value);
              setError(undefined);
            },
          ),
          subscriptions.register("clipboard-changed", () => void refresh()),
          subscriptions.register("launcher-opened", () => void refresh()),
        ]);
        if (!disposed) await refresh();
      } catch (reason) {
        if (!disposed) setError(String(reason));
      }
    })();
  });
  onCleanup(() => {
    disposed = true;
    subscriptions.dispose();
  });

  return (
    <Show when={status()?.total}>
      <section class="paste-queue" aria-label="Paste queue">
        <div class="paste-queue-summary">
          <strong>
            {status()?.next
              ? `Next: ${status()!.position + 1} of ${status()!.total}`
              : "Paste queue complete"}
          </strong>
          <div class="paste-queue-actions">
            <button
              disabled={busy() || !status()?.next}
              onClick={() => void act("next")}
            >
              Paste next
            </button>
            <button
              disabled={busy() || !status()?.next}
              onClick={() => void act("skip")}
            >
              Skip entry
            </button>
            <button disabled={busy()} onClick={() => void act("cancel")}>
              {status()?.next ? "Cancel queue" : "Dismiss queue"}
            </button>
          </div>
        </div>
        <Show when={status()?.next} keyed>
          {(entry) => (
            <details>
              <summary>Preview next entry</summary>
              <pre>{entry.content}</pre>
            </details>
          )}
        </Show>
        <Show when={error()}>
          <p role="alert">{error()}</p>
        </Show>
      </section>
    </Show>
  );
}
