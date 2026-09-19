// Diagnostic overlay shared by the real and minimal pages. Retain metadata only.
import { invoke } from "@tauri-apps/api/core";

const state = window as unknown as {
  __memoryTrace: unknown[];
  __memoryQuery: (query: string) => Promise<unknown>;
  __memoryMinimal?: boolean;
  __memoryTraceEnabled?: boolean;
  __memoryCountBytes?: boolean;
};
state.__memoryTrace = [];
export async function memorySearch(query: string, mode: string) {
  const args = { query, mode };
  const result = await invoke("search", args);
  if (state.__memoryTraceEnabled === false) return result;
  const response = result as { results: { id: string; title: string }[] };
  state.__memoryTrace.push({
    args,
    bytes: state.__memoryCountBytes ? JSON.stringify(result).length : undefined,
    ids: response.results.map((row) => row.id),
    titles: response.results.map((row) => row.title),
  });
  if (state.__memoryTrace.length > 128) state.__memoryTrace.shift();
  return result;
}
state.__memoryQuery = async (query: string) => {
  state.__memoryTrace.length = 0;
  if (state.__memoryMinimal) {
    await memorySearch(query, "all");
  } else {
    const input =
      document.querySelector<HTMLInputElement>("input[role=combobox]") ??
      document.querySelector<HTMLInputElement>(".search-input");
    if (!input) throw new Error("Launcher input missing");
    input.value = query;
    input.dispatchEvent(new Event("input", { bubbles: true }));
    const deadline = performance.now() + 3000;
    while (!state.__memoryTrace.length && performance.now() < deadline)
      await new Promise((resolve) => setTimeout(resolve, 2));
    if (!state.__memoryTrace.length) throw new Error("Search did not finish");
    await new Promise((resolve) => setTimeout(resolve, 0));
  }
  return state.__memoryTrace;
};
