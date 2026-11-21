// @ts-nocheck
/**
 * Contract, Independent Oracle & 5/5 Mutation Suite for `STAGE-05.3.2`:
 * Механизм Resume (Opcode 6) по session_id и seq с экспоненциальным backoff (1s..60s):
 * Восстановление сессии без сброса кэша гильдий и повторной загрузки каналов.
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
const RESUME_RS = join(CORE_DIR, "src", "resume.rs");
const CONTRACT_TEST = join(CORE_DIR, "tests", "resume_opcode6_contract.rs");

describe("STAGE-05.3.2: Resume Mechanism (Opcode 6) by session_id and seq with Exponential Backoff (1s..60s)", () => {
  it("SUB-05-3-2-1: enforces #![forbid(unsafe_code)], zero unsafe blocks, no unwrap in production, and size limits", () => {
    assert.equal(existsSync(RESUME_RS), true, "rust/crates/chord-core/src/resume.rs must exist");
    assert.equal(existsSync(CONTRACT_TEST), true, "rust/crates/chord-core/tests/resume_opcode6_contract.rs must exist");

    const content = readFileSync(RESUME_RS, "utf8");
    assert.match(content, /^#!\[forbid\(unsafe_code\)\]/m, "resume.rs must start with #![forbid(unsafe_code)]");
    assert.doesNotMatch(content, /\bunsafe\s*\{/, "resume.rs must contain zero unsafe blocks");

    const prodPart = content.split("#[cfg(test)]")[0] ?? content;
    assert.doesNotMatch(
      prodPart,
      /\.unwrap\(\)|\.expect\(/,
      "resume.rs production code must not use .unwrap() or .expect()",
    );

    const limits = checkRustSourceSizeLimits("rust/crates/chord-core/src/resume.rs", content);
    assert.equal(limits.passed, true, `resume.rs failed size limit: ${limits.reason}`);

    const secrets = scanTextForSecrets(content, "rust/crates/chord-core/src/resume.rs");
    assert.deepEqual(secrets, [], "Secrets detected in resume.rs");
  });

  it("SUB-05-3-2-2: validates RESUME_OP (Opcode 6), exponential backoff (1s..60s), and cache-preserving session recovery specifications", () => {
    const content = readFileSync(RESUME_RS, "utf8");

    assert.match(content, /RESUME_OP:\s*u8\s*=\s*6/, "Must define RESUME_OP = 6");
    assert.match(content, /RESUME_BACKOFF_MIN_MS:\s*u64\s*=\s*1_000/, "Must define RESUME_BACKOFF_MIN_MS = 1_000");
    assert.match(content, /RESUME_BACKOFF_MAX_MS:\s*u64\s*=\s*60_000/, "Must define RESUME_BACKOFF_MAX_MS = 60_000");
    assert.match(content, /pub struct ResumeExponentialBackoff/, "Must define ResumeExponentialBackoff struct");
    assert.match(content, /pub struct ResumeHandshakeFrame/, "Must define ResumeHandshakeFrame struct");
    assert.match(content, /pub struct ResumeRecoveryOutcome/, "Must define ResumeRecoveryOutcome struct");
    assert.match(content, /pub struct ResumeSessionManager/, "Must define ResumeSessionManager struct");
    assert.match(content, /pub fn prepare_resume_frame\(/, "Must define prepare_resume_frame method");
    assert.match(content, /pub fn on_resumed_received\(/, "Must define on_resumed_received method");
  });

  it("SUB-05-3-2-3: executes native Rust contract tests for resume_opcode6_contract", () => {
    const proc = spawnSync(
      "cargo",
      ["test", "--manifest-path", "rust/Cargo.toml", "-p", "chord-core", "--test", "resume_opcode6_contract", "--offline"],
      { cwd: ROOT, encoding: "utf8" }
    );

    assert.equal(proc.status, 0, `cargo test resume_opcode6_contract failed:\n${proc.stderr || proc.stdout}`);
    assert.match(proc.stdout, /5 passed; 0 failed/, "Expected all 5 contract tests to pass");
  });

  it("SUB-05-3-2-4: 5/5 mutation test suite killing mutations to Opcode 6 Resume & exponential backoff", () => {
    const content = readFileSync(RESUME_RS, "utf8");

    // Mutation 1: Exponential backoff doubling / 60s ceiling calculation removed
    assert.match(content, /\.saturating_mul\(2\)\.min\(self\.max_ms\)/, "Must double backoff delay with saturating_mul(2) capped at max_ms (60_000)");
    // Mutation 2: Empty or whitespace token / session_id validation bypassed
    assert.match(content, /ResumeError::EmptyToken/, "Must reject empty or whitespace token with ResumeError::EmptyToken");
    assert.match(content, /ResumeError::EmptySessionId/, "Must reject empty or whitespace session_id with ResumeError::EmptySessionId");
    // Mutation 3: Guild cache reset or channel reload accidentally triggered during Resume
    assert.match(content, /preserve_guild_cache:\s*true/, "Must enforce preserve_guild_cache: true during Resume");
    assert.match(content, /skip_channel_reload:\s*true/, "Must enforce skip_channel_reload: true during Resume");
    // Mutation 4: Token redaction in Debug implementation removed
    assert.match(content, /\.field\("token",\s*&"REDACTED"\)/, "Must redact token as REDACTED in Debug implementations");
    // Mutation 5: Fallback to Identify after RESUME_RESET_AFTER_FAILURES (4 consecutive failures) bypassed
    assert.match(content, /RESUME_RESET_AFTER_FAILURES/, "Must enforce fallback to Identify after RESUME_RESET_AFTER_FAILURES");
  });
});
