// @ts-check
/**
 * Контрактные и модульные тесты прозрачного страничного шифрования SQLite
 * (AES-256-GCM 4096 Б) (TASK-1021B):
 * - SUB-1021B-0: Статический и компиляционный контракт `rust/crates/chord-store/src/encrypted_sqlite.rs`
 *   (`#![forbid(unsafe_code)]`, 0 `.unwrap()`/`.expect()` в продуктовом коде, лимиты `scripts/quality-gate.mjs`:
 *   функции < 50 строк, файл < 800 строк, `rustfmt --check --edition 2024`, компиляция
 *   `rustc --test --edition 2024 -D warnings` без предупреждений и прогон встроенных Rust-тестов).
 * - SUB-1021B-1: Отсутствие открытого текста на диске (zero plaintext strings in hex dump).
 * - SUB-1021B-2: Страничное шифрование 4096 Б через AES-256-GCM с уникальным IV на страницу и AAD.
 * - SUB-1021B-3: Целостность WAL-журнала и устойчивость к сбою питания (crash recovery).
 * - SUB-1021B-4: Мутационный гейт `5/5` (`100%` kill rate): повторное использование IV,
 *   повреждённый GCM-тег аутентификации, утечка открытого текста в WAL и др.
 */
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import {
  existsSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { after, before, describe, it } from "node:test";
import { fileURLToPath } from "node:url";

import {
  checkRustSourceSizeLimits,
  evaluateMutationGate,
} from "../../scripts/quality-gate.mjs";
import { scanTextForSecrets } from "../../scripts/secret-scan.mjs";

const ROOT = fileURLToPath(new URL("../../", import.meta.url));
const RUST_MODULE_REL_PATH = "rust/crates/chord-store/src/encrypted_sqlite.rs";
const RUST_MODULE_PATH = join(ROOT, RUST_MODULE_REL_PATH);

/** @type {string} */
let workDir = "";
/** @type {string} */
let testBinPath = "";

/**
 * Компилирует `rust/crates/chord-store/src/encrypted_sqlite.rs` через `rustc --test --edition 2024 -D warnings`.
 * @param {string} sourcePath
 * @param {string} outBin
 */
function compileRustTests(sourcePath, outBin) {
  const res = spawnSync(
    "rustc",
    ["--test", "--edition", "2024", "-D", "warnings", sourcePath, "-o", outBin],
    { encoding: "utf8" }
  );
  if (res.status !== 0) {
    throw new Error(
      `rustc failed (${res.status}):\n${res.stdout}\n${res.stderr}`
    );
  }
}

before(() => {
  workDir = mkdtempSync(join(tmpdir(), "encrypted-sqlite-1021b-"));
  testBinPath = join(workDir, "encrypted_sqlite_test");
});

after(() => {
  if (workDir && existsSync(workDir)) {
    rmSync(workDir, { recursive: true, force: true });
  }
});

describe("TASK-1021B: Transparent SQLite Page Encryption (AES-256-GCM 4096 B)", () => {
  it("SUB-1021B-0: module exists, passes quality-gate, rustfmt, secret-scan, and rustc --test -D warnings", () => {
    assert.ok(
      existsSync(RUST_MODULE_PATH),
      `Expected module at ${RUST_MODULE_REL_PATH}`
    );
    const source = readFileSync(RUST_MODULE_PATH, "utf8");
    const prodCode = source.split("#[cfg(test)]")[0] ?? source;

    assert.equal(
      /\bunwrap\s*\(/.test(prodCode) || /\bexpect\s*\(/.test(prodCode),
      false,
      "Production code must not use .unwrap() or .expect()"
    );

    const sizeCheck = checkRustSourceSizeLimits(RUST_MODULE_REL_PATH, source);
    assert.equal(
      sizeCheck.passed,
      true,
      `Size limit violations: ${sizeCheck.reason}`
    );

    const secretFindings = scanTextForSecrets(source, RUST_MODULE_REL_PATH);
    assert.equal(
      secretFindings.length,
      0,
      `Secret scan findings: ${JSON.stringify(secretFindings)}`
    );

    const fmt = spawnSync(
      "rustfmt",
      ["--check", "--edition", "2024", RUST_MODULE_PATH],
      { encoding: "utf8" }
    );
    assert.equal(fmt.status, 0, `rustfmt failed:\n${fmt.stdout}\n${fmt.stderr}`);

    compileRustTests(RUST_MODULE_PATH, testBinPath);
    const run = spawnSync(testBinPath, ["--nocapture"], { encoding: "utf8" });
    assert.equal(
      run.status,
      0,
      `Rust unit tests failed:\n${run.stdout}\n${run.stderr}`
    );
  });

  it("SUB-1021B-4: 5/5 mutation gate kills IV reuse, corrupted GCM tag, WAL leak, wrong key, and page-swap", () => {
    const source = readFileSync(RUST_MODULE_PATH, "utf8");
    const specs = [
      {
        id: "M1_iv_reuse_constant_nonce",
        operator: "IV_REUSE",
        target: "nonce[4..12].copy_from_slice(&write_counter.to_le_bytes());",
        replacement: "nonce[4..12].copy_from_slice(&0u64.to_le_bytes());",
      },
      {
        id: "M2_corrupted_gcm_tag_bypass",
        operator: "AUTH_TAG_BYPASS",
        target: "if !constant_time_eq(&expected_tag, tag) {",
        replacement: "if false && !constant_time_eq(&expected_tag, tag) {",
      },
      {
        id: "M3_wal_plaintext_leak",
        operator: "WAL_PLAINTEXT_LEAK",
        target: "let encrypted_frame = vfs.encrypt_page(page_no, plaintext_page)?;",
        replacement:
          "let mut encrypted_frame = plaintext_page.to_vec(); encrypted_frame.resize(ENCRYPTED_PAGE_SIZE, 0);",
      },
      {
        id: "M4_wrong_aad_page_number_ignored",
        operator: "AAD_PAGE_SWAP",
        target: "aad[..4].copy_from_slice(&page_no.to_le_bytes());",
        replacement: "aad[..4].copy_from_slice(&0u32.to_le_bytes());",
      },
      {
        id: "M5_no_encryption_passthrough",
        operator: "NO_ENCRYPTION",
        target:
          "let (ciphertext, tag) = aes_256_gcm_encrypt(&self.key, &nonce, &aad, plaintext);",
        replacement:
          "let ciphertext = plaintext.to_vec(); let tag = [0u8; GCM_TAG_LEN];",
      },
    ];

    const mutants = specs.map((s) => {
      const mutatedSource = source.replace(s.target, s.replacement);
      assert.notEqual(
        mutatedSource,
        source,
        `Mutant ${s.id} failed to match target`
      );
      return { id: s.id, moduleName: 'encrypted_sqlite.rs', operator: s.operator, mutatedSource };
    });

    const report = evaluateMutationGate(mutants, (mutant) => {
      const mutFile = join(workDir, `${mutant.id}.rs`);
      const mutBin = join(workDir, `${mutant.id}_bin`);
      writeFileSync(mutFile, mutant.mutatedSource, "utf8");
      try {
        compileRustTests(mutFile, mutBin);
      } catch {
        return { killed: true, reason: "compile error" };
      }
      const run = spawnSync(mutBin, [], { encoding: "utf8" });
      return {
        killed: run.status !== 0,
        reason: run.status !== 0 ? "test failed" : "survived",
      };
    });

    assert.equal(
      report.passed,
      true,
      `Mutation gate failed: ${JSON.stringify(report)}`
    );
  });
});
