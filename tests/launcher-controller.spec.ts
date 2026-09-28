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
    const selected = controller.current();
    reply = {
      ...response,
      results: [response.results[1], response.results[0]],
      notice: "Refreshed",
    };
    await controller.search("fixture", true);
    const refreshed = {
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
    const controller = createLauncherController({
      desktop: true,
      send: () => {
        calls++;
        return new Promise((resolve) => {
          complete = resolve;
        });
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
    };
  }, contracts.response);
  expect(result).toEqual({
    afterHidden: { count: 0, pending: false, calls: 1 },
    reopenedCount: 11,
    finalCount: 11,
    calls: 3,
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
      message: controller.error(),
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
