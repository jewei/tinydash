// These public guides existed before the documentation reorganization.
const legacyPublicGuides = new Set([
  "docs/install.md",
  "docs/desktop-checks.md",
  "docs/data-recovery.md",
  "docs/search-controls.md",
]);

export function publicDocumentPath(path: string): boolean {
  return (
    !path.startsWith("docs/") ||
    path === "docs/README.md" ||
    /^docs\/(?:tutorials|how-to|reference|explanation|assets)\//.test(path)
  );
}

export function privatePath(path: string): boolean {
  return (
    /^(?:\.local|designs|\.hallmark|site|site-dist|dist|artifacts|test-results|playwright-report|native-build)(?:\/|$)/.test(
      path,
    ) ||
    /(?:^|\/)(?:node_modules|__pycache__|\.DS_Store)(?:\/|$)/.test(path) ||
    /^src-tauri\/(?:target|gen)(?:\/|$)/.test(path) ||
    /^scripts\/(?:site\/|perf\/memory\/|perf\/.*\/(?:target[^/]*|bin|results)\/)/.test(
      path,
    ) ||
    path === ".github/workflows/site-checks.yml" ||
    (!publicDocumentPath(path) && !legacyPublicGuides.has(path))
  );
}
