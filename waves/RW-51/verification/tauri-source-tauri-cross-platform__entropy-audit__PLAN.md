# Chord core identity entropy: bounded source audit

Audited application revision: `3c4be0c2787856df38e6da8703cb3461b82c0470`. This is a read-only plan. No source was changed, no Cargo command or native test was run, and no native platform qualification or ECC execution is claimed.

## Finding and callers

`rust/crates/chord-core/src/rest/identity.rs:48` implements `SystemSources::random_bytes` exclusively by opening `/dev/urandom` and discarding both open and `read_exact` failures. On ordinary Windows, where that Unix device path is absent, the zero-initialized UUID buffer is unchanged; `uuid_v4` at line307 consequently formats `00000000-0000-4000-8000-000000000000`. This Windows outcome is a source-based inference, not an observed native execution. A partial Unix read can also modify some bytes before an ignored error; the existing best-effort comment does not guarantee unchanged contents.

`build` at lines343–346 must keep lazy configuration overrides and the exact random-consumption order: launch ID, launch signature, heartbeat session ID. `ClientIdentity::rw_client` caches one successful identity in `OnceLock`; repeated calls intentionally return identical IDs. Tauri's `IdentityContext::detect` (`rust/gui-spike/src/auth.rs:419`) likewise caches a session identity, and locale-specific contexts explicitly retain its three IDs. A uniqueness test must use fresh `uuid_v4(&SystemSources)` or separate `ClientIdentity::build` calls, not repeated cached-context calls.

Existing `tests/rest_identity.rs` covers deterministic injected bytes, exact formatted UUID strings, version/variant masking, ordering, configuration overrides and environment mapping. The in-file identity tests also preserve the shared REST/Gateway values. They do not currently exercise real `SystemSources` entropy on Windows or failure handling.

## Smallest dependency-safe change

`chord-core` directly depends on `rustls` through the workspace. It does **not** directly depend on `ring`, `getrandom`, `uuid` or `rand`. Calling `ring::rand` directly today would therefore require a manifest change even though ring is already locked transitively.

Prefer the existing direct dependency's public RNG capability:

```rust
rustls::crypto::ring::default_provider()
    .secure_random
    .fill(out)
    .expect("OS entropy unavailable for client identity");
```

The workspace already explicitly enables rustls's `ring` feature. Its locked provider delegates to `ring::rand::SystemRandom`, which delegates to the locked `getrandom` OS backend. This route needs no dependency/feature/lockfile change, handwritten OS FFI, CPUID checks, clock/PID fallback or new entropy layer. Do not install or modify a process-wide default crypto provider merely to generate an ID.

`default_provider()` allocates cipher-suite/KX vectors; three one-time identity draws make that a small bounded cost. Avoid a cache or provider abstraction until measured need warrants it. If the reviewer prefers direct ring for separation from TLS, adding it as a direct, coordinator-owned dependency is an explicit alternative, not something already legal via transitive resolution.

The existing public `IdentitySources::random_bytes` returns `()`. Keeping its signature and all injected-source implementations while making **SystemSources** fail closed means a clear panic on OS RNG failure. Silently ignoring the error, returning unchanged/partial bytes or substituting zeros is unacceptable. Update the trait/source comments to distinguish injected deterministic sources from SystemSources's panic-on-entropy-failure behavior. A recoverable `Result` API is a separate migration because it would propagate through UUID/build/cached identity and auth callers; do not disguise it as a minimal compatibility fix.

For deterministic error-path testing, a tiny private helper accepting `&dyn rustls::crypto::SecureRandom` may perform this exact fill/expect operation. The production SystemSources path must call it. Test doubles can return `GetRandomFailed` before writing or after partially writing. Keep the public unit struct, constructor, trait signatures, UUID formatter and field ordering unchanged.

## Exact locked implementation and platform scope

The audit verified cached `.crate` archive SHA256 values against `rust/Cargo.lock`, then checked each inspected source file byte-for-byte against its archive member. See `SOURCE-EVIDENCE.json` for hashes and paths.

| Locked package | Verified source contract |
| --- | --- |
| rustls0.23.46 | `src/crypto/mod.rs:213` exposes `secure_random: &'static dyn SecureRandom`; lines311–322 define the public fallible fill trait. `src/crypto/ring/mod.rs:32–52` constructs the provider and delegates fill to ring SystemRandom. |
| ring0.17.14 | `src/rand.rs:100–190` selects getrandom for Linux, Windows and Apple OSs. `build.rs:172–222` explicitly includes Linux/Apple/Windows AArch64 and x86_64 assembly target entries; Cargo.toml has additional AArch64 Windows/Apple/Linux dependency configurations. |
| getrandom0.2.17 | `src/lib.rs` selects Windows by OS cfg without an x86-only restriction; `src/windows.rs` uses BCryptGenRandom, with its documented system fallback/error path. macOS uses getentropy; Linux x86_64/AArch64 uses getrandom plus the qualified Unix-device fallback. |

This establishes source support for the six intended desktop triples (x86_64/aarch64 on Linux GNU, Windows MSVC and macOS Darwin). It does not prove that their SDK/toolchain, linker, full dependency graph, application bundle or GUI runtime works. Windows ARM64 requires its actual native runner; an x86_64 Windows or Linux result cannot substitute for it. The existing ARM CPU/ABI/package release blockers remain independent.

The primary [getrandom0.2.17 documentation](https://docs.rs/getrandom/0.2.17/getrandom/) describes OS backends and its fail-over-insecure-bytes policy. The exact rustls/ring implementation findings above are grounded in hash-verified locked package source; unavailable web documentation for those exact versions was not treated as runtime evidence.

## TDD and acceptance sequence for the next serial writer

1. Lease only core identity source, its focused tests and any explicitly assigned integration metadata after the current platform writer finishes. Preserve the user-selected application base and record the accepted revision. No Cargo work runs concurrently with the integration lane.
2. Add a real SystemSources test and run it natively on Windows before the fix: generate256 fresh UUIDs, verify RFC version/variant/lowercase shape, reject the known zero-buffer UUID and require all IDs distinct. Add fresh identity-build uniqueness, but assert cached `rw_client` calls remain stable. The old implementation should fail the native Windows fresh-ID test for the entropy defect; missing execution infrastructure is not a passing RED.
3. Preserve every existing deterministic-source expectation, including the three exact strings/call order and lazy config overrides. Add injected private RNG tests for successful complete fill, empty output, explicit failure and partial-write-then-error. Failure must prevent a normal return/UUID publication; do not use a fake deterministic identity as fallback. No global OS RNG monkeypatch or live auth/network activity is needed.
4. Implement the minimal existing-provider fill and explicit failure behavior. Run focused core tests (`rest_identity` plus in-file identity tests), then the integration coordinator's required gate. Test compilation alone is not proof of native behavior.
5. Prove at least five controls: restore Unix-only entropy on Windows; swallow the RNG error; substitute all-zero success; break UUID version/variant masking; reorder the three random draws or eagerly consume overridden UUIDs. Preserve immutable RED/GREEN/control logs and tie reports to source hashes.
6. Run the native focused tests on all six claimed triples through real runners. Record OS/architecture/triple, toolchain, command/exit, source/dependency/binary hashes and actual test counts. Complete the user's full Tauri builds separately; this entropy slice cannot certify the whole application or a release.

Rollback: isolate/revert the slice for diagnosis while keeping Windows/ARM release promotion blocked. Never restore silent fixed-identifier output as an accepted fallback.
