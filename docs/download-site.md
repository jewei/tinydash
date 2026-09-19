# Download site

The static site is in [`site/`](../site/). It uses TinyDash's fonts and the supplied Mac screenshot. The desktop build and the site build have separate output folders.

No public release is configured. The default page states that installers are not available yet. It does not contain installer links.

## Preview and verify

Run these commands from the repository root:

```sh
bun scripts/site/build.ts
bun scripts/site/serve.ts
```

Open `http://127.0.0.1:4174/tinydash/`. The local server listens only on the local computer. The `/tinydash/` prefix checks the paths required by a GitHub Pages project site.

Run the site checks:

```sh
bunx --no-install tsc --project site/tsconfig.json
bunx --no-install playwright test --config site/playwright.config.ts
```

The test command starts the build and server if the local preview is not running. CI always starts a new server. It runs separately from the desktop UI tests and writes images and traces to `artifacts/site-tests/`.

The [site check workflow](../.github/workflows/site-checks.yml) runs the type check and browser tests. It uploads the static output and test images as `download-site-preview`. It does not deploy the site.

## Connect a verified stable release

Complete the native checks, external user checks, and stable promotion in the [release verification record](release-verification.md) first. Keep the exact candidate installers that passed those checks.

Use the public tag, the folder containing those installers, and the tested Mac support requirement:

```sh
bun scripts/site/release.ts v0.1.0 artifacts/reviewed-candidate "macOS 26" artifacts/site-release.json
TINYDASH_RELEASE_MANIFEST=artifacts/site-release.json bun scripts/site/build.ts
```

The version, folder, and Mac requirement above are examples. Use the values from the completed candidate record. Do not infer tested Mac support from `minimumSystemVersion` or the browser's user agent.

The preparation script performs these checks:

- The tag is a stable numeric version.
- GitHub exposes that version as its latest stable release.
- The candidate folder contains one installer for each supported system.
- All three public installers are available without authentication.
- The downloaded bytes have the same length and SHA-256 as the reviewed local files.

It refuses draft releases, prereleases, older releases, missing platforms, inaccessible downloads, and changed files. A failed check keeps the previous manifest. It writes `site-release.json` and `site-release.json.evidence.json` only after the download checks pass. Save both with the release evidence.

These checks compare files. They do not replace signing, notarization, installation, or user tests. The candidate directory must contain the files already approved by those checks.

The final site is in `site-dist/`. Upload the contents of this folder to the selected static host after L1 passes. Keep `release.json`, the HTML, JavaScript, CSS, fonts, and screenshot in the same deployment. GitHub Pages remains the proposed host. Hosting setup and public deployment are pending.

## Download behavior

The built HTML contains the verified download links. The links work when JavaScript is off. JavaScript suggests a main download by operating system. It does not infer the Mac processor type. The page always shows Apple silicon or x64 beside the relevant installer.

Mobile, iPad, ChromeOS, and unknown browsers keep the manual download list. The page never starts a download automatically. A failed refresh keeps the verified links in the built HTML. An unconfigured build keeps its download controls unavailable.

The page lists the Windows WebView2 internet requirement and the Ubuntu manual update procedure. It links the installation and removal guide, release notes, source repository, and issue tracker.

The screenshot shows only macOS. Native Windows and Ubuntu images and a short product recording remain required for W4. No other platform image or recording was fabricated.

## Local evidence from 19 September 2026

- Site build and TypeScript check passed.
- All 25 site tests passed, including a new server run with `CI=true`.
- The tests cover platform selection, complete release data, checksum comparison, failure handling, downloads without JavaScript, keyboard focus, and color contrast.
- Images were checked at 320, 375, 414, 768, and 1440 pixels. A 1280 by 800 check verifies that the title and main download action fit without scrolling.
- Publication tests use local test files and simulated GitHub responses. The version, size, and Mac support values in their images are test data.
- Public download verification, hosting, and native installer checks remain pending.
