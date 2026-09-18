import { Match, Switch } from "solid-js";
import type { SearchResult } from "../bridge";
import Icon from "./Icon";
import AppAvatar from "./AppAvatar";

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
          <Icon
            name={
              props.result.id === "system:lock"
                ? "lock"
                : props.result.id === "system:sleep"
                  ? "sleep"
                  : props.result.id === "system:restart"
                    ? "refresh"
                    : props.result.id === "system:shutdown"
                      ? "quit"
                      : "system"
            }
            size={24}
          />
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
    </Switch>
  );
}
