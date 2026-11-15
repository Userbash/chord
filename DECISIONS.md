# Verification maintenance decisions

## CP-001 — Resolve the completed Rust namespace migration

The selected baseline `e46193e` already declares `chord-*` Cargo packages and
stores their sources in `rust/crates/chord-*` and `rust/chord-gpui`. Its commit
message also identifies migration of legacy crate references. Baseline Node
verification still referred to removed `discord-*` source paths.

| Previous reference | Current reference | Independent provenance |
| --- | --- | --- |
| `rust/crates/discord-<role>` | `rust/crates/chord-<role>` | Checked-in package names and directories in `rust/Cargo.toml` and member manifests |
| `rust/discord-gpui` | `rust/chord-gpui` | GUI manifest and tracked source tree |
| Cargo `-p discord-<role>` | Cargo `-p chord-<role>` | Actual Cargo package identities; native workspace tests executed successfully |

Only source locations and Cargo selectors change. Payload expectations,
assertions, thresholds, test counts and clean-room evidence identifiers stay
intact. Legacy release names under `rust/dist/discord-*` retain their published
compatibility contract from `CONTRIBUTING.md`.

Accepted historical parity records remain unchanged. The explicitly bounded
`canonicalRustSourcePath` resolver maps only the known historical Rust source
namespace to current packages; every resolved file must still exist. Its tests
derive expected current paths from the independent checked-in Cargo manifests
and reject rewriting unknown packages, release artifacts or evidence paths.

Baseline evidence is preserved outside the source tree in
`/root/projects/chord-verification`: the initial meta run had 139 passes and 11
failures; the initial unit run had 2007 passes and 323 failures. Those failures
are never reclassified as passing without executing the repaired checks.

## CP-002 — Preserve filesystem failure assertions across execution privileges

Mode `0555` is not an unwritable-location oracle for a root process, and it does
not express a Windows ACL. A regular file used as the parent of the requested
output path instead causes a real directory-creation failure on each platform.
The tests retain their exit-code, diagnostic and record-count assertions.

## CP-003 — Keep unavailable native verification observable

GPUI must be fetched from its committed gitlink and checked for local changes.
Native core and GUI manifests are checked independently for each operating
system. Missing dependencies, unsupported native backends, missing live test
services and unavailable RSS measurements must remain errors or explicitly
reported unmet prerequisites; none count as a successful feature verification.

## CP-004 — Repair verification without changing feature expectations

The checked-in reference documents are English. Documentation checks retain
their IDs, numeric limits, permission masks and protocol values while matching
the current labels and units. Stream source checks use complete function or
handler boundaries instead of fixed character windows. The OP 18 check follows
the immediate voice-state delegate, including `self_stream` and the real user
ID; independent call-removal and misrouting mutations must still fail.

Positive browser fixtures provide the real IPC contract's unit-return success
responses. Settings readiness waits for the requested debounced payload rather
than an unrelated startup save. All existing result assertions remain.
Browser children and servers close before temporary profiles are removed.
Explicit `bootstrap: null` serves static fixtures unchanged; every non-null
Tauri bootstrap retains the required module-marker guard.

## CP-005 — Share the native Unix descriptor transport

GPUI delegates descriptor transfer and duplication to `chord-platform`, using
target-native libc types and constants. A single receive preserves short reads
and EOF, while the exact reader retains its premature-EOF and timeout contract.
All delivered descriptors must be closed on an ancillary error, including when
an unexpected credentials header precedes a valid rights header. Kernel-backed
tests verify ownership, truncation cleanup and `FD_CLOEXEC`.

Darwin compilation and constant checks do not establish native runtime behavior.
The non-Linux receive fallback sets close-on-exec after receipt; its existing
documented concurrency limitation must not be presented as atomic protection.

## CP-006 — Require genuine ARM64 build and packaging inputs

The baseline ARM64 script generated a small C program with a fixed success
report and a non-executing AppImage-shaped container. Neither established that
Chord was built or ran. The replacement invokes locked Cargo for the actual
GUI and requires explicitly supplied, hash-pinned official packaging tools.
It performs no automatic tool downloads. Missing ARM development prerequisites
remain environment failures before release outputs are published.

Legacy output filenames stay compatible. Genuine container integrity,
deployment dependencies, and native ARM runtime/GPU/RSS validation are separate
claims. Packaging metadata must disclose host ABI requirements and unexecuted
native validation. Existing release checksums and historical package evidence
are not regenerated to make this source refactor appear release-qualified.

