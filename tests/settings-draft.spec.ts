import { expect, test } from "@playwright/test";
import type { SettingsValues } from "../src/bridge";
import { mergeDraft, mergeSettingsDraft } from "../src/settingsDraft";
import { contracts } from "./fixtures/ipc-contract";

const settings = (): SettingsValues => structuredClone(contracts.settings);

test("merges incoming settings without losing local changes or mutating inputs", () => {
  const saved = settings();
  const draft = settings();
  const incoming = settings();
  draft.shortcut = "Control+Shift+KeyB";
  draft.appPreferences["app:example"].aliases = ["local"];
  incoming.hideOnBlur = false;
  incoming.appPreferences["app:example"].hidden = false;
  incoming.appPreferences["app:new"] = { aliases: [], hidden: true };
  const before = structuredClone({ saved, draft, incoming });
  const result = mergeSettingsDraft(saved, draft, incoming, "custom");
  expect(result.draft.shortcut).toBe(draft.shortcut);
  expect(result.draft.hideOnBlur).toBe(false);
  expect(result.draft.appPreferences).toEqual({
    "app:example": { aliases: ["local"], hidden: false },
    "app:new": { aliases: [], hidden: true },
  });
  expect(result.saved).toEqual(incoming);
  expect({ saved, draft, incoming }).toEqual(before);
});

test("merges local deletions and additions while accepting remote deletions", () => {
  expect(
    mergeDraft({ a: 1, b: 1 }, { b: 2, c: 3 }, { a: 2, b: 3, d: 4 }),
  ).toEqual({ b: 2, c: 3, d: 4 });
  const saved = settings();
  const incoming = settings();
  incoming.appPreferences = {};
  expect(
    mergeSettingsDraft(saved, settings(), incoming, "custom").draft
      .appPreferences,
  ).toEqual({});
  const draft = settings();
  draft.appPreferences = {};
  expect(
    mergeSettingsDraft(saved, draft, settings(), "custom").draft.appPreferences,
  ).toEqual({});
});

test("remote unhide removes the saved preference but preserves dirty aliases", () => {
  const saved = settings();
  saved.appPreferences["app:example"] = { aliases: [], hidden: true };
  const draft = structuredClone(saved);
  draft.appPreferences["app:example"].aliases = ["local"];
  const incoming = structuredClone(saved);
  delete incoming.appPreferences["app:example"];
  const before = structuredClone({ saved, draft, incoming });
  const result = mergeSettingsDraft(saved, draft, incoming, "custom");
  expect(result.draft.appPreferences["app:example"]).toEqual({
    aliases: ["local"],
    hidden: false,
  });
  expect(result.saved.appPreferences).toEqual({});
  expect({ saved, draft, incoming }).toEqual(before);
});

test("preserves incomplete folder mode and locally edited exclusion text", () => {
  const saved = settings();
  saved.fileSearchRoots = null;
  const draft = structuredClone(saved);
  draft.fileSearchExcludedDirs = ["local", "unfinished"];
  const incoming = settings();
  incoming.fileSearchExcludedDirs = ["remote"];
  const result = mergeSettingsDraft(saved, draft, incoming, "custom");
  expect(result.updateFolders).toBe(false);
  expect(result.updateExcluded).toBe(false);
  expect(result.draft.fileSearchExcludedDirs).toEqual(["local", "unfinished"]);
  const untouched = mergeSettingsDraft(saved, saved, incoming, "default");
  expect(untouched.updateFolders).toBe(true);
  expect(untouched.updateExcluded).toBe(true);
  expect(untouched.draft.fileSearchRoots).toEqual(["/example"]);
});

test("item drafts preserve edited aliases while accepting remote shortcut and visibility resets", () => {
  const saved = settings();
  saved.itemPreferences["library:example"] = {
    aliases: [],
    shortcut: "Control+Shift+KeyL",
    hidden: true,
    disabled: true,
  };
  const draft = structuredClone(saved);
  draft.itemPreferences["library:example"].aliases = ["local"];
  const incoming = structuredClone(saved);
  delete incoming.itemPreferences["library:example"];
  incoming.itemPreferences["system:new"] = {
    aliases: ["remote"],
    shortcut: "",
    hidden: false,
    disabled: false,
  };
  const before = structuredClone({ saved, draft, incoming });
  const result = mergeSettingsDraft(saved, draft, incoming, "custom");
  expect(result.draft.itemPreferences).toEqual({
    "library:example": {
      aliases: ["local"],
      shortcut: "",
      hidden: false,
      disabled: false,
    },
    "system:new": incoming.itemPreferences["system:new"],
  });
  expect(result.saved).toEqual(incoming);
  expect({ saved, draft, incoming }).toEqual(before);
});

test("first load normalizes categories and resets text baselines", () => {
  const incoming = settings();
  incoming.visibleCategories = ["apps", "apps", "files"];
  const result = mergeSettingsDraft(undefined, undefined, incoming, "off");
  expect(result.draft.visibleCategories).toEqual(["apps", "files"]);
  expect(result.updateFolders).toBe(true);
  expect(result.updateExcluded).toBe(true);
});
