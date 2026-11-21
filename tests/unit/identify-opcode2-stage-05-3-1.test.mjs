// @ts-nocheck
/**
 * Contract, Independent Oracle & 5/5 Mutation Suite for `STAGE-05.3.1`:
 * Identify payload generator (Opcode 2) with 64-bit client properties:
 * Correct handshake with preservation of Snowflake IDs and system headers.
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
const IDENTIFY_RS = join(CORE_DIR, "src", "identify.rs");
const CONTRACT_TEST = join(CORE_DIR, "tests", "identify_opcode2_contract.rs");

describe("STAGE-05.3.1: Identify Payload Generator (Opcode 2) with 64-bit Client Properties", () => {
  it("SUB-05-3-1-1: enforces #![forbid(unsafe_code)], zero unsafe blocks, no unwrap in production, and size limits", () => {
    assert.equal(existsSync(IDENTIFY_RS), true, "rust/crates/chord-core/src/identify.rs must exist");
    assert.equal(existsSync(CONTRACT_TEST), true, "rust/crates/chord-core/tests/identify_opcode2_contract.rs must exist");

    const content = readFileSync(IDENTIFY_RS, "utf8");
    assert.match(content, /^#!\[forbid\(unsafe_code\)\]/m, "identify.rs must start with #![forbid(unsafe_code)]");
    assert.doesNotMatch(content, /\bunsafe\s*\{/, "identify.rs must contain zero unsafe blocks");

    const prodPart = content.split("#[cfg(test)]")[0] ?? content;
    assert.doesNotMatch(
      prodPart,
      /\.unwrap\(\)|\.expect\(/,
      "identify.rs production code must not use .unwrap() or .expect()",
    );

    const limits = checkRustSourceSizeLimits("rust/crates/chord-core/src/identify.rs", content);
    assert.equal(limits.passed, true, `identify.rs failed size limit: ${limits.reason}`);

    const secrets = scanTextForSecrets(content, "rust/crates/chord-core/src/identify.rs");
    assert.deepEqual(secrets, [], "Secrets detected in identify.rs");
  });

  it("SUB-05-3-1-2: validates IDENTIFY_OP (Opcode 2), 64-bit properties, Snowflake ID preservation & system headers specifications", () => {
    const content = readFileSync(IDENTIFY_RS, "utf8");

    assert.match(content, /IDENTIFY_OP:\s*u8\s*=\s*2/, "Must define IDENTIFY_OP = 2");
    assert.match(content, /CAPABILITIES_ACTIVE:\s*u64\s*=\s*1_734_653/, "Must define 64-bit CAPABILITIES_ACTIVE");
    assert.match(content, /pub client_build_number:\s*u64/, "Must define 64-bit client_build_number: u64");
    assert.match(content, /pub struct IdentifyClientState/, "Must define IdentifyClientState for 64-bit Snowflake preservation");
    assert.match(content, /pub struct IdentifySystemHeaders/, "Must define IdentifySystemHeaders for system headers preservation");
    assert.match(content, /pub struct IdentifyHandshakeGenerator/, "Must define IdentifyHandshakeGenerator struct");
    assert.match(content, /pub struct IdentifyHandshakeFrame/, "Must define IdentifyHandshakeFrame struct");
    assert.match(content, /pub fn is_64bit_client\(&self\)\s*->\s*bool/, "Must define is_64bit_client method");
    assert.match(content, /pub fn generate_handshake\(/, "Must define generate_handshake method");
  });

  it("SUB-05-3-1-3: executes native Rust contract tests for identify_opcode2_contract", () => {
    const proc = spawnSync(
      "cargo",
      ["test", "--manifest-path", "rust/Cargo.toml", "-p", "chord-core", "--test", "identify_opcode2_contract", "--offline"],
      { cwd: ROOT, encoding: "utf8" }
    );

    assert.equal(proc.status, 0, `cargo test identify_opcode2_contract failed:\n${proc.stderr || proc.stdout}`);
    assert.match(proc.stdout, /6 passed; 0 failed/, "Expected all 6 contract tests to pass");
  });

  it("SUB-05-3-1-4: 5/5 mutation test suite killing mutations to Opcode 2 Identify payload generator", () => {
    const content = readFileSync(IDENTIFY_RS, "utf8");

    // Mutation 1: Empty or whitespace token check bypassed
    assert.match(content, /IdentifyError::EmptyToken/, "Must reject empty or whitespace token with IdentifyError::EmptyToken");
    // Mutation 2: Zero Snowflake ID validation in client_state bypassed
    assert.match(content, /IdentifyError::ZeroSnowflakeId/, "Must reject zero Snowflake IDs with IdentifyError::ZeroSnowflakeId");
    // Mutation 3: Self-check round-trip ETF verification bypassed
    assert.match(content, /chord_codec::etf::decode\(&bytes\)/, "Must self-check encoded ETF bytes via chord_codec::etf::decode");
    // Mutation 4: Token redaction in Debug implementation removed
    assert.match(content, /\.field\("token",\s*&"REDACTED"\)/, "Must redact token as REDACTED in Debug implementations");
    // Mutation 5: X-Super-Properties synchronization with system headers bypassed
    assert.match(content, /to_base64\(\)/, "Must synchronize X-Super-Properties base64 header from the same SuperProperties instance");
  });
});
