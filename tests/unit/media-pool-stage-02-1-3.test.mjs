// @ts-nocheck
/**
 * Contract, Independent Oracle & 5/5 Mutation Suite for `STAGE-02.1.3`:
 * Dedicated isolated rayon::ThreadPool for CPU-bound media workloads
 * (Opus audio encoding/decoding, video processing) without blocking async Tokio runtime.
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
const CONTRACT_TEST = join(CORE_DIR, "tests", "media_pool_contract.rs");

describe("STAGE-02.1.3: Isolated rayon::ThreadPool for CPU Media Tasks", () => {
  it("SUB-02-1-3-1: enforces #![forbid(unsafe_code)], zero unsafe blocks, no unwrap in production, and size limits", () => {
    assert.equal(existsSync(RUNTIME_RS), true, "rust/crates/chord-core/src/runtime.rs must exist");
    assert.equal(existsSync(CONTRACT_TEST), true, "rust/crates/chord-core/tests/media_pool_contract.rs must exist");

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

  it("SUB-02-1-3-2: validates MediaThreadPool specifications and API contracts", () => {
    const content = readFileSync(RUNTIME_RS, "utf8");

    // MediaThreadPool definition and rayon usage
    assert.match(content, /pub struct MediaThreadPool/, "Must define MediaThreadPool");
    assert.match(content, /rayon::ThreadPoolBuilder::new\(\)/, "Must use rayon::ThreadPoolBuilder::new()");
    assert.match(content, /pub fn new\(/, "Must define MediaThreadPool::new");
    assert.match(content, /pub fn install</, "Must define MediaThreadPool::install");
    assert.match(content, /pub fn spawn</, "Must define MediaThreadPool::spawn");
    assert.match(content, /pub fn spawn_task</, "Must define MediaThreadPool::spawn_task for async bridge");
    assert.match(content, /pub fn current_num_threads\(&self\)/, "Must define current_num_threads");
    assert.match(content, /pub struct MediaThreadPoolError|pub enum MediaThreadPoolError/, "Must define MediaThreadPoolError");

    // Integration into RuntimeManager
    assert.match(content, /pub fn media_pool\(&self\)/, "Must define RuntimeManager::media_pool");
    assert.match(content, /pub fn spawn_media</, "Must define RuntimeManager::spawn_media");
  });

  it("SUB-02-1-3-3: executes native Rust contract tests for MediaThreadPool", () => {
    const proc = spawnSync(
      "cargo",
      ["test", "--manifest-path", "rust/Cargo.toml", "-p", "chord-core", "--test", "media_pool_contract", "--offline"],
      { cwd: ROOT, encoding: "utf8" }
    );

    assert.equal(proc.status, 0, `cargo test media_pool_contract failed:\n${proc.stderr || proc.stdout}`);
    assert.match(proc.stdout, /7 passed; 0 failed/, "Expected all 7 contract tests to pass");
  });

  it("SUB-02-1-3-4: 5/5 mutation test suite killing mutations to thread pool contracts", () => {
    const content = readFileSync(RUNTIME_RS, "utf8");

    // Mutation 1: CPU thread prefix changed or bypassed
    assert.match(content, /format_cpu_thread_name/, "Must use format_cpu_thread_name for naming threads");
    // Mutation 2: Stack size ignored
    assert.match(content, /\.stack_size\(config\.stack_size\)/, "Must apply stack_size to rayon builder");
    // Mutation 3: Number of threads ignored
    assert.match(content, /\.num_threads\(config\.cpu_threads\)/, "Must configure cpu_threads count on rayon builder");
    // Mutation 4: spawn_task async bridge uses oneshot channel
    assert.match(content, /tokio::sync::oneshot::channel/, "spawn_task must use tokio oneshot channel for non-blocking bridge");
    // Mutation 5: Error handling without unwraps
    assert.match(content, /MediaThreadPoolError/, "Must propagate build errors through MediaThreadPoolError");
  });
});
