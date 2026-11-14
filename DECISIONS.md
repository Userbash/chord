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
