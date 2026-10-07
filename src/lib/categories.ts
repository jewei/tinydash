import type { Category } from "../generated/Category";

/** The launcher's tab names, in the default order. */
export const CATEGORY_LABELS: Record<Category, string> = {
  all: "All",
  apps: "Apps",
  files: "Files",
  clipboard: "Clipboard",
  snippets: "Snippets",
  emoji: "Emoji",
  system: "System",
};

/** The tabs after All that Settings can show, hide, and order; All is always first. */
export const OPTIONAL_TABS = (Object.keys(CATEGORY_LABELS) as Category[]).filter(
  (tab) => tab !== "all",
);
