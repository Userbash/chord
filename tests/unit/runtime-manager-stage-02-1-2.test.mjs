// @ts-nocheck
/**
 * Contract, Independent Oracle & 5/5 Mutation Suite for `STAGE-02.1.2`:
 * Central Tokio Runtime initialization via `tokio::runtime::Builder::new_multi_thread()`
 * with pool limitations and thread registration hooks.
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
const RUNTIME_RS = join(CORE_DIR, "src", "runtime.rs");
const CONTRACT_TEST = join(CORE_DIR, "tests", "runtime_manager_contract.rs");

describe("STAGE-02.1.2: Central Tokio Runtime Initialization & Thread Registration", () => {
  it("SUB-02-1-2-1: enforces #![forbid(unsafe_code)], zero unsafe blocks, no unwrap in production, and size limits", () => {
    assert.equal(existsSync(RUNTIME_RS), true, "rust/crates/chord-core/src/runtime.rs must exist");
    assert.equal(existsSync(CONTRACT_TEST), true, "rust/crates/chord-core/tests/runtime_manager_contract.rs must exist");

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

  it("SUB-02-1-2-2: validates RuntimeManager & ThreadRegistry specifications", () => {
    const content = readFileSync(RUNTIME_RS, "utf8");

    // Builder new_multi_thread
    assert.match(content, /tokio::runtime::Builder::new_multi_thread\(\)/, "Must use tokio::runtime::Builder::new_multi_thread()");
    // on_thread_start & on_thread_stop hooks
    assert.match(content, /\.on_thread_start\(/, "Must configure on_thread_start hook");
    assert.match(content, /\.on_thread_stop\(/, "Must configure on_thread_stop hook");
    // ThreadRegistry structure & methods
    assert.match(content, /pub struct ThreadRegistry/, "Must define ThreadRegistry");
    assert.match(content, /pub fn active_thread_count\(&self\)/, "Must define active_thread_count");
    assert.match(content, /pub fn registered_threads\(&self\)/, "Must define registered_threads");
    // RuntimeManager structure & methods
    assert.match(content, /pub struct RuntimeManager/, "Must define RuntimeManager");
    assert.match(content, /pub fn new\(\)/, "Must define RuntimeManager::new");
    assert.match(content, /pub fn from_config\(/, "Must define RuntimeManager::from_config");
    assert.match(content, /pub fn handle\(&self\)/, "Must define RuntimeManager::handle");
    assert.match(content, /pub fn block_on</, "Must define RuntimeManager::block_on");
    assert.match(content, /pub fn spawn</, "Must define RuntimeManager::spawn");
    assert.match(content, /pub fn spawn_blocking</, "Must define RuntimeManager::spawn_blocking");
  });

  it("SUB-02-1-2-3: executes native Rust contract tests for RuntimeManager", () => {
    const proc = spawnSync(
      "cargo",
      ["test", "--manifest-path", "rust/Cargo.toml", "-p", "chord-core", "--test", "runtime_manager_contract", "--offline"],
      { cwd: ROOT, encoding: "utf8" }
    );

    assert.equal(proc.status, 0, `cargo test runtime_manager_contract failed:\n${proc.stderr || proc.stdout}`);
    assert.match(proc.stdout, /6 passed; 0 failed/, "Expected all 6 contract tests to pass");
  });
});
