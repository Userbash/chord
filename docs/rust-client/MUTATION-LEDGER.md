# Mutation Ledger: Injected Faults, Test Kills, and Defect Prevention

Registry of mutation testing runs across the Rust client development waves (`waves/B01`, `waves/RW-17` ... `waves/RW-35`). The goal is not "percentages for vanity metrics", but empirical verification that every critical contractual branch and boundary is guarded by an active, sensitive test assertion. Any surviving mutant is investigated and converted into a regression test or an explicit backlog item.

The source of truth is wave execution logs and reports; summary figures are mirrored in `waves/STATE.md` §1.

## 1. Summary

| Metric | Accepted Baseline (RW-35 = 6/6) | With Local f11/f13 (RW-35 = 8/8) |
|---|---:|---:|
| Planned Mutants | **281** | **283** |
| Caught (`KILLED`, expected targeted test failed) | **278** | **280** |
| Equivalent Mutants (excluded from denominator, documented) | **2** | **2** |
| `INCONCLUSIVE` (uncounted / environmental) | **1** | **1** |
| Catch Rate across Non-Equivalent Mutants | **278/279 = 99.6%** | **280/281 = 99.6%** |

Calculation and Verification Rules:

- `KILLED`: A mutant is considered killed ONLY when the **expected** targeted test fails with a non-zero exit code on a clean baseline. Compilation errors, harness timeouts, environmental skips, or unrelated test failures are classified as `INCONCLUSIVE`.
- An equivalent mutant is excluded from the denominator only with formal justification explaining why no possible input state produces distinct observable behavior.
- After every injected mutant, the codebase is immediately restored and verified via sha256 checksums; a `touch` or `cargo clean -p` follows the restore to invalidate cached compiler artifacts.
- Mutation scripts must guarantee zero dirty workspace state: `try...finally` restore blocks and sha256 verification are mandatory.

## 2. Wave Breakdown

| Wave | Tasks / Domain | Mutants | KILLED | Equivalent | INCONCLUSIVE | Restore |
|---|---|---:|---:|---:|---:|---|
| B01 | P2-10 voice states, P2-11 typing | 16 | 16 | 0 | 0 | sha256 |
| RW-17 | P2-07 read-state | 7 | 7 | 0 | 0 | sha256 |
| RW-18 | P2-09 presence | 13 | 13 | 0 | 0 | sha256 |
| RW-21 | P1-01 ETF, P1-02 zstd, P2-08 settings | 24 | 24 | 0 | 0 | sha256 |
| RW-22 | P3-15 i18n, P4-01 voice signaling, P7-04 API, P8-03 CI | 18 | 18 | 0 | 0 | copy |
| RW-23 | P4-02 UDP, P8-04 SBOM | 22 | 21 | 0 | 1 | sha256 |
| RW-24 | P7-01 i18n full | 20 | 20 | 0 | 0 | sha256 |
| RW-25 | P4-03 SRTP, P7-02 perf, P8-01 updater | 18 | 18 | 0 | 0 | sha256 |
| RW-26 | P4-07 RTCP/stats, P4-11 regions | 19 | 19 | 0 | 0 | sha256 |
| RW-27 | P7-01b list, P6-03 autostart, P6-04 deep links | 22 | 20 | 2 | 0 | sha256 |
| RW-28 | P7-01c relative time | 8 | 8 | 0 | 0 | sha256 |
| RW-29 | P4-04 RTP, P4-12 reconnect, P7-01e timestamps | 23 | 23 | 0 | 0 | sha256 |
| RW-30 | P7-01d duration, P6-02 D-Bus notifications | 18 | 18 | 0 | 0 | sha256 |
| RW-31 | P6-06 power/idle, P2-03 SQLite | 14 | 14 | 0 | 0 | sha256 |
| RW-32 | P6-07 spellcheck, P6-02b hints | 14 | 14 | 0 | 0 | sha256 |
| RW-33 | P1-10 keyring | 7 | 7 | 0 | 0 | sha256 |
| RW-34 | P6-02c abstract D-Bus, P6-02d hints ext | 12 | 12 | 0 | 0 | sha256 |
| RW-35 | P6-06c fd-passing, logind inhibitor | 6 (8 local) | 6 (8) | 0 | 0 | sha256 |
| **Total** | | **281 (283)** | **278 (280)** | **2** | **1** | |

Execution Artifacts:
- `waves/<wave>/run-mutations*.py` — mutation executor; `waves/<wave>/mutations*.log` — execution log.
- Baseline for RW-35 was accepted at 6/6; local additions verified at 8/8.

## 3. Mutant Classification Taxonomy

Classification is by observable functional defect rather than file location:

