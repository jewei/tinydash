import { expect, test } from "@playwright/test";
import type { SearchResponse } from "../src/bridge";
import { contracts } from "./fixtures/ipc-contract";

// Run standalone Solid state owners under the browser export condition, without
// rendering App. Node's Solid server build intentionally does not update stores.
test("controller reconciles rows and preserves user selection only for refreshes", async ({
  page,
}) => {
  await page.goto("/");
  const result = await page.evaluate(async (response: SearchResponse) => {
    const path = "/src/launcherController.ts";
    const { createLauncherController } = (await import(
      path
    )) as typeof import("../src/launcherController");
    let reply = response;
    const controller = createLauncherController({
      desktop: true,
      send: async () => reply,
    });
    controller.setQuery("fixture");
    await controller.search();
    controller.setSelected(1);
    controller.markSelectionChanged();
    // Solid 2 commits signal updates after the current synchronous turn.
    await new Promise<void>((resolve) => queueMicrotask(resolve));
    const selected = controller.current();
    reply = {
      ...response,
      results: [response.results[1], response.results[0]],
      notice: "Refreshed",
    };
    await controller.search("fixture", true);
    const refreshed = {
      selectedId: selected.id,
      id: controller.current().id,
      index: controller.selected(),
      sameRow: selected === controller.current(),
      pending: controller.pending(),
      notice: controller.notice(),
    };
    controller.setQuery("new query");
    reply = response;
    await controller.search();
    const newQuery = controller.current().id;
    controller.dispose();
    return { refreshed, newQuery };
  }, contracts.response);
  expect(result).toEqual({
    refreshed: {
      selectedId: "result:1",
      id: "result:1",
      index: 0,
      sameRow: true,
      pending: false,
      notice: "Refreshed",
    },
    newQuery: "result:0",
  });
});

test("hidden and disposed controllers reject late replies and do not send hidden searches", async ({
  page,
}) => {
  await page.goto("/");
  const result = await page.evaluate(async (response: SearchResponse) => {
    const path = "/src/launcherController.ts";
    const { createLauncherController } = (await import(
      path
    )) as typeof import("../src/launcherController");
    let complete!: (response: SearchResponse) => void;
    let calls = 0;
    const requests: (number | undefined)[] = [];
    const cancellations: number[] = [];
    const controller = createLauncherController({
      desktop: true,
      send: (_value, _mode, requestId) => {
        calls++;
        requests.push(requestId);
        return new Promise((resolve) => {
          complete = resolve;
        });
      },
      cancelBackend: async (requestId) => {
        cancellations.push(requestId);
      },
    });
    const hidden = controller.search("hidden");
    controller.hidden();
    await controller.search("not sent");
    complete(response);
    await hidden;
    const afterHidden = {
      count: controller.results().length,
      pending: controller.pending(),
      calls,
    };
    controller.setVisible(true);
    const reopened = controller.search("reopened");
    complete(response);
    await reopened;
    const reopenedCount = controller.results().length;
    const closing = controller.search("closing");
    controller.dispose();
    complete({ ...response, results: [] });
    await closing;
    await controller.search("disposed");
    return {
      afterHidden,
      reopenedCount,
      finalCount: controller.results().length,
      calls,
      cancellationMatches:
        cancellations.length === 2 &&
        cancellations[0] === requests[0] &&
        cancellations[1] === requests[2],
      validRequestIds: requests.every(
        (id) => Number.isSafeInteger(id) && (id ?? 0) > 0,
      ),
    };
  }, contracts.response);
  expect(result).toEqual({
    afterHidden: { count: 0, pending: false, calls: 1 },
    reopenedCount: 11,
    finalCount: 11,
    calls: 3,
    cancellationMatches: true,
    validRequestIds: true,
  });
});

test("hide then reopen before the old reply settles keeps the new search pending", async ({
  page,
}) => {
  await page.goto("/");
  const result = await page.evaluate(async (response: SearchResponse) => {
    const path = "/src/launcherController.ts";
    const { createLauncherController } = (await import(
      path
    )) as typeof import("../src/launcherController");
    const calls: string[] = [];
    const replies: ((response: SearchResponse) => void)[] = [];
    let started!: () => void;
    const newStarted = new Promise<void>((resolve) => {
      started = resolve;
    });
    const controller = createLauncherController({
      desktop: true,
      send: (value) => {
        calls.push(value);
        return new Promise((resolve) => {
          replies.push(resolve);
          if (value === "new session") started();
        });
      },
    });
    const old = controller.search("old session");
    controller.hidden();
    controller.setVisible(true);
    const reopened = controller.search("new session");
    const beforeOldReply = { calls: [...calls], pending: controller.pending() };
    replies[0]({
      ...response,
      notice: "stale",
      storageError: {
        code: "storageUnavailable",
        message: "stale",
        retryable: false,
      },
    });
    // Cancellation acknowledgement adds microtasks. Observe actual submission,
    // not an assumed number of Promise ticks, before resolving the new reply.
    await newStarted;
    const afterOldReply = {
      calls: [...calls],
      pending: controller.pending(),
      count: controller.results().length,
      notice: controller.notice() ?? null,
      warning: controller.storageError() ?? null,
    };
    replies[1]({ ...response, notice: "current" });
    await Promise.all([old, reopened]);
    const settled = {
      pending: controller.pending(),
      count: controller.results().length,
      notice: controller.notice(),
    };
    controller.dispose();
    return { beforeOldReply, afterOldReply, settled };
  }, contracts.response);
  expect(result).toEqual({
    beforeOldReply: { calls: ["old session"], pending: true },
    afterOldReply: {
      calls: ["old session", "new session"],
      pending: true,
      count: 0,
      notice: null,
      warning: null,
    },
    settled: { pending: false, count: 11, notice: "current" },
  });
});

