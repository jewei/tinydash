import { For, Show } from "solid-js";
import type { Action, SearchMode, SearchResult } from "../bridge";
import type { PinOption } from "../categories";
import ClipboardPreview from "./ClipboardPreview";
import Icon from "./Icon";
import ResultIcon from "./ResultIcon";
import ToolDetails from "./ToolDetails";
import FilePreview from "./FilePreview";

export default function ResultPreview(props: {
  result?: SearchResult;
  welcome: boolean;
  hasQuery?: boolean;
  searchFailed?: boolean;
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
      case "folder":
        return "Folder";
      case "clipboard":
        return "Clipboard history";
      case "calculation":
        return "Calculation";
      case "systemCommand":
        return "System command";
      case "password":
        return "Password generator";
      case "timezone":
        return "Datetime";
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
        return "Open application";
      case "file":
        return "Open file";
      case "folder":
        return "Open folder";
      case "clipboard":
        return "Copy saved text";
      case "calculation":
        return "Copy result";
      case "systemCommand":
        return "Run selected command";
      case "password":
        return "Copy generated password";
      case "timezone":
        return props.result?.detail?.type === "dateCalculation"
          ? "Copy this date"
          : "Copy this time";
      case "cleanedUrl":
        return "Copy cleaned URL";
      case "webSearch":
        return "Open search in browser";
      default:
        return "Copy selected emoji";
    }
  };
  const pathParts = () => {
    const path = props.result?.path ?? props.result?.subtitle ?? "";
    const separator = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
    return {
      directory: separator >= 0 ? path.slice(0, separator) || "/" : "—",
      file: path.slice(separator + 1),
    };
  };

  return (
    <aside
      class={{ "result-preview": true, "tool-preview": !!props.result?.detail }}
      aria-label="Selected item details"
    >
      <Show
        when={props.result && !props.welcome}
        fallback={
          <div class="welcome-panel">
            <span class="welcome-mark">
              <Icon name="search" size={19} />
            </span>
            <h1>
              {props.searchFailed
                ? "Search unavailable."
                : props.hasQuery
                  ? "Try another search."
                  : "Start typing."}
            </h1>
            <p>
              {props.searchFailed
                ? "Edit your search to try again. You can also change the category."
                : props.hasQuery
                  ? "Try a shorter name or choose a different category."
                  : "Find an app, a file, or the answer to a quick calculation."}
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
          </div>
        }
      >
        <div class="preview-content">
          <div
            class={{
              "preview-summary": true,
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
                      aria-pressed={option.pinned ? "true" : "false"}
                      aria-busy={props.pinBusy ? "true" : "false"}
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
                props.result!.kind !== "file" &&
                props.result!.kind !== "folder"
              }
            >
              <p class="preview-subtitle">{props.result!.subtitle}</p>
            </Show>
            <Show
              when={
                props.result!.kind === "app" ||
                props.result!.kind === "file" ||
                props.result!.kind === "folder"
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
                  <dt>{props.result!.kind === "folder" ? "Folder" : "File"}</dt>
                  <dd title={pathParts().file}>{pathParts().file}</dd>
                </div>
              </dl>
            </Show>
            <Show when={props.result!.confirmation}>
              <p class="preview-notice">
                <Icon name="lock" size={14} />
                You will be asked to confirm this command.
              </p>
            </Show>
          </div>
          <Show
            when={
              props.previewReady &&
              (props.result?.kind === "file" || props.result?.kind === "folder")
            }
          >
            <FilePreview result={props.result!} enabled={props.enabled} />
          </Show>
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
          <Show
            when={
              props.result?.primaryAction === "copy" ||
              props.result?.secondaryActions.includes("copy")
            }
          >
            <button
              class="preview-action"
              disabled={!props.enabled}
              onClick={() => props.onAction("paste")}
            >
              <span class="action-label">
                <Icon name="clipboard" size={15} />
                Paste to previous app
              </span>
              <kbd>{props.modifier} ⇧ ↵</kbd>
            </button>
          </Show>
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
              <span class="action-label">
                <Icon name="delete" size={15} />
                Delete saved text
              </span>
              <kbd>{props.modifier} ⌫</kbd>
            </button>
          </Show>
          <Show when={props.result?.secondaryActions.includes("copy")}>
            <button
              class="preview-action"
              disabled={!props.enabled}
              onClick={() => props.onAction("copy")}
            >
              <span class="action-label">
                <Icon name="copy" size={15} />
                Copy search URL
              </span>
            </button>
          </Show>
          <Show when={props.result?.secondaryActions.includes("open")}>
            <button
              class="preview-action"
              disabled={!props.enabled}
              onClick={() => props.onAction("open")}
            >
              <span class="action-label">
                <Icon name="link" size={15} />
                Open cleaned URL
              </span>
            </button>
          </Show>
          <Show when={props.result?.secondaryActions.includes("regenerate")}>
            <button
              class="preview-action"
              disabled={!props.enabled}
              onClick={() => props.onAction("regenerate")}
            >
              <span class="action-label">
                <Icon name="refresh" size={15} />
                Generate another
              </span>
            </button>
          </Show>
        </div>
      </Show>
    </aside>
  );
}
