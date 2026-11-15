# ARM follow-up: explicit release blocker

Operator instruction: account for the ARM bug. Added 2026-11-15 to the current library-modernization plan and worker mandates.

Scope assumption: the already known Linux ARM64 build/packaging gap, plus concrete CPU/ABI/test risks found in the current source. macOS Apple Silicon and any Windows ARM64 support are tracked separately. No specific native crash trace or device was supplied; an actual SIGILL is **not** claimed as reproduced.

Current execution update: the CPU-flag and ELF source defects below now have bounded fixes and independent review in [ARM-SOURCE-BATCH.md](ARM-SOURCE-BATCH.md). The table records their original pinned-baseline findings. Full batch acceptance, genuine builds and native ARM release qualification remain open.

## Confirmed state and limits

| Finding | Evidence | Classification |
| --- | --- | --- |
| Linux ARM64 Rust std is absent on this WSL host | Fresh `rustup target list --installed` and `rustc --print target-libdir --target aarch64-unknown-linux-gnu` inspection | Confirmed environment blocker; no real native build/runtime qualification. |
| `generic` globally enables CRC/AES/SHA2 and C crypto extensions | `scripts/arm64/build.py` and `packaging/arm64/cargo-config.toml` | Source-verified portability risk; not an observed hardware crash. |
| ELF `p_align` is reported as `page_size_bytes` | `scripts/arm64/verify.py` | Metadata/loader-contract defect; segment alignment is not a measured kernel page size. |
| Legacy ARM tests expect fake AppImage format/build behavior | `tests/unit/arm64-packaging-990.test.mjs` versus the real builder/verifier | Contract reconciliation needs independent provenance and retained stronger release coverage. |
| Wrong-architecture test uses `/bin/true` | `tests/unit/arm64-build-integrity.test.mjs` | Host-dependent assumption: on an ARM host that executable is itself AArch64. Use a deliberately different-architecture fixture. |
| Hashes, EM_AARCH64 and cross-compilation do not prove GUI/media execution | Current build metadata retains native validation as not run | Native CPU/loader/ABI/package/GUI evidence remains mandatory. |

The existing integrity suite passed **12/12** on the current x86_64 WSL host. This validates its present integrity checks; it does not resolve ARM native runtime or the architecture-dependent test assumption.

Rust's [CPU feature guidance](https://doc.rust-lang.org/std/arch/index.html) requires a binary's enabled features to match the running CPU and describes runtime dispatch for portable binaries. The [code-generation reference](https://doc.rust-lang.org/reference/attributes/codegen.html) explains target-feature constraints. The [Linux ARM64 documentation](https://docs.kernel.org/arch/arm64/hugetlbpage.html) distinguishes page configurations; loader compatibility must be measured against the actual claimed configuration.

## Eight explicit tasks

| Card | Bounded outcome |
| --- | --- |
| LIB-ARM-01 | Reproducible failure/environment ledger; distinguish known missing prerequisites from unobserved runtime hypotheses. |
| LIB-ARM-02 | Portable generic versus optimized CPU profiles; explicit Rust/native dependency feature contract. |
| LIB-ARM-03 | Early dependency/framework admission matrix for the actual ARM OS/triples/CPU/ABI. |
| LIB-ARM-04 | Genuine pinned Rust/C/C++ SDK, sysroot/linker and target pkg-config provenance with negative prerequisite tests. |
| LIB-ARM-05 | Real Cargo GUI artifact plus strict tar/deb/AppImage, loader, provenance and checksum verification. |
| LIB-ARM-06 | Independently justified legacy-test reconciliation and host-independent wrong-architecture fixtures. |
| LIB-ARM-07 | Genuine native ARM runner and GUI/package/media/resource qualification tied to accepted hashes. |
| LIB-ARM-08 | Correct ELF bounds/alignment/congruence checks and truthful segment-versus-kernel-page metadata. |

Full instructions, exact file leases, dependencies and verification commands are on the corresponding Kanban cards and in `TASKS.json`.

## Early admission and final release gates

All dependency-integration cards now consult `LIB-ARM-03` before intake. Source-only qualification may authorize a bounded spike; it cannot clear native ARM production support. Crypto/SIMD backends, C/assembly, atomics, FFI layouts, build scripts and native SDKs require target-specific evidence.

`LIB-QA-05` requires the real artifact, contract and native-runner tasks. `LIB-QA-07` binds the resulting evidence to accepted source/dependency hashes. Missing hardware/SDK/CI stays explicitly unqualified; it does not become green through a dry-run, a success marker or an x86 result. Unrelated security fixes can still be investigated while ARM release remains blocked.

Linux aarch64 GNU/Asahi packages, macOS aarch64 Mach-O and Windows aarch64 PE/MSVC require distinct evidence if supported. An `apple-m1` CPU setting in the Linux builder does not qualify macOS. ARM32 is outside the current claim.

The fix is accepted only when a compatible real application artifact launches and preserves required behavior on the declared native ARM targets. Planning updates, metadata fixes and a source-only audit do not close this blocker.

## Worker handoff and current verification

A genuine additional ECC Codex audit completed as session `af4a762f`, gpt-6.1-sol/medium/read-only. Its seven findings use a pinned, hash-checked packet of seven ARM source files; it ran no tools/builds or native program. The report is [reports/LIB-ARM-01.json](reports/LIB-ARM-01.json); the actual session receipt is in `DISPATCH-RECEIPTS.json`.

The coordinator incorporated those findings into the eight cards. The [failure ledger](ARM-FAILURE-LEDGER.json) separates observed environment state from portability hypotheses. CPU/ELF source fixes now have a separate implementation batch. Runtime crash reproduction, remaining integration/package fixes, real SDK/build and native acceptance remain outstanding.
