# SECURITY — Threat Model and Secure Development Standards

This document establishes **what assets we protect**, **where trust boundaries lie**, and **which checks are mandatory before acceptance**. It complements `CODE-STANDARD.md` §5 and `LEGAL.md`; the formal acceptance process is governed by `PROCESS.md` §4.3–§4.4.

Core Principle: Security is not a separate delayed milestone, but an intrinsic property of every architectural layer. Each task card must explicitly identify its applicable threat model vectors; non-applicable sections must be marked with justification.

## 1. Threat Model

### 1.1 Assets to Protect (Assets)

| Asset | Storage Location | Security Impact |
|---|---|---|
| User Token / Session ID / Cookies | Keyring, Memory, HTTP Headers | Direct account compromise |
| Message Contents & Attachments | State, History Store, SQLite, UI | User communication privacy |
| Voice Encryption Keys & E2EE Material | Voice Signaling, Memory | Wiretapping or media injection |
| Credentials & Multi-Factor Auth (MFA) | Login Flows, Keyring | Account takeover |
| Local Database & State Caches | `discord-store`, Filesystem | Message history and metadata |
| Application Logs / Crashes / Errors | Stderr, Log Files, Reports | Accidental credential and PII leakage |
| Deep Links / CLI Arguments | `discord://`, Process Argv | URL protocol hijacking / token injection |
| Updater Manifest & Release Binaries | Network / Local Disk | Code execution and tampering |
| D-Bus / Secret Service / FFI | System Shared Libraries | System privileges, fd leaks, memory corruption |

### 1.2 Adversaries and Threat Actors

- **Untrusted Network / Malicious Endpoints**: Corrupted, truncated, replayed, oversized, or contradictory payloads; MITM attempts without strict TLS certificate validation.
- **Untrusted Local Input**: Command-line arguments, deep link URIs, clipboard contents, fixture files, environment variables.
- **Compromised Supply Chain**: Malicious or vulnerable third-party dependencies, license traps.
- **Client Implementation Bugs**: Credential leakage in `Debug` implementations, path traversal vulnerabilities, race conditions, file descriptor leaks, unbounded buffers, silent fallbacks.
- **Process & Test Flaws**: Tests tailored to flawed code, disabled checks, fraudulent mutation logs.

### 1.3 Trust Boundaries

| Boundary | Inbound Input | Mandatory Enforcement at Boundary |
|---|---|---|
| Network -> Gateway | WebSocket frames, zstd streams, ETF payloads | Strict size and recursion limits, fatal error parsing, unknown tags preserved safely, zero panics |
| Network -> REST | HTTP/1.1, TLS, JSON payloads | Strict TLS verification, header sanitization, bounded body limits, deterministic 401/429/5xx handling, idempotent-only retries |
| Voice -> Media | RTC JSON signaling, UDP, RTP/SRTP | Strict opcode validation, SSRC/peer binding, nonce/counter sequence checks, authentication tags, bounded jitter buffers |
| Disk -> State / Store | Cached state snapshots, SQLite files, fixtures | Schema and type assertions, NUL-byte and UTF-8 verification, strict bounds, atomic transactions, provenance tracking |
| System -> Platform | D-Bus, logind, Keyring, hunspell, libsqlite3 | Explicit auth and address checks, strict file descriptor ownership, C ABI safety, NUL termination, error cleanup |
| User -> UI / CLI | Process argv, deep link URIs, clipboard, file drops | Canonical path normalization, control character stripping, length ceilings, safe parsing |
| Network -> Updater | Release manifests, binary update packages | Ed25519 cryptographic signatures, SHA-256 integrity hashes, anti-downgrade enforcement, path traversal rejection |
| Internal State -> External Logs | `Debug` representations, error messages, reports | Strict redaction of secrets, zero raw binary payloads, zero user tokens, zero PII |

Rule: **Trusted data inside the core engine** is allowed only after formal boundary verification. Internal components preserve invariants without re-parsing already validated types.

## 2. STRIDE Threat Analysis Matrix

