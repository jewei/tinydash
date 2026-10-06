// Test environment gaps in jsdom, and cleanup between tests.
import { cleanup } from "@solidjs/testing-library";
import { clearMocks } from "@tauri-apps/api/mocks";
import { afterEach } from "vite-plus/test";

window.matchMedia ??= (query: string) =>
  ({
    matches: false,
    media: query,
    addEventListener: () => {},
    removeEventListener: () => {},
  }) as unknown as MediaQueryList;

// jsdom has no layout, so scrolling is a no-op.
Element.prototype.scrollIntoView = () => {};

afterEach(async () => {
  cleanup();
  // Components stop their event listeners asynchronously; let that finish
  // before the mocked event system goes away.
  await new Promise((resolve) => setTimeout(resolve));
  clearMocks();
});
