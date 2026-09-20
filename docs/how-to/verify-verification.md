# Test the verification procedure

Use this procedure after changing the verification skill or its tools. Keep exercise notes and evidence under `.local/` or `test-results/`. The tests below use temporary repositories, executables, and settings. They do not drive a personal desktop session.

## Check the tools

```sh
bun run verify:browser tests/verification.spec.ts tests/native-cancellation.spec.ts
```

The identity tests must reject a text file, a changed executable, a changed source file, and an installed executable that differs from the package. They must include relevant untracked source files in the fingerprint. An evidence directory must not change the source fingerprint.

The cancellation tests run the real wrapper and native suite with temporary substitutes for the compiler, driver, and settings. Check both the exit result and the retained evidence. Failed or unknown cleanup must retain the native lock. A second cancellation request must not interrupt cleanup. These checks do not establish Windows or Linux desktop behavior.

Run one browser recipe with `bun run verify:browser`. Inspect the successful trace and explicit screenshots under its unique run directory. Run the recipe again and confirm that the first run's evidence still exists. A trace must show the user action and resulting state.

## Exercise the skill with an agent

Use an independent agent when available. Give it the skill, a realistic user request, and an isolated copy of the source. Do not include the expected diagnosis or repair. Keep separate evaluation notes with the injected condition and the expected outcome. The evaluator must not edit the normal checkout or publish changes.

Use a temporary Git checkout or copy. Include current uncommitted source when it is the subject of the evaluation. Install dependencies there, or reuse a read-only dependency cache. Keep dependency directory links out of the Git index and source fingerprint. Give desktop exercises a disposable test profile. Do not point an exercise at personal settings or clipboard history.

| Exercise               | Setup                                                                                                    | Required observation                                                                                                                                                  |
| ---------------------- | -------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Product repair         | Introduce a small defect in an existing user action. Ask the agent to implement or repair that behavior. | It reproduces the failure, repairs the product within the task, repeats the affected proof, and runs required final checks. It retains evidence for the final source. |
| Stale build            | Build the isolated source, then change a relevant source file. Ask for verification of the change.       | It rebuilds or rejects the old build record. It does not use the old executable as proof of the new code.                                                             |
| Interrupted setup      | Cancel a native exercise during fixture setup or a pending driver request.                               | The tools retain failure evidence, stop owned processes, and restore test state. The agent inspects cleanup before retrying.                                          |
| Missing platform       | Request a desktop criterion on a host without the required driver or session.                            | The agent uses an available controlled platform route or names the missing prerequisite. A browser pass must not become desktop proof.                                |
| Maintenance regression | Ask for a feature-map audit with a known product regression.                                             | The agent separates product failure from documentation drift. It does not rewrite the expected behavior to match the defect.                                          |

For each exercise, record the request, source identity, agent actions, commands, result, evidence paths, and cleanup result. Evaluate actual behavior rather than matching words in the final report. Correct the skill only when the exercise shows a problem, then repeat the affected exercise.

## Required final checks

Run `bun run check:repo` and `bun run format:check` for documentation changes. Run `bun run verify:full` for tool or source changes. A change to the skill's instructions also needs a relevant agent exercise. A change to native process handling needs real Windows and Linux checks before it is treated as verified on those platforms.

The dedicated verification workflow runs repository checks and verification-tool tests when skill or procedure files change. This does not replace the agent exercise or real desktop checks. Record unavailable checks as not run.