| Threat Category | Project Concrete Vector | Mitigating Security Control | Verification Level |
|---|---|---|---|
| Spoofing | Spoofed voice signaling endpoint; deep link with malicious host | TLS pinning, peer/SSRC binding, strict URI scheme/host parsing | T-VOICE, T-SYS-04 |
| Tampering | Manipulated updater binary; corrupted SQLite database | Ed25519 signature + SHA-256 verification; atomic transactions | T-SYS-11, T-DATA-05 |
| Repudiation | Unaccounted state transitions | Structured audit and diagnostic logging without sensitive data | Unit & Contract tests |
| Information Disclosure | Token printed in `Debug` output; raw payload exposed in error | Automatic redaction, dedicated `Secret` wrapper types, scanners | T-SEC-02, T-SEC-03 |
| Denial of Service | Decompression bomb, recursive ETF nesting, 24k member storm | Strict recursion limits, bounded queues, request timeouts | T-ENC-05, T-DATA, T-SYS-06 |
| Elevation of Privilege | Path traversal in updater; shell injection in desktop autostart; FFI bugs | Canonicalization, exec without shell, safe Rust FFI wrappers | T-SYS-03, T-SYS-11, FFI Review |

## 3. Secrets Management and Redaction Policy

- Authentication tokens, session identifiers, cookies, voice encryption keys, and MLS/DAVE secrets **must never** appear in `Debug` or `Display` formatters, error messages, disk logs, reports, file names, or git commits.
- Public structs containing sensitive fields must implement custom redacted `Debug` displaying `[REDACTED]` or `<redacted>`. For raw cryptographic keys, print only byte length or truncated cryptographic hash.
- Test fixtures and documentation must contain strictly sanitized placeholders. Automated scanning via `T-SEC-02/03` is mandatory in CI.
- System Keyring (libsecret / Secret Service / macOS Keychain / Windows Credential Manager) is the sole sanctioned persistent token storage. Plaintext configuration files and SQLite databases must never store auth tokens.
- Foreign library error handlers (libsecret, hunspell, SQLite, D-Bus) must never echo raw input buffers.
- Suspected leakage protocol: immediately abort live execution, rotate compromised credentials, invalidate local caches, notify project owner, and log security incident without leaking sensitive payloads.

## 4. FFI and Safe Memory Practices

Unsafe code blocks (`unsafe`) are strictly isolated inside dedicated low-level wrapper modules exposing 100% safe, idiomatic Rust public interfaces. Crates `discord-platform` and `discord-store` enforce `#![forbid(unsafe_code)]` with surgical, auditable `#[allow(unsafe_code)]` annotations exclusively on FFI bridge boundaries.

Mandatory review checklist for every unsafe FFI block:

- [ ] System C library ABI, calling convention, and struct alignment match header declarations.
- [ ] Raw pointers are checked for null, proper memory alignment, and valid lifetime bounds.
- [ ] Buffer lengths are explicitly passed alongside pointers; out-of-bounds reads/writes prevented; strings are validated for NUL termination.
- [ ] Resource ownership (allocation and free responsibility), RAII `Drop` implementations, and error unwind paths are verified.
- [ ] Panics across FFI boundaries are strictly forbidden (`catch_unwind`); C return codes are mapped into typed Rust errors.
- [ ] Thread safety and reentrancy: verified whether functions are thread-safe or require mutex synchronization.
- [ ] Detailed `// SAFETY:` comments explain precisely why the invariants hold.
- [ ] Comprehensive test suite covers error paths, double-free prevention, descriptor leaks, buffer limits, and missing dynamic libraries.
- [ ] Code compiles under `cargo clippy -D warnings`, verified under ASan/TSan or Miri where applicable.

Known FFI boundary points: `libsecret` (Keyring), system `libsqlite3` (Store), `libhunspell` / `libenchant` (Spellchecking), `libc` (`sendmsg/recvmsg`, SCM_RIGHTS, logind inhibitor). Each boundary requires dedicated contract testing and security review.

## 5. Supply Chain Security

