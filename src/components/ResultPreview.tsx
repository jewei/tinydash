import { For, Show } from "solid-js";
import type { Action, SearchMode, SearchResult } from "../bridge";
import type { PinOption } from "../categories";
import ClipboardPreview from "./ClipboardPreview";
import Icon from "./Icon";
import ResultIcon from "./ResultIcon";
import ToolDetails from "./ToolDetails";

export default function ResultPreview(props: {
  result?: SearchResult;
  welcome: boolean;
  previewReady: boolean;
  enabled: boolean;
  modifier: string;
  pinOptions: PinOption[];
  pinBusy: boolean;
  onPin: (category: SearchMode) => void;
  onAction: (action: Action) => void;
}) {
  const kindLabel = () => {
    switch (props.result?.kind) {
      case "app":
        return "Application";
      case "file":
        return "File";
      case "clipboard":
        return "Clipboard history";
      case "calculation":
        return "Calculation";
      case "systemCommand":
        return "System command";
      case "password":
        return "Password generator";
      case "timezone":
        return "Time zones";
      case "cleanedUrl":
        return "Cleaned URL";
      case "webSearch":
        return "Web search";
      default:
        return "Emoji";
    }
  };
  const actionLabel = () => {
    switch (props.result?.kind) {
      case "app":
        return "Launch application";
      case "file":
        return "Open file";
      case "clipboard":
        return "Copy saved text";
      case "calculation":
        return "Copy answer";
      case "systemCommand":
        return "Run selected command";
      case "password":
        return "Copy generated password";
      case "timezone":
        return "Copy this time";
      case "cleanedUrl":
        return "Copy cleaned URL";
      case "webSearch":
        return "Open search in browser";
      default:
        return "Copy selected emoji";
    }
  };
  const pathParts = () => {
    const path = props.result?.subtitle ?? "";
    const separator = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
    return {
      directory: separator >= 0 ? path.slice(0, separator) || "/" : "—",
      file: path.slice(separator + 1),
    };
  };

  return (
    <aside
      class="result-preview"
      classList={{ "tool-preview": !!props.result?.detail }}
      aria-label="Selected item details"
    >
      <Show
        when={props.result && !props.welcome}
        fallback={
          <div class="welcome-panel">
            <span class="welcome-mark">
              <Icon name="search" size={30} />
            </span>
            <p class="eyebrow">A LITTLE LESS SEARCHING</p>
            <h1>Start typing.</h1>
            <p>
              Your apps, files, and small everyday tasks. All a few keys away.
            </p>
            <dl class="welcome-shortcuts">
              <div>
                <dt>Move through results</dt>
                <dd>
                  <kbd>↑</kbd>
                  <kbd>↓</kbd>
                </dd>
              </div>
              <div>
                <dt>Open selected item</dt>
                <dd>
                  <kbd>↵</kbd>
                </dd>
              </div>
              <div>
                <dt>Next category</dt>
                <dd>
                  <kbd>Tab</kbd>
                </dd>
              </div>
              <div>
                <dt>See all actions</dt>
                <dd>
                  <kbd>{props.modifier} K</kbd>
                </dd>
              </div>
            </dl>
            <p class="welcome-examples">
              Try an app name, <strong>:coffee</strong>, or{" "}
              <strong>12 * 8</strong>.
            </p>
          </div>
        }
      >
        <div class="preview-content">
          <div
            class="preview-summary"
            classList={{
              "calculation-summary": props.result?.kind === "calculation",
              "password-summary": props.result?.kind === "password",
              "url-summary": props.result?.kind === "cleanedUrl",
            }}
          >
            <div class="preview-topline">
              <div class="preview-icon">
                <ResultIcon result={props.result!} />
              </div>
              <div class="pin-controls" role="group" aria-label="Pin item">
                <For each={props.pinOptions}>
                  {(option) => (
                    <button
                      class="pin-button"
                      aria-label={option.label}
                      aria-pressed={option.pinned}
                      aria-busy={props.pinBusy}
                      disabled={!props.enabled || props.pinBusy}
                      onClick={() => props.onPin(option.category)}
                      title={option.label}
                    >
                      <Icon name="pin" size={14} />
                      {option.label}
                    </button>
                  )}
                </For>
              </div>
            </div>
            <p class="eyebrow">{kindLabel()}</p>
            <h1 class="preview-title">
              {props.result!.kind === "clipboard"
                ? "Saved text"
                : props.result!.title}
            </h1>
            <Show
              when={
                props.result!.kind !== "clipboard" &&
                props.result!.kind !== "timezone" &&
                props.result!.kind !== "app" &&
                props.result!.kind !== "file"
              }
            >
              <p class="preview-subtitle">{props.result!.subtitle}</p>
            </Show>
            <Show
              when={
                props.result!.kind === "app" || props.result!.kind === "file"
              }
            >
              <dl class="preview-metadata">
                <div>
                  <dt>Location</dt>
                  <dd title={pathParts().directory}>
                    <For
                      each={
                        pathParts().directory.match(/[^\\/]*[\\/]|[^\\/]+$/g) ??
                        []
                      }
                    >
                      {(segment) => (
                        <>
                          {segment}
                          <wbr />
                        </>
                      )}
                    </For>
                  </dd>
                </div>
                <div>
                  <dt>File</dt>
                  <dd title={pathParts().file}>{pathParts().file}</dd>
                </div>
              </dl>
            </Show>
            <Show when={props.result!.confirmation}>
              <p class="preview-notice">
                You will be asked to confirm this command.
              </p>
            </Show>
          </div>
          <ToolDetails detail={props.result?.detail} />
          <Show when={props.result?.kind === "clipboard"}>
            <Show when={props.previewReady}>
              <ClipboardPreview id={props.result!.id} />
            </Show>
          </Show>
        </div>
        <div class="preview-actions">
          <button
            class="preview-action primary"
            disabled={!props.enabled}
            onClick={() => props.onAction(props.result!.primaryAction)}
          >
            <span class="action-label">
              <Icon
                name={props.result?.primaryAction === "copy" ? "copy" : "arrow"}
                size={15}
              />
              {actionLabel()}
            </span>
            <kbd>↵</kbd>
          </button>
          <Show when={props.result?.secondaryActions.includes("reveal")}>
            <button
              class="preview-action"
              disabled={!props.enabled}
              onClick={() => props.onAction("reveal")}
            >
              <span class="action-label">
                <Icon name="folder" size={15} />
                Show in enclosing folder
              </span>
              <kbd>{props.modifier} ↵</kbd>
            </button>
          </Show>
          <Show when={props.result?.secondaryActions.includes("delete")}>
            <button
              class="preview-action"
              disabled={!props.enabled}
              onClick={() => props.onAction("delete")}
            >
              Delete saved text<kbd>{props.modifier} ⌫</kbd>
            </button>
          </Show>
          <Show when={props.result?.secondaryActions.includes("copy")}>
            <button
              class="preview-action"
              disabled={!props.enabled}
              onClick={() => props.onAction("copy")}
            >
              Copy search URL
            </button>
          </Show>
          <Show when={props.result?.secondaryActions.includes("open")}>
            <button
              class="preview-action"
              disabled={!props.enabled}
              onClick={() => props.onAction("open")}
            >
              Open cleaned URL
            </button>
          </Show>
          <Show when={props.result?.secondaryActions.includes("regenerate")}>
            <button
              class="preview-action"
              disabled={!props.enabled}
              onClick={() => props.onAction("regenerate")}
            >
              Generate another
            </button>
          </Show>
        </div>
      </Show>
    </aside>
  );
}
