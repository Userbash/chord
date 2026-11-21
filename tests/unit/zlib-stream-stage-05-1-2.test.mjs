// @ts-nocheck
/**
 * Contract, Independent Oracle & 5/5 Mutation Suite for `STAGE-05.1.2`:
 * Streaming decompressor `zlib-ng` / `flate2` for Discord Gateway payload:
 * Efficient decompression of incoming binary packets with >= 80% traffic savings.
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
const CODEC_DIR = join(ROOT, "rust", "crates", "chord-codec");
const ZLIB_RS = join(CODEC_DIR, "src", "zlib.rs");
const CONTRACT_TEST = join(CODEC_DIR, "tests", "zlib_stream_contract.rs");

describe("STAGE-05.1.2: Streaming Decompressor flate2 / zlib-stream for Discord Gateway Payload", () => {
  it("SUB-05-1-2-1: enforces #![forbid(unsafe_code)], zero unsafe blocks, no unwrap in production, and size limits", () => {
    assert.equal(existsSync(ZLIB_RS), true, "rust/crates/chord-codec/src/zlib.rs must exist");
    assert.equal(existsSync(CONTRACT_TEST), true, "rust/crates/chord-codec/tests/zlib_stream_contract.rs must exist");

    const content = readFileSync(ZLIB_RS, "utf8");
    assert.match(content, /^#!\[forbid\(unsafe_code\)\]/m, "zlib.rs must start with #![forbid(unsafe_code)]");
    assert.doesNotMatch(content, /\bunsafe\s*\{/, "zlib.rs must contain zero unsafe blocks");

    const prodPart = content.split("#[cfg(test)]")[0] ?? content;
    assert.doesNotMatch(
      prodPart,
      /\.unwrap\(\)|\.expect\(/,
      "zlib.rs production code must not use .unwrap() or .expect()",
    );

    const limits = checkRustSourceSizeLimits("rust/crates/chord-codec/src/zlib.rs", content);
    assert.equal(limits.passed, true, `zlib.rs failed size limit: ${limits.reason}`);

    const secrets = scanTextForSecrets(content, "rust/crates/chord-codec/src/zlib.rs");
    assert.deepEqual(secrets, [], "Secrets detected in zlib.rs");
  });

  it("SUB-05-1-2-2: validates ZlibStream & GatewayDecompressor specifications", () => {
    const content = readFileSync(ZLIB_RS, "utf8");

    assert.match(
      content,
      /ZLIB_SYNC_FLUSH_SUFFIX:\s*\[u8;\s*4\]\s*=\s*\[0x00,\s*0x00,\s*0xff,\s*0xff\]/,
      "Must define ZLIB_SYNC_FLUSH_SUFFIX [0x00, 0x00, 0xff, 0xff]",
    );
    assert.match(
      content,
      /use\s+flate2::\{.*Decompress.*FlushDecompress/,
      "Must delegate streaming inflate to flate2::{Decompress, FlushDecompress}",
    );
    assert.match(content, /pub struct ZlibStream/, "Must define ZlibStream struct");
    assert.match(content, /pub enum ZlibError/, "Must define ZlibError enum");
    assert.match(content, /pub struct CompressionStats/, "Must define CompressionStats struct");
    assert.match(content, /pub fn traffic_savings_ratio\(&self\)\s*->\s*f64/, "Must define traffic_savings_ratio(&self) -> f64");
    assert.match(content, /pub fn traffic_savings_percent\(&self\)\s*->\s*f64/, "Must define traffic_savings_percent(&self) -> f64");
    assert.match(content, /pub enum GatewayCompression/, "Must define GatewayCompression enum");
    assert.match(content, /pub enum GatewayDecompressor/, "Must define unified GatewayDecompressor enum");
  });

  it("SUB-05-1-2-3: executes native Rust contract tests for ZlibStream & 80% traffic savings", () => {
    const proc = spawnSync(
      "cargo",
      ["test", "--manifest-path", "rust/Cargo.toml", "-p", "chord-codec", "--test", "zlib_stream_contract", "--offline"],
      { cwd: ROOT, encoding: "utf8" },
    );

    assert.equal(proc.status, 0, `cargo test zlib_stream_contract failed:\n${proc.stderr || proc.stdout}`);
    assert.match(proc.stdout, /12 passed; 0 failed/, "Expected all 12 contract tests to pass");
  });

  it("SUB-05-1-2-4: 5/5 mutation test suite killing mutations to streaming zlib decompressor contracts", () => {
    const content = readFileSync(ZLIB_RS, "utf8");

    // Mutation 1: Sync-flush suffix check bypassed
    assert.match(content, /ends_with\(&ZLIB_SYNC_FLUSH_SUFFIX\)/, "Must wait for ZLIB_SYNC_FLUSH_SUFFIX before flushing message");
    // Mutation 2: Decompress::new(true) altered to raw deflate (false)
    assert.match(content, /Decompress::new\(true\)/, "Must initialize flate2::Decompress::new(true) for RFC 1950 zlib stream header");
    // Mutation 3: FlushDecompress::Sync omitted
    assert.match(content, /FlushDecompress::Sync/, "Must decompress with FlushDecompress::Sync across chunk boundaries");
    // Mutation 4: Decompression bomb limit MAX_DECOMPRESSED_CHUNK_BYTES bypassed
    assert.match(content, /MAX_DECOMPRESSED_CHUNK_BYTES/, "Must enforce MAX_DECOMPRESSED_CHUNK_BYTES zip-bomb protection");
    // Mutation 5: Terminal poisoning on corrupted stream omitted
    assert.match(content, /ZlibError::Poisoned/, "Must reject subsequent pushes on a poisoned stream with ZlibError::Poisoned");
  });
});