- `Cargo.lock` represents the authoritative dependency tree; introducing new dependencies requires explicit owner approval.
- Offline compilation by default; network access during builds is disabled.
- Automated dependency auditing via `cargo-deny` and SBOM generation (`scripts/sbom.mjs`): checks for licenses, security advisories, and pinned dependency versions.
- New dependencies are vetted for maintenance activity, permissive licensing, transitive tree weight, build scripts, unsafe usage, and binary footprint.
- Dynamically linked system libraries are probed for ABI compatibility; missing libraries fail closed with exit code 2 / skip rather than silently degrading.
- Release artifacts are cryptographically signed; the updater verifies cryptographic signatures prior to deployment.

## 6. Live Operations and Cleanroom Reference Artifacts

- Attaching to, spawning, or tracing live Discord processes is prohibited without an explicit authorization window from the project owner; default behavior is exit code 2 or skip.
- Reference directories (`logs/`, `functions/`, `evidence/`, `clean-room-oracle/`) are strictly read-only. Modifying, renaming, or committing unredacted artifacts is forbidden.
- Tests claiming live operational contracts must fail if the required environment is missing; standard offline test runs skip with an explicit message.
- Spawning shell processes from unsanitized strings is strictly forbidden; all process executions utilize canonical path validation.

## 7. Security Testing Levels

| Test Level | Scope of Verification | Example Test Suites |
|---|---|---|
| L0 Static | Clippy `-D warnings`, `cargo-deny`, SBOM, secret scanning, unsafe inventory | CI pipeline, `scripts/ci.sh`, `scripts/sbom.mjs` |
| L1 Unit | String redaction, input bounds, parser edge cases, crypto KAT vectors | `Debug` formatting tests, bounds validation, RFC KATs |
| L2 Contract | Wire protocol conformance, NUL byte stripping, schema validation | Cleanroom RE fixtures, byte-exact conformance |
| L3 Integration | FFI error handling, D-Bus authentication, SQLite memory limits | `keyring_contract`, `dbus_contract`, `store_contract` |
| L4 System | Updater signature verification, deep link handling, autostart isolation | `updater_contract`, `deep_link_contract` |
| L5 Acceptance | Authentication lifecycle, credential revocation, secure logout | Manual QA protocol & owner acceptance |
| L6 Non-Functional | Denial of service limits, fuzzing, timeout behavior, memory leak detection | Fuzz targets, property-based tests, chaos engineering |
| L7 Mutation | Mutation testing of security guards, boundary checks, and secret masks | Mutation ledger, killed mutants verification |

## 8. Mandatory Security Review Triggers

A formal security review is required for changes touching:
- Cryptographic implementations, key generation, nonces, MLS/DAVE protocols.
- Foreign Function Interface (FFI) bindings and `unsafe` blocks.
- Authentication, token management, session handling, and system Keyring.
- Auto-updater mechanisms, cryptographic signature verification, file downloads.
- URL schemes, deep links, command-line arguments, desktop autostart entries.
- Wire protocol parsers and boundary limits (ETF, zstd, JSON, RTP, SRTP).
- Live operations and golden test fixture handling.
- Addition of third-party dependencies to `Cargo.toml`.

Code reviews must be conducted by an independent reviewer (never the original author).

## 9. Current Operational Constraints

- Wire compatibility for SRTP/DAVE is verified against cleanroom RFC vectors; live keys are not stored in repository fixtures.
- `discord-store` executes single-threaded (`!Send + !Sync`); WAL journaling is enabled for file-backed databases.
- `discord-platform` isolates FFI blocks with surgical safety comments and dedicated tests.
- D-Bus transport conforms to standard system bus specifications.
- Live desktop notifications and inhibitors have been validated against system services.
- Full GUI threat model for GPUI native rendering is verified under WCAG and memory safety audits.

## 10. Security Incident Response Procedure

1. Immediately halt live operations and stop test executions.
2. Never record leaked secrets in tickets or logs; document only the vulnerability category, timestamp, and affected component.
3. Invalidate compromised authentication tokens or keys, clear local cache files and session logs, verify git history.
4. Notify the project owner and prepare an incident post-mortem with regression test coverage.
5. Update security checklists and automated linter rules to prevent future occurrences.
