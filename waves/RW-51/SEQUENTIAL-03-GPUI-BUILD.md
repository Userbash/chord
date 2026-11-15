# GPUI build prerequisite: genuine Linux binary verified

The release/package failures need a real application input. The pinned `gpui-kit` submodule was missing; it is now checked out at `957ae8b55f1d02ea0f322951a9fa3b3f0cbbb282`. Locked/offline dependency resolution and the release build succeeded without source or lockfile edits.

- Actual Cargo GUI executable: **43,687,496 bytes**, stripped x86_64 ELF PIE.
- SHA256: `b0a642752c4e885f72c42733c0f768f065db4a27e0c7d14d1ab69e14af45e8d7`.
- Build ID: `111e9cfcce3a46851c61ae996d8ebc0571be0a13`.
- Existing system-requirement checker passes; no dynamic dependency is missing.
- Mandatory libc floor:2.34; highest nonweak GLIBC requirement:2.35. The observed2.39 requirement is explicitly weak, not an unconditional2.39 floor.
- Headless smoke succeeds on the WSL host and in the digest-pinned Ubuntu22.04 runtime container with GLIBC2.35. RSS observations are11.21/11.29MiB respectively.

The older-runtime loader emits an optional-version warning, preserved in the raw log, and the process still exits0. The [upstream loader source](https://github.com/bminor/glibc/blob/master/elf/dl-version.c) distinguishes weak version requirements from fatal missing mandatory versions. Actual older-runtime execution is recorded separately from this source interpretation.

Headless checks exercise the smoke path, not a rendered window, native GPU FPS, live account/media, Windows/macOS or ARM functionality. Packages and full parent acceptance remain open. The build reports two unused-dependency warnings; they were retained rather than changing unqualified platform features. Immutable historical release files were not replaced and nothing was published.

All exact commands, lock hashes, image digests and logs: [SEQUENTIAL-03-GPUI-BUILD.json](SEQUENTIAL-03-GPUI-BUILD.json).

After the debug prebuild populated the cache, the unchanged worker gate enabled all GPUI phases: smoke passed, E2E contracts **55/55**, and visual contracts **10/10**. Typechecks, meta **210/210**, GUI-spike Rust and workspace Rust **1490 passed / 12 ignored** also passed. Smoke had **19 passed / 3 skipped**. The overall gate is **failed** solely in JS unit (**2614 passed / 34 failed**). Ignored and skipped tests are not native qualification. The full machine report is [preserved here](verification/sequential-worker-gpui-enabled-20261115T164751333Z.json).
