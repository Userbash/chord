// @ts-check
/**
 * Контрактные и модульные тесты верификации Forward Secrecy и смены эпох DAVE MLS v1
 * при выходе пира (TASK-979):
 * - SUB-979-0: Статический и структурный контракт `rust/discord-voice/src/dave/tests.rs`
 *   (`#![forbid(unsafe_code)]`, 0 `.unwrap()`/`.expect()` в prod-коде, лимиты `quality-gate.mjs`
 *   < 50 строк на функцию и < 800 строк на файл, `rustfmt --check --edition 2024`,
 *   `rustc --test --edition 2024 -D warnings`, 0 утечек в `secret-scan.mjs`, маскирование
 *   `<скрыто>` в `Debug`, очистка `zeroize_slice` через `black_box(0)` и `Drop`, сверка с
 *   независимыми оракулами `/home/sanya/re-deep/voice-deep.md`, `/home/sanya/re-deep/native-re.md`,
 *   `waves/kanban-board.json` и `rust/crates/discord-voice/src/dave_v1/`).
 * - SUB-979-1: Независимый крипто-оракул `node:crypto` для RFC 9420 MLS 1.0 Key Schedule
 *   (SHA-256, HMAC-SHA256, HKDF-Extract/Expand, `VLBytes`, `GroupContext`, `ExpandWithLabel`,
 *   `joiner_secret`, `welcome_secret`, `epoch_secret`, `exporter_secret`, `epoch_authenticator`,
 *   `confirmation_key`, `membership_key`, `init_secret`, `confirmation_tag`, экспортёр
 *   `"Discord Secure Frames v0"` и `ProposalRef`).
 * - SUB-979-2: Немедленная инвалидация и физическое зануление старого `EpochSecret` при
 *   `RemoveProposal` (op:27) + `Commit` (op:29) + `ExecuteTransition` (op:22), ротация
 *   персональных `DaveHashRatchet`, удаление дешифратора вышедшего пира и внутриэпоховая
 *   Forward Secrecy (`delete_keys_up_to`).
 * - SUB-979-3: Защита от повторов (`AntiReplayWindow` на 1024 пакета) и гарантированное
 *   отклонение повторных/запоздавших пакетов старой эпохи (`StaleEpochPacketRejected` /
 *   `ReplayOrExpiredNonce`) и пакетов удалённого пира (`PeerRemovedFromEpoch`).
 * - SUB-979-4: Покрытие всех 9 обязательных измерений `TEST-POLICY.md` (позитивные,
 *   негативные, граничные `u64::MAX`/`u32::MAX`/`gap=250`, ошибки `VLBytes`/фреймов,
 *   пустые входы, переходы и DTX тишина `[0xF8, 0xFF, 0xFE]`, изоляция прав, восстановление
 *   через `REVOKE` и отказ битого `Commit`, наблюдаемость `ForwardSecrecyAuditReport`).
 * - SUB-979-5: Мутационный гейт `5/5` (`100%` kill rate) по контракту `scripts/quality-gate.mjs`.
 */
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash, createHmac } from "node:crypto";
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
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
const MODULE_REL_PATH = "rust/discord-voice/src/dave/tests.rs";
const MODULE_PATH = join(ROOT, MODULE_REL_PATH);
const KANBAN_PATH = join(ROOT, "waves", "kanban-board.json");
const CRATE_DAVE_MOD_PATH = join(
  ROOT,
  "rust",
  "crates",
  "discord-voice",
  "src",
  "dave_v1",
  "mod.rs",
);
const CRATE_DAVE_MLS_PATH = join(
  ROOT,
  "rust",
  "crates",
  "discord-voice",
  "src",
  "dave_v1",
  "mls.rs",
);
const CRATE_DAVE_SESSION_PATH = join(
  ROOT,
  "rust",
  "crates",
  "discord-voice",
  "src",
  "dave_v1",
  "session.rs",
);
const RE_VOICE_DEEP_PATH = "/home/sanya/re-deep/voice-deep.md";
const RE_NATIVE_PATH = "/home/sanya/re-deep/native-re.md";

/** @type {string} */
let workDir = "";
/** @type {string} */
let testBinPath = "";

