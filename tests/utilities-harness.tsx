import { mockIPC } from "@tauri-apps/api/mocks";
import { render } from "solid-js/web";
import { createSignal, Show } from "solid-js";
import UtilitiesPanel, {
  UtilitiesAwakeIndicator,
} from "../src/components/UtilitiesPanel";
import type { AwakeStatus } from "../src/utilities-bridge";

const calls: { command: string; args: Record<string, unknown> }[] = [];
Object.assign(window, { utilityCalls: calls });
let awake: AwakeStatus = { active: false, endsAt: null, remainingSeconds: 0 };
mockIPC((command, args = {}) => {
  const values = args as Record<string, unknown>;
  calls.push({ command, args: values });
  switch (command) {
    case "utility_capabilities":
      return {
        processes: "Your processes only",
        eyedropper: "System sampler: Escape cancels",
        nativeEyedropper: true,
        awake: "Idle sleep only",
        media: "OS-selected player",
        windows: "Capture a window first",
      };
    case "utility_processes":
      return [
        { pid: 400, identity: "start-one", name: "Fixture editor" },
        { pid: 401, identity: "start-two", name: "Fixture player" },
      ];
    case "utility_prepare_process":
      return {
        token: "one-use-ticket",
        process: values.process,
        force: values.force,
      };
    case "utility_confirm_process":
      if (values.token !== "one-use-ticket" || !values.confirmed)
        throw new Error("Confirmation rejected");
      return null;
    case "utility_cancel_process":
      return null;
    case "utility_eyedropper":
      if (calls.filter((call) => call.command === command).length === 1)
        return null;
      return {
        hex: "#00FF00",
        rgb: "rgba(0, 255, 0, 1.000)",
        hsl: "hsla(120.0, 100.0%, 50.0%, 1.000)",
        alpha: 1,
      };
    case "utility_color":
      if (values.input === "bad") throw new Error("Invalid color");
      return {
        hex: "#FF0000",
        rgb: "rgba(255, 0, 0, 1.000)",
        hsl: "hsla(0.0, 100.0%, 50.0%, 1.000)",
        alpha: 1,
      };
    case "utility_copy_color":
      return null;
    case "utility_awake_status":
      return awake;
    case "utility_set_awake":
      awake = {
        active: !!values.minutes,
        endsAt: values.minutes
          ? Date.now() + Number(values.minutes) * 60000
          : null,
        remainingSeconds: Number(values.minutes) * 60,
      };
      return awake;
    case "utility_media":
      return null;
    case "utility_capture_window":
      return "Fixture editor (PID 400)";
    case "utility_window":
      if (values.action === "maximize")
        throw new Error("Accessibility permission denied");
      return null;
    default:
      throw new Error(`Unexpected utility command ${command}`);
  }
});
function Harness() {
  const [open, setOpen] = createSignal(true);
  return (
    <main>
      <UtilitiesAwakeIndicator />
      <Show
        when={open()}
        fallback={<button onClick={() => setOpen(true)}>Open utilities</button>}
      >
        <UtilitiesPanel onClose={() => setOpen(false)} />
      </Show>
    </main>
  );
}
render(() => <Harness />, document.getElementById("root")!);
