import { describe, expect, it } from "vite-plus/test";

import { latestOnly } from "./latest";

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => (resolve = done));
  return { promise, resolve };
}

describe("latestOnly", () => {
  it("runs one call at a time and skips superseded inputs", async () => {
    const calls: string[] = [];
    const pending = new Map<string, ReturnType<typeof deferred<string>>>();
    const delivered: string[] = [];
    const run = latestOnly(
      (input: string) => {
        calls.push(input);
        const reply = deferred<string>();
        pending.set(input, reply);
        return reply.promise;
      },
      (output) => delivered.push(output),
      () => {},
    );

    const first = run("a");
    void run("ab");
    void run("abc");
    expect(calls).toEqual(["a"]);

    pending.get("a")!.resolve("A");
    await Promise.resolve();
    await Promise.resolve();
    expect(calls).toEqual(["a", "abc"]);
    expect(delivered).toEqual([]);

    pending.get("abc")!.resolve("ABC");
    await first;
    expect(delivered).toEqual(["ABC"]);
  });

  it("reports failures only for the newest input", async () => {
    const failures: string[] = [];
    const run = latestOnly(
      (input: string) => Promise.reject(new Error(input)),
      () => {},
      (_, input) => failures.push(input),
    );
    await run("x");
    expect(failures).toEqual(["x"]);
  });
});
