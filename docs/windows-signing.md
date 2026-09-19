# Windows signing options

Status: resumed on 19 September 2026. The maintainer accepted an unsigned
Windows preview for the first release. Publisher signing is deferred.
See [release setup](release-setup.md) for the active configuration.

TinyDash is a small open source project maintained by one individual in
Malaysia. The maintainer excludes services that cost more than $10. Use free
services as the default. No subscription or purchase is authorized.

The earlier SSL.com recommendation is withdrawn because it exceeds the budget.
Microsoft Artifact Signing does not accept individual publishers in Malaysia
under its current [eligibility rules](https://learn.microsoft.com/en-us/azure/artifact-signing/quickstart).
A free Microsoft Store account is a different option.

## Free Windows options

| Option                    | What it provides                                                 | Limit                                                                                              |
| ------------------------- | ---------------------------------------------------------------- | -------------------------------------------------------------------------------------------------- |
| SignPath Foundation       | Publicly trusted signing for accepted open source projects.      | Project approval is required. Its terms require a prior release and verifiable reputation.         |
| Microsoft Store with MSIX | Microsoft signs the accepted package and provides Store updates. | TinyDash needs MSIX packaging, compatibility tests, and Store review.                              |
| Unsigned Windows preview  | Uses the existing NSIS package and costs no signing fee.         | Users can see warnings. Smart App Control or organization policy can block execution.              |
| Self-signed certificate   | A signature for development and controlled testing.              | Windows does not trust our certificate by default. It does not solve public installation warnings. |

SignPath requires an OSI-approved license, an identifiable maintainer,
documented functionality, and a signing policy. Its certificate names
SignPath Foundation as publisher. TinyDash's eligibility is unverified.
See the [terms](https://signpath.org/terms) and [application page](https://signpath.org/apply).

Microsoft's new [individual account registration](https://learn.microsoft.com/en-us/windows/apps/publish/whats-new-individual-developer)
is free through [storedeveloper.microsoft.com](https://storedeveloper.microsoft.com/).
The Store signs MSIX submissions. It does not provide this signing service for
our existing NSIS EXE submission. See Microsoft's
[distribution comparison](https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/choose-distribution-path).
TinyDash's Store compatibility and account verification remain untested.

Unsigned downloads and self-signed files have trust limits. Do not instruct
public users to install our own root certificate or disable Windows security.
See Microsoft's [signing options](https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/code-signing-options).

## Cloudflare and hosting

Cloudflare Pages can host the static download page on its free plan.
Each file must be at most 25 MiB. Link the installer buttons to GitHub Releases.
See the [Pages limits](https://developers.cloudflare.com/pages/platform/limits/)
and [GitHub release limits](https://docs.github.com/en/repositories/releasing-projects-on-github/about-releases).

Cloudflare's [free HTTPS certificates](https://developers.cloudflare.com/ssl/)
protect website traffic. They do not provide Windows Authenticode publisher
signatures. No public Windows code-signing service was found in Cloudflare's
documentation.

R2 has a [free allowance and charges above it](https://developers.cloudflare.com/r2/pricing/).
It adds billing and storage setup that this release does not need.
[GitHub Pages](https://docs.github.com/en/pages/getting-started-with-github-pages)
is also free for a public repository and fits the existing site.

## What we can do ourselves

Build the installers, publish release notes and SHA-256 hashes, and host them
on GitHub. Tauri also lets us generate an update-signing key locally, without
a paid service. The app checks downloaded updates against its embedded public
key. See [Tauri update signing](https://v2.tauri.app/plugin/updater/#signing-updates).

These update signatures and hashes do not establish a trusted Windows publisher
for the first installation. Keep that distinction clear on the download page.

## Selected route

Keep GitHub Releases and the existing static site. Use GitHub Pages or
Cloudflare Pages with its supplied domain. Publish the existing NSIS installer
as an unsigned Windows preview. Add SHA-256 checksums and keep Tauri update
signatures. The workflow no longer imports a Windows PFX or requires Windows
publisher credentials. No signing service or Store conversion is needed now.

State the warning beside the Windows download. Test installation and launch
on Windows 11 with a standard user account. Record the exact trust prompt.
Systems that block unsigned software are outside preview support. Do not call
a blocked installation a pass or ask users to disable security controls.
The combined Mac, Windows, and Ubuntu release scope remains unchanged.

Keep the completed backup and site work. Reuse the existing CI. Retain the
basic install, launch, update, data-preservation, and removal checks.
There is no tester quota. Keep one report per supported system. Revisit
SignPath if project eligibility improves, or the Store if users request it.
