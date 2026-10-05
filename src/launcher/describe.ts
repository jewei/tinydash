// Labels and shortcut hints derived from results. Kept apart from the
// components so both the list and the preview agree.
import type { Action } from "../generated/Action";
import type { ResultKind } from "../generated/ResultKind";
import type { SearchResult } from "../generated/SearchResult";
import { modKey } from "../lib/keys";
import type { GlyphName } from "../ui/Icon";

export const KIND_LABELS: Record<ResultKind, string> = {
  app: "Application",
  file: "File",
  folder: "Folder",
  clipboard: "Clipboard",
  snippet: "Snippet",
  quicklink: "Quicklink",
  emoji: "Emoji",
  calculation: "Calculation",
  dateTime: "Date & Time",
  password: "Password",
  url: "Clean URL",
  webSearch: "Web Search",
  system: "Command",
};

export const FALLBACK_GLYPHS: Record<ResultKind, GlyphName> = {
  app: "app",
  file: "file",
  folder: "folder",
  clipboard: "text",
  snippet: "snippet",
  quicklink: "link",
  emoji: "text",
  calculation: "calculator",
  dateTime: "clock",
  password: "key",
  url: "link",
  webSearch: "globe",
  system: "settings",
};

/** Keys that run an action of the selected result directly. */
export function shortcutFor(result: SearchResult, action: Action): string[] | undefined {
  const index = result.actions.findIndex((entry) => entry.action === action);
  if (index === 0) return ["↩"];
  if (index === 1) return [modKey(), "↩"];
  if (action.type === "deleteClip") return [modKey(), "⌫"];
  return undefined;
}

/** Answers the user reads and copies, shown large and selectable. */
const ANSWERS: readonly ResultKind[] = ["calculation", "dateTime", "password", "url"];
export const isAnswer = (kind: ResultKind) => ANSWERS.includes(kind);
