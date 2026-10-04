import solid from "vite-plugin-solid";
import { defineConfig } from "vite-plus";

export default defineConfig({
  plugins: [solid()],
  clearScreen: false,
  server: {
    host: "127.0.0.1",
    port: 1420,
    strictPort: true,
    watch: { ignored: ["**/src-tauri/**"] },
  },
  build: {
    // macOS 12 ships Safari 15; Windows and Linux webviews are newer.
    target: ["es2022", "safari15"],
    rolldownOptions: {
      input: { launcher: "index.html", settings: "settings.html" },
    },
  },
  test: {
    environment: "jsdom",
    include: ["src/**/*.test.{ts,tsx}"],
    restoreMocks: true,
  },
  lint: {
    ignorePatterns: ["dist/**", "src-tauri/**"],
    options: { typeAware: true, typeCheck: true },
  },
  fmt: {
    ignorePatterns: ["dist/**", "src-tauri/**", "src/generated/**"],
  },
  staged: {
    "*.{ts,tsx,js,json,css,html,md,yml,yaml}": "vp check --fix",
    "*.rs": "rustfmt --edition 2024",
  },
});
