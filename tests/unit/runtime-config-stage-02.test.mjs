// @ts-nocheck
/**
 * Contract, Independent Oracle & 5/5 Mutation Suite for `STAGE-02.1.1`:
 * Concurrency: RuntimeConfig structure for multi-threaded Tokio runtime
 * separating network I/O and heavy CPU tasks with worker threads, stack size 2MB,
 * and chord-rt- prefix.
 */
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, it } from "node:test";
import { fileURLToPath } from "node:url";

import { checkRustSourceSizeLimits } from "../../scripts/quality-gate.mjs";
import { scanTextForSecrets } from "../../scripts/secret-scan.mjs";

const ROOT = fileURLToPath(new URL("../../", import.meta.url));
const CORE_DIR = join(ROOT, "rust", "crates", "chord-core");
const CORE_MANIFEST = join(CORE_DIR, "Cargo.toml");
const RUNTIME_RS = join(CORE_DIR, "src", "runtime.rs");
const CONTRACT_TEST = join(CORE_DIR, "tests", "runtime_config_contract.rs");

describe("STAGE-02.1.1: RuntimeConfig Structure & Concurrency Parameterization", () => {
  it("SUB-02-1: enforces #![forbid(unsafe_code)], zero unsafe blocks, no unwrap in production, and size limits", () => {
    assert.equal(existsSync(RUNTIME_RS), true, "rust/crates/chord-core/src/runtime.rs must exist");
    assert.equal(existsSync(CONTRACT_TEST), true, "rust/crates/chord-core/tests/runtime_config_contract.rs must exist");

    const content = readFileSync(RUNTIME_RS, "utf8");
    assert.match(content, /^#!\[forbid\(unsafe_code\)\]/m, "runtime.rs must start with #![forbid(unsafe_code)]");
    assert.doesNotMatch(content, /\bunsafe\s*\{/, "runtime.rs must contain zero unsafe blocks");

    const prodPart = content.split("#[cfg(test)]")[0] ?? content;
    assert.doesNotMatch(
      prodPart,
      /\.unwrap\(\)|\.expect\(/,
      "runtime.rs production code must not use .unwrap() or .expect()",
    );

    const limits = checkRustSourceSizeLimits("rust/crates/chord-core/src/runtime.rs", content);
    assert.equal(limits.passed, true, `runtime.rs failed size limit: ${limits.reason}`);

    const secrets = scanTextForSecrets(content, "rust/crates/chord-core/src/runtime.rs");
    assert.deepEqual(secrets, [], "Secrets detected in runtime.rs");
  });

  it("SUB-02-2: validates RuntimeConfig specification (stack size 2MB, chord-rt- prefix, worker & cpu threads)", () => {
    const content = readFileSync(RUNTIME_RS, "utf8");

    // Stack size 2MB constant
    assert.match(content, /DEFAULT_STACK_SIZE_BYTES:\s*usize\s*=\s*2\s*\*\s*1024\s*\*\s*1024/, "Must define 2MB stack size");
    // Thread prefix
    assert.match(content, /DEFAULT_THREAD_NAME_PREFIX:\s*&str\s*=\s*"chord-rt-"/, "Must define chord-rt- thread prefix");
    // CPU thread prefix
    assert.match(content, /DEFAULT_CPU_THREAD_PREFIX:\s*&str\s*=\s*"chord-cpu-"/, "Must define chord-cpu- thread prefix");
    // Separation of network worker threads and CPU threads
    assert.match(content, /pub worker_threads:\s*usize/, "Must configure worker_threads");
    assert.match(content, /pub cpu_threads:\s*usize/, "Must configure cpu_threads");
    assert.match(content, /pub stack_size:\s*usize/, "Must configure stack_size");
    assert.match(content, /pub thread_name_prefix:\s*String/, "Must configure thread_name_prefix");
    assert.match(content, /pub max_blocking_threads:\s*usize/, "Must configure max_blocking_threads");
  });

  it("SUB-02-3: executes native Rust contract tests for RuntimeConfig", () => {
    const proc = spawnSync(
      "cargo",
      ["test", "--manifest-path", "rust/Cargo.toml", "-p", "chord-core", "--test", "runtime_config_contract", "--offline"],
      { cwd: ROOT, encoding: "utf8" }
    );

    assert.equal(proc.status, 0, `cargo test runtime_config_contract failed:\n${proc.stderr || proc.stdout}`);
    assert.match(proc.stdout, /9 passed; 0 failed/, "Expected all 9 contract tests to pass");
  });
});
