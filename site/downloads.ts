export type Platform = "macos" | "windows" | "linux";
export interface Download {
  url: string;
  bytes: number;
  sha256: string;
  requirement: string;
}
export interface Release {
  version: string | null;
  releaseUrl: string;
  publishedAt: string | null;
  downloads: Partial<Record<Platform, Download>>;
}
const repository = "https://github.com/jewei/tinydash/releases";
export const platforms: Platform[] = ["macos", "windows", "linux"];
const labels: Record<Platform, string> = {
  macos: "macOS",
  windows: "Windows",
  linux: "Ubuntu",
};
const architectures = { macos: "Apple silicon", windows: "x64", linux: "x64" };

export function validateRelease(value: unknown): Release {
  if (!value || typeof value !== "object")
    throw new Error("Missing release data.");
  const release = value as Release;
  if (
    !release.downloads ||
    typeof release.downloads !== "object" ||
    Array.isArray(release.downloads)
  )
    throw new Error("Missing downloads.");
  if (release.version === null) {
    if (
      Object.keys(release.downloads).length ||
      release.releaseUrl !== repository ||
      release.publishedAt !== null
    )
      throw new Error("An unreleased version cannot have downloads.");
    return release;
  }
  if (
    typeof release.version !== "string" ||
    !/^\d+\.\d+\.\d+$/.test(release.version)
  )
    throw new Error("Use a stable numeric version.");
  if (release.releaseUrl !== `${repository}/tag/v${release.version}`)
    throw new Error("The release URL does not match the version.");
  if (
    typeof release.publishedAt !== "string" ||
    !Number.isFinite(Date.parse(release.publishedAt))
  )
    throw new Error("Missing publication date.");
  if (Object.keys(release.downloads).length !== platforms.length)
    throw new Error("Publish all three platforms together.");
  const suffixes = {
    macos: "_aarch64.dmg",
    windows: "_x64-setup.exe",
    linux: "_amd64.deb",
  };
  for (const platform of platforms) {
    const asset = release.downloads[platform];
    if (!asset || typeof asset !== "object")
      throw new Error(`Missing ${platform} installer.`);
    const prefix = `${repository}/download/v${release.version}/`;
    if (typeof asset.url !== "string" || !asset.url.startsWith(prefix))
      throw new Error(`Invalid ${platform} URL.`);
    const filename = asset.url.slice(prefix.length);
    if (
      !/^[A-Za-z0-9._-]+$/.test(filename) ||
      !filename.endsWith(`_${release.version}${suffixes[platform]}`)
    )
      throw new Error(`Incorrect ${platform} package or version.`);
    if (
      !Number.isSafeInteger(asset.bytes) ||
      asset.bytes <= 0 ||
      !/^[a-f0-9]{64}$/.test(asset.sha256)
    )
      throw new Error(`Missing ${platform} size or checksum.`);
    if (typeof asset.requirement !== "string" || !asset.requirement.trim())
      throw new Error(`Missing ${platform} system requirements.`);
  }
  return release;
}

export function detectPlatform(
  agent: string,
  touchPoints = 0,
): Platform | null {
  if (
    /Android|iPhone|iPad|iPod|Mobile|CrOS/i.test(agent) ||
    (/Macintosh/i.test(agent) && touchPoints > 1)
  )
    return null;
  if (/Windows NT/i.test(agent)) return "windows";
  if (/Macintosh|Mac OS X/i.test(agent)) return "macos";
  if (/Linux/i.test(agent)) return "linux";
  return null;
}

export function formatSize(bytes: number): string {
  return `${(bytes / 1_048_576).toFixed(1)} MB`;
}

export async function loadDownloads(): Promise<void> {
  const status = document.querySelector<HTMLElement>("#release-status")!;
  try {
    const response = await fetch(new URL("./release.json", document.baseURI), {
      cache: "no-cache",
    });
    if (!response.ok) throw new Error("The release list could not be loaded.");
    const release = validateRelease(await response.json());
    if (!release.version) return;
    for (const platform of platforms) {
      const asset = release.downloads[platform]!;
      const row = document.querySelector<HTMLElement>(
        `[data-platform="${platform}"]`,
      )!;
      const link = row.querySelector<HTMLAnchorElement>("[data-download]")!;
      link.href = asset.url;
      link.textContent = `Download for ${labels[platform]}`;
      link.removeAttribute("aria-disabled");
      link.removeAttribute("tabindex");
      row.querySelector<HTMLElement>("[data-detail]")!.textContent =
        `v${release.version} · ${formatSize(asset.bytes)} · ${asset.requirement}`;
      row.querySelector<HTMLElement>("[data-checksum]")!.textContent =
        asset.sha256;
      row.querySelector<HTMLDetailsElement>("details")!.hidden = false;
    }
    document.querySelector<HTMLAnchorElement>("#release-notes")!.href =
      release.releaseUrl;
    status.textContent = `Version ${release.version}. Choose the installer for your computer.`;
    const platform = detectPlatform(
      navigator.userAgent,
      navigator.maxTouchPoints,
    );
    const primary =
      document.querySelector<HTMLAnchorElement>("#primary-download")!;
    const detail = document.querySelector<HTMLElement>("#primary-detail")!;
    if (platform) {
      primary.href = release.downloads[platform]!.url;
      primary.textContent = `Download for ${labels[platform]}`;
      detail.textContent = `${architectures[platform]} · v${release.version} · ${formatSize(release.downloads[platform]!.bytes)} · ${release.downloads[platform]!.requirement}`;
      if (platform === "windows")
        detail.textContent +=
          " · Unsigned preview. Windows may show a warning or block installation.";
    } else {
      detail.textContent =
        "For desktop computers. Choose your operating system below.";
    }
  } catch {
    status.textContent =
      document.querySelectorAll("[data-download][href]").length === 3
        ? "Could not check for a newer release. The listed downloads are still available."
        : "The download list is unavailable. Check GitHub releases, or reload this page.";
  }
}

if (typeof document !== "undefined") void loadDownloads();
