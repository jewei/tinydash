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
    merged.appPreferences = mergeDraft(
      previous.appPreferences,
      current.appPreferences,
      next.appPreferences,
    );
    for (const [id, preference] of Object.entries(current.appPreferences)) {
      if (next.appPreferences[id]) {
        merged.appPreferences[id] = mergeDraft(
          previous.appPreferences[id] ?? { aliases: [], hidden: false },
          preference,
          next.appPreferences[id],
        );
      }
    }
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
