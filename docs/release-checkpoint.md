# Release checkpoint

Date: 19 September 2026.
Status: **Paused at the maintainer's request.** Resume only when requested.

This checkpoint records this release task. Another coding task is active in the
same workspace. Its work can continue independently.

## Project limits

- TinyDash is a small open source project with one maintainer.
- The publisher is an individual based in Malaysia.
- Exclude services that cost more than $10. Use free services as the default.
  The billing period was not specified. Do not assume a recurring charge is
  acceptable. No purchase or subscription is authorized.
- Keep the process small. Reuse the existing build tools, CI, and static site.
- Mac signing credentials are available according to the maintainer. Actual
  signed installer verification is still pending.
- The requested first release still includes macOS, Windows, and Linux.

The previous paid Windows signing recommendation is withdrawn. The detailed
release plan predates these limits. Revise it before resuming implementation.
Do not add signing infrastructure, storage services, or Store packaging during
this pause.

## Work saved

The working tree is based on commit
`bccaa245284cc22a7bc0c96d3083dace3a1950ca`. Changes remain uncommitted.

The local source archive is
`artifacts/checkpoints/release-2026-09-19.tar.gz`. It contains tracked files and
non-ignored new files from the shared working tree, plus Git state metadata.
It includes work from both active tasks. It excludes Git history, dependencies,
build outputs, ignored files, and local credentials. It is a local copy, not
a commit, release candidate, or remote backup. Extract it into a separate
directory for review. Do not extract it over ongoing work.

Completed work from this release task:

- Database backup before migration, with failure handling.
- The download page, using the supplied Mac screenshot.
- Download verification against reviewed installer bytes.
- A correction to the Windows file-path test assertion.
- Release planning and verification notes.

The last recorded local checks passed: 19 database tests, 25 site tests, the
file-path test on Mac, Rust library Clippy, and site build/type/format checks.
These results belong to the earlier working-tree state. They do not verify a
new source commit, a signed package, or a Windows or Linux installation.

The other task owns the release workflow, updater, and settings changes.
Those changes remain in the workspace. This checkpoint does not claim that
they have completed release verification.

## Current release state

- No public release or live installer links have been verified.
- Windows signing has no selected service or verified certificate.
- The draft workflow still expects a Windows PFX. It needs review after the
  free distribution route is selected.
- The source license remains undecided. MIT was recommended but not selected.
- Product price and the exact certificate publisher name remain undecided.
- Windows and Linux candidate installation checks remain pending.

The [verification record](release-verification.md) keeps the existing evidence.
The [free signing options](windows-signing.md) replace the paid setup guide.

## Small release proposal

Use GitHub Releases for the three installers and checksums. Use the existing
site on GitHub Pages or Cloudflare Pages. Start with the supplied free domain.

Check SignPath Foundation eligibility for free Windows signing. If it cannot
accept this new project, discuss an unsigned Windows preview or Microsoft
Store MSIX packaging. An unsigned preview has installation warnings and can
be blocked by Windows policy. It is not yet approved as a release route.

Keep practical checks on each system: download, install, launch, update or
manual replacement, preserve data, and remove the app. Keep the backup tests.
Reduce the formal tester quota and further automation before restarting work.
The earlier five-testers-per-system rule is part of the plan to simplify,
not a reason to add more infrastructure now.

When work resumes:

1. Agree on the free Windows route and source license.
2. Review the other task's changes and reduce the release plan to the chosen
   route. Preserve completed work.
3. Test one source commit and its actual packages on all three systems.

No service application, external message, purchase, or publication was made
as part of this checkpoint.
