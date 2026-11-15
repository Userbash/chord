# LIB-ARM-02: portable compiler inputs

This source slice removes a confirmed portability hazard. It does not qualify an ARM release or reproduce a native SIGILL.

| Profile | Rust CPU | C and C++ CPU selection |
| --- | --- | --- |
| generic | generic | `-march=armv8-a` |
| rpi5 | cortex-a76 | `-mcpu=cortex-a76` |
| asahi | apple-m1 | `-mcpu=apple-m1` |

The generated and checked-in generic configuration no longer globally enables CRC, AES or SHA2. Optimized builds use the selected CPU's compiler feature set without an additional blanket extension override. Optional optimized code in dependencies still requires a target audit and an appropriate minimum CPU or runtime dispatch. Rust's [code generation contract](https://doc.rust-lang.org/reference/attributes/codegen.html#the-target_feature-attribute) makes executing unsupported target features unsafe; changing compiler inputs is only one prerequisite for portable binaries.

The existing target triple, profile names, locked Cargo GUI build, forced target build environment, Build ID, RELRO, NOW, non-executable stack and 16 KiB linker layout are retained. This slice adds no dependency, SDK, handwritten dispatch layer or simulated package. Asahi remains Linux GNU; selecting an Apple CPU does not change the OS ABI to macOS.

Six behavior tests import the real builder, generate configurations and parse TOML with Python's standard library. The CPU worker recorded four expected failures before its source edits, then all six new tests plus the twelve existing integrity checks passed. Five isolated behavior mutations were killed. Raw commands, exits and source hashes are in the batch verification evidence.

Native CPU/loader/GUI/media testing, dependency assembly/build-script admission and genuine package qualification remain open. The 16 KiB linker policy alone establishes neither a running kernel's page size nor 64 KiB compatibility. `LIB-ARM-08` may use this reviewed compiler-input contract for its bounded source checks; neither source slice clears ARM production admission or final release gates.

Rollback: revert the isolated source changes for diagnosis while keeping ARM promotion blocked. A rollback is not permission to distribute a generic binary requiring optional instructions.