## CP-007 — Keep fuzz dependency pins and verification provenance

The fuzz workspace's four path packages and Rust imports follow the current
Chord namespace. Its 77 third-party lockfile entries remain byte-identical.
Compilation and uninstrumented libFuzzer execution smoke are recorded separately
from sanitizer or coverage-guided fuzzing, which they do not replace.

New native CI jobs cover core and GUI independently on Linux, Windows and
macOS. Build/test results remain observable after a failed lint step, with all
failures still blocking. Local configuration checks are not GitHub runner
executions. Detailed local logs and real window evidence live outside this
source tree under `/root/projects/chord-verification`.

## CP-008 — Synchronize fixtures with real lifecycle boundaries

Mock D-Bus authentication must not read ahead into a binary Hello frame and
discard the buffered bytes. Both affected fixtures keep auth reads bounded;
a coalesced BEGIN/Hello regression establishes the original race without
increasing timeouts. Browser actions wait for their actual document and render
state. Temporary profile removal retries only transient filesystem conflicts
within a finite budget after shutdown; persistent conflicts remain failures.

Xvfb lifecycle checks derive the first free display from an independent
read-only occupancy snapshot, so an outer test display does not invalidate
the fixture. Real X11 sockets and locks are never modified by these checks.

## CP-009 — ARM source portability and truthful ELF metadata (2026-11-15)

Owner/coordinator: RW-51. Scope: LIB-ARM-02, LIB-ARM-08 source slices and
LIB-ARM-06.I1 source-configuration test reconciliation. Full parent acceptance
and native release qualification remain open.

| Old contract | New contract | Independent basis |
| --- | --- | --- |
| Generic source flags globally enable CRC/AES/SHA2 and C crypto. | ARMv8-A generic inputs; optimized profiles select an explicit CPU without blanket extension overrides. | Rust target-feature safety contract and actual compiler configuration inputs; see `waves/RW-51/ARM-ABI-DECISION.md`. |
| SUB-990-1 requires optional instruction flags in historical DIST and current SOURCE alike. | Historical DIST retains all four original positive feature assertions. Current SOURCE requires the portable generic CPU, rejects optional extensions and checks exact matching C/C++ flags. | Historical DIST is immutable provenance; current source follows the reviewed CPU contract. Five isolated mutants of the new source expectation are rejected. |
| ELF segment alignment is reported as a numeric kernel page size. | Report `load_segment_alignment_bytes` and unknown `kernel_page_size_bytes`; validate loader ranges and congruence separately. | System V gABI, independent readelf and linker-produced format fixtures; see `waves/RW-51/ARM-ELF-ABI-CONTRACT.md`. |

No golden artifact or remaining package/runtime assertion is changed. The full
ARM06 legacy-build contract is still pending genuine build/package evidence;
this decision does not approve a fake output, missing SDK, test skip or release.
Raw red/green/mutation logs and independent review records are referenced in
`waves/RW-51/ARM-SOURCE-BATCH.md`. The full worker gate remains separately
reported; a focused source result cannot replace it. Reverting a source slice
does not authorize distributing an unqualified ARM artifact.

## CP-010 — Follow the extracted voice implementation in stream contracts (2026-11-15)

Scope: sequential gate repair, three failing source assertions in
`call_stream_dispatch.test.mjs` and `call_stream_lifecycle.test.mjs`.
Upstream commit `1351e03f489ccb47890e406f91b84ec1b3b559bc` extracted the voice
implementation into `voice_plugin.rs`; `main.rs` retains the public IPC wrappers.
The old test location no longer describes where behavior is implemented.

| Old check | Current check | Independent basis |
| --- | --- | --- |
| Read OP18/user-ID implementation from `main.rs`. | Require the wrapper to delegate to the plugin, then retain all existing voice/data/Gateway assertions against the actual plugin body. | The upstream extraction diff and executable Rust data/IPC contracts. |
| Find stop-key derivation and STREAM permission somewhere in `main.rs`. | Check the scoped plugin functions, both stop-key aliases/fallback, and the permission guard before the voice-state request. | The extracted functions preserve these semantics; disconnected wrappers and disabled checks must fail. |

Application code, argument names, permission semantics, wire opcodes and the
remaining assertions are unchanged. The test repair does not change a voice
runtime capability or restore disabled Krisp behavior. Five isolated source
mutations exercise lost delegation, stream flag, actual user ID, permission
guard and stop-key alias. Independent review and exact red/green evidence are
required before accepting this test-only slice. Native live voice and ARM
qualification remain separate.

