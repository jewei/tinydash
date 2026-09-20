---
name: verify-tinydash
description: Verify TinyDash changes with repository checks, browser tests, Rust tests, and real desktop checks. Use when validating launcher behavior or a repository change. Browser tests use a mock backend; native checks need a controlled Windows or Linux session.
---

# Verify TinyDash

Read [the verification procedure](../../../docs/how-to/verify.md) and the relevant page in [the feature map](../../../docs/reference/features/README.md). These documents define the commands, supported entry points, expected results, and proof limits.

## Launch and initial check

Run `bun run verify` from the repository root for fast feedback. It creates an isolated browser run, starts Vite on an unused loopback port, and checks readiness before tests start. It never reuses the user's development server.

Run `bun run verify:full` before treating a source change as verified. For native behavior, follow the platform prerequisites and use `bun run verify:native` in a controlled test session. The native wrapper checks for an executable and an existing app process before it changes test state.

When startup fails, inspect the first failed command log, port ownership, and the selected build. Do not drive another running TinyDash instance to bypass the failure. Native tests alter clipboard history; a normal Windows user profile is unsuitable.

## Drive

Use the existing test for the feature. Browser tests use accessible names such as `Search TinyDash`, `Search categories`, and the feature's action buttons. They prove frontend behavior against mocked IPC.

Native tests use the real Rust backend. For an application launch, select a temporary app fixture through the launcher and check its output marker. For calculator or emoji copying, check the displayed result and the system clipboard. For file watching, create, rename, and delete files in the fixture root and check the visible results.

Use every entry point required by the feature page for the change under review. Do not replace user actions with internal setters as proof of the whole feature. Keep power operations canceled during automated checks.

## Evidence and cleanup

Report the tested commit and working-tree state, commands, result, and coverage limits. Wrapper output gives the evidence directory under `test-results/verification/`. Review its command logs, result JSON, and browser traces or native records.

Cleanup must stop only processes started by the test and preserve evidence. Confirm that the result file and relevant proof still exist after cleanup. An unsupported native platform is a coverage gap, not a passing test. macOS currently uses the documented desktop checks.

Keep the feature map current. Do not create another feature catalog in this skill.
