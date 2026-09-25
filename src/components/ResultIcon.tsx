import { Match, Switch } from "solid-js";
import type { SearchResult } from "../bridge";
import Icon, { type IconName } from "./Icon";
import AppAvatar from "./AppAvatar";

const systemIcons: Readonly<Record<string, IconName>> = {
  "system:lock": "lock",
  "system:sleep": "sleep",
  "system:restart": "refresh",
  "system:shutdown": "quit",
  "system:settings": "system",
  "system:appearance": "appearance",
  "system:empty-trash": "delete",
  "system:logout": "logout",
  "system:desktop": "desktop",
  "system:mute": "volume",
};

export default function ResultIcon(props: { result: SearchResult }) {
  return (
    <Switch>
      <Match
        when={["password", "timezone", "cleanedUrl", "webSearch"].includes(
          props.result.kind,
        )}
      >
        <span class="clipboard-icon" aria-hidden="true">
          <Icon
            name={
              props.result.kind === "password"
                ? "lock"
                : props.result.kind === "timezone"
                  ? "clock"
                  : props.result.kind === "cleanedUrl"
                    ? "link"
                    : "globe"
            }
            size={24}
          />
        </span>
      </Match>
      <Match when={props.result.kind === "systemCommand"}>
        <span class="clipboard-icon" aria-hidden="true">
          <Icon name={systemIcons[props.result.id] ?? "system"} size={24} />
        </span>
      </Match>
      <Match when={props.result.kind === "app"}>
        <AppAvatar icon={props.result.icon} />
      </Match>
      <Match when={props.result.kind === "emoji"}>
        <span class="emoji-icon" aria-hidden="true">
          {props.result.icon}
        </span>
      </Match>
      <Match when={props.result.kind === "calculation"}>
        <span class="calculation-icon" aria-hidden="true">
          <Icon name="calculator" size={26} />
        </span>
      </Match>
      <Match when={props.result.kind === "clipboard"}>
        <span class="clipboard-icon" aria-hidden="true">
          <Icon name="clipboard" size={24} />
        </span>
      </Match>
      <Match when={props.result.kind === "file"}>
        <span class="file-icon" aria-hidden="true">
          <Icon name="file" size={24} />
        </span>
      </Match>
      <Match when={props.result.kind === "folder"}>
        <span class="file-icon" aria-hidden="true">
          <Icon name="folder" size={24} />
        </span>
      </Match>
    </Switch>
  );
}
