import type { SearchMode, SearchResult } from "./bridge";

export interface SearchRequest {
  value: string;
  mode: SearchMode;
  preserveSelection: boolean;
}

export interface SearchQueue {
  /** Queue a search. The promise settles when the queue is empty. */
  submit(request: SearchRequest): Promise<void>;
  /** Drop waiting input and ignore the reply that is still running. */
  cancel(): void;
  dispose(): void;
}

/**
 * Keeps one search in flight. Native IPC replies can arrive out of order, so
 * newer input replaces waiting input without a debounce timer, and only the
 * reply to the newest request reaches `apply` or `fail`.
 */
export function createSearchQueue<Response>(handlers: {
  send: (request: SearchRequest) => Promise<Response>;
  apply: (request: SearchRequest, response: Response) => void;
  fail: (request: SearchRequest, reason: unknown) => void;
  /** Runs when the queue becomes empty, unless it was disposed. */
  settled: () => void;
}): SearchQueue {
  let sequence = 0;
  let disposed = false;
  let task: Promise<void> | undefined;
  let queued: { request: SearchRequest; id: number } | undefined;

  async function drain() {
    try {
      while (queued && !disposed) {
        const { request, id } = queued;
        queued = undefined;
        try {
          const response = await handlers.send(request);
          if (disposed || id !== sequence) continue;
          handlers.apply(request, response);
        } catch (reason) {
          if (disposed || id !== sequence) continue;
          handlers.fail(request, reason);
        }
      }
    } finally {
      task = undefined;
      if (!disposed) handlers.settled();
    }
  }

  return {
    submit(request) {
      if (disposed) return Promise.resolve();
      queued = { request, id: ++sequence };
      return (task ??= drain());
    },
    cancel() {
      queued = undefined;
      sequence += 1;
    },
    dispose() {
      disposed = true;
      sequence += 1;
    },
  };
}

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
