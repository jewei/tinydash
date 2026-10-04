import type { SearchMode, SearchResult } from "./bridge";

import type { SearchRequest } from "./searchQueue";
export { createSearchQueue } from "./searchQueue";
export type { SearchRequest, SearchQueue } from "./searchQueue";

const resultKey = (result?: SearchResult) => result?.pin?.key ?? result?.id;

/**
 * Chooses the selected row after a reply. A refresh of the displayed query
 * keeps the selected result. A new query selects the first result, or the
 * newest clipboard entry in an empty Clipboard search.
 */
export function chooseSelection(options: {
  request: SearchRequest;
  results: SearchResult[];
  preferredSelectionId?: string | null;
  displayed?: { value: string; mode: SearchMode };
  current?: SearchResult;
  selected: number;
  selectionChangedByUser: boolean;
}): number {
  const { request, results, preferredSelectionId, displayed, current } =
    options;
  // Read selection after the response. The user can navigate while a
  // refresh runs, but a different query must select its first result.
  const followNewestClipboard =
    request.mode === "clipboard" &&
    !request.value.trim() &&
    !options.selectionChangedByUser &&
    preferredSelectionId != null;
  const selectedId =
    request.preserveSelection &&
    !followNewestClipboard &&
    displayed?.value === request.value &&
    displayed.mode === request.mode
      ? resultKey(current)
      : undefined;
  // Tool rows have new IDs on each reply. Keep the same position instead.
  const stableToolIndex =
    selectedId && ["timezone", "webSearch"].includes(current?.kind ?? "")
      ? Math.min(options.selected, results.length - 1)
      : 0;
  const matchedIndex = results.findIndex(
    (result) => resultKey(result) === selectedId,
  );
  const preferredIndex = results.findIndex(
    (result) => result.id === preferredSelectionId,
  );
  return Math.max(
    0,
    matchedIndex >= 0
      ? matchedIndex
      : selectedId
        ? stableToolIndex
        : preferredIndex,
  );
}
