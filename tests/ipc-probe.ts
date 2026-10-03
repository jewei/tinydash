import { mockIPC, clearMocks } from "@tauri-apps/api/mocks";
import { backend, type SettingsValues } from "../src/bridge";
import { richClipboardBackend } from "../src/clipboardBridge";
import { fileBackend } from "../src/file-actions-bridge";
import { libraryBackend } from "../src/library-bridge";
import { utilities } from "../src/utilities-bridge";

export const ipcBridges = {
  ...backend,
  richClipboardHistory: richClipboardBackend.history,
  richClipboardPreview: richClipboardBackend.preview,
  copyRichClipboard: richClipboardBackend.copy,
  deleteRichClipboard: richClipboardBackend.delete,
  filePreview: fileBackend.preview,
  executeFileAction: fileBackend.execute,
  libraryList: libraryBackend.list,
  libraryGet: libraryBackend.get,
  librarySave: libraryBackend.save,
  libraryDelete: libraryBackend.delete,
  libraryExecute: libraryBackend.execute,
  utilityCapabilities: utilities.capabilities,
  utilityProcesses: utilities.processes,
  utilityPrepareProcess: utilities.prepareProcess,
  utilityPrepareApp: utilities.prepareApp,
  utilityConfirmProcess: utilities.confirmProcess,
  utilityCancelProcess: utilities.cancelProcess,
  utilityColor: utilities.color,
  utilityCopyColor: utilities.copyColor,
  utilityEyedropper: utilities.sampleScreen,
  utilityAwakeStatus: utilities.awakeStatus,
  utilitySetAwake: utilities.setAwake,
  utilityMedia: utilities.media,
  utilityCaptureWindow: utilities.captureWindow,
  utilityWindow: utilities.window,
};

/** Exercise the real bridge wrappers without starting the native application. */
export async function probeCommands(settings: SettingsValues) {
  let wrapper: keyof typeof ipcBridges;
  const calls: {
    wrapper: keyof typeof ipcBridges;
    command: string;
    args: Record<string, unknown>;
  }[] = [];
  mockIPC((command, args) => {
    calls.push({
      wrapper,
      command,
      args: (args ?? {}) as Record<string, unknown>,
    });
    if (command === "utility_eyedropper") {
      return {
        hex: "#ffffff",
        rgb: "rgb(255, 255, 255)",
        hsl: "hsl(0, 0%, 100%)",
        alpha: 1,
      };
    }
  });
  const probes = {
    setLauncherAppearance: () => backend.setLauncherAppearance("dark"),
    syncAppearance: () =>
      backend.syncAppearance({ kind: "appearance", value: "dark" }),
    ready: backend.ready,
    openSettings: backend.openSettings,
    settings: backend.settings,
    saveSettings: () => backend.saveSettings(settings),
    chooseClipboardHistory: () => backend.chooseClipboardHistory(true),
    appCatalog: backend.appCatalog,
    itemCatalog: backend.itemCatalog,
    paste: () => backend.paste("clipboard:1"),
    pasteQueue: () => backend.pasteQueue("start", ["clipboard:1"]),
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
    search: () => backend.search("fixture", "all", 1),
    cancelSearch: () => backend.cancelSearch(1),
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
    richClipboardHistory: richClipboardBackend.history,
    richClipboardPreview: () => richClipboardBackend.preview(1),
    copyRichClipboard: () => richClipboardBackend.copy(1),
    deleteRichClipboard: () => richClipboardBackend.delete(1),
    filePreview: () => fileBackend.preview("file:fixture"),
    executeFileAction: () =>
      fileBackend.execute("file:fixture", "openWith", {
        appId: "app:example",
        confirmed: true,
      }),
    libraryList: () => libraryBackend.list("fixture"),
    libraryGet: () => libraryBackend.get("library:fixture"),
    librarySave: () =>
      libraryBackend.save(null, {
        kind: "snippet",
        name: "Fixture",
        keywords: "example",
        content: "Synthetic text",
      }),
    libraryDelete: () => libraryBackend.delete("library:fixture"),
    libraryExecute: () =>
      libraryBackend.execute(
        "library:fixture",
        "copy",
        { query: "fixture" },
        false,
      ),
    utilityCapabilities: utilities.capabilities,
    utilityProcesses: utilities.processes,
    utilityPrepareProcess: () =>
      utilities.prepareProcess(
        { pid: 123, identity: "fixture", name: "Fixture" },
        false,
      ),
    utilityPrepareApp: () => utilities.prepareApp("app:example", false),
    utilityConfirmProcess: () => utilities.confirmProcess("fixture-token"),
    utilityCancelProcess: utilities.cancelProcess,
    utilityColor: () => utilities.color("#ffffff"),
    utilityCopyColor: () => utilities.copyColor("#ffffff", "hex"),
    utilityEyedropper: () =>
      utilities.sampleScreen(new AbortController().signal, true),
    utilityAwakeStatus: utilities.awakeStatus,
    utilitySetAwake: () => utilities.setAwake(1),
    utilityMedia: () => utilities.media("playPause"),
    utilityCaptureWindow: () => utilities.captureWindow(true),
    utilityWindow: () => utilities.window("center"),
  } satisfies Record<keyof typeof ipcBridges, () => Promise<unknown>>;
  try {
    for (const [name, probe] of Object.entries(probes)) {
      wrapper = name as keyof typeof ipcBridges;
      await probe();
    }
    wrapper = "execute";
    await backend.execute("app:example", "launch");
    wrapper = "appCatalog";
    await fileBackend.apps();
    return calls;
  } finally {
    clearMocks();
  }
}
