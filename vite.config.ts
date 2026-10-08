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
      input: { launcher: "index.html", settings: "settings.html", hud: "hud.html" },
    },
  },
  test: {
    environment: "jsdom",
    include: ["src/**/*.test.{ts,tsx}"],
    setupFiles: ["src/test/setup.ts"],
    restoreMocks: true,
  },
  lint: {
    ignorePatterns: ["dist/**", "src-tauri/**"],
    // A warning fails `vp check`, so it cannot pass verify or CI unseen.
    options: { typeAware: true, typeCheck: true, denyWarnings: true },
    rules: {
      // Solid assigns `let el!: T` through `ref={el}`, which this rule cannot see.
      "no-unassigned-vars": "off",
    },
  },
  fmt: {
    ignorePatterns: ["dist/**", "src-tauri/**", "src/generated/**"],
  },
  staged: {
    "*.{ts,tsx,js,json,css,html,md,yml,yaml}": "vp check --fix",
    "*.rs": "rustfmt --edition 2024",
  },
});
