import { batch, createSignal } from "solid-js";

import type { Category } from "../generated/Category";
import type { ResultAction } from "../generated/ResultAction";
import type { SearchResult } from "../generated/SearchResult";
import * as ipc from "../lib/ipc";
import { latestOnly } from "./latest";

interface Input {
  query: string;
  category: Category;
}

/** An action that asked for confirmation, waiting for the user. */
interface Pending {
  action: ResultAction;
  resultId?: string;
}

/** Which row to run: a numbered row, or whichever row is selected then. */
type Target = number | "selected";

/** A run that waits for the results of what was typed. */
interface Queued {
  target: Target;
  position: number;
}

/** Launcher state and behavior, without any rendering. */
export function createLauncher() {
  const [query, setQueryValue] = createSignal("");
  const [category, setCategoryValue] = createSignal<Category>("all");
  const [results, setResults] = createSignal<SearchResult[]>([]);
  const [selectedIndex, setSelectedIndex] = createSignal(0);
  const [revision, setRevision] = createSignal(0);
  const [searchError, setSearchError] = createSignal<string>();
  const [actionError, setActionError] = createSignal<string>();
  const [pending, setPending] = createSignal<Pending>();
  const [running, setRunning] = createSignal(false);

  const selected = () => results()[selectedIndex()];

  // The input whose results are on screen, and a run that waits for the
  // results of the latest input.
  let shown: Input | undefined;
  let queued: Queued | undefined;
  const isCurrent = () => shown?.query === query() && shown?.category === category();

  const request = latestOnly(
    (input: Input) => ipc.search(input.query, input.category),
    (found, input) => {
      const same = shown?.query === input.query && shown?.category === input.category;
      shown = input;
      batch(() => {
        // A refresh keeps the selected result, or its position when it is
        // gone (after a delete); a new input selects the first result.
        const kept = same ? found.findIndex((result) => result.id === selected()?.id) : 0;
        const index = kept >= 0 ? kept : Math.min(selectedIndex(), found.length - 1);
        setResults(found);
        setSelectedIndex(Math.max(index, 0));
        setRevision((value) => value + 1);
        setSearchError(undefined);
      });
      if (queued && isCurrent()) {
        const { target, position } = queued;
        queued = undefined;
        activate(target, position);
      }
    },
    (error, input) => {
      // The failed input counts as shown, with no results, so Enter does
      // nothing now and never waits to run on a later query.
      shown = input;
      queued = undefined;
      batch(() => {
        setResults([]);
        setSelectedIndex(0);
        setSearchError(ipc.message(error));
      });
    },
  );

  const refresh = () => request({ query: query(), category: category() });

  // A waiting Enter belongs to the input it was pressed on; an edit or a
  // tab change drops it, so it never runs on a query the user did not confirm.
  function setQuery(value: string) {
    queued = undefined;
    batch(() => {
      setQueryValue(value);
      setActionError(undefined);
    });
    void refresh();
  }

  function setCategory(value: Category) {
    queued = undefined;
    batch(() => {
      setCategoryValue(value);
      setActionError(undefined);
    });
    void refresh();
  }

  /** Go to the next or previous of the tabs shown, in their order. */
  function moveCategory(by: 1 | -1, tabs: readonly Category[]) {
    const index = tabs.indexOf(category());
    const next = tabs[(index + by + tabs.length) % tabs.length];
    // With one tab there is nowhere to go; starting it over would drop a
    // waiting Enter and the error on screen.
    if (next && next !== category()) setCategory(next);
  }

  function move(by: 1 | -1) {
    const last = results().length - 1;
    setSelectedIndex(Math.min(Math.max(selectedIndex() + by, 0), Math.max(last, 0)));
  }

  /** The launcher opened: start over with an empty query. */
  function reset(next: Category | null) {
    shown = undefined;
    queued = undefined;
    batch(() => {
      setQueryValue("");
      setCategoryValue(next ?? "all");
      setActionError(undefined);
      setPending(undefined);
    });
    void refresh();
  }

  /**
   * Run the action at `position` of the target row. While a newer search is
   * on its way, wait for it, so Enter never runs a result of an older query;
   * "selected" then means the row selected in the new results.
   */
  function activate(target: Target, position = 0) {
    if (!isCurrent()) {
      queued = { target, position };
      return;
    }
    const result = results()[target === "selected" ? selectedIndex() : target];
    const action = result?.actions[position];
    if (result && action) run(action, result);
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
    revision,
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
    isCurrent,
    cancelQueued: () => {
      queued = undefined;
    },
    activate,
    run,
    confirm,
    cancel: () => setPending(undefined),
  };
}
