import { createSignal, onCleanup, onSettled } from "solid-js";
import { utilities, type ProcessConfirmation } from "../utilities-bridge";
import ConfirmDialog from "./ConfirmDialog";

export default function AppQuitDialog(props: {
  id: string;
  title: string;
  force: boolean;
  platform: string;
  onClose: () => void;
  onRequested: () => void;
}) {
  const [request, setRequest] = createSignal<ProcessConfirmation>();
  const [busy, setBusy] = createSignal(true);
  const [error, setError] = createSignal<string>();
  let alive = true;

  async function prepare() {
    setBusy(true);
    setError(undefined);
    try {
      const confirmation = await utilities.prepareApp(props.id, props.force);
      if (alive) setRequest(confirmation);
    } catch (reason) {
      if (alive) setError(String(reason));
    } finally {
      if (alive) setBusy(false);
    }
  }

  async function confirm() {
    if (busy()) return;
    const confirmation = request();
    if (!confirmation) {
      await prepare();
      return;
    }
    setBusy(true);
    setError(undefined);
    // A failed or expired request consumes the token. Check again before retrying.
    setRequest(undefined);
    try {
      await utilities.confirmProcess(confirmation.token);
      if (alive) props.onRequested();
    } catch (reason) {
      if (alive) setError(String(reason));
    } finally {
      if (alive) setBusy(false);
    }
  }

  onSettled(() => void prepare());
  onCleanup(() => {
    alive = false;
    if (request()) void utilities.cancelProcess().catch(() => {});
  });

  return (
    <ConfirmDialog
      title={`${props.force ? "Force Quit" : "Quit"} ${props.title}?`}
      description={
        props.force
          ? "Unsaved work will be lost. This stops the selected app."
          : props.platform === "linux"
            ? "The app will receive a termination request. It may exit without saving work."
            : "Ask this app to quit. It can show a save dialog or refuse to close."
      }
      confirmLabel={
        request() ? (props.force ? "Force Quit" : "Quit") : "Check app again"
      }
      busyLabel={request() ? "Checking..." : "Please wait..."}
      busy={busy()}
      error={error()}
      onClose={props.onClose}
      onConfirm={() => void confirm()}
    />
  );
}
