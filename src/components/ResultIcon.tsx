import { Match, Switch } from "solid-js";
import type { SearchResult } from "../bridge";
import AppAvatar from "./AppAvatar";
import Icon from "./Icon";

export default function ResultIcon(props: { result: SearchResult }) {
  return (
    <Switch>
      <Match when={props.result.kind === "app"}>
        <AppAvatar name={props.result.title} />
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
    </Switch>
  );
}
