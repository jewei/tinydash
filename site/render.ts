import {
  formatSize,
  platforms,
  validateRelease,
  type Release,
} from "./downloads";

const escapeHtml = (value: string) =>
  value.replace(
    /[&<>"']/g,
    (character) =>
      ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[
        character
      ]!,
  );

/** Render verified downloads at build time. JavaScript only suggests an OS. */
export function renderPage(template: string, value: Release): string {
  const release = validateRelease(value);
  const replacements: Record<string, string> = {
    primaryDetail: release.version
      ? `Version ${escapeHtml(release.version)} for macOS, Windows, and Ubuntu.`
      : "Planned for macOS, Windows, and Ubuntu.",
    releaseStatus: release.version
      ? `Version ${escapeHtml(release.version)}. Choose the installer for your computer.`
      : "The first public release is in preparation. Installers are not available yet.",
    releaseNotesUrl: escapeHtml(release.releaseUrl),
  };
  const labels = { macos: "macOS", windows: "Windows", linux: "Ubuntu" };
  for (const platform of platforms) {
    const asset = release.downloads[platform];
    replacements[`${platform}Detail`] = asset
      ? `v${escapeHtml(release.version!)} · ${formatSize(asset.bytes)} · ${escapeHtml(asset.requirement)}`
      : "";
    const link = asset
      ? `<a class="button installer" data-download href="${escapeHtml(asset.url)}">Download for ${labels[platform]}</a>`
      : '<a class="button installer" data-download aria-disabled="true" tabindex="-1">Not released yet</a>';
    replacements[`${platform}Download`] =
      `${link}<details class="checksum"${asset ? "" : " hidden"}><summary>SHA-256 checksum</summary><code data-checksum>${asset?.sha256 ?? ""}</code></details>`;
  }
  return template.replace(/\{\{(\w+)\}\}/g, (_, key: string) => {
    if (!(key in replacements))
      throw new Error(`Unknown website field: ${key}`);
    return replacements[key];
  });
}