test("controller clears only a current search failure on a same-query refresh", async ({
  page,
}) => {
  await page.goto("/");
  const result = await page.evaluate(async (response: SearchResponse) => {
    const path = "/src/launcherController.ts";
    const { createLauncherController } = (await import(
      path
    )) as typeof import("../src/launcherController");
    let fail = true;
    const controller = createLauncherController({
      desktop: true,
      send: async () => {
        if (fail) throw new Error("Search failed");
        return response;
      },
    });
    controller.setQuery("fixture");
    await controller.search();
    const failure = controller.searchError();
    fail = false;
    await controller.search(controller.query(), true);
    const recovered = {
      query: controller.query(),
      error: controller.searchError() ?? null,
      count: controller.results().length,
      pending: controller.pending(),
    };
    controller.dispose();
    return { failure, recovered };
  }, contracts.response);
  expect(result).toEqual({
    failure: "Error: Search failed",
    recovered: { query: "fixture", error: null, count: 11, pending: false },
  });
});

test("a superseded controller success leaves the failure intact until the current retry settles", async ({
  page,
}) => {
  await page.goto("/");
  const result = await page.evaluate(async (response: SearchResponse) => {
    const path = "/src/launcherController.ts";
    const { createLauncherController } = (await import(
      path
    )) as typeof import("../src/launcherController");
    const replies: {
      resolve: (response: SearchResponse) => void;
      reject: (reason: Error) => void;
    }[] = [];
    let nextStarted!: () => void;
    const latestStarted = new Promise<void>((resolve) => {
      nextStarted = resolve;
    });
    const controller = createLauncherController({
      desktop: true,
      send: () =>
        new Promise((resolve, reject) => {
          replies.push({ resolve, reject });
          if (replies.length === 3) nextStarted();
        }),
    });
    controller.setQuery("fixture");
    const initial = controller.search();
    replies[0].reject(new Error("First failure"));
    await initial;
    const old = controller.search(controller.query(), true);
    const latest = controller.search(controller.query(), true);
    const beforeOld = replies.length;
    replies[1].resolve(response);
    await latestStarted;
    const afterOld = {
      error: controller.searchError(),
      pending: controller.pending(),
      count: controller.results().length,
    };
    replies[2].reject(new Error("Latest failure"));
    await Promise.all([old, latest]);
    const afterLatest = {
      error: controller.searchError(),
      pending: controller.pending(),
    };
    const recovery = controller.search(controller.query(), true);
    replies[3].resolve(response);
    await recovery;
    const recovered = controller.searchError() ?? null;
    controller.dispose();
    return { beforeOld, afterOld, afterLatest, recovered };
  }, contracts.response);
  expect(result).toEqual({
    beforeOld: 2,
    afterOld: { error: "Error: First failure", pending: true, count: 0 },
    afterLatest: { error: "Error: Latest failure", pending: false },
    recovered: null,
  });
});

test("controller clears stale welcome rows, reports failures, and gates non-desktop search", async ({
  page,
}) => {
  await page.goto("/");
  const result = await page.evaluate(async (response: SearchResponse) => {
    const path = "/src/launcherController.ts";
    const { createLauncherController } = (await import(
      path
    )) as typeof import("../src/launcherController");
    let fail = false;
    const send = async () => {
      if (fail) throw new Error("Search unavailable");
      return response;
    };
    const controller = createLauncherController({ desktop: true, send });
    controller.setQuery("fixture");
    await controller.search();
    fail = true;
    controller.setQuery("");
    const pending = controller.search();
    const clearedImmediately = controller.results().length;
    await pending;
    const failed = {
      message: controller.searchError(),
      pending: controller.pending(),
    };
    const preview = createLauncherController({
      desktop: false,
      send: () => {
        throw new Error("must not send");
      },
    });
    await preview.search();
    const previewPending = preview.pending();
    controller.dispose();
    preview.dispose();
    return { clearedImmediately, failed, previewPending };
  }, contracts.response);
  expect(result).toEqual({
    clearedImmediately: 0,
    failed: { message: "Error: Search unavailable", pending: false },
    previewPending: false,
  });
});
