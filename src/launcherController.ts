import { batch, createSignal } from "solid-js";
import { createStore, reconcile } from "solid-js/store";
import type {
  CurrencyStatus,
  FileStatus,
  LauncherInfo,
  LauncherWarning,
  SettingsValues,
  SearchMode,
  SearchResponse,
  SearchResult,
} from "./bridge";
import { chooseSelection, createSearchQueue } from "./search";

/** Clear only warning categories whose cause a successful settings save repairs. */
export function receiveLauncherSettings(
  current: LauncherInfo,
  settings: SettingsValues,
): LauncherInfo {
  const shortcutsChanged =
    current.settings.shortcut !== settings.shortcut ||
    JSON.stringify(current.settings.categoryShortcuts) !==
      JSON.stringify(settings.categoryShortcuts);
  return {
    ...current,
    settings,
    warnings: current.warnings.filter(
      (warning) =>
        warning.code !== "settingsRead" &&
        !(warning.code === "shortcutRegistration" && shortcutsChanged),
    ),
  };
}

/** Owns the launcher search session, not DOM focus or native command execution. */
export function createLauncherController(options: {
  desktop: boolean;
  send: (
    value: string,
    mode: SearchMode,
    requestId?: number,
  ) => Promise<SearchResponse>;
  cancelBackend?: (requestId: number) => Promise<void>;
}) {
  const [visible, setVisible] = createSignal(true);
  const [query, setQuery] = createSignal("");
  const [mode, setMode] = createSignal<SearchMode>("all");
  const [view, setView] = createStore<{ results: SearchResult[] }>({
    results: [],
  });
  const results = () => view.results;
  const setResults = (next: SearchResult[]) =>
    setView("results", reconcile(next, { key: "id" }));
  const [selected, setSelected] = createSignal(0);
  const current = () => results()[selected()];
  const [pending, setPending] = createSignal(false);
  const [total, setTotal] = createSignal(0);
  const [indexing, setIndexing] = createSignal(options.desktop);
  const [files, setFiles] = createSignal<FileStatus>({
    total: 0,
    phase: options.desktop ? "queued" : "idle",
    warning: null,
  });
  const [currency, setCurrency] = createSignal<CurrencyStatus>({
    asOf: null,
    refreshing: false,
    warning: null,
  });
  const [error, setError] = createSignal<string>();
  const [indexError, setIndexError] = createSignal<string>();
  const [storageError, setStorageError] = createSignal<LauncherWarning>();
  const [notice, setNotice] = createSignal<string>();
  let disposed = false;
  let displayedQuery: { value: string; mode: SearchMode } | undefined;
  let selectionChangedByUser = false;

  const searches = createSearchQueue({
    send: ({ value, mode, requestId }) => options.send(value, mode, requestId),
    cancelBackend: options.cancelBackend ?? (async () => {}),
    apply(request, response) {
      const index = chooseSelection({
        request,
        results: response.results,
        preferredSelectionId: response.preferredSelectionId,
        displayed: displayedQuery,
        current: current(),
        selected: selected(),
        selectionChangedByUser,
      });
      displayedQuery = { value: request.value, mode: request.mode };
      batch(() => {
        setResults(response.results);
        setSelected(index);
        setTotal(response.total);
        setIndexing(response.indexing);
        setFiles(response.files);
        setCurrency(response.currency);
        setIndexError(response.indexError ?? undefined);
        setStorageError(response.storageError ?? undefined);
        setNotice(response.notice ?? undefined);
      });
    },
    fail(_request, reason) {
      setResults([]);
      setError(String(reason));
    },
    settled: () => setPending(false),
  });

  function search(value = query(), preserveSelection = false) {
    if (!options.desktop || !visible() || disposed) return Promise.resolve();
    if (!preserveSelection) selectionChangedByUser = false;
    setPending(true);
    if (
      mode() === "all" &&
      !value.trim() &&
      (displayedQuery?.mode !== "all" || displayedQuery.value.trim())
    )
      setResults([]);
    setNotice(undefined);
    return searches.submit({ value, mode: mode(), preserveSelection });
  }

  function changeQuery(value: string) {
    setQuery(value);
    setError(undefined);
    void search(value);
  }

  return {
    visible,
    setVisible,
    query,
    setQuery,
    mode,
    setMode,
    results,
    selected,
    setSelected,
    current,
    pending,
    total,
    indexing,
    setIndexing,
    files,
    setFiles,
    currency,
    error,
    setError,
    indexError,
    storageError,
    notice,
    setNotice,
    search,
    changeQuery,
    markSelectionChanged() {
      selectionChangedByUser = true;
    },
    hidden() {
      setVisible(false);
      // Invalidate delivery immediately and ask Rust to stop cooperatively.
      searches.cancel();
      setPending(false);
    },
    dispose() {
      disposed = true;
      searches.dispose();
    },
  };
}
