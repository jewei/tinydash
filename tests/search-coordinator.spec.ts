import { expect, test } from "@playwright/test";
import type { SearchResult } from "../src/bridge";
import {
  chooseSelection,
  createSearchQueue,
  type SearchRequest,
} from "../src/search";

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((done, fail) => {
    resolve = done;
    reject = fail;
  });
  return { promise, resolve, reject };
}

function harness() {
  const replies: {
    request: SearchRequest;
    reply: ReturnType<typeof deferred<string>>;
  }[] = [];
  const events: string[] = [];
  const queue = createSearchQueue<string>({
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