## CP-011 — Pin the independent theme oracle inside the test fixtures (2026-11-15)

The theme suite depended on an ignored local Serein checkout that is absent
from a clean clone. The missing file caused two failures. Restoring the exact
upstream data made all six original tests pass without changing application
palettes or assertions.

| Old input | Pinned input | Independent basis |
| --- | --- | --- |
| Ignored `docs/reference-projects/sources/Serein/extensions/themes/midnight.serein-extension`. | Tracked `tests/fixtures/themes/serein/midnight.serein-extension`, with upstream metadata and license snapshots. | Official `ViceVerse-cz/Serein` commit `04f348092910a77cc317516b25577ad006cb708b`; Git blob `58d7e3b17bf440c0fa3d6435b8da5f8d8f2dc142`, SHA256 `f7ffb0acdb7269f1e2fdb235536870cef344f3fac761543174ddc091bb2aebb6`. |

The data bytes and both upstream license files are unchanged. The test now
checks commit and content hash in addition to the existing palette, hot-switch,
contrast, injection and mutation checks. No Serein implementation is executed,
linked or copied into application code. Updating this independent oracle
requires fresh provenance and independent review, rather than fitting it to
new palette values.


## ADR-RW51-TAURI-PORTS-20261115 — Preserve native boundaries and fail closed

The Tauri transition's Windows source could not compile because Unix UID, socket and file-descriptor APIs were unconditional. Native GitHub builds independently exposed the missing Windows resource icon and a D-Bus diagnostic without operation context. The reviewed implementation scopes Unix transports and their unchanged fixtures to Unix, retains portable wire models and RPC/policy/CPU-frame contracts on every target, and returns typed `UnsupportedCapability` errors for unavailable transports. It never returns fake IDs, descriptors or successful no-op service results. Linux portal/ptrace calls remain Linux-only. The original Unix test bodies and assertion counts are preserved; a Windows-specific assertion checks the explicit unavailable notification capability.

Windows credentials use the explicit `keyring = 3.6.3` `WinCredential` backend, with existing raw-secret UTF-8 and redacted-error contracts. MSVC Hunspell discovery uses `vcpkg = 0.2.15` and installed port metadata; all existing dependency versions and checksums are retained. The two lockfile token normalizations retain exactly the same dependency graph. Native icons were generated from the unchanged product PNG by the official pinned Tauri CLI. All twelve new target dependency licenses were read from checksum-verified registry package manifests; the license allowlist is unchanged.

`SystemSources` uses the already-direct rustls ring provider's operating-system CSPRNG instead of `/dev/urandom`. The existing infallible identity API fails closed on entropy errors; formatter, draw order, configuration overrides and cached session identity remain unchanged. A real Linux device-absent RED/GREEN check and five behavioral mutants verify this boundary. Native Windows/ARM runtime evidence is separate.

Independent author/reviewer receipts, old/new contract mapping, exact source hashes, RED/GREEN commands and native build limitations are retained in `waves/RW-51/TAURI-COMPILE-BATCH.json` and its linked reports. These source decisions do not accept full native GUI, notification, capture, audio, RPC or ISA parity. Reverting the dedicated source/CI commits restores the previous behavior and dependency graph. No release or merge is authorized by a draft push.


## ADR-RW51-NATIVE-TIMING-EOL-20261115 — Canonical checkout bytes and caller-relative deadline

The first repaired native matrix compiled all six desktop releases. Real Windows tests exposed CRLF-sensitive source contracts, and artifact verification independently found the same conversion in Cargo.lock bytes. Keep the original assertions and package graph; canonical Git LF rules for Rust and lockfiles preserve the repository blob bytes across native checkouts. A real Git regression, hostile-config isolation and bad-attribute mutants verify the boundary. No Rust oracle or lockfile contents changed for this correction.

A missing coalesced update in the Intel macOS contract led to independent scheduling reproduction. The original coalescer started its 120 ms sleep only after a new thread began running. With exactly three child entries delayed80ms, the unchanged 160 ms observation fails; a caller-captured Instant deadline plus remaining-duration sleep passes. This small standard-library correction preserves all sequence/active/channel cancellation guards and flag payloads. Independent RED/GREEN and five runtime mutants are retained; no sleep budget or assertion was weakened. The original macOS scheduling history and hard real-time guarantees are not inferred from this reproduction. See the linked compile batch and independent review reports.
