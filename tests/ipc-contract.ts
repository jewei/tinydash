import type {
  Action,
  FileStatus,
  ClipboardEntry,
  LauncherInfo,
  SearchMode,
  SearchResponse,
  SearchResult,
  SettingsImport,
  SettingsInfo,
  SettingsValues,
  UpdateStatus,
} from "../src/bridge";

/** Compile-time consumer of the Rust-serialized examples; no runtime validator/SDK. */
export interface ContractFixture {
  actions: Action[];
  fileStatuses: FileStatus[];
  modes: SearchMode[];
  response: SearchResponse;
  warningResponse: SearchResponse;
  fullResult: SearchResult;
  details: NonNullable<SearchResult["detail"]>[];
  settings: SettingsValues;
  defaults: SettingsValues;
  clipboard: ClipboardEntry;
  usedClipboard: ClipboardEntry;
  imported: SettingsImport;
  importedDefaults: SettingsImport;
  update: UpdateStatus;
  noUpdate: UpdateStatus;
  launcher: LauncherInfo;
  settingsInfo: SettingsInfo;
}
