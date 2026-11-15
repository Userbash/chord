# ARM source batch — ready for product review

The user selected backlog work on 2026-11-15. Two bounded source slices are implemented: `LIB-ARM-02` portable CPU inputs and `LIB-ARM-08` ELF integrity/metadata. The dependent source-test subtask `LIB-ARM-06.I1` reconciles the current configuration contract. Parent cards and native ARM release qualification remain open. No branch was merged or published by this batch.

The working branch includes the two upstream release commits through `4653bdec`: chat plugin and reactive signals. Planning's measured baseline remains `2b7cfb52`; immutable earlier audit packets retain their original revisions. CPU and ELF work used disjoint source leases; the coordinator retained shared metadata and ARM release-qualification ownership.

| Slice | Implemented behavior | Remaining acceptance |
| --- | --- | --- |
| LIB-ARM-02 | Generic C/C++ uses ARMv8-A; Rust and optimized CPU profiles avoid blanket optional CRC/AES/SHA2 overrides. Real build/hardening behavior is retained. | Dependency feature/build-script audit, genuine target build and native CPU evidence. |
| LIB-ARM-08 | Check load/header bounds, sizes, address overflow, power-of-two alignment, minimum page and declared alignment congruence, interpreter and ordering. Report segment alignment separately from an unknown kernel page size. | Genuine application/package and native loader/page/GUI/media evidence. |
| LIB-ARM-06.I1 | Current SOURCE CPU assertions follow the portable contract; historical DIST keeps its original four feature assertions and immutable bytes. | Full legacy real-build/package contract, host-independent architecture fixture and parent prerequisites. |
| Strict test typing | Fix typed profile tuples and structural Kanban/plan/receipt/helper types; add explicit missing-card guards. | No test suppression or relaxed tsconfig. |

The code continues to use existing Python standard-library and installed linker/readelf facilities for this small packaging boundary. No framework, Rust dependency, SDK, generated client or new runtime dispatch wrapper was introduced. This batch corrects defects; it does not claim a Rust application LOC reduction or whole-application functional qualification.

| Verification | Final observed result |
| --- | --- |
| CPU + ELF + retained integrity tests | 39 passed, 0 failed, 0 skipped on x86_64 Linux. |
| Initial failing contracts | CPU: 4 expected failures; ELF: 9; reviewer runtime-alignment case: 1; legacy source CPU contract: 1. Immutable earlier logs retained. |
| Behavior mutations | CPU 5/5; final ELF 6/6; legacy source CPU contract 5/5 killed; each unmutated control passed. |
| Full typecheck and UI typecheck | Passed after strict typing fixes. |
| Full metadata suite | 210 passed, 0 failed, 0 skipped. |
| Full JS unit suite | 2648 tests: 2609 passed, 39 failed, 0 skipped. Comparable upstream: 2621 tests, 2582 passed, 39 failed. |
| Rust workspace | 1490 passed, 0 failed; 12 expected live tests ignored, separately reported. |
| Smoke | 19 passed, 0 failed; 3 explicit environment skips, not counted as passing. |
| Final worker gate | **Failed**: unit failures and offline GUI Rust dependency resolution. |
| Native ARM | Not run; release remains unqualified. |

The full JS unit baseline comparison initially found one new failure: legacy `SUB-990-1` demanded the removed optional CPU flags. CP-009 in `DECISIONS.md` documents the independently justified source/historical distinction; its focused test and mutation evidence now pass. Other failed package, binary, source-contract and system checks remain visible. Their exact names and log hashes are in the baseline comparison artifact. The GUI Rust phase cannot resolve locked `jiff 0.2.44` from the available offline index, which contains `0.2.38`; no lockfile or gate was weakened to bypass that error.

Two real ECC Codex source-review sessions completed: `4d808efd`, then `d4137071` after independent review found and reproduced a runtime congruence gap. The first review's model reported no findings; the independent reviewer still found the P2, and its failing reproduction drove the fix. The final source review approved the corrected packet. A later type-only CPU tuple annotation has its own hash-bound review supplement. These are real read-only source reviews, not native runs or full-card acceptance.

The genuine CPU implementation worker, coordinator ELF implementation, model receipts, final file hashes, independent review and raw command records are listed in [ARM-SOURCE-BATCH.json](ARM-SOURCE-BATCH.json) and [DISPATCH-RECEIPTS.json](DISPATCH-RECEIPTS.json). Verification logs are archived losslessly with original byte hashes. Earlier failed gate/transport/mutation-harness attempts remain evidence rather than being overwritten.

The Kanban parent cards are **Ready for Product Review**, not complete. Remaining production library admission, full batch acceptance and native target evidence continue to block ARM release. Reverting a slice is available for diagnosis; it cannot authorize an incompatible or simulated release artifact.