| Fault Class | Injected Defect | Project Concrete Example | Verification Layer |
|---|---|---|---|
| Boundary & Off-by-One | `>` <-> `>=`, buffer length, array bounds, recursion depth | B02 depth `>` -> `>=`; RW-17 max-guard; RW-23 discovery length; RW-35 fd limit | Unit + Contract |
| Condition Inversion | `==` <-> `!=`, inverted boolean branch, flip option check | B01 `Some/None`; RW-21 tuple threshold; RW-29 reconnect branch; RW-30 locale fallback | Unit + Integration |
| Guard Removal | Removed duplicate checks, range validation, NUL byte stripping | RW-18 stale guard; RW-21 duplicate-key; RW-23 peer check; RW-27 control chars; RW-35 `MSG_CTRUNC` | Contract + Adversarial |
| Ordering & Tie-Breaking | Scope resolution, region selection, median sorting | RW-18 displayable tie-break; RW-22 region tie; RW-26 median even-count | Unit + Oracle |
| Encoding & Format | Endianness flip, integer sign, header offsets | B04 keepalive BE/LE; RW-21 big-int sign; RW-31 SQLite flags; RW-35 `UNIX_FDS` header | Byte-exact Contract |
| Resource Lifecycle | Descriptor leak, failed RAII Drop, inverted cleanup sequence | RW-32 Drop; RW-35 inhibitor fd, `close` on error-path, `sendmsg` cmsg | Integration + Live |
| Security Redaction | Removed `Debug`/`Display` redaction masks | RW-21 settings Debug; RW-22 voice raw; RW-27 DeepLink token | Unit + Security |
| Protocol Constants | Inverted opcode, corrupt packet type, missing tag | B02 ETF tags; B04 RR type; RW-25 SRTP tag; RW-35 message type | Conformance |
| Cryptography | Corrupted S-box, HMAC pad inversion, AAD modification, nonce reuse | RW-25 AES S-box, KDF pad, AAD, nonce reuse | KAT + NIST/RFC |
| Caps & Quotas | Max fields, recursion limit, queue buffer limit | RW-21 max fields; RW-24 markdown depth; RW-31 flags; RW-35 fd caps | Boundary + Fuzzing |
| Tooling & CI | Script step skips, regex inversion in validators | RW-22 `ci.sh`/`release.sh`; RW-23 SBOM allowlist | Meta + Dry-run |
| Test Fragility | Tautological oracle, invariant constant duplication | B01 V3/V5; RW-27 M3/M4; RW-34 boundary constant; RW-35 shared fd counter | Review + Mutation |

## 4. Survivor Triage and Resolution

| Wave | Mutant ID | Root Cause Analysis | Remediation Action |
|---|---|---|---|
| B01 | V3 | `voice_start_time >= 0` -> `> 0` survived because `Some(0)` epoch was missing in input vectors | Red-first test added with `Some(0)` valid epoch; mutant caught |
| B01 | V5 | `voice_users_in_channel` without guild survived because channel ID was unique across test guilds | Added test fixture with identical channel ID across two guilds; mutant caught |
| RW-23 | #10 | Receive buffer adjustment 128 -> 74 gave `INCONCLUSIVE` due to skipped loopback environment | Marked inconclusive; preserved skip-aware isolation |
| RW-27 | M3 | `middle` -> `start` proved equivalent because all 279 catalog entries shared `middle == start` | Documented equivalent mutant; variant M3b introduced to verify loop bounds |
| RW-27 | M4 | Fallback `en-US` unreachable due to exhaustive 31x3x3 localization matrix | Documented as defensive fallback |
| RW-30 | — | False `KILLED` report caused by implicit string match on `SKIP` log line | Enforced strict test ID and exit code parsing for test results |
| RW-31 | — | Fragile source code mutation anchor invalidated by rustfmt | Anchors migrated to exact unique line matches with post-fmt checksums |
| RW-35 | f11/f13 | Missing test coverage for duplicate file descriptor indices and send caps | Added targeted test suite; mutants m7 and m8 are now KILLED |

Conclusion: The vast majority of surviving mutants highlight a missing edge case in the test oracle rather than production code defects. Every surviving mutant (survivor) must trigger one of three actions: a new regression test, a dedicated task card, or formal mathematical proof of equivalency.

## 5. Principles of Mutation Engineering

1. **Mutations must alter observable output:** Modifying a variable that has no effect on external boundaries is inherently equivalent. Duplicate detection requires distinct payloads for identical keys.
2. **Single mutation per iteration:** Multi-point mutations confound root causes and invalidate review logs.
3. **Green baseline is mandatory:** Always verify clean targeted test execution before injecting mutations.
4. **Skips are never kills:** Tests skipped due to environmental constraints must be marked `INCONCLUSIVE`.
5. **Exact restoration via checksums:** Source files must be restored to bit-exact state matching their recorded sha256 checksums before proceeding.
6. **Self-cleaning scripts:** Mutation drivers must use `try...finally` blocks to prevent dirty repo state on interrupt.
7. **Regression tests for defects:** Any production bug discovered during code review must yield a regression test capable of catching the defect as a killed mutant.

## 6. Reproduction Workflow

```sh
cd <project-root>
python3 waves/RW-35/run-mutations-w35a.py --filter m7-duplicate-index-allowed
python3 waves/RW-28/run-mutations.py
```

## 7. Scope Boundaries

This ledger tracks formal wave deliverables with complete execution logs. Ad-hoc exploratory scripts from early iterations (RW-01..RW-16) are archived in wave directories. Mutation test coverage complements, but does not replace, end-to-end integration and cleanroom conformance testing.
