/**
 * Run `task` for the newest input only. One call runs at a time; inputs that
 * arrive meanwhile replace each other, and only the last one runs next. A
 * result is delivered only if no newer input arrived while it ran, so a slow
 * reply can never overwrite a fresher one.
 */
export function latestOnly<Input, Output>(
  task: (input: Input) => Promise<Output>,
  deliver: (output: Output, input: Input) => void,
  fail: (error: unknown, input: Input) => void,
): (input: Input) => Promise<void> {
  let running = false;
  let waiting: { input: Input } | undefined;
  return async (input) => {
    waiting = { input };
    if (running) return;
    running = true;
    while (waiting) {
      const current = waiting.input;
      waiting = undefined;
      try {
        const output = await task(current);
        if (!waiting) deliver(output, current);
      } catch (error) {
        if (!waiting) fail(error, current);
      }
    }
    running = false;
  };
}
