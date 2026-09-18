import { For } from "solid-js";
import Icon from "./Icon";

export default function WelcomeSuggestions(props: {
  appsAvailable: boolean;
  onBrowseApps: () => void;
  onQuery: (query: string) => void;
}) {
  const suggestions = () => [
    {
      label: props.appsAvailable ? "Find an app" : "Search the web",
      icon: props.appsAvailable ? ("apps" as const) : ("globe" as const),
      run: () =>
        props.appsAvailable
          ? props.onBrowseApps()
          : props.onQuery("web weather"),
    },
    {
      label: "128 × 1.08",
      icon: "calculator" as const,
      run: () => props.onQuery("128 * 1.08"),
    },
    {
      label: "Find an emoji",
      icon: "emoji" as const,
      run: () => props.onQuery(":smile"),
    },
  ];

  return (
    <section class="welcome-suggestions" aria-labelledby="welcome-heading">
      <h2 id="welcome-heading">What will you do next?</h2>
      <p>Type to search, or try one of these.</p>
      <div class="suggestion-list">
        <For each={suggestions()}>
          {(suggestion) => (
            <button class="suggestion-button" onClick={suggestion.run}>
              <Icon name={suggestion.icon} size={18} />
              <span>{suggestion.label}</span>
              <kbd aria-hidden="true">↵</kbd>
            </button>
          )}
        </For>
      </div>
    </section>
  );
}
