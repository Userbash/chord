// @ts-nocheck
/**
 * Contract, Independent Crypto-Oracle & 5/5 Mutation Suite for `LIB-SIMPLIFY-05`:
 * Comparative Spike Prototype: Discord `libdave` (C++17 OpenSSL FFI) vs Serein `davey + OpenMLS/RustCrypto`
 * via transparent `DaveVoiceBridge` — with 0 lines of custom cryptographic primitives.
 */
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createCipheriv, createHmac } from "node:crypto";
import { existsSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, it } from "node:test";
import { fileURLToPath } from "node:url";

import { checkRustSourceSizeLimits, evaluateMutationGate } from "../../scripts/quality-gate.mjs";
import { scanTextForSecrets } from "../../scripts/secret-scan.mjs";

const ROOT = fileURLToPath(new URL("../../", import.meta.url));
const SPIKE_DIR = join(ROOT, "rust", "dave-spike");
const SPIKE_MANIFEST = join(SPIKE_DIR, "Cargo.toml");
const SPIKE_SRC_DIR = join(SPIKE_DIR, "src");
const SPIKE_CPP_HEADER = join(SPIKE_DIR, "cpp", "dave.h");
const SPIKE_CPP_SOURCE = join(SPIKE_DIR, "cpp", "dave_c_api.cpp");
const SPEC_DOC = join(ROOT, "docs", "specs", "dave-e2ee-spike-comparison.md");

/**
 * Encodes RFC 9420 `VLBytes` using independent Node.js logic for crypto oracle verification.
 * @param {Buffer} buf
 * @returns {Buffer}
 */
function encodeVlBytes(buf) {
  if (buf.length < 64) {
    return Buffer.concat([Buffer.from([buf.length]), buf]);
  }
  const hdr = Buffer.alloc(2);
  hdr.writeUInt16BE(0x4000 | buf.length, 0);
  return Buffer.concat([hdr, buf]);
}

/**
 * Independent Node.js `node:crypto` oracle for `mlspp::HashRatchet` (`MLS 1.0 key` / `nonce` / `secret`).
 * @param {Buffer} secret16
 * @param {string} shortLabel
 * @param {number} generation
 * @param {number} outLen
 * @returns {Buffer}
 */
function nodeMlsDeriveTreeSecret(secret16, shortLabel, generation, outLen) {
  const fullLabel = Buffer.from(`MLS 1.0 ${shortLabel}`, "utf8");
  const genCtx = Buffer.alloc(4);
  genCtx.writeUInt32BE(generation >>> 0, 0);
  const lenBe = Buffer.alloc(2);
  lenBe.writeUInt16BE(outLen, 0);
  const info = Buffer.concat([lenBe, encodeVlBytes(fullLabel), encodeVlBytes(genCtx)]);

  const chunks = [];
  let prev = Buffer.alloc(0);
  let counter = 0;
  let total = 0;
  while (total < outLen) {
    counter += 1;
    const h = createHmac("sha256", secret16);
    h.update(prev);
    h.update(info);
    h.update(Buffer.from([counter]));
    prev = h.digest();
    chunks.push(prev);
    total += prev.length;
  }
  return Buffer.concat(chunks).subarray(0, outLen);
}

