import { Show } from "solid-js";
import type { SearchResult } from "../bridge";

export default function ToolDetails(props: {
  detail?: SearchResult["detail"];
}) {
  return (
    <div class="tool-details">
      <Show when={props.detail?.type === "password" ? props.detail : undefined}>
        {(detail) => (
          <div class="password-strength">
            <div>
              <span>Strength estimate</span>
              <strong>{detail().strength}</strong>
            </div>
            <meter
              min="0"
              max="128"
              low="40"
              high="60"
              optimum="128"
              value={Math.min(128, detail().entropyBits)}
              aria-label="Password entropy"
            />
            <p>
              {detail().entropyBits} bits of randomness. Generated on this
              device.
            </p>
            <p>Copying here skips TinyDash's clipboard history.</p>
          </div>
        )}
      </Show>
      <Show when={props.detail?.type === "timezone" ? props.detail : undefined}>
        {(detail) => (
          <>
            <dl class="time-details">
              <div>
                <dt>{detail().sourceZone.replaceAll("_", " ")}</dt>
                <dd>{detail().source}</dd>
              </div>
              <div>
                <dt>Your local time</dt>
                <dd>{detail().local}</dd>
              </div>
            </dl>
            <Show when={detail().ambiguous}>
              <p class="preview-notice">
                This time occurs twice when the clocks move back. Each result
                has a different UTC offset.
              </p>
            </Show>
          </>
        )}
      </Show>
      <Show
        when={props.detail?.type === "cleanedUrl" ? props.detail : undefined}
      >
        {(detail) => (
          <>
            <p class="tool-note">
              {detail().removed === 0
                ? "No known tracking fields found."
                : "Tracking fields removed. Other URL parameters stay in place."}
            </p>
            <details class="original-url">
              <summary>Original URL</summary>
              <code>{detail().original}</code>
            </details>
          </>
        )}
      </Show>
      <Show
        when={props.detail?.type === "webSearch" ? props.detail : undefined}
      >
        {(detail) => (
          <p class="tool-note">
            Open this search in {detail().engine} with your default browser.
          </p>
        )}
      </Show>
    </div>
  );
}
