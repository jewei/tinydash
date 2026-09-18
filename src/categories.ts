import type { SearchMode, SearchResult } from "./bridge";

export const resultCategories: Record<SearchResult["kind"], SearchMode> = {
  app: "apps",
  file: "files",
  clipboard: "clipboard",
  emoji: "emoji",
  calculation: "calculator",
  systemCommand: "system",
  password: "password",
  timezone: "timezone",
  cleanedUrl: "url",
  webSearch: "web",
};

export interface PinOption {
  category: SearchMode;
  label: string;
  pinned: boolean;
}

export const categories = [
  { id: "all", label: "All" },
  { id: "apps", label: "Apps" },
  { id: "files", label: "Files" },
  { id: "clipboard", label: "Clipboard" },
  { id: "emoji", label: "Emoji" },
  { id: "calculator", label: "Calculator" },
  { id: "system", label: "System" },
  { id: "password", label: "Passwords" },
  { id: "timezone", label: "Time zones" },
  { id: "url", label: "URLs" },
  { id: "web", label: "Web" },
] as const satisfies readonly { id: SearchMode; label: string }[];

export const defaultCategories: SearchMode[] = categories.map(({ id }) => id);

// Keep one stable display order and support settings saved before this field existed.
export function normalizeCategories(value: unknown): SearchMode[] {
  const selected = Array.isArray(value)
    ? defaultCategories.filter((id) => value.includes(id))
    : [];
  return selected.length ? selected : [...defaultCategories];
}
