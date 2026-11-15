# Validation and practical limits

Latest environment checkpoint: [sequential step 1](SEQUENTIAL-01-DEPENDENCIES.md)
restored the exact locked Linux GUI dependencies. Its 748 offline Rust tests pass;
the lockfile is unchanged. The rerun worker gate passes `spike-rust`, with 39 JS
unit failures still open. Earlier offline-cache failures below are historical.

Measured application baseline: `2b7cfb52024be0453e3b9333aba292b2372e610e`. The current branch includes upstream release `4653bdec519bec8cd6719e92d9736e5299c38468` and bounded ARM builder/verifier source fixes. Rust application and dependency changes from upstream were retained; the ARM slice changes no dependency lock. Current execution and gate evidence supersede earlier planning-only observations below: see [ARM-SOURCE-BATCH.md](ARM-SOURCE-BATCH.md).

| Check | Observed result | Scope |
| --- | --- | --- |
| Full `node --test --test-concurrency=1 tests/meta/**/*.test.mjs` | **210 passed, 0 failed, 0 skipped** | Includes explicit ARM blocker/intake/CPU-ABI guards in addition to the original plan/DAG/receipt, board and launcher checks. |
| Kanban process contracts | **8/8 Linux; 8/8 native Windows** | Relocated checkout, literal metadata round trips, update compatibility and truthful header progress. |
| ECC launcher process contracts | **8/8 Linux** | Real exec/PID/exit/argument propagation, explicit override, PATH recursion/alias handling, user-local fallback and exit2. |
| Original Kanban portability/metadata mutations | **5/5 detected** | Original path/metadata/criteria/default preservation changes; original immutable evidence retained. |
| ECC launcher behavioral mutations | **5/5 detected** | Dropped args, lost exec, ignored override, recursion and false-success exit. |
| New board header regression | Expected RED, then GREEN | Replaced an old fixed `61/61 Pass (100%)` badge with actual task/audit counts. No application-test success is inferred from planning data. |
| Authentic ECC runtime smoke | **14/14 checks** | Real native alpha CLI and persisted control-plane/diagnostic sessions. The printf diagnostic is not counted as a model worker. |
| Genuine ECC model sessions | **3 completed planning audits** | Actual Codex threads/model-generated source inspections, gpt-6.1-sol/medium/read-only, isolated clean source worktrees. |
| Worker references | **173 existing paths; 34 findings** | Source audit evidence, not executed application tests or migration approval. |
| Browser verification | Passed | Actual page shows124 tasks/3 ECC audits and card instructions/session report; native Windows HTTP request returned200. |
| Historical records | Preserved | 220 old card objects equal the immutable base; journal updates are append-only; existing golden directories and application release artifacts unchanged. |
| `git diff --check` | Passed | Final source formatting/whitespace check. |

Evidence is under `verification/`; real planning reports are under `reports/`. Source-only worker reports explicitly contain `migration_completed=false` and `tests_executed=[]`.

The first full meta run exposed an existing registry gap for six recently added upstream unit-test files plus the three new meta files. Only their accurate `TDD-PLAN.md` registrations were added/moved into section8; assertions and tests were retained.

The native Windows transport had failed/stopped configuration attempts before the successful Linux Codex transport. Those are retained in the session history. The read-only sandbox stayed enabled and authentication secrets were not copied into the project.

This result does not qualify whole-application functionality, native Windows/macOS/ARM GUI behavior, live Discord/media behavior or actual library migrations. Those have explicit independent-contract, integration and native tasks in the plan. Static board exports report their recorded observation time; live ECC status is available from the documented runtime directory.

ARM follow-up: the unchanged integrity suite passes12/12 on x86_64 WSL, while actual Linux ARM64 Rust std is absent. A fourth real ECC source audit produced7 findings (no tools/build/native execution). Its records are in `reports/LIB-ARM-01.json` and `verification/arm-audit-receipt.json`. The newly added three meta guards were RED before the explicit ARM tasks/admission/release rules existed, then passed; no ARM code fix or native runtime success is inferred.
