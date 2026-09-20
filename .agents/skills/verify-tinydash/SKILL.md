---
name: verify-tinydash
description: Verify and repair TinyDash changes with focused browser tests, Rust checks, and identified desktop builds. Use during development and before reporting a change complete, or when auditing the verification procedure. Browser tests mock IPC; desktop proof needs a controlled platform session.
---

# Verify TinyDash

Read [the verification procedure](../../../docs/how-to/verify.md) and the affected pages in [the feature map](../../../docs/reference/features/README.md). Keep feature recipes there. Do not create another feature catalog in this skill.

## Select the proof

Use the task, changed source, and feature pages to state observable acceptance criteria. Include affected entry points, error paths, persistence, and OS effects. Existing tests are evidence only for behavior they exercise. Add or adapt a regression test when a behavior change has no suitable proof. For a bug, reproduce the failure before the repair when possible.

Keep a short acceptance record under `.local/` or `test-results/`: criterion, required platform, command or user actions, expected UI and external result, outcome, and evidence path. Distinguish passed, failed, and not run. A missing required platform is a blocker for that proof.

Choose the required level for each criterion. Browser tests prove SolidJS behavior against mocked IPC. Rust tests prove backend rules. Desktop checks prove the integrated application and OS effects. A pass at one level cannot replace a required check at another level.

## Run and repair

1. Run `bun run verify` for initial feedback. Use `bun run verify:browser tests/<feature>.spec.ts` for focused browser proof with retained successful traces. Use the feature's focused Rust command while repairing backend behavior. Each wrapper run has its own evidence directory and browser server.
2. For desktop proof, follow the guide's local or CI route. Identify the tested build and test source separately. Rebuild after relevant source edits. Never use an older package as proof of a new local change. Quit or isolate an existing app before testing; do not attach to a personal instance. Native checks change clipboard contents and require a disposable Windows profile.
3. Drive the required user entry points. Use accessible names such as `Search TinyDash` and `Search categories`. Check the action, the resulting UI state, and external effects. Application launch needs the selected fixture's output marker. Copy needs the exact system clipboard value. File watching needs creation, rename, and deletion without manual refresh. Internal setters or direct IPC can support a backend check but cannot replace the user path. Keep power operations canceled.
4. On failure, retain the evidence and classify the cause as product, test tool, or environment. Repair within the authorized development task. Fix missing test coverage instead of accepting a mock as desktop proof. Do not weaken an assertion, change the expected behavior, or skip a required test to obtain a pass.
5. Before retrying, inspect the failed command and build identity. Recheck readiness and reset fixture state or restart the owned instance after surprising behavior. Confirm cleanup before another native run. Do not delete a retained lock until its owned processes and recovery state have been checked. Each retry needs a repair, a reset, or a new diagnostic step. Continue while useful diagnosis or repair remains possible.

After the final relevant edit, repeat the affected proofs and run `bun run verify:full` for source changes. Repeat required native checks against the resulting build. Documentation-only changes need repository and formatting checks; changes to verification instructions also need the exercise below. Do not repeat a passing check without a change or unresolved concern that invalidates it.

## Finish with evidence

Inspect command logs, `result.json`, and the relevant browser traces or native records. Retain a successful trace for each changed browser journey. Evidence must identify the final source, executable when applicable, platform, commands, outcomes, and coverage limits. Check that proof files still exist after cleanup. A wrapper pass reports its commands; use the acceptance record to decide whether the task is complete.

Report **verified** only when all required criteria pass for the final source and identified build, evidence remains available, and cleanup is complete. State the features and platforms covered. A skipped check or unsupported platform is not a pass. If access, credentials, desktop capabilities, or a decision outside the task prevents completion, report the exact blocker, attempts, evidence, and behavior still unverified. Do not return a repairable failure merely because the first check failed.

## Maintain and test this skill

During ordinary development, update the affected feature recipes. During a requested maintenance audit, check every feature page against source and exercise every feature live at its required level. Separate documentation drift, test-tool gaps, and product regressions. Repair documentation and test tools within the audit; report product regressions unless product repair is also authorized. Do not change the documented behavior to hide a regression.

After a substantial skill or runner change, follow [the verification exercises](../../../docs/how-to/verify-verification.md). Use an isolated checkout with a realistic task. Check actual agent actions and evidence, including recovery from a failed attempt. Static Markdown validation alone does not prove that the agent can use the skill.
