# LIB-ARM-08: ELF structure and truthful metadata

The independent oracle is [System V gABI program loading](https://gabi.xinuos.com/elf/07-pheader.html), checked against real clang/lld-produced AArch64 format fixtures and an independent `readelf` control. The tiny fixtures are test inputs only, never Chord binaries, SDK probes accepted as releases, or published artifacts. No target executable is run by this suite.

The verifier now checks ELF header version/size and program-table bounds, each load segment's file/memory ranges, ELF64 address overflow, alignment and offset/address congruence, load ordering, and the interpreter path. A file-backed segment cannot exceed the actual file. Runtime checks retain the same structural rules while allowing their distinct alignment and static-loader contract. Unused `PT_NULL` members are ignored, as the ABI requires.

| Previous behavior | New contract | Reason |
| --- | --- | --- |
| `page_size_bytes = min(PT_LOAD.p_align)` | `load_segment_alignment_bytes` records that alignment; `kernel_page_size_bytes` is null | Static segment metadata does not measure a kernel. |
| Accept load file size greater than memory size | Reject with an integrity error | A mapped image must contain its file-backed bytes. |
| Accept arbitrary multiple-of-16-KiB alignment | Require a power of two and congruent file/virtual addresses | Checking a divisible number alone misses malformed layouts. |
| Runtime checks stop after machine/header identification | Check load structure and bounds before the runtime/GUI split | A valid architecture number is insufficient to describe a loadable image. |
| Decode arbitrary interpreter bytes before validation | Compare the terminated expected byte string, then decode | Malformed bytes produce a typed integrity failure. |

Existing GUI 16 KiB minimum alignment, GNU_RELRO, NOW, non-executable stack, Build ID and interpreter requirements remain. Linker-produced 4 KiB runtime and 16/64 KiB GUI-layout controls test structural distinctions. BSS, no-alignment runtime declarations and ignored headers guard against rejecting valid format cases. None of these controls establish native execution or kernel compatibility.

The initial thirteen tests produced nine expected failures against the original verifier. Independent review additionally reproduced an unconstrained runtime address when `p_align` is zero or one. A new test failed before the minimum 4 KiB Linux ARM64 page-congruence check was added. This constraint does not measure a kernel page size; stricter declared alignment is checked separately. The expanded suite has twenty-one cases. No existing assertion or test was changed; final integrated and mutation counts are recorded in the batch report.

The legacy `arm64-packaging-990` suite still expects the old report key and obsolete fake-builder behavior. Its contract reconciliation belongs to `LIB-ARM-06`; it remains visible and must retain stronger real-build coverage. The new report deliberately does not preserve a misleading numeric kernel-page claim.

Native ARM SDK/build/package/runtime checks and the complete worker gate are separate acceptance evidence. A green format suite cannot clear `ARM64_RELEASE_NOT_QUALIFIED`. Review results and gate failures are recorded in `ARM-SOURCE-BATCH.md` before any acceptance decision.
