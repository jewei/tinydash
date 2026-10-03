import { invoke } from "@tauri-apps/api/core";

export type UtilityTab = "processes" | "colors" | "awake" | "media" | "windows";
export interface UtilityProcess {
  pid: number;
  identity: string;
  name: string;
}
export interface ProcessConfirmation {
  token: string;
  process: UtilityProcess;
  force: boolean;
}
export interface UtilityColor {
  hex: string;
  rgb: string;
  hsl: string;
  alpha: number;
}
export interface AwakeStatus {
  active: boolean;
  endsAt: number | null;
  remainingSeconds: number;
}
export interface UtilityCapabilities {
  processes: string;
  eyedropper: string;
  nativeEyedropper: boolean;
  awake: string;
  media: string;
  windows: string;
}
export type MediaAction =
  | "playPause"
  | "next"
  | "previous"
  | "volumeUp"
  | "volumeDown"
  | "mute";
export type WindowAction = "left" | "right" | "maximize" | "center" | "restore";

type Dropper = {
  open(options?: { signal?: AbortSignal }): Promise<{ sRGBHex: string }>;
};
function eyedropperConstructor() {
  return (window as unknown as { EyeDropper?: new () => Dropper }).EyeDropper;
}
export const awakeChangedEvent = "tinydash:utility-awake-changed";
export const utilities = {
  capabilities: () => invoke<UtilityCapabilities>("utility_capabilities"),
  processes: () => invoke<UtilityProcess[]>("utility_processes"),
  prepareProcess: (process: UtilityProcess, force: boolean) =>
    invoke<ProcessConfirmation>("utility_prepare_process", { process, force }),
  prepareApp: (id: string, force: boolean) =>
    invoke<ProcessConfirmation>("utility_prepare_app", { id, force }),
  confirmProcess: (token: string) =>
    invoke<void>("utility_confirm_process", { token, confirmed: true }),
  cancelProcess: () => invoke<void>("utility_cancel_process"),
  color: (input: string) => invoke<UtilityColor>("utility_color", { input }),
  copyColor: (input: string, format: "hex" | "rgb" | "hsl") =>
    invoke<void>("utility_copy_color", { input, format }),
  canSampleScreen: () => !!eyedropperConstructor(),
  sampleScreen: async (signal: AbortSignal, native = false) => {
    if (native) {
      const result = await invoke<UtilityColor | null>("utility_eyedropper");
      if (!result || signal.aborted)
        throw new DOMException("Color sampling canceled", "AbortError");
      return result;
    }
    const EyeDropper = eyedropperConstructor();
    if (!EyeDropper)
      throw new Error("Screen sampling is unavailable in this WebView");
    // Must be called directly in a user gesture. Conversion still belongs to Rust.
    const result = await new EyeDropper().open({ signal });
    return invoke<UtilityColor>("utility_color", { input: result.sRGBHex });
  },
  awakeStatus: () => invoke<AwakeStatus>("utility_awake_status"),
  setAwake: async (minutes: number) => {
    const status = await invoke<AwakeStatus>("utility_set_awake", { minutes });
    window.dispatchEvent(
      new CustomEvent(awakeChangedEvent, { detail: status }),
    );
    return status;
  },
  media: (action: MediaAction) => invoke<void>("utility_media", { action }),
  captureWindow: (delayed = true) =>
    invoke<string>("utility_capture_window", { delayed }),
  window: (action: WindowAction) => invoke<void>("utility_window", { action }),
};