/**
 * Компилирует `rust/discord-voice/src/dave/tests.rs` через `rustc --test --edition 2024 -D warnings`.
 * @param {string} sourcePath
 * @param {string} outputBin
 * @returns {{ status: number | null, stdout: string, stderr: string }}
 */
function compileRustModuleTests(sourcePath, outputBin) {
  const res = spawnSync(
    "rustc",
    [
      "--test",
      "--edition",
      "2024",
      "-D",
      "warnings",
      sourcePath,
      "-o",
      outputBin,
    ],
    {
      cwd: ROOT,
      encoding: "utf8",
      timeout: 30_000,
    },
  );
  return {
    status: res.status,
    stdout: res.stdout ?? "",
    stderr: res.stderr ?? "",
  };
}

/**
 * Запускает скомпилированный тестовый бинарник с опциональным фильтром.
 * @param {string} binPath
 * @param {string} [filter]
 * @returns {{ status: number | null, stdout: string, stderr: string }}
 */
function runCompiledRustTests(binPath, filter) {
  const args = filter ? [filter, "--nocapture"] : ["--nocapture"];
  const res = spawnSync(binPath, args, {
    cwd: ROOT,
    encoding: "utf8",
    timeout: 15_000,
  });
  return {
    status: res.status,
    stdout: res.stdout ?? "",
    stderr: res.stderr ?? "",
  };
}

/**
 * Независимая реализация `VLBytes` RFC 9420 §2.1.2 на Node.js для крипто-оракула.
 * @param {Buffer} buf
 * @returns {Buffer}
 */
function oracleEncodeVlBytes(buf) {
  const len = buf.length;
  if (len < 64) {
    return Buffer.concat([Buffer.from([len]), buf]);
  }
  if (len < 16384) {
    const hdr = Buffer.alloc(2);
    hdr.writeUInt16BE(0x4000 | len, 0);
    return Buffer.concat([hdr, buf]);
  }
  const hdr = Buffer.alloc(4);
  hdr.writeUInt32BE((0x80000000 | len) >>> 0, 0);
  return Buffer.concat([hdr, buf]);
}

/**
 * Независимый `HKDF-Extract(salt, ikm)` на `node:crypto` HMAC-SHA256.
 * @param {Buffer} salt
 * @param {Buffer} ikm
 * @returns {Buffer}
 */
function oracleHkdfExtract(salt, ikm) {
  const key = salt.length === 0 ? Buffer.alloc(32, 0) : salt;
  return createHmac("sha256", key).update(ikm).digest();
}

/**
 * Независимый `HKDF-Expand(prk, info, length)` на `node:crypto` HMAC-SHA256.
 * @param {Buffer} prk
 * @param {Buffer} info
 * @param {number} outLen
 * @returns {Buffer}
 */
function oracleHkdfExpand(prk, info, outLen) {
  /** @type {Buffer[]} */
  const chunks = [];
  let prev = Buffer.alloc(0);
  let total = 0;
  let counter = 1;
  while (total < outLen) {
    prev = createHmac("sha256", prk)
      .update(prev)
      .update(info)
      .update(Buffer.from([counter]))
      .digest();
    chunks.push(prev);
    total += prev.length;
    counter += 1;
  }
  return Buffer.concat(chunks).subarray(0, outLen);
}

/**
 * Независимый `ExpandWithLabel(Secret, Label, Context, Length)` по RFC 9420 §8.
 * @param {Buffer} secret
 * @param {string} label
 * @param {Buffer} context
 * @param {number} length
 * @returns {Buffer}
 */
function oracleMlsExpandWithLabel(secret, label, context, length) {
  const lenBuf = Buffer.alloc(2);
  lenBuf.writeUInt16BE(length, 0);
  const fullLabel = Buffer.from(`MLS 1.0 ${label}`, "utf8");
  const kdfLabel = Buffer.concat([
    lenBuf,
    oracleEncodeVlBytes(fullLabel),
    oracleEncodeVlBytes(context),
  ]);
  return oracleHkdfExpand(secret, kdfLabel, length);
}

/**
 * Независимая сериализация `GroupContext` RFC 9420 §8.1.
 * @param {{ groupId: Buffer, epoch: bigint, treeHash: Buffer, confirmedTranscriptHash: Buffer, extensions: Buffer }} gc
 * @returns {Buffer}
 */
