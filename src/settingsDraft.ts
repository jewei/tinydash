import type { SettingsValues } from "./bridge";
import { normalizeCategories } from "./categories";

export type FolderMode = "default" | "custom" | "off";
export const folderModeFor = (value: SettingsValues): FolderMode =>
  value.fileSearchRoots === null
    ? "default"
    : value.fileSearchRoots.length
      ? "custom"
      : "off";

const equal = (a: unknown, b: unknown) =>
  JSON.stringify(a) === JSON.stringify(b);

/** Incoming saved values win only where the local draft is unchanged. */
export function mergeDraft<T extends object>(
  previous: T,
  draft: T,
  incoming: T,
): T {
  const merged = { ...incoming };
  const keys = new Set([...Object.keys(previous), ...Object.keys(draft)]);
  for (const key of keys as Set<keyof T>) {
    if (equal(draft[key], previous[key])) continue;
    if (Object.hasOwn(draft, key)) merged[key] = draft[key];
    else delete merged[key];
  }
  return merged;
}

function mergePreferences<T extends object>(
  previous: Record<string, T>,
  current: Record<string, T>,
  incoming: Record<string, T>,
  defaults: T,
): Record<string, T> {
  const merged = mergeDraft(previous, current, incoming);
  for (const [id, preference] of Object.entries(current)) {
    // Rust omits default preferences. A remote deletion restores defaults
    // for unchanged fields while preserving locally edited fields.
    if (incoming[id] || !equal(preference, previous[id])) {
      merged[id] = mergeDraft(
        previous[id] ?? defaults,
        preference,
        incoming[id] ?? defaults,
      );
    }
  }
  return merged;
}

export function mergeSettingsDraft(
  previous: SettingsValues | undefined,
  current: SettingsValues | undefined,
  settings: SettingsValues,
  folderMode: FolderMode,
) {
  const next = {
    ...settings,
    visibleCategories: normalizeCategories(settings.visibleCategories),
  };
  const merged =
    previous && current ? mergeDraft(previous, current, next) : { ...next };
  if (previous && current) {
    merged.appPreferences = mergePreferences(
      previous.appPreferences,
      current.appPreferences,
      next.appPreferences,
      { aliases: [], hidden: false },
    );
    merged.itemPreferences = mergePreferences(
      previous.itemPreferences,
      current.itemPreferences,
      next.itemPreferences,
      { aliases: [], shortcut: "", hidden: false, disabled: false },
    );
  }
  return {
    saved: next,
    draft: merged,
    // Incomplete folder text is UI state and must survive unrelated events.
    updateFolders:
      !previous ||
      !current ||
      (equal(current.fileSearchRoots, previous.fileSearchRoots) &&
        folderMode === folderModeFor(previous)),
    updateExcluded:
      !previous ||
      !current ||
      equal(current.fileSearchExcludedDirs, previous.fileSearchExcludedDirs),
  };
}
