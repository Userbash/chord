# Validation and practical limits

Latest full worker gate, 2026-11-15 16:47 UTC: **failed only in JS unit** (**2614 passed / 34 failed / 0 skipped**). Typechecks, meta **210/210**, GUI-spike Rust, workspace **1490 passed / 12 ignored**, GPUI smoke, GPUI E2E contracts **55/55** and GPUI visual contracts **10/10** passed. Smoke recorded **19 passed / 3 skipped**. Full report: [GPUI-enabled gate](verification/sequential-worker-gpui-enabled-20261115T164751333Z.json).

The [genuine Linux release binary](SEQUENTIAL-03-GPUI-BUILD.md) also passes headless startup on the host and in digest-pinned Ubuntu 22.04 with GLIBC 2.35. Native windows/media/ARM and release packages remain unqualified. No application source or dependency locks changed in this checkpoint.

Latest test-repair checkpoint: [sequential step 2](SEQUENTIAL-02-UNIT-REPAIRS.md)
resolves five failures while preserving independent protocol/palette assertions.
The observed full JS unit run is 2614 pass / 34 fail / 0 skip, with no new failed
names. Source-only Windows checks, clean-checkout fixture proof and reviews are
reported separately from native UI/voice/ARM qualification.

Latest environment checkpoint: [sequential step 1](SEQUENTIAL-01-DEPENDENCIES.md)
restored the exact locked Linux GUI dependencies. Its 748 offline Rust tests pass;
the lockfile is unchanged. The first rerun passed `spike-rust` with 39 JS
unit failures; the later five-case repair above leaves 34 failures. Earlier offline-cache failures below are historical.

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

CPU instruction clarification: [contract and failed control](CPU-ISA-CONTRACT.md). Host std detection and masked-feature headless execution are recorded separately from instruction enforcement and native qualification. The AVX2 negative diagnostic unexpectedly exited0 instead of132, so the emulator does not certify the CPU floor. No production source/lock changes were made.

Final metadata check after CPU/board annotations: **210/210**, no skips. The [independent CPU requirements review](reports/CPU-ISA-INDEPENDENT-REVIEW.json) is a read-only collaboration review, not an additional ECC session. The reviewed five-case test repair is commit `ef9c819642ec9811333b29dea13eb0c96cd628d0`.