function oracleSerializeGroupContext(gc) {
  const header = Buffer.from([0x00, 0x01, 0x00, 0x01]);
  const epochBuf = Buffer.alloc(8);
  epochBuf.writeBigUInt64BE(gc.epoch, 0);
  return Buffer.concat([
    header,
    oracleEncodeVlBytes(gc.groupId),
    epochBuf,
    oracleEncodeVlBytes(gc.treeHash),
    oracleEncodeVlBytes(gc.confirmedTranscriptHash),
    oracleEncodeVlBytes(gc.extensions),
  ]);
}

/**
 * Независимая деривация всех секретов эпохи MLS 1.0 (RFC 9420 §8) + медиа-ключей Discord DAVE v1.
 * @param {Buffer} initSecret
 * @param {Buffer} commitSecret
 * @param {Buffer} groupContextBytes
 * @param {Buffer} confirmedTranscriptHash
 * @param {bigint[]} userIds
 */
function oracleDeriveEpoch(
  initSecret,
  commitSecret,
  groupContextBytes,
  confirmedTranscriptHash,
  userIds,
) {
  const extracted = oracleHkdfExtract(initSecret, commitSecret);
  const joinerSecret = oracleMlsExpandWithLabel(
    extracted,
    "joiner",
    groupContextBytes,
    32,
  );
  const memberSecret = oracleHkdfExtract(joinerSecret, Buffer.alloc(32, 0));
  const welcomeSecret = oracleMlsExpandWithLabel(
    memberSecret,
    "welcome",
    Buffer.alloc(0),
    32,
  );
  const epochSecret = oracleMlsExpandWithLabel(
    memberSecret,
    "epoch",
    groupContextBytes,
    32,
  );
  const exporterSecret = oracleMlsExpandWithLabel(
    epochSecret,
    "exporter",
    Buffer.alloc(0),
    32,
  );
  const epochAuthenticator = oracleMlsExpandWithLabel(
    epochSecret,
    "authentication",
    Buffer.alloc(0),
    32,
  );
  const confirmationKey = oracleMlsExpandWithLabel(
    epochSecret,
    "confirm",
    Buffer.alloc(0),
    32,
  );
  const membershipKey = oracleMlsExpandWithLabel(
    epochSecret,
    "membership",
    Buffer.alloc(0),
    32,
  );
  const nextInitSecret = oracleMlsExpandWithLabel(
    epochSecret,
    "init",
    Buffer.alloc(0),
    32,
  );
  const confirmationTag = createHmac("sha256", confirmationKey)
    .update(confirmedTranscriptHash)
    .digest();

  /** @type {Record<string, string>} */
  const userMediaKeys = {};
  for (const uid of userIds) {
    const uidLe = Buffer.alloc(8);
    uidLe.writeBigUInt64LE(uid, 0);
    const derived = oracleMlsExpandWithLabel(
      exporterSecret,
      "Discord Secure Frames v0",
      Buffer.alloc(0),
      32,
    );
    const ctxHash = createHash("sha256").update(uidLe).digest();
    const mediaKey = oracleMlsExpandWithLabel(derived, "exported", ctxHash, 16);
    userMediaKeys[uid.toString()] = mediaKey.toString("hex");
  }

  return {
    joinerSecret: joinerSecret.toString("hex"),
    welcomeSecret: welcomeSecret.toString("hex"),
    epochSecret: epochSecret.toString("hex"),
    exporterSecret: exporterSecret.toString("hex"),
    epochAuthenticator: epochAuthenticator.toString("hex"),
    confirmationKey: confirmationKey.toString("hex"),
    membershipKey: membershipKey.toString("hex"),
    initSecret: nextInitSecret.toString("hex"),
    confirmationTag: confirmationTag.toString("hex"),
    userMediaKeys,
    nextInitSecretBuf: nextInitSecret,
  };
}

