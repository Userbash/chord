# CPU instruction compatibility and acceleration contract

The user clarified the x86 instruction list from the screenshot: MMX(+), SSE/SSE2/SSE3/SSSE3/SSE4.1/SSE4.2/SSE4a, x86-64, AES, AVX, AVX2, AVX-VNNI, AVX-512, FMA3 and SHA. This records compatibility requirements; it does not claim every optimized application path is implemented or qualified. ARM remains a separate release blocker.

Use the existing libraries and Rust standard feature detection for optional acceleration. Keep a portable platform baseline and tested fallback. Global `target-cpu=native`, `-march=native` and optional feature forcing are prohibited for universal releases. No handwritten CPUID/SIMD abstraction is introduced.

The [Rust standard detector](https://doc.rust-lang.org/std/arch/macro.is_x86_feature_detected.html) checks individual features. SSE4a differs from SSE4.1/4.2; FMA3 uses `fma`; AVX-VNNI differs from AVX512-VNNI. AVX-512 must be checked by exact subsets. MMX+ is not a Rust std detector argument, so its presence is not invented. Libraries must check the CPU and required OS register-state support. Global compile-time feature enablement can make a runtime macro return constant true.

## Measured compiler defaults

Rust 1.100.0 was queried directly for all six target triples. These cfg observations differ by OS and are not complete binary ISA certification or native builds. Preserve the target ABI rather than disabling mandatory platform features. In particular, Apple ARM defaults must not be replaced with the generic Linux ARM policy.

| Queried target | Observed default `target_feature` values | Runtime qualification |
| --- | --- | --- |
| `x86_64-unknown-linux-gnu` | `fxsr`, `sse`, `sse2` | Not qualified |
| `aarch64-unknown-linux-gnu` | `neon` | Not qualified |
| `x86_64-pc-windows-msvc` | `cmpxchg16b`, `fxsr`, `sse`, `sse2`, `sse3` | Not qualified |
| `aarch64-pc-windows-msvc` | `neon` | Not qualified |
| `x86_64-apple-darwin` | `cmpxchg16b`, `fxsr`, `sse`, `sse2`, `sse3`, `sse4.1`, `ssse3` | Not qualified |
| `aarch64-apple-darwin` | `aes`, `crc`, `dit`, `dotprod`, `dpb`, `dpb2`, `fcma`, `fhm`, `flagm`, `fp16`, `frintts`, `jsconv`, `lor`, `lse`, `neon`, `paca`, `pacg`, `pan`, `pmuv3`, `ras`, `rcpc`, `rcpc2`, `rdm`, `sb`, `sha2`, `sha3`, `ssbs`, `vh` | Not qualified |

The tracked-source scan covered **1682 files**, including the pinned GPUI SDK, and recorded all 27 matching compiler/feature-related lines. It does not audit the complete downloaded transitive source graph or certify SDK assembly. Exact compiler outputs and locked native/CPU dependency candidates are preserved in the JSON evidence. No active Rust/C/C++ feature overrides were found in the observed environment.

## Executed observations and a failed control

- A standalone Rust std detector reports 30 individual feature booleans on the current WSL host. Several AVX-512 subsets are absent despite AVX512F being present. This detector does not execute application acceleration paths.
- The genuine `b0a64275...` GUI executable exits 0 in Ubuntu 22.04 under QEMU 6.2 with `qemu64,-pni`. The standard detector in that same image reports SSE3/SSSE3/SSE4.x, AES, SHA, AVX/AVX2/AVX-VNNI, FMA and the queried AVX-512 subsets absent. The weak GLIBC warning remains in the raw log.
- **Instruction enforcement is not validated:** the standalone negative control intentionally contains `vpxor %ymm0,%ymm0,%ymm0`, expects SIGILL/exit 132, but exits 0 under that model. The headless result therefore cannot certify a baseline instruction set. The unexpected result is retained, not converted to a passing control. This is not an application crash.

## Ordered acceptance work

1. Record per-target CPU/ABI baselines, exact Rust and C/C++ inputs, native SDK/dependency feature graphs, and artifact/source hashes. Inventory includes compiler builtins and bundled shared libraries.
2. For each library path, declare the exact optional instruction subsets and OS-state checks; use its existing dispatch and fallback, with independent crypto/media/function oracles. Optimized profile packages must declare their CPU floor.
3. Exercise feature-present and feature-absent paths, error/recovery and concurrency. Validate negative instruction controls before using emulation as evidence.
4. Qualify actual older and current Intel/AMD hardware on Linux/Windows/macOS as applicable. Native GUI/audio/video tests are separate from headless checks.
5. Qualify ARM64 independently, including NEON/platform baseline, optional crypto/CRC/LSE/SVE-family paths, native page size, actual packages and hardware. Missing evidence keeps release blocked.

Exact groups, commands, compiler target matrices and unqualified states: [CPU-ISA-CONTRACT.json](CPU-ISA-CONTRACT.json).
