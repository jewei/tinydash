import { expect, test } from "@playwright/test";
import type { SearchResult } from "../src/bridge";
import type { SearchDispatch, SearchQueueTiming } from "../src/searchQueue";
import { chooseSelection, createSearchQueue } from "../src/search";

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((done, fail) => {
    resolve = done;
    reject = fail;
  });
  return { promise, resolve, reject };
}

function harness(
  cancelBackend: (id: number) => Promise<void> = async () => {},
) {
  const replies: {
    request: SearchDispatch;
    reply: ReturnType<typeof deferred<string>>;
  }[] = [];
  const events: string[] = [];
  const queue = createSearchQueue<string>({
    cancelBackend,
    send(request) {
      const reply = deferred<string>();
      replies.push({ request, reply });
      return reply.promise;
    },
    apply: (request, response) =>
      events.push(`apply ${request.value}=${response}`),
    fail: (request, reason) =>
      events.push(`fail ${request.value}: ${String(reason)}`),
    settled: () => events.push("settled"),
  });
  const submit = (value: string) =>
    queue.submit({ value, mode: "all", preserveSelection: false });
  return { queue, replies, events, submit };
}

test("keeps one search in flight and sends only the newest waiting input", async () => {
  const { replies, events, submit } = harness();
  const done = submit("a");
  void submit("ab");
  void submit("abc");
  expect(replies.map(({ request }) => request.value)).toEqual(["a"]);

  replies[0].reply.resolve("first");
  await expect.poll(() => replies.length).toBe(2);
  expect(replies[1].request.value).toBe("abc");
  replies[1].reply.resolve("third");
  await done;
  // The reply to "a" is stale because newer input was queued.
  expect(events).toEqual(["apply abc=third", "settled"]);
});

test("reports a failure only for the newest request", async () => {
  const { replies, events, submit } = harness();
  const done = submit("a");
  void submit("b");
  replies[0].reply.reject("old failure");
  await expect.poll(() => replies.length).toBe(2);
  replies[1].reply.reject("new failure");
  await done;
  expect(events).toEqual(["fail b: new failure", "settled"]);
});

test("cancel ignores the running reply, and dispose stops all callbacks", async () => {
  const { queue, replies, events, submit } = harness();
  const cancelled = submit("hidden");
  void submit("waiting");
  queue.cancel();
  replies[0].reply.resolve("late");
  await cancelled;
  expect(replies).toHaveLength(1);
  expect(events).toEqual(["settled"]);

  const disposed = submit("closing");
  queue.dispose();
  replies[1].reply.resolve("late");
  await disposed;
  expect(events).toEqual(["settled"]);
  await submit("after dispose");
  expect(replies).toHaveLength(2);
});

test("new input cancels the exact active request immediately, once, and drains its acknowledgement", async () => {
  const acknowledgement = deferred<void>();
  const cancellations: number[] = [];
  const { replies, submit, events } = harness((id) => {
    cancellations.push(id);
    return acknowledgement.promise;
  });
  const done = submit("first");
  void submit("second");
  void submit("third");
  expect(cancellations).toEqual([replies[0].request.requestId]);
  expect(Number.isSafeInteger(cancellations[0])).toBe(true);
  replies[0].reply.resolve("superseded");
  await Promise.resolve();
  await Promise.resolve();
  expect(replies).toHaveLength(1);
  void submit("latest while cancel awaits");
  acknowledgement.resolve();
  await expect.poll(() => replies.length).toBe(2);
  expect(replies[1].request.value).toBe("latest while cancel awaits");
  expect(replies[1].request.requestId).not.toBe(cancellations[0]);
  replies[1].reply.resolve("latest");
  await done;
  expect(events).toEqual([
    "apply latest while cancel awaits=latest",
    "settled",
  ]);
});

test("a failed cancellation preserves single-flight and latest waiting semantics", async () => {
  for (const synchronous of [false, true]) {
    const { replies, submit, events } = harness(() => {
      if (synchronous) throw new Error("transport unavailable");
      return Promise.reject(new Error("transport unavailable"));
    });
    const done = submit("first");
    void submit("discarded");
    void submit("last");
    expect(replies).toHaveLength(1);
    replies[0].reply.reject("cancelled backend");
    await expect.poll(() => replies.length).toBe(2);
    expect(replies[1].request.value).toBe("last");
    replies[1].reply.resolve("answer");
    await done;
    expect(events).toEqual(["apply last=answer", "settled"]);
  }
});

