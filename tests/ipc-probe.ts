import { mockIPC, clearMocks } from "@tauri-apps/api/mocks";
import { backend, type SettingsValues } from "../src/bridge";

/** Exercise the real bridge wrappers without starting the native application. */
export async function probeCommands(settings: SettingsValues) {
  const calls: { command: string; args: Record<string, unknown> }[] = [];
  mockIPC((command, args) => {
    calls.push({ command, args: (args ?? {}) as Record<string, unknown> });
  });
  const probes = {
    setLauncherAppearance: () => backend.setLauncherAppearance("dark"),
    ready: backend.ready,
    openSettings: backend.openSettings,
    settings: backend.settings,
    saveSettings: () => backend.saveSettings(settings),
    chooseClipboardHistory: () => backend.chooseClipboardHistory(true),
    appCatalog: backend.appCatalog,
    setAppPreference: () =>
      backend.setAppPreference("app:example", ["alias"], true),
    previewWebSearch: () =>
      backend.previewWebSearch(settings.webSearches[0], "query"),
    exportSettings: () => backend.exportSettings("dark", true, false),
    importSettings: backend.importSettings,
    revealBackup: backend.revealBackup,
    checkUpdate: backend.checkUpdate,
    installUpdate: backend.installUpdate,
    recordShortcut: () => backend.recordShortcut(true),
    revealSettings: () => backend.revealSettings(true),
    search: () => backend.search("fixture", "all"),
    setPinned: () => backend.setPinned("app:example", "apps", true),
    execute: () => backend.execute("app:example", "launch", true),
    clipboardPreview: () => backend.clipboardPreview("clipboard:1"),
    clearClipboard: () => backend.clearClipboard(true),
    editClipboardCopy: () => backend.editClipboardCopy("clipboard:1", "text"),
    copyClipboardSelection: () =>
      backend.copyClipboardSelection(["clipboard:1"], "\n"),
    saveClipboardFile: () => backend.saveClipboardFile("clipboard:1"),
    hide: backend.hide,
    resetPosition: backend.resetPosition,
    refresh: backend.refresh,
    refreshFiles: backend.refreshFiles,
    refreshCurrency: backend.refreshCurrency,
    quit: backend.quit,
  } satisfies Record<keyof typeof backend, () => Promise<unknown>>;
  try {
    for (const probe of Object.values(probes)) await probe();
    await backend.execute("app:example", "launch");
    return calls;
  } finally {
    clearMocks();
  }
}
