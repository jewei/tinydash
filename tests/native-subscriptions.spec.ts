import { expect, test } from "@playwright/test";
import { createNativeSubscriptions } from "../src/nativeSubscriptions";

function harness() {
  const pending: {
    callback: (event: { payload: unknown }) => void;
    resolve: (stop: () => void) => void;
    reject: (reason: unknown) => void;
  }[] = [];
  const owner = createNativeSubscriptions(
    (_name, callback) =>
      new Promise((resolve, reject) => {
        pending.push({ callback, resolve, reject });
      }),
  );
  return { owner, pending };
}

test("owns late native registrations and suppresses events after disposal", async () => {
  const { owner, pending } = harness();
  const values: unknown[] = [];
  let stopped = 0;
  const registration = owner.register("settings-changed", (value) =>
    values.push(value),
  );
  pending[0].callback({ payload: "before" });
  owner.dispose();
  pending[0].callback({ payload: "after" });
  pending[0].resolve(() => stopped++);
  expect(await registration).toBe(false);
  owner.dispose();
  expect(stopped).toBe(1);
  expect(values).toEqual(["before"]);
});

test("cleans successful subscriptions when a sibling registration fails", async () => {
  const { owner, pending } = harness();
  let stopped = 0;
  const first = owner.register("apps-changed", () => {});
  const second = owner.register("files-changed", () => {});
  pending[0].resolve(() => stopped++);
  expect(await first).toBe(true);
  pending[1].reject(new Error("listener unavailable"));
  await expect(second).rejects.toThrow("listener unavailable");
  owner.dispose();
  expect(stopped).toBe(1);
});

test("appearance subscriptions share the same asynchronous lifetime", async () => {
  const { owner } = harness();
  let stopped = 0;
  let changed = 0;
  const callback = owner.guard(() => changed++);
  let resolve!: (stop: () => void) => void;
  const active = owner.own(
    new Promise((done) => {
      resolve = done;
    }),
  );
  owner.dispose();
  callback(undefined);
  resolve(() => stopped++);
  expect(await active).toBe(false);
  expect({ stopped, changed }).toEqual({ stopped: 1, changed: 0 });
});
