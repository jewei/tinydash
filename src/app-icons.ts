import { listen } from "@tauri-apps/api/event";
import { backend } from "./bridge";

type Consumer = (url: string | undefined) => void;
interface Entry {
  key: string;
  pixels: number;
  request: string;
  consumers: Set<Consumer>;
  url?: string;
  waiting: boolean;
  inFlight: boolean;
}

// These entries belong only to mounted, visible images. Rust owns the cache.
const entries = new Map<string, Entry>();
const viewId = Array.from(crypto.getRandomValues(new Uint32Array(4)), (value) =>
  value.toString(16),
).join("-");
let sequence = 0;
let stop: (() => void) | undefined;
let listening: Promise<void> | undefined;
let disposed = false;
let capacityRevision = 0;

function retryWaiting() {
  for (const entry of entries.values()) if (entry.waiting) void load(entry);
}

function startListening() {
  return (listening ??= listen("app-icons-ready", () => {
    capacityRevision += 1;
    retryWaiting();
  })
    .then((unlisten) => {
      if (disposed) unlisten();
      else stop = unlisten;
    })
    .catch(() => {
      /* In-flight completions also retry waiting visible images. */
    }));
}

async function load(entry: Entry) {
  if (entry.inFlight) return;
  entry.waiting = false;
  await startListening();
  if (disposed || entries.get(`${entry.key}/${entry.pixels}`) !== entry) return;
  let capacity = false;
  const revision = capacityRevision;
  entry.inFlight = true;
  try {
    const url = await backend.appIcon(entry.key, entry.pixels, entry.request);
    entry.inFlight = false;
    if (disposed || entries.get(`${entry.key}/${entry.pixels}`) !== entry)
      return;
    entry.url = url;
    for (const consumer of entry.consumers) consumer(url);
    capacity = true;
  } catch (reason) {
    entry.inFlight = false;
    if (entries.get(`${entry.key}/${entry.pixels}`) !== entry) return;
    entry.waiting = reason === "busy";
    if (entry.waiting && revision !== capacityRevision)
      queueMicrotask(() => {
        if (entry.waiting) void load(entry);
      });
    capacity = !entry.waiting;
  } finally {
    if (capacity) retryWaiting();
  }
}

export function loadAppIcon(
  key: string,
  pixels: number,
  consumer: Consumer,
): () => void {
  const identity = `${key}/${pixels}`;
  let entry = entries.get(identity);
  if (!entry) {
    // A launcher has at most 30 rows and one preview. Bound callers too.
    if (disposed || entries.size >= 64) return () => {};
    entry = {
      key,
      pixels,
      request: `${viewId}:${++sequence}`,
      consumers: new Set(),
      waiting: false,
      inFlight: false,
    };
    entries.set(identity, entry);
    void load(entry);
  }
  entry.consumers.add(consumer);
  consumer(entry.url);
  return () => {
    entry.consumers.delete(consumer);
    if (entry.consumers.size) return;
    entries.delete(identity);
    if (entry.inFlight)
      void backend
        .cancelAppIcon(entry.request)
        .then(retryWaiting)
        .catch(() => {});
  };
}

export function disposeAppIcons() {
  disposed = true;
  stop?.();
  for (const entry of entries.values()) {
    if (entry.inFlight)
      void backend.cancelAppIcon(entry.request).catch(() => {});
  }
  entries.clear();
}
