import type { SearchMode } from "./bridge";

export interface SearchRequest {
  value: string;
  mode: SearchMode;
  preserveSelection: boolean;
}

export interface SearchDispatch extends SearchRequest {
  /** Opaque safe integer, also passed to the lightweight cancellation command. */
  requestId: number;
}

export interface SearchQueue {
  /** Settles when both the search and cancellation queue are empty. */
  submit(request: SearchRequest): Promise<void>;
  cancel(): void;
  dispose(): void;
}

/** Aggregate durations only: no queries, IDs, paths, result text, or secrets. */
export interface SearchQueueTiming {
  waitingMs: number;
  responseMs: number;
  superseded: boolean;
}

function requestId(): number {
  // Random 53-bit IDs survive a webview reload without reusing a canceled
  // sequence number. They are exactly representable by JS and Rust's u64.
  const words = crypto.getRandomValues(new Uint32Array(2));
  return (words[0] & 0x1fffff) * 0x100000000 + words[1] || 1;
}

/**
 * One expensive search in flight and only the latest waiting input. A separate
 * cheap cancellation command interrupts superseded work, even before the next
 * search is sent. Its acknowledgement is drained first so it cannot race the
 * next dispatch. Stale replies are still rejected independently of cancellation.
 */
export function createSearchQueue<Response>(handlers: {
  send: (request: SearchDispatch) => Promise<Response>;
  cancelBackend: (requestId: number) => Promise<void>;
  apply: (request: SearchRequest, response: Response) => void;
  fail: (request: SearchRequest, reason: unknown) => void;
  settled: () => void;
  timing?: (timing: SearchQueueTiming) => void;
}): SearchQueue {
  let sequence = 0;
  let disposed = false;
  let task: Promise<void> | undefined;
  let cancellation: Promise<void> | undefined;
  let active: { requestId: number; cancelled: boolean } | undefined;
  let queued:
    { request: SearchRequest; id: number; queuedAt: number } | undefined;

  function cancelActive() {
    if (!active || active.cancelled) return;
    active.cancelled = true;
    // Invoke immediately, not after the expensive request resolves. Older
    // backends/IPC errors may reject cancellation; stale-reply filtering and
    // latest-waiting semantics must continue to work in that case.
    try {
      cancellation = handlers.cancelBackend(active.requestId).catch(() => {});
    } catch {
      cancellation = undefined;
    }
  }

  // An adapter can throw before returning a promise. Normalize that into an
  // asynchronous rejection so drain cannot finish before `task` is assigned.
  async function send(request: SearchDispatch): Promise<Response> {
    return handlers.send(request);
  }

  async function drain() {
    try {
      while (queued && !disposed) {
        const { request, id, queuedAt } = queued;
        queued = undefined;
        const started = performance.now();
        active = { requestId: requestId(), cancelled: false };
        try {
          const response = await send({
            ...request,
            requestId: active.requestId,
          });
          if (!disposed && id === sequence) handlers.apply(request, response);
        } catch (reason) {
          if (!disposed && id === sequence) handlers.fail(request, reason);
        } finally {
          active = undefined;
          if (!disposed) {
            handlers.timing?.({
              waitingMs: started - queuedAt,
              responseMs: performance.now() - started,
              superseded: id !== sequence,
            });
          }
          if (cancellation) await cancellation;
          cancellation = undefined;
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
      queued = { request, id: ++sequence, queuedAt: performance.now() };
      cancelActive();
      return (task ??= drain());
    },
    cancel() {
      queued = undefined;
      sequence += 1;
      cancelActive();
    },
    dispose() {
      disposed = true;
      queued = undefined;
      sequence += 1;
      cancelActive();
    },
  };
}
