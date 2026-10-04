import { createSignal, For, Show } from "solid-js";
import type { SettingsValues } from "../bridge";

const commands = [
  { id: "command:window-left", label: "Left half", key: "ArrowLeft" },
  { id: "command:window-right", label: "Right half", key: "ArrowRight" },
  { id: "command:window-maximize", label: "Maximize", key: "ArrowUp" },
  { id: "command:window-center", label: "Center", key: "KeyC" },
  { id: "command:window-restore", label: "Restore", key: "ArrowDown" },
] as const;

export default function WindowShortcutPreferences(props: {
  value: SettingsValues["itemPreferences"];
  available: boolean;
  platform: string;
  preparing: boolean;
  recording: string | null;
  shortcutKeys: (id: string) => string[];
  onRecord: (id: string) => void;
  onChange: (value: SettingsValues["itemPreferences"]) => void;
}) {
  const [preset, setPreset] = createSignal("standard");
  const locked = () => props.preparing || props.recording !== null;
  const modifier = () =>
    props.platform === "macos" ? "Control + Option" : "Control + Alt";

  function applyPreset(clear = false) {
    const next = { ...props.value };
    for (const command of commands) {
      const previous = next[command.id];
      if (clear && !previous) continue;
      next[command.id] = {
        aliases: previous?.aliases ?? [],
        hidden: previous?.hidden ?? false,
        disabled: clear ? (previous?.disabled ?? false) : false,
        shortcut: clear
          ? ""
          : `Control+Alt+${preset() === "shift" ? "Shift+" : ""}${command.key}`,
      };
    }
    props.onChange(next);
  }

  return (
    <div class="settings-group">
      <h2>Window placement shortcuts</h2>
      <p>
        Move the active app window without opening TinyDash. Shortcuts are off
        by default. Apply a preset or record each shortcut, then save changes.
        Saving checks for conflicts with launch, category, and item shortcuts.
      </p>
      <p class="settings-hint">
        {props.platform === "macos"
          ? "Requires Accessibility access in macOS Privacy & Security."
          : props.platform === "windows"
            ? "Some elevated windows can refuse changes."
            : "Requires X11, xdotool, xprop, and wmctrl. Wayland is not supported."}{" "}
        Restore keeps the original size and position for up to 16 windows used
        in the last 30 minutes. It does not survive a restart.
      </p>
      <label class="settings-field">
        Window shortcut preset
        <select
          value={preset()}
          disabled={locked() || !props.available}
          onChange={(event) => setPreset(event.currentTarget.value)}
        >
          <option value="standard">{modifier()} + arrows / C</option>
          <option value="shift">{modifier()} + Shift + arrows / C</option>
        </select>
      </label>
      <div class="utility-toolbar">
        <button
          type="button"
          class="settings-button"
          disabled={locked() || !props.available}
          onClick={() => applyPreset()}
        >
          Apply window preset
        </button>
        <button
          type="button"
          class="settings-button"
          disabled={
            locked() || !commands.some(({ id }) => props.value[id]?.shortcut)
          }
          onClick={() => applyPreset(true)}
        >
          Clear window shortcuts
        </button>
      </div>
      <p class="settings-hint">
        Left/Right: halves. Up: maximize. C: center. Down: restore. Applying a
        preset replaces these five bindings and enables their commands.
      </p>
      <For each={commands}>
        {(command) => {
          const binding = () => props.value[command.id];
          return (
            <div class="shortcut-category-row">
              <span>
                <strong>{command.label}</strong>
                <span class="settings-hint">
                  {binding()?.shortcut
                    ? props.shortcutKeys(command.id).join(" + ")
                    : "No shortcut"}
                  {binding()?.disabled ? " (command disabled in Search)" : ""}
                </span>
              </span>
              <button
                type="button"
                class="settings-button"
                aria-label={`Record ${command.label} window shortcut`}
                disabled={props.preparing || !props.available}
                onClick={() => props.onRecord(command.id)}
              >
                {props.recording === `item:${command.id}`
                  ? "Cancel"
                  : binding()?.shortcut
                    ? "Change"
                    : "Record"}
              </button>
              <Show when={binding()?.shortcut}>
                <button
                  type="button"
                  class="settings-button"
                  aria-label={`Remove ${command.label} window shortcut`}
                  disabled={locked()}
                  onClick={() =>
                    props.onChange({
                      ...props.value,
                      [command.id]: { ...binding()!, shortcut: "" },
                    })
                  }
                >
                  Remove
                </button>
              </Show>
            </div>
          );
        }}
      </For>
    </div>
  );
}