describe("TASK-979: Верификация Forward Secrecy и смены эпох DAVE MLS v1 при выходе пира (rust/discord-voice/src/dave/tests.rs)", () => {
  before(() => {
    workDir = mkdtempSync(join(tmpdir(), "dave-mls-fs-979-"));
    testBinPath = join(workDir, "dave_mls_fs_979_test");
  });

  after(() => {
    if (workDir && existsSync(workDir)) {
      rmSync(workDir, { recursive: true, force: true });
    }
  });

  it("SUB-979-0: модуль rust/discord-voice/src/dave/tests.rs существует, проходит rustfmt 2024, лимиты размера (<50 строк на fn, <800 на файл), secret-scan и компилируется без предупреждений", () => {
    assert.ok(
      existsSync(MODULE_PATH),
      `Модуль ${MODULE_REL_PATH} должен существовать`,
    );

    const source = readFileSync(MODULE_PATH, "utf8");

    // 1. Запрет unsafe, отсутствие .unwrap()/.expect() в prod-коде, наличие zeroize_slice и <скрыто>
    assert.match(source, /#!\[forbid\(unsafe_code\)\]/);
    assert.match(source, /pub fn zeroize_slice/);
    assert.match(source, /black_box\(0\)/);
    assert.match(source, /impl Drop for DaveMlsEpochSecrets/);
    assert.match(source, /impl Drop for DaveHashRatchet/);
    assert.match(source, /<скрыто>/);

    const prodCode = source.split("#[cfg(test)]")[0] ?? source;
    assert.doesNotMatch(
      prodCode,
      /\.unwrap\(\)|\.expect\(/,
      "В prod-части модуля не должно быть вызовов .unwrap() или .expect()",
    );

    // 2. Лимиты размера кода по контракту scripts/quality-gate.mjs (< 50 строк на fn, < 800 строк на файл)
    const sizeCheck = checkRustSourceSizeLimits(MODULE_REL_PATH, source);
    assert.equal(
      sizeCheck.passed,
      true,
      `Нарушение лимитов размера кода: ${sizeCheck.reason}`,
    );
    assert.ok(sizeCheck.fileLines > 200 && sizeCheck.fileLines < 800);

    // 3. Проверка отсутствия зашитых секретов через scripts/secret-scan.mjs
    const secretHits = scanTextForSecrets(source, MODULE_REL_PATH);
    assert.deepEqual(
      secretHits,
      [],
      `Обнаружены потенциальные секреты в ${MODULE_REL_PATH}: ${JSON.stringify(secretHits)}`,
    );

    // 4. Независимый оракул: карточка TASK-979 в waves/kanban-board.json
    const kanban = JSON.parse(readFileSync(KANBAN_PATH, "utf8"));
    const card979 = kanban.cards.find((/** @type {any} */ c) => c.id === "TASK-979");
    assert.ok(card979, "Карточка TASK-979 должна быть в waves/kanban-board.json");
    assert.equal(card979.department, "G5 Voice / Platform");
    assert.equal(card979.interactive_states[0].element, "peer-leave");
    assert.equal(card979.interactive_states[0].default, "epoch N");
    assert.equal(card979.interactive_states[0].hover, "trigger remove");
    assert.equal(card979.interactive_states[0].active, "epoch N+1 derived");
    assert.match(source, /"peer-leave"/);
    assert.match(source, /"epoch N"/);
    assert.match(source, /"trigger remove"/);
    assert.match(source, /"epoch N\+1 derived"/);

    // 5. Сверка с RE-оракулами (/home/sanya/re-deep/voice-deep.md, native-re.md) и крейтом discord-voice
    if (existsSync(RE_VOICE_DEEP_PATH)) {
      const voiceDeep = readFileSync(RE_VOICE_DEEP_PATH, "utf8");
      assert.match(voiceDeep, /dave_protocol_version:1/);
      assert.match(voiceDeep, /DAVE protocol init with protocol version: 1/);
    }
    if (existsSync(RE_NATIVE_PATH)) {
      const nativeRe = readFileSync(RE_NATIVE_PATH, "utf8");
      assert.match(nativeRe, /src\/mls_key_ratchet\.cpp/);
      assert.match(nativeRe, /src\/key_schedule\.cpp/);
    }
    const crateDaveMod = readFileSync(CRATE_DAVE_MOD_PATH, "utf8");
    const crateDaveMls = readFileSync(CRATE_DAVE_MLS_PATH, "utf8");
    const crateDaveSession = readFileSync(CRATE_DAVE_SESSION_PATH, "utf8");
    assert.match(crateDaveMod, /Discord Secure Frames v0/);
    assert.match(crateDaveMls, /delete_keys_up_to/);
    assert.match(crateDaveSession, /DaveMlsProposal/);

    // 6. Проверка форматирования rustfmt --edition 2024
    const fmtCheck = spawnSync(
      "rustfmt",
      ["--check", "--edition", "2024", MODULE_PATH],
      { cwd: ROOT, encoding: "utf8" },
    );
    assert.equal(
      fmtCheck.status,
      0,
      `rustfmt --check не прошёл:\n${fmtCheck.stderr || fmtCheck.stdout}`,
    );

    // 7. Компиляция rustc --test --edition 2024 -D warnings
    const compiled = compileRustModuleTests(MODULE_PATH, testBinPath);
    assert.equal(
      compiled.status,
      0,
      `Ошибка компиляции rustc --test -D warnings:\n${compiled.stderr}`,
    );
    assert.equal(
      compiled.stderr.includes("warning:"),
      false,
      `Не должно быть предупреждений компилятора:\n${compiled.stderr}`,
    );
  });

  it("SUB-979-1: деривация секретов эпох RFC 9420 MLS 1.0 и экспортёра 'Discord Secure Frames v0' побитово совпадает с независимым оракулом node:crypto", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    const run = runCompiledRustTests(testBinPath, "test_sub_979_1_");
    assert.equal(
      run.status,
      0,
      `Тесты SUB-979-1 упали:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_979_1_.*ok/);

    const markerLine = run.stdout
      .split("\n")
      .find((line) => line.startsWith("DAVE_FS_VECTOR_JSON:"));
    assert.ok(
      markerLine,
      "Тест SUB-979-1 должен вывести контрольный вектор DAVE_FS_VECTOR_JSON",
    );
    const rustVec = JSON.parse(
      markerLine.slice("DAVE_FS_VECTOR_JSON:".length),
    );

    // Независимый расчёт эпохи 1 и эпохи 2 (после выхода пира 3003) через node:crypto
    const initSecret1 = Buffer.alloc(32, 0x21);
    const commitSecret1 = Buffer.alloc(32, 0x42);
    const gc1Bytes = oracleSerializeGroupContext({
      groupId: Buffer.from("voice-chan-979", "utf8"),
      epoch: 1n,
      treeHash: Buffer.alloc(32, 0x11),
      confirmedTranscriptHash: Buffer.alloc(32, 0x22),
      extensions: Buffer.alloc(0),
    });
    const oracleE1 = oracleDeriveEpoch(
      initSecret1,
      commitSecret1,
      gc1Bytes,
      Buffer.alloc(32, 0x22),
      [1001n, 2002n, 3003n],
    );

    assert.equal(rustVec.epoch1.joiner_secret, oracleE1.joinerSecret);
    assert.equal(rustVec.epoch1.welcome_secret, oracleE1.welcomeSecret);
    assert.equal(rustVec.epoch1.epoch_secret, oracleE1.epochSecret);
    assert.equal(rustVec.epoch1.exporter_secret, oracleE1.exporterSecret);
    assert.equal(rustVec.epoch1.epoch_authenticator, oracleE1.epochAuthenticator);
    assert.equal(rustVec.epoch1.confirmation_key, oracleE1.confirmationKey);
    assert.equal(rustVec.epoch1.membership_key, oracleE1.membershipKey);
    assert.equal(rustVec.epoch1.init_secret, oracleE1.initSecret);
    assert.equal(rustVec.epoch1.confirmation_tag, oracleE1.confirmationTag);
    assert.deepEqual(rustVec.epoch1.user_media_keys, oracleE1.userMediaKeys);

    // Эпоха 2 после выхода пира 3003 (init_secret берётся из эпохи 1!)
    const commitSecret2 = Buffer.alloc(32, 0x99);
    const gc2Bytes = oracleSerializeGroupContext({
      groupId: Buffer.from("voice-chan-979", "utf8"),
      epoch: 2n,
      treeHash: Buffer.alloc(32, 0x33),
      confirmedTranscriptHash: Buffer.alloc(32, 0x44),
      extensions: Buffer.alloc(0),
    });
    const oracleE2 = oracleDeriveEpoch(
      oracleE1.nextInitSecretBuf,
      commitSecret2,
      gc2Bytes,
      Buffer.alloc(32, 0x44),
      [1001n, 2002n],
    );

    assert.equal(rustVec.epoch2.joiner_secret, oracleE2.joinerSecret);
    assert.equal(rustVec.epoch2.welcome_secret, oracleE2.welcomeSecret);
    assert.equal(rustVec.epoch2.epoch_secret, oracleE2.epochSecret);
    assert.equal(rustVec.epoch2.exporter_secret, oracleE2.exporterSecret);
    assert.equal(rustVec.epoch2.epoch_authenticator, oracleE2.epochAuthenticator);
    assert.equal(rustVec.epoch2.confirmation_key, oracleE2.confirmationKey);
    assert.equal(rustVec.epoch2.membership_key, oracleE2.membershipKey);
    assert.equal(rustVec.epoch2.init_secret, oracleE2.initSecret);
    assert.equal(rustVec.epoch2.confirmation_tag, oracleE2.confirmationTag);
    assert.deepEqual(rustVec.epoch2.user_media_keys, oracleE2.userMediaKeys);

    // Проверка, что все секреты и медиа-ключи эпохи 2 гарантированно изменились относительно эпохи 1
    assert.notEqual(oracleE2.epochSecret, oracleE1.epochSecret);
    assert.notEqual(oracleE2.exporterSecret, oracleE1.exporterSecret);
    assert.notEqual(oracleE2.initSecret, oracleE1.initSecret);
    assert.notEqual(
      oracleE2.userMediaKeys["1001"],
      oracleE1.userMediaKeys["1001"],
    );
    assert.notEqual(
      oracleE2.userMediaKeys["2002"],
      oracleE1.userMediaKeys["2002"],
    );
  });

  it("SUB-979-2: при выходе пира (RemoveProposal op:27 + Commit op:29 + ExecuteTransition op:22) старый EpochSecret немедленно инвалидируется и зануляется, а рэтчеты участников меняются", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    const source = readFileSync(MODULE_PATH, "utf8");
    for (const requiredSymbol of [
      "DaveMlsEpochSecrets",
      "DaveMlsKeySchedule",
      "DaveHashRatchet",
      "DaveVoiceSession",
      "DaveMlsProposal",
      "invalidate_and_zeroize",
      "delete_keys_up_to",
      "verify_peer_leave_forward_secrecy",
      "ForwardSecrecyAuditReport",
    ]) {
      assert.match(
        source,
        new RegExp(`\\b${requiredSymbol}\\b`),
        `В модуле должен присутствовать контракт Forward Secrecy: ${requiredSymbol}`,
      );
    }

    const run = runCompiledRustTests(testBinPath, "test_sub_979_2_");
    assert.equal(
      run.status,
      0,
      `Тесты SUB-979-2 (peer leave epoch invalidation & secret rotation) упали:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_979_2_.*ok/);
  });

  it("SUB-979-3: отклоняет повторные пакеты (AntiReplayWindow) и любые пакеты старой эпохи или вышедшего пира после смены эпохи", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    const source = readFileSync(MODULE_PATH, "utf8");
    for (const requiredSymbol of [
      "AntiReplayWindow",
      "DAVE_REPLAY_WINDOW_SIZE",
      "ReplayOrExpiredNonce",
      "StaleEpochPacketRejected",
      "PeerRemovedFromEpoch",
      "purge_previous_epochs",
    ]) {
      assert.match(
        source,
        new RegExp(`\\b${requiredSymbol}\\b`),
        `В модуле должен присутствовать контракт защиты от реплея и старых эпох: ${requiredSymbol}`,
      );
    }

    const run = runCompiledRustTests(testBinPath, "test_sub_979_3_");
    assert.equal(
      run.status,
      0,
      `Тесты SUB-979-3 (replay & stale epoch rejection) упали:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_979_3_.*ok/);
  });

  it("SUB-979-4: покрывает все 9 измерений TEST-POLICY.md (граничные Snowflake/epoch/generation, ошибки VLBytes/Commit, DTX тишина, REVOKE отмена удаления, наблюдаемость)", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    const run = runCompiledRustTests(testBinPath, "test_sub_979_4_");
    assert.equal(
      run.status,
      0,
      `Тесты SUB-979-4 (9 dimensions of TEST-POLICY.md) упали:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_979_4_.*ok/);
  });

  it("SUB-979-5: убивает 5/5 контрольных мутаций модуля rust/discord-voice/src/dave/tests.rs (100% mutation kill rate)", () => {
    assert.ok(existsSync(MODULE_PATH), "Исходник модуля должен существовать");
    const source = readFileSync(MODULE_PATH, "utf8");

    const mutationSpecs = [
      {
        id: "MUT-dave_fs-01",
        operator: "EpochSecretZeroizeOmission",
        from: "self.current.invalidate_and_zeroize();",
        to: "let _ = &self.current;",
      },
      {
        id: "MUT-dave_fs-02",
        operator: "StaleEpochPurgeBypassOnPeerLeave",
        from: "if prepared.has_peer_removal {",
        to: "if false && prepared.has_peer_removal {",
      },
      {
        id: "MUT-dave_fs-03",
        operator: "ReplayWindowDuplicateCheckBypass",
        from: "if !self.can_process_nonce(generation, truncated_nonce) {",
        to: "if false && !self.can_process_nonce(generation, truncated_nonce) {",
      },
      {
        id: "MUT-dave_fs-04",
        operator: "ConfirmationTagVerificationBypass",
        from: "if ct_eq(&expected, candidate_tag) {",
        to: "if true || ct_eq(&expected, candidate_tag) {",
      },
      {
        id: "MUT-dave_fs-05",
        operator: "HashRatchetDeleteKeysBypass",
        from: "if let Some((mut k, mut n)) = self.cache.remove(&g) {",
        to: "if let Some((mut k, mut n)) = self.cache.get(&g).copied() {",
      },
    ];

    const cratesVoiceSrcDir = join(
      ROOT,
      "rust",
      "crates",
      "discord-voice",
      "src",
    ).replaceAll("\\", "/");
    const mutants = mutationSpecs.map((spec) => {
      assert.ok(
        source.includes(spec.from),
        `Целевая строка для мутации ${spec.id} должна присутствовать в исходнике: ${spec.from}`,
      );
      return {
        id: spec.id,
        moduleName: "dave/tests.rs",
        operator: spec.operator,
        mutatedSource: source
          .replaceAll(
            "../../../crates/discord-voice/src/",
            `${cratesVoiceSrcDir}/`,
          )
          .replace(spec.from, spec.to),
      };
    });

    /** @type {Array<{ id: string, reason: string }>} */
    const killReasons = [];
    const gate = evaluateMutationGate(mutants, (mutant, idx) => {
      const mutantSrcPath = join(workDir, `mutant_${idx + 1}.rs`);
      const mutantBinPath = join(workDir, `mutant_${idx + 1}_bin`);
      writeFileSync(mutantSrcPath, mutant.mutatedSource, "utf8");

      const compiled = compileRustModuleTests(mutantSrcPath, mutantBinPath);
      if (compiled.status !== 0) {
        killReasons.push({ id: mutant.id, reason: "compilation_error" });
        return { killed: true, reason: "compilation_error" };
      }
      const run = runCompiledRustTests(mutantBinPath);
      if (run.status !== 0) {
        killReasons.push({ id: mutant.id, reason: "test_assertion_failed" });
        return { killed: true, reason: "test_assertion_failed" };
      }
      killReasons.push({ id: mutant.id, reason: "mutant_survived" });
      return { killed: false, reason: "mutant_survived" };
    });

    assert.equal(
      gate.passed,
      true,
      `Мутационный гейт не пройден (выжили: ${gate.survivors.join(", ")})`,
    );
    for (const res of killReasons) {
      assert.equal(
        res.reason,
        "test_assertion_failed",
        `Мутант ${res.id} должен быть убит падением теста, а не ошибкой компиляции (${res.reason})`,
      );
    }
    assert.equal(gate.killed, 5);
    assert.equal(gate.total, 5);
    assert.equal(gate.killRate, 1.0);
  });
});