test("hide and dispose cancel active work without dispatching waiting input", async () => {
  for (const action of ["cancel", "dispose"] as const) {
    const cancellations: number[] = [];
    const { queue, replies, events, submit } = harness(async (id) => {
      cancellations.push(id);
    });
    const done = submit("running");
    queue[action]();
    queue[action]();
    expect(cancellations).toEqual([replies[0].request.requestId]);
    replies[0].reply.reject("Search canceled.");
    await done;
    expect(replies).toHaveLength(1);
    expect(events).toEqual(action === "dispose" ? [] : ["settled"]);
  }
});

test("a synchronous transport error does not wedge later searches and timing stays aggregate-only", async () => {
  const events: string[] = [];
  const timings: SearchQueueTiming[] = [];
  let calls = 0;
  const queue = createSearchQueue<string>({
    send() {
      if (++calls === 1) throw new Error("transport");
      return Promise.resolve("ok");
    },
    cancelBackend: async () => {},
    apply: (_, response) => events.push(response),
    fail: () => events.push("failed"),
    settled: () => events.push("settled"),
    timing: (timing) => timings.push(timing),
  });
  await queue.submit({
    value: "private input",
    mode: "all",
    preserveSelection: false,
  });
  await queue.submit({
    value: "second input",
    mode: "all",
    preserveSelection: false,
  });
  expect(events).toEqual(["failed", "settled", "ok", "settled"]);
  expect(timings).toHaveLength(2);
  for (const timing of timings) {
    expect(Object.keys(timing).sort()).toEqual([
      "responseMs",
      "superseded",
      "waitingMs",
    ]);
    expect(timing.responseMs).toBeGreaterThanOrEqual(0);
    expect(timing.waitingMs).toBeGreaterThanOrEqual(0);
    expect(timing.superseded).toBe(false);
  }
});

const result = (
  id: string,
  extra: Partial<SearchResult> = {},
): SearchResult => ({
  id,
  kind: "app",
  title: id,
  subtitle: "",
  score: 0,
  icon: null,
  primaryAction: "launch",
  secondaryActions: [],
  ...extra,
});

test("a refresh keeps the selected result, and a new query selects the first", () => {
  const results = [result("a"), result("b"), result("c")];
  const refresh = {
    request: { value: "x", mode: "all" as const, preserveSelection: true },
    results: [result("c"), result("a"), result("b")],
    displayed: { value: "x", mode: "all" as const },
    current: results[1],
    selected: 1,
    selectionChangedByUser: true,
  };
  expect(chooseSelection(refresh)).toBe(2);
  expect(
    chooseSelection({ ...refresh, displayed: { value: "y", mode: "all" } }),
  ).toBe(0);
  expect(
    chooseSelection({
      ...refresh,
      request: { ...refresh.request, preserveSelection: false },
    }),
  ).toBe(0);
  // A pin key identifies the same item when its result ID changes.
  const pin = { key: "app:a", categories: ["all" as const] };
  expect(
    chooseSelection({
      ...refresh,
      current: result("old", { pin }),
      results: [result("b"), result("new", { pin })],
    }),
  ).toBe(1);
  // Tool rows get new IDs on each reply. The position stays.
  expect(
    chooseSelection({
      ...refresh,
      current: result("tool:1", { kind: "timezone" }),
      selected: 4,
      results: [result("tool:7"), result("tool:8")],
    }),
  ).toBe(1);
});

test("an empty clipboard search follows the newest entry until the user moves", () => {
  const options = {
    request: { value: "", mode: "clipboard" as const, preserveSelection: true },
    results: [result("pinned"), result("newest"), result("older")],
    preferredSelectionId: "newest",
    displayed: { value: "", mode: "clipboard" as const },
    current: result("older"),
    selected: 2,
    selectionChangedByUser: false,
  };
  expect(chooseSelection(options)).toBe(1);
  expect(chooseSelection({ ...options, selectionChangedByUser: true })).toBe(2);
  // A missing preferred entry falls back to the first row.
  expect(chooseSelection({ ...options, preferredSelectionId: "missing" })).toBe(
    0,
  );
});
