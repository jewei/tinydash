import { batch, createSignal } from "solid-js";

import type { Category } from "../generated/Category";
import type { ResultAction } from "../generated/ResultAction";
import type { SearchResult } from "../generated/SearchResult";
import * as ipc from "../lib/ipc";
import { latestOnly } from "./latest";

export const CATEGORIES: ReadonlyArray<{ id: Category; label: string }> = [
  { id: "all", label: "All" },
  { id: "apps", label: "Apps" },
  { id: "files", label: "Files" },
  { id: "clipboard", label: "Clipboard" },
  { id: "snippets", label: "Snippets" },
  { id: "emoji", label: "Emoji" },
  { id: "system", label: "System" },
];

interface Input {
  query: string;
  category: Category;
}

/** An action that asked for confirmation, waiting for the user. */
export interface Pending {
  action: ResultAction;
  resultId?: string;
}

export type Launcher = ReturnType<typeof createLauncher>;

/** Launcher state and behavior, without any rendering. */
export function createLauncher() {
  const [query, setQueryValue] = createSignal("");
  const [category, setCategoryValue] = createSignal<Category>("all");
  const [results, setResults] = createSignal<SearchResult[]>([]);
  const [selectedIndex, setSelectedIndex] = createSignal(0);
  const [searchError, setSearchError] = createSignal<string>();
  const [actionError, setActionError] = createSignal<string>();
  const [pending, setPending] = createSignal<Pending>();
  const [running, setRunning] = createSignal(false);

  const selected = () => results()[selectedIndex()];

  // The input whose results are on screen. A refresh of the same input
  // keeps the selected result; a new input selects the first one.
  let shown: Input | undefined;
  const request = latestOnly(
    (input: Input) => ipc.search(input.query, input.category),
    (found, input) => {
      const same = shown?.query === input.query && shown?.category === input.category;
      const keep = same ? selected()?.id : undefined;
      shown = input;
      batch(() => {
        setResults(found);
        setSelectedIndex(
          Math.max(
            0,
            found.findIndex((result) => result.id === keep),
          ),
        );
        setSearchError(undefined);
      });
    },
    (error) => setSearchError(ipc.message(error)),
  );

  const refresh = () => request({ query: query(), category: category() });

  function setQuery(value: string) {
    batch(() => {
      setQueryValue(value);
      setActionError(undefined);
    });
    void refresh();
  }

  function setCategory(value: Category) {
    batch(() => {
      setCategoryValue(value);
      setActionError(undefined);
    });
    void refresh();
  }

  function moveCategory(by: 1 | -1) {
    const index = CATEGORIES.findIndex((entry) => entry.id === category());
    const next = CATEGORIES[(index + by + CATEGORIES.length) % CATEGORIES.length];
    if (next) setCategory(next.id);
  }

  function move(by: 1 | -1) {
    const last = results().length - 1;
    setSelectedIndex(Math.min(Math.max(selectedIndex() + by, 0), Math.max(last, 0)));
  }

  /** The launcher opened: start over with an empty query. */
  function reset(next: Category | null) {
    batch(() => {
      setQueryValue("");
      setCategoryValue(next ?? "all");
      setActionError(undefined);
      setPending(undefined);
    });
    void refresh();
  }

  /** Run an action, or ask first when it needs confirmation. */
  function run(action: ResultAction, result?: SearchResult) {
    // Usage ranking learns only from a result's main action.
    const resultId = result && result.actions[0] === action ? result.id : undefined;
    if (action.confirm) setPending({ action, resultId });
    else void perform(action, resultId);
  }

  function confirm() {
    const waiting = pending();
    setPending(undefined);
    if (waiting) void perform(waiting.action, waiting.resultId);
  }

  async function perform(action: ResultAction, resultId?: string) {
    if (running()) return;
    setRunning(true);
    setActionError(undefined);
    try {
      await ipc.runAction(action.action, resultId);
    } catch (error) {
      setActionError(ipc.message(error));
    } finally {
      setRunning(false);
    }
  }

  return {
    query,
    category,
    results,
    selected,
    selectedIndex,
    select: setSelectedIndex,
    searchError,
    actionError,
    pending,
    running,
    setQuery,
    setCategory,
    moveCategory,
    move,
    reset,
    refresh,
    run,
    confirm,
    cancel: () => setPending(undefined),
  };
}
