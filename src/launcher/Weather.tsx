import { type JSX, Match, Show, Switch } from "solid-js";

import type { ResultAction } from "../generated/ResultAction";
import type { WeatherIcon } from "../generated/WeatherIcon";
import type { WeatherView } from "../generated/WeatherView";

/** A cloud on the 24-unit grid, filled so a sun behind it stays hidden. */
const cloud = (d: string) => <path class="cloud" d={d} />;
const RAIN_CLOUD = "M7 14h10.5a4 4 0 0 0 .4-7.98A6 6 0 0 0 6.3 7.2 3.5 3.5 0 0 0 7 14z";
const SMALL_CLOUD = "M9 20.5h8.5a3.6 3.6 0 0 0 .3-7.18A5.2 5.2 0 0 0 8 14.3 3.1 3.1 0 0 0 9 20.5z";

const ICONS: Record<WeatherIcon, () => JSX.Element> = {
  clearDay: () => (
    <>
      <circle class="sun" cx="12" cy="12" r="4" />
      <path
        class="sun"
        d="M12 2.5v2M12 19.5v2M2.5 12h2M19.5 12h2M5.3 5.3l1.4 1.4M17.3 17.3l1.4 1.4M5.3 18.7l1.4-1.4M17.3 6.7l1.4-1.4"
      />
    </>
  ),
  clearNight: () => (
    <>
      <path class="sun" d="M19.5 14.6A8 8 0 1 1 9.4 4.5a6.3 6.3 0 0 0 10.1 10.1z" />
      <path class="sun thin" d="M17 4v2M16 5h2" />
    </>
  ),
  partlyDay: () => (
    <>
      <circle class="sun" cx="9" cy="8.5" r="3.4" />
      <path class="sun" d="M9 2.5v1.3M3 8.5h1.3M4.8 4.3l.9.9M13.2 4.3l-.9.9" />
      {cloud(SMALL_CLOUD)}
    </>
  ),
  partlyNight: () => (
    <>
      <path class="sun" d="M12.6 8.6A4.4 4.4 0 1 1 7.2 3.2a3.5 3.5 0 0 0 5.4 5.4z" />
      {cloud(SMALL_CLOUD)}
    </>
  ),
  cloudy: () => (
    <>
      <path class="cloud faint" d="M4.6 12.6A3.2 3.2 0 0 1 6 6.6a4.8 4.8 0 0 1 8.9-1" />
      {cloud("M7 19.5h10.5a4 4 0 0 0 .4-7.98A6 6 0 0 0 6.3 12.7 3.5 3.5 0 0 0 7 19.5z")}
    </>
  ),
  fog: () => (
    <>
      {cloud("M7 13.5h10.5a4 4 0 0 0 .4-7.98A6 6 0 0 0 6.3 6.7 3.5 3.5 0 0 0 7 13.5z")}
      <path class="cloud" d="M4 17.5h16M6.5 21h11" />
    </>
  ),
  rain: () => (
    <>
      {cloud(RAIN_CLOUD)}
      <path class="drop" d="M9 17.2l-1 3M13 17.2l-1 3M17 17.2l-1 3" />
    </>
  ),
  thunder: () => (
    <>
      {cloud(RAIN_CLOUD)}
      <path class="sun" d="M13 15.2l-2.6 4h3.4l-2 3.6" />
    </>
  ),
  snow: () => (
    <>
      {cloud(RAIN_CLOUD)}
      <path
        class="drop flake"
        d="M8.5 17.6v.01M12 18.6v.01M15.5 17.6v.01M10.2 21.2v.01M13.8 21.2v.01"
      />
    </>
  ),
};

function Icon(props: { icon: WeatherIcon; size: number }) {
  return (
    <svg
      class="weather-icon"
      width={props.size}
      height={props.size}
      viewBox="0 0 24 24"
      aria-hidden="true"
    >
      {ICONS[props.icon]()}
    </svg>
  );
}

const hourMinute = new Intl.DateTimeFormat(undefined, {
  hour: "2-digit",
  minute: "2-digit",
  hourCycle: "h23",
});

/** How long ago a Unix time was, in words: "5 min ago", "3 h ago". */
function ago(seconds: number, now: number) {
  const minutes = Math.max(0, Math.floor((now / 1000 - seconds) / 60));
  if (minutes < 1) return "just now";
  if (minutes < 60) return `${minutes} min ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours} h ago`;
  const days = Math.floor(hours / 24);
  return days === 1 ? "1 day ago" : `${days} days ago`;
}

const OPEN_SETTINGS: ResultAction = {
  label: "Open Settings",
  action: { type: "openSettings" },
  confirm: null,
};

/** The weather of the city in Settings, or what keeps it from showing. */
export function Weather(props: {
  view: WeatherView;
  now: number;
  onRun: (action: ResultAction) => void;
}) {
  const settingsButton = () => (
    <button
      type="button"
      class="weather-settings"
      onMouseDown={(event) => event.preventDefault()}
      onClick={() => props.onRun(OPEN_SETTINGS)}
    >
      Open Settings
    </button>
  );
  return (
    <section class="widget weather" aria-label="Weather">
      <Switch>
        <Match when={props.view.type === "ready" && props.view}>
          {(weather) => (
            <>
              <span class="widget-note widget-title">{weather().place}</span>
              <div class="weather-now">
                <span class="weather-temperature" classList={{ offline: weather().offline }}>
                  {weather().temperature}°
                </span>
                <Icon icon={weather().icon} size={32} />
              </div>
              <span>{weather().condition}</span>
              <span class="widget-note">
                H {weather().high}° · L {weather().low}°
                <Show when={weather().rainChance !== null}> · Rain {weather().rainChance}%</Show>
              </span>
              <span class="widget-note weather-stamp">
                {weather().offline
                  ? `Offline · updated ${ago(weather().updatedAt, props.now)}`
                  : `Updated ${hourMinute.format(weather().updatedAt * 1000)}`}
              </span>
            </>
          )}
        </Match>
        <Match when={props.view.type === "loading" && props.view}>
          {(loading) => (
            <>
              <span class="widget-note widget-title">{loading().city}</span>
              <span class="widget-note weather-stamp" aria-busy="true">
                Getting the weather…
              </span>
            </>
          )}
        </Match>
        <Match when={props.view.type === "noCity"}>
          <Icon icon="partlyDay" size={28} />
          <span>Set a city in Settings.</span>
          {settingsButton()}
        </Match>
        <Match when={props.view.type === "notFound" && props.view}>
          {(missing) => (
            <>
              <span>No place is named “{missing().city}”.</span>
              {settingsButton()}
            </>
          )}
        </Match>
        <Match when={props.view.type === "failed" && props.view}>
          {(failed) => (
            <>
              <span class="widget-note widget-title">{failed().city}</span>
              <span class="widget-note">Could not get the weather. {failed().message}</span>
            </>
          )}
        </Match>
      </Switch>
    </section>
  );
}