describe("LIB-SIMPLIFY-05: Comparative Spike Prototype (libdave C++ FFI vs Serein davey/OpenMLS) & DaveVoiceBridge", () => {
  it("SUB-501-0: enforces zero custom crypto primitives, #![forbid(unsafe_code)] in src/, size limits (<50 fn / <800 file), secret-scan, and ADR spec completeness", () => {
    assert.equal(existsSync(SPIKE_MANIFEST), true, "rust/dave-spike/Cargo.toml must exist");
    assert.equal(existsSync(SPIKE_CPP_HEADER), true, "rust/dave-spike/cpp/dave.h must exist");
    assert.equal(existsSync(SPIKE_CPP_SOURCE), true, "rust/dave-spike/cpp/dave_c_api.cpp must exist");
    assert.equal(existsSync(SPEC_DOC), true, "docs/specs/dave-e2ee-spike-comparison.md must exist");

    const cargoToml = readFileSync(SPIKE_MANIFEST, "utf8");
    assert.match(cargoToml, /unsafe_code\s*=\s*"deny"/);
    assert.match(cargoToml, /aes-gcm/);
    assert.match(cargoToml, /ring/);
    assert.match(cargoToml, /hkdf/);
    assert.match(cargoToml, /hmac/);
    assert.match(cargoToml, /sha2/);
    assert.match(cargoToml, /zeroize/);

    const srcFiles = readdirSync(SPIKE_SRC_DIR).filter((f) => f.endsWith(".rs"));
    assert.ok(srcFiles.length >= 6, "expected modular Rust source layout in rust/dave-spike/src/");

    const libRs = readFileSync(join(SPIKE_SRC_DIR, "lib.rs"), "utf8");
    assert.match(libRs, /#!\[forbid\(unsafe_code\)\]/);

    // Forbidden custom crypto primitive markers (FIPS-197 S-box, SHA-256 K table, ChaCha constants, manual GHASH)
    const forbiddenCustomCryptoPatterns = [
      /0x428a2f98/i, // SHA-256 K[0]
      /0x6a09e667/i, // SHA-256 H[0]
      /0x6745_?2301/i, // SHA-1 H[0]
      /0x6170_?7865/i, // ChaCha20 "expa"
      /fn\s+sub_bytes\b/,
      /fn\s+mix_columns\b/,
      /fn\s+quarter_round\b/,
    ];

    for (const file of srcFiles) {
      const rel = `rust/dave-spike/src/${file}`;
      const content = readFileSync(join(SPIKE_SRC_DIR, file), "utf8");

      assert.doesNotMatch(content, /\bunsafe\s*\{/, `${rel} must contain zero unsafe blocks`);

      const prodPart = content.split("#[cfg(test)]")[0] ?? content;
      assert.doesNotMatch(
        prodPart,
        /\.unwrap\(\)|\.expect\(/,
        `${rel} production code must not use .unwrap() or .expect()`,
      );

      for (const pat of forbiddenCustomCryptoPatterns) {
        assert.doesNotMatch(content, pat, `${rel} must not contain hand-rolled crypto primitive ${pat}`);
      }

      const limits = checkRustSourceSizeLimits(rel, content);
      assert.equal(limits.passed, true, limits.reason);

      const secrets = scanTextForSecrets(content, rel);
      assert.deepEqual(secrets, [], `Secrets detected in ${rel}`);
    }

    // Verify C++ side uses OpenSSL EVP_* exclusively and has zero hand-rolled crypto tables
    const cppSource = readFileSync(SPIKE_CPP_SOURCE, "utf8");
    assert.match(cppSource, /#include <openssl\/evp\.h>/);
    assert.match(cppSource, /EVP_aes_128_gcm/);
    assert.match(cppSource, /OPENSSL_cleanse/);
    for (const pat of forbiddenCustomCryptoPatterns) {
      assert.doesNotMatch(cppSource, pat, `cpp/dave_c_api.cpp must not contain custom crypto ${pat}`);
    }

    const specText = readFileSync(SPEC_DOC, "utf8");
    assert.match(specText, /libdave/);
    assert.match(specText, /davey/);
    assert.match(specText, /DaveVoiceBridge/);
    assert.match(specText, /```mermaid/);
  });

  it("SUB-501-1: independent node:crypto oracle confirms mlspp HashRatchet and AES-128-GCM 8-byte tag vectors", () => {
    const baseSecret = Buffer.from([
      206, 221, 97, 177, 184, 161, 202, 105, 4, 101, 84, 40, 44, 247, 11, 123,
    ]);
    const key0 = nodeMlsDeriveTreeSecret(baseSecret, "key", 0, 16);
    const nonce0 = nodeMlsDeriveTreeSecret(baseSecret, "nonce", 0, 12);
    assert.deepEqual(
      [...key0],
      [117, 48, 249, 169, 148, 94, 45, 46, 6, 208, 101, 31, 123, 42, 134, 75],
    );
    assert.deepEqual([...nonce0], [48, 30, 95, 75, 116, 9, 15, 152, 94, 114, 107, 178]);

    // NIST SP 800-38D TC4 8-byte truncated tag via Node OpenSSL
    const tc4Key = Buffer.from("feffe9928665731c6d6a8f9467308308", "hex");
    const tc4Iv = Buffer.from("cafebabefacedbaddecaf888", "hex");
    const tc4Aad = Buffer.from("feedfacedeadbeeffeedfacedeadbeefabaddad2", "hex");
    const tc4Pt = Buffer.from(
      "d9313225f88406e5a55909c5aff5269a86a7a9531534f7da2e4c303d8a318a721" +
        "c3c0c95956809532fcf0e2449a6b525b16aedf5aa0de657ba637b39",
      "hex",
    );
    const cipher = createCipheriv("aes-128-gcm", tc4Key, tc4Iv, { authTagLength: 8 });
    cipher.setAAD(tc4Aad);
    const ct = Buffer.concat([cipher.update(tc4Pt), cipher.final()]);
    const tag8 = cipher.getAuthTag();
    assert.equal(tag8.toString("hex"), "5bc94fbc3221a5db");
    assert.equal(ct.length, tc4Pt.length);
  });

  it("SUB-501-2: executes full Rust + C++ FFI comparative spike & cross-backend interop suite cleanly (--offline)", () => {
    const targetDir = join(SPIKE_DIR, "target");
    const run = spawnSync(
      "cargo",
      ["test", "--offline", "--manifest-path", SPIKE_MANIFEST, "--", "--nocapture"],
      {
        cwd: ROOT,
        encoding: "utf8",
        env: { ...process.env, CARGO_TARGET_DIR: targetDir },
        timeout: 120_000,
      },
    );
    assert.equal(
      run.status,
      0,
      `cargo test on dave-spike failed:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /kat_vectors_match_identically_across_libdave_cpp_and_davey_rust \.\.\. ok/);
    assert.match(run.stdout, /subtask_501_1_libdave_cpp_ffi_prototype_full_lifecycle \.\.\. ok/);
    assert.match(run.stdout, /subtask_501_2_davey_rust_and_libdave_cpp_cross_backend_interoperability \.\.\. ok/);
    assert.match(run.stdout, /subtask_502_1_dave_voice_bridge_transparent_gateway_signaling_both_backends \.\.\. ok/);
    assert.match(run.stdout, /subtask_501_3_comparative_benchmark_and_security_dependency_audit \.\.\. ok/);
  });

  it("SUB-501-3: kills 5/5 control mutations in rust/dave-spike (100% mutation kill rate)", () => {
    const targetDir = join(SPIKE_DIR, "target");
    /**
     * Applies a temporary mutation in-place within an isolated copy or restores byte-for-byte in finally,
     * Wait: to never touch the live workspace if interrupted, copy `rust/dave-spike` (excluding `target`) to tmpdir
     * and reuse `CARGO_TARGET_DIR`!
     */
    const scratchRoot = mkdtempSync(join(tmpdir(), "dave-spike-mut-"));
    try {
      const cpRes = spawnSync(
        "cp",
        ["-r", join(SPIKE_DIR, "Cargo.toml"), join(SPIKE_DIR, "Cargo.lock"), join(SPIKE_DIR, "build.rs"), join(SPIKE_DIR, "cpp"), join(SPIKE_DIR, "src"), join(SPIKE_DIR, "tests"), scratchRoot],
        { encoding: "utf8" },
      );
      assert.equal(cpRes.status, 0, `cp to scratch failed: ${cpRes.stderr}`);

      const mutants = [
        {
          id: "MUT-DAVE-01",
          file: "src/protocol.rs",
          description: "Corrupt DAVE magic marker 0xFAFA -> 0xFAFB",
          target: "pub const DAVE_MAGIC_MARKER: [u8; 2] = [0xFA, 0xFA];",
          replacement: "pub const DAVE_MAGIC_MARKER: [u8; 2] = [0xFA, 0xFB];",
        },
        {
          id: "MUT-DAVE-02",
          file: "src/davey_crypto.rs",
          description: "Corrupt MLS exporter label 'Discord Secure Frames v0'",
          target: "pub const USER_MEDIA_KEY_BASE_LABEL: &str = \"Discord Secure Frames v0\";",
          replacement: "pub const USER_MEDIA_KEY_BASE_LABEL: &str = \"Discord Secure Frames v9\";",
        },
        {
          id: "MUT-DAVE-03",
          file: "src/davey_frame.rs",
          description: "Disable AntiReplayWindow duplicate nonce rejection",
          target: "if (self.bitmap[word] & mask) != 0 {\n            return false;\n        }",
          replacement: "if (self.bitmap[word] & mask) != 0 {\n            return true;\n        }",
        },
        {
          id: "MUT-DAVE-04",
          file: "src/bridge.rs",
          description: "Bypass protocol downgrade rejection (allow protocol_version == 0 when active)",
          target: "if protocol_version == 0 && self.active_protocol_version >= DAVE_PROTOCOL_VERSION_V1 {",
          replacement: "if false && protocol_version == 0 && self.active_protocol_version >= DAVE_PROTOCOL_VERSION_V1 {",
        },
        {
          id: "MUT-DAVE-05",
          file: "src/davey_crypto.rs",
          description: "Bypass constant-time confirmation_tag verification in Commit",
          target: "bool::from(expected_tag.ct_eq(received_tag))",
          replacement: "true || bool::from(expected_tag.ct_eq(received_tag))",
        },
      ];

      const gate = evaluateMutationGate(
        mutants.map((m) => ({
          id: m.id,
          moduleName: m.file,
          operator: m.description,
          mutatedSource: m.replacement,
          file: m.file,
          target: m.target,
          replacement: m.replacement,
        })),
        (m) => {
          const spec = /** @type {any} */ (m);
          const filePath = join(scratchRoot, spec.file);
          const orig = readFileSync(filePath, "utf8");
          assert.ok(orig.includes(spec.target), `Mutation target missing in ${spec.file} for ${spec.id}`);
          writeFileSync(filePath, orig.replace(spec.target, spec.replacement), "utf8");
          try {
            const res = spawnSync(
              "cargo",
              ["test", "--offline", "--manifest-path", join(scratchRoot, "Cargo.toml"), "--test", "comparative_spike_contract"],
              {
                cwd: scratchRoot,
                encoding: "utf8",
                env: { ...process.env, CARGO_TARGET_DIR: targetDir },
                timeout: 90_000,
              },
            );
            return {
              killed: res.status !== 0,
              reason: res.status !== 0 ? "test failed as expected" : "mutant survived",
            };
          } finally {
            writeFileSync(filePath, orig, "utf8");
          }
        },
      );
      assert.equal(gate.passed, true, `Surviving mutants: ${JSON.stringify(gate)}`);
      assert.equal(gate.killed, 5);
      assert.equal(gate.total, 5);
    } finally {
      rmSync(scratchRoot, { recursive: true, force: true });
    }
  });
});
