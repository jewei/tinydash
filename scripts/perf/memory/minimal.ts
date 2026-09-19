import { memorySearch } from "./memory-hooks";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

const state = window as unknown as {
  __memoryMinimal: boolean;
  __memoryReady: boolean;
};
state.__memoryMinimal = true;
const search = () => memorySearch("", "all");
await Promise.all([
  listen("launcher-opened", () => {
    void search();
  }),
  ...[
    "apps-changed",
    "files-changed",
    "clipboard-changed",
    "currency-changed",
  ].map((name) =>
    listen(name, () => {
      void search();
    }),
  ),
]);
await invoke("launcher_ready");
await search();
state.__memoryReady = true;
document.body.dataset.page = "minimal";
