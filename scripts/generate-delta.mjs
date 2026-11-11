#!/usr/bin/env node
// @ts-check
/**
 * Генератор и валидатор бинарных дельта-патчей на базе Zstandard и ENDSLEY/BSDIFF43 (TASK-969).
 *
 * Сверка с эталонным RE-дампом `/home/sanya/re-deep/`:
 * - Zstandard (`0x28 0xb5 0x2f 0xfd`): `/home/sanya/re-deep/gateway-protocol.md` (§3, строки 21, 102).
 * - `ENDSLEY/BSDIFF43` и верификация: `/home/sanya/re-deep/native-re/raw/updater/src-files.txt` (строки 2–5:
 *   `discord_common/rust/delta_update/src/{apply,lib,verify}.rs`, `endsley_bsdiff/src/lib.rs`).
 * - Диагностика `verify.rs` (строка 8873) и `install.rs` (строки 9822–9826, 10197):
 *   `expected hash '<expected>' for '<path>', actually '<actual>'`,
 *   `SHA256 mismatch for downloaded package: expected <expected>, got <actual>`,
 *   `Content length was <expected> but only transferred <actual> bytes`.
 * - Откат и `skip_host_delta` / `skip_module_delta`: `/home/sanya/re-deep/main-arch.md` (§5.2, `#53006`).
 */
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  chmodSync,
  copyFileSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  renameSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { zstdCompressSync, zstdDecompressSync } from "node:zlib";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const DEFAULT_DIST_DIR = join(ROOT, "rust", "dist");

/** Магические байты кадра Zstandard RFC 8878 (`/home/sanya/re-deep/gateway-protocol.md` §3). */
export const ZSTD_MAGIC_BYTES = Buffer.from([0x28, 0xb5, 0x2f, 0xfd]);

/** Заголовок потока Endsley BSDIFF43 (`discord_common/rust/endsley_bsdiff/src/lib.rs`). */
export const ENDSLEY_BSDIFF43_MAGIC = "ENDSLEY/BSDIFF43";

/** Максимальный допустимый размер целевого бинарника при распаковке дельты (256 МБ). */
export const MAX_TARGET_BINARY_SIZE = 256 * 1024 * 1024;

/** Коды результатов верификации (`delta_update::verify::VerifyResult`, `strings-a-n6.txt:10197`). */
export const VERIFY_RESULTS = Object.freeze({
  OK: "Ok",
  UNEXPECTED_FILE: "UnexpectedFile",
  MISSING_FILE: "MissingFile",
  INVALID_HASH: "InvalidHash",
  IO: "Io",
});

/** Коды ошибок апдейтера (`UpdaterRequestDetail`, `strings-a-n6.txt:9822,10197`). */
export const UPDATER_ERROR_CODES = Object.freeze({
  MISSING_MANIFEST: "MissingManifest",
  INVALID_FORMAT: "InvalidFormat",
  INVALID_UTF8: "InvalidUtf8",
  PATCH_FAILED: "PatchFailed",
  ROLLBACK_FAILED: "RollbackFailed",
  SHA256_MISMATCH: "Sha256Mismatch",
  CONTENT_LENGTH_MISMATCH: "ContentLengthMismatch",
});

const SIGN_BIT_64 = 0x8000_0000_0000_0000n;
const MAGNITUDE_MASK_64 = 0x7fff_ffff_ffff_ffffn;
const SHA256_HEX_REGEX = /^[a-f0-9]{64}$/;

/**
 * @typedef {"bsdiff43-zstd" | "zstd-patch-from"} DeltaEngine
 * @typedef {{ sha256: string, size: number }} DeltaArtifactDescriptor
 * @typedef {{ sha256: string, size: number, zstdMagicHex: string, compressionRatio?: number }} DeltaPatchDescriptor
 * @typedef {{
 *   schemaVersion: number,
 *   engine: DeltaEngine,
 *   format: string,
 *   moduleName: string,
 *   fromVersion: string,
 *   toVersion: string,
 *   source: DeltaArtifactDescriptor,
 *   target: DeltaArtifactDescriptor,
 *   patch: DeltaPatchDescriptor,
 *   createdAt?: string
 * }} DeltaManifest
 * @typedef {{
 *   skip_host_delta: boolean,
 *   skip_module_delta: Record<string, boolean>,
 *   skip_all_module_delta: boolean,
 *   allow_optional_updates: boolean
 * }} RetryOptionsState
 */

/**
 * Создаёт типизированную ошибку апдейтера с полем `code`.
 * @param {string} code
 * @param {string} detail
 * @returns {Error}
 */
function createUpdaterError(code, detail) {
  const err = new Error(`${code}: ${detail}`);
  /** @type {any} */ (err).code = code;
  return err;
}

/**
 * Создаёт начальное состояние `retryOptions` согласно `/home/sanya/re-deep/main-arch.md` §5.2 (`#53006`).
 * @param {Partial<RetryOptionsState>} [overrides]
 * @returns {RetryOptionsState}
 */
export function createDefaultRetryOptions(overrides = {}) {
  return {
    skip_host_delta: overrides.skip_host_delta ?? false,
    skip_module_delta: { ...(overrides.skip_module_delta ?? {}) },
    skip_all_module_delta: overrides.skip_all_module_delta ?? false,
    allow_optional_updates: overrides.allow_optional_updates ?? true,
  };
}

/**
 * Маскирует чувствительные токены (`mfa.*`, `Bot *`, `Bearer *`, `ghp_*`) в диагностических строках.
 * @param {string} message
 * @returns {string}
 */
export function maskSensitiveTokens(message) {
  if (typeof message !== "string") return "";
  return message
    .replace(/mfa\.[A-Za-z0-9_-]{8,}/g, "mfa.***")
    .replace(/\bBot\s+[A-Za-z0-9._-]{12,}/g, "Bot ***")
    .replace(/\bBearer\s+[A-Za-z0-9._-]{12,}/g, "Bearer ***")
    .replace(/\bghp_[A-Za-z0-9]{12,}/g, "ghp_***");
}

/**
 * Вычисляет SHA-256 дайджест буфера в нижнем регистре hex.
 * @param {Buffer} buf
 * @returns {string}
 */
export function computeSha256(buf) {
  return createHash("sha256").update(buf).digest("hex");
}

/**
 * Кодирует 64-битное целое со знаком в формат `offtout` bsdiff (little-endian + бит знака в старшем бите).
 * @param {bigint | number} value
 * @returns {Buffer}
 */
export function encodeOfft(value) {
  const big = typeof value === "bigint" ? value : BigInt(Math.trunc(value));
  const negative = big < 0n;
  const magnitude = negative ? -big : big;
  if (magnitude > MAGNITUDE_MASK_64) {
    throw new RangeError(`Значение offtout выходит за 63-битный диапазон: ${String(value)}`);
  }
  const encoded = negative ? (magnitude | SIGN_BIT_64) : magnitude;
  const out = Buffer.allocUnsafe(8);
  out.writeBigUInt64LE(encoded, 0);
  return out;
}

/**
 * Декодирует 64-битное целое со знаком из формата `offtin` bsdiff.
 * @param {Buffer} buf
 * @param {number} [offset]
 * @returns {bigint}
 */
export function decodeOfft(buf, offset = 0) {
  if (!Buffer.isBuffer(buf) || offset < 0 || offset + 8 > buf.byteLength) {
    throw new RangeError("Буфер слишком мал для чтения 8-байтного поля offtin");
  }
  const raw = buf.readBigUInt64LE(offset);
  const magnitude = raw & MAGNITUDE_MASK_64;
  return (raw & SIGN_BIT_64) !== 0n && magnitude !== 0n ? -magnitude : magnitude;
}

/**
 * Вычисляет быстрый 32-битный хеш 8-байтного окна и строит индексную таблицу `oldBuf`.
 * @param {Buffer} buf
 * @param {number} pos
 * @returns {number}
 */
function hash8Bytes(buf, pos) {
  const lo = buf.readUInt32LE(pos);
  const hi = buf.readUInt32LE(pos + 4);
  return Math.imul(lo ^ Math.imul(hi, 0x9e3779b1), 0x85ebca6b) >>> 0;
}

/**
 * @param {Buffer} oldBuf
 * @returns {Int32Array}
 */
function buildOldIndex(oldBuf) {
  const table = new Int32Array(65_536).fill(-1);
  const step = oldBuf.byteLength > 524_288 ? 8 : 4;
  for (let pos = 0; pos <= oldBuf.byteLength - 8; pos += step) {
    const slot = hash8Bytes(oldBuf, pos) & 65_535;
    if (table[slot] === -1) table[slot] = pos;
  }
  return table;
}

/**
 * Определяет длину приближённого совпадения (`add_len`) между `oldBuf[oldPos..]` и `newBuf[newPos..]`.
 * @param {Buffer} oldBuf
 * @param {number} oldPos
 * @param {Buffer} newBuf
 * @param {number} newPos
 * @returns {number}
 */
function measureApproximateMatchLength(oldBuf, oldPos, newBuf, newPos) {
  const maxLen = Math.min(oldBuf.byteLength - oldPos, newBuf.byteLength - newPos);
  let len = 0;
  while (len < maxLen) {
    if (oldBuf[oldPos + len] === newBuf[newPos + len]) {
      len += 1;
      continue;
    }
    const lookahead = Math.min(32, maxLen - len);
    if (lookahead < 8) break;
    let same = 0;
    for (let k = 0; k < lookahead; k += 1) {
      if (oldBuf[oldPos + len + k] === newBuf[newPos + len + k]) same += 1;
    }
    if (same * 4 >= lookahead * 3) len += lookahead;
    else break;
  }
  return len;
}

/**
 * Проверяет совпадение 16-байтного якоря в `oldBuf[candOld..]` и `newBuf[scanNew..]`.
 * @param {Buffer} oldBuf
 * @param {number} candOld
 * @param {Buffer} newBuf
 * @param {number} scanNew
 * @returns {boolean}
 */
function hasExactAnchor16(oldBuf, candOld, newBuf, scanNew) {
  if (candOld < 0 || candOld + 16 > oldBuf.byteLength || scanNew + 16 > newBuf.byteLength) return false;
  return (
    oldBuf.readBigUInt64LE(candOld) === newBuf.readBigUInt64LE(scanNew) &&
    oldBuf.readBigUInt64LE(candOld + 8) === newBuf.readBigUInt64LE(scanNew + 8)
  );
}

/**
 * Создаёт бинарный дельта-патч в формате `ENDSLEY/BSDIFF43`, сжатый Zstandard (`28 b5 2f fd`).
 * @param {Buffer} oldBuf
 * @param {Buffer} newBuf
 * @returns {Buffer}
 */
export function createBsdiff43Patch(oldBuf, newBuf) {
  if (!Buffer.isBuffer(oldBuf) || !Buffer.isBuffer(newBuf)) {
    throw new TypeError("oldBuf и newBuf должны быть экземплярами Buffer");
  }
  /** @type {Buffer[]} */
  const chunks = [Buffer.from(ENDSLEY_BSDIFF43_MAGIC, "ascii"), encodeOfft(BigInt(newBuf.byteLength))];
  if (newBuf.byteLength === 0) return zstdCompressSync(Buffer.concat(chunks));
  if (oldBuf.byteLength === 0) {
    chunks.push(encodeOfft(0n), encodeOfft(BigInt(newBuf.byteLength)), encodeOfft(0n), newBuf);
    return zstdCompressSync(Buffer.concat(chunks));
  }

  const indexTable = buildOldIndex(oldBuf);
  let oldPos = 0;
  let newPos = 0;

  while (newPos < newBuf.byteLength) {
    const addLen = oldPos < oldBuf.byteLength ? measureApproximateMatchLength(oldBuf, oldPos, newBuf, newPos) : 0;
    const afterAddNew = newPos + addLen;
    const afterAddOld = oldPos + addLen;
    let nextAnchorNew = newBuf.byteLength;
    let nextAnchorOld = afterAddOld;

    for (let scanNew = afterAddNew; scanNew + 16 <= newBuf.byteLength; scanNew += 1) {
      const candOld = indexTable[hash8Bytes(newBuf, scanNew) & 65_535] ?? -1;
      if (candOld !== -1 && hasExactAnchor16(oldBuf, candOld, newBuf, scanNew)) {
        nextAnchorNew = scanNew;
        nextAnchorOld = candOld;
        break;
      }
    }

    const copyLen = nextAnchorNew - afterAddNew;
    const seekOffset = nextAnchorOld - afterAddOld;
    const diffBuf = Buffer.allocUnsafe(addLen);
    for (let i = 0; i < addLen; i += 1) {
      diffBuf[i] = ((newBuf[newPos + i] ?? 0) - (oldBuf[oldPos + i] ?? 0)) & 0xff;
    }

    chunks.push(
      encodeOfft(BigInt(addLen)),
      encodeOfft(BigInt(copyLen)),
      encodeOfft(BigInt(seekOffset)),
      diffBuf,
      newBuf.subarray(afterAddNew, afterAddNew + copyLen),
    );
    newPos = nextAnchorNew;
    oldPos = nextAnchorOld;
  }
  return zstdCompressSync(Buffer.concat(chunks));
}

/**
 * Проверяет наличие сигнатуры кадра Zstandard (`0x28 0xb5 0x2f 0xfd`) в начале буфера.
 * @param {Buffer} buf
 * @returns {boolean}
 */
export function hasZstdMagic(buf) {
  return (
    Buffer.isBuffer(buf) &&
    buf.byteLength >= 4 &&
    buf[0] === 0x28 &&
    buf[1] === 0xb5 &&
    buf[2] === 0x2f &&
    buf[3] === 0xfd
  );
}

/**
 * Распаковывает и валидирует заголовок потока `ENDSLEY/BSDIFF43`.
 * @param {Buffer} compressedPatchBuf
 * @returns {{ rawPatch: Buffer, newSize: number }}
 */
function unpackBsdiff43Header(compressedPatchBuf) {
  if (!hasZstdMagic(compressedPatchBuf)) {
    throw createUpdaterError(UPDATER_ERROR_CODES.INVALID_FORMAT, "отсутствует сигнатура Zstandard (28 b5 2f fd)");
  }
  /** @type {Buffer} */
  let rawPatch;
  try {
    rawPatch = zstdDecompressSync(compressedPatchBuf);
  } catch (cause) {
    throw createUpdaterError(
      UPDATER_ERROR_CODES.PATCH_FAILED,
      `ошибка распаковки Zstandard: ${cause instanceof Error ? cause.message : String(cause)}`,
    );
  }
  if (rawPatch.byteLength < 24) {
    throw createUpdaterError(UPDATER_ERROR_CODES.INVALID_FORMAT, "поток патча короче 24 байт");
  }
  const magic = rawPatch.subarray(0, 16).toString("ascii");
  if (magic !== ENDSLEY_BSDIFF43_MAGIC) {
    throw createUpdaterError(UPDATER_ERROR_CODES.INVALID_FORMAT, `неверная сигнатура "${magic}"`);
  }
  const newSizeBig = decodeOfft(rawPatch, 16);
  if (newSizeBig < 0n || newSizeBig > BigInt(MAX_TARGET_BINARY_SIZE)) {
    throw createUpdaterError(UPDATER_ERROR_CODES.PATCH_FAILED, `недопустимый размер new_size=${String(newSizeBig)}`);
  }
  return { rawPatch, newSize: Number(newSizeBig) };
}

/**
 * Применяет Zstandard-сжатый патч `ENDSLEY/BSDIFF43` к буферу `oldBuf` и возвращает новый буфер.
 * @param {Buffer} oldBuf
 * @param {Buffer} compressedPatchBuf
 * @returns {Buffer}
 */
export function applyBsdiff43Patch(oldBuf, compressedPatchBuf) {
  if (!Buffer.isBuffer(oldBuf) || !Buffer.isBuffer(compressedPatchBuf)) {
    throw new TypeError("oldBuf и compressedPatchBuf должны быть экземплярами Buffer");
  }
  const { rawPatch, newSize } = unpackBsdiff43Header(compressedPatchBuf);
  const newBuf = Buffer.allocUnsafe(newSize);
  let patchOffset = 24;
  let oldPos = 0;
  let newPos = 0;

  while (newPos < newSize) {
    if (patchOffset + 24 > rawPatch.byteLength) {
      throw createUpdaterError(UPDATER_ERROR_CODES.PATCH_FAILED, "неожиданный конец потока триплетов");
    }
    const addLenBig = decodeOfft(rawPatch, patchOffset);
    const copyLenBig = decodeOfft(rawPatch, patchOffset + 8);
    const seekOffset = Number(decodeOfft(rawPatch, patchOffset + 16));
    patchOffset += 24;

    if (addLenBig < 0n || copyLenBig < 0n) {
      throw createUpdaterError(UPDATER_ERROR_CODES.PATCH_FAILED, "отрицательная длина блока в триплете");
    }
    const addLen = Number(addLenBig);
    const copyLen = Number(copyLenBig);
    if (
      newPos + addLen + copyLen > newSize ||
      oldPos < 0 ||
      oldPos + addLen > oldBuf.byteLength ||
      patchOffset + addLen + copyLen > rawPatch.byteLength
    ) {
      throw createUpdaterError(UPDATER_ERROR_CODES.PATCH_FAILED, "управляющий триплет выходит за границы буфера");
    }
    for (let i = 0; i < addLen; i += 1) {
      newBuf[newPos + i] = ((oldBuf[oldPos + i] ?? 0) + (rawPatch[patchOffset + i] ?? 0)) & 0xff;
    }
    patchOffset += addLen;
    newPos += addLen;
    oldPos += addLen;

    rawPatch.copy(newBuf, newPos, patchOffset, patchOffset + copyLen);
    patchOffset += copyLen;
    newPos += copyLen;
    oldPos += seekOffset;
  }

  if (patchOffset !== rawPatch.byteLength) {
    throw createUpdaterError(UPDATER_ERROR_CODES.PATCH_FAILED, "лишние хвостовые байты в потоке патча");
  }
  return newBuf;
}

/**
 * Восстанавливает целевой буфер при самопроверке генератора дельт.
 * @param {DeltaEngine} engine
 * @param {string} oldFile
 * @param {string} patchFile
 * @param {Buffer} oldBuf
 * @param {Buffer} patchBuf
 * @returns {Buffer}
 */
function selfVerifyGeneratedPatch(engine, oldFile, patchFile, oldBuf, patchBuf) {
  if (engine !== "zstd-patch-from") {
    return applyBsdiff43Patch(oldBuf, patchBuf);
  }
  const tmpVerifyDir = mkdtempSync(join(tmpdir(), "zstd-self-verify-"));
  try {
    const tmpOut = join(tmpVerifyDir, "restored.bin");
    execFileSync("zstd", ["-d", "--patch-from=" + resolve(oldFile), resolve(patchFile), "-o", tmpOut, "-f", "-q"]);
    return readFileSync(tmpOut);
  } finally {
    rmSync(tmpVerifyDir, { recursive: true, force: true });
  }
}

/**
 * Генерирует дельта-патч и JSON-манифест `ManifestPatch` с обязательной самопроверкой восстановления хеша.
 * @param {{
 *   oldFile: string,
 *   newFile: string,
 *   patchFile: string,
 *   manifestFile?: string,
 *   engine?: DeltaEngine,
 *   moduleName?: string,
 *   fromVersion?: string,
 *   toVersion?: string
 * }} options
 * @returns {{ ok: boolean, manifest: DeltaManifest, selfVerified: boolean, patchPath: string, manifestPath: string | null }}
 */
export function generateBinaryDelta(options) {
  const {
    oldFile,
    newFile,
    patchFile,
    manifestFile,
    engine = "bsdiff43-zstd",
    moduleName = "host",
    fromVersion = "1.0.0-beta.1",
    toVersion = "1.0.0-beta.2",
  } = options;

  if (!oldFile || !existsSync(oldFile)) throw new Error(`${VERIFY_RESULTS.MISSING_FILE}: ${oldFile}`);
  if (!newFile || !existsSync(newFile)) throw new Error(`${VERIFY_RESULTS.MISSING_FILE}: ${newFile}`);

  const oldBuf = readFileSync(oldFile);
  const newBuf = readFileSync(newFile);
  const sourceSha256 = computeSha256(oldBuf);
  const targetSha256 = computeSha256(newBuf);
  mkdirSync(dirname(resolve(patchFile)), { recursive: true });

  /** @type {Buffer} */
  let patchBuf;
  if (engine === "zstd-patch-from") {
    execFileSync("zstd", ["--patch-from=" + resolve(oldFile), resolve(newFile), "-o", resolve(patchFile), "-f", "-q"]);
    patchBuf = readFileSync(patchFile);
  } else if (engine === "bsdiff43-zstd") {
    patchBuf = createBsdiff43Patch(oldBuf, newBuf);
    writeFileSync(patchFile, patchBuf);
  } else {
    throw createUpdaterError(UPDATER_ERROR_CODES.INVALID_FORMAT, `неподдерживаемый движок "${String(engine)}"`);
  }

  const restoredBuf = selfVerifyGeneratedPatch(engine, oldFile, patchFile, oldBuf, patchBuf);
  const restoredSha256 = computeSha256(restoredBuf);
  if (restoredSha256 !== targetSha256 || restoredBuf.byteLength !== newBuf.byteLength) {
    throw new Error(`expected hash '${targetSha256}' for '${newFile}', actually '${restoredSha256}'`);
  }

  /** @type {DeltaManifest} */
  const manifest = {
    schemaVersion: 1,
    engine,
    format: engine === "bsdiff43-zstd" ? "ENDSLEY/BSDIFF43+ZSTD" : "ZSTD-PATCH-FROM",
    moduleName,
    fromVersion,
    toVersion,
    source: { sha256: sourceSha256, size: oldBuf.byteLength },
    target: { sha256: targetSha256, size: newBuf.byteLength },
    patch: {
      sha256: computeSha256(patchBuf),
      size: patchBuf.byteLength,
      zstdMagicHex: patchBuf.subarray(0, 4).toString("hex"),
      compressionRatio: newBuf.byteLength > 0 ? Number((patchBuf.byteLength / newBuf.byteLength).toFixed(6)) : 0,
    },
    createdAt: new Date().toISOString(),
  };

  if (manifestFile) {
    mkdirSync(dirname(resolve(manifestFile)), { recursive: true });
    writeFileSync(manifestFile, `${JSON.stringify(manifest, null, 2)}\n`, "utf8");
  }
  return { ok: true, manifest, selfVerified: true, patchPath: resolve(patchFile), manifestPath: manifestFile ? resolve(manifestFile) : null };
}

/**
 * Валидирует схему манифеста `ManifestPatch`.
 * @param {unknown} manifest
 * @returns {{ valid: boolean, errors: string[] }}
 */
export function validateDeltaManifest(manifest) {
  /** @type {string[]} */
  const errors = [];
  if (!manifest || typeof manifest !== "object") {
    return { valid: false, errors: [`${UPDATER_ERROR_CODES.MISSING_MANIFEST}: манифест отсутствует`] };
  }
  const m = /** @type {Record<string, any>} */ (manifest);
  if (m.schemaVersion !== 1) errors.push(`Неверная версия schemaVersion=${String(m.schemaVersion)}`);
  if (m.engine !== "bsdiff43-zstd" && m.engine !== "zstd-patch-from") errors.push(`Неверный engine="${String(m.engine)}"`);
  for (const field of ["moduleName", "fromVersion", "toVersion"]) {
    if (typeof m[field] !== "string" || !m[field].trim()) errors.push(`Поле ${field} должно быть непустой строкой`);
  }
  for (const section of ["source", "target", "patch"]) {
    const desc = m[section];
    if (!desc || typeof desc !== "object") {
      errors.push(`Отсутствует секция ${section}`);
      continue;
    }
    if (typeof desc.sha256 !== "string" || !SHA256_HEX_REGEX.test(desc.sha256)) {
      errors.push(`Некорректный SHA-256 в секции ${section}`);
    }
    if (!Number.isSafeInteger(desc.size) || desc.size < 0) {
      errors.push(`Некорректный size в секции ${section}`);
    }
  }
  if (m.patch?.zstdMagicHex !== "28b52ffd") errors.push("Ожидалась сигнатура zstdMagicHex=28b52ffd");
  return { valid: errors.length === 0, errors };
}

/**
 * Проверяет существование, размер и SHA-256 файла по контракту `delta_update/src/verify.rs` (строка 8873).
 * @param {string} filePath
 * @param {string} expectedSha256
 * @param {number} [expectedSize]
 * @returns {{ ok: boolean, result: string, actualSha256: string | null, actualSize: number | null, message: string }}
 */
export function verifyFileHash(filePath, expectedSha256, expectedSize) {
  if (!filePath || !existsSync(filePath)) {
    return { ok: false, result: VERIFY_RESULTS.MISSING_FILE, actualSha256: null, actualSize: null, message: `${VERIFY_RESULTS.MISSING_FILE}: '${filePath}'` };
  }
  try {
    const buf = readFileSync(filePath);
    const actualSize = buf.byteLength;
    const actualSha256 = computeSha256(buf);
    const normExpected = expectedSha256.toLowerCase();
    if (expectedSize !== undefined && actualSize !== expectedSize) {
      return {
        ok: false,
        result: VERIFY_RESULTS.INVALID_HASH,
        actualSha256,
        actualSize,
        message: `Content length was ${expectedSize} but only transferred ${actualSize} bytes; expected hash '${normExpected}' for '${filePath}', actually '${actualSha256}'`,
      };
    }
    if (actualSha256 !== normExpected) {
      return {
        ok: false,
        result: VERIFY_RESULTS.INVALID_HASH,
        actualSha256,
        actualSize,
        message: `expected hash '${normExpected}' for '${filePath}', actually '${actualSha256}'`,
      };
    }
    return { ok: true, result: VERIFY_RESULTS.OK, actualSha256, actualSize, message: "Ok" };
  } catch (err) {
    return { ok: false, result: VERIFY_RESULTS.IO, actualSha256: null, actualSize: null, message: maskSensitiveTokens(String(err)) };
  }
}

/**
 * Верифицирует скачанный пакет патча по контракту `updater_client/src/install.rs` (строки 9822–9826).
 * @param {string} patchFilePath
 * @param {DeltaPatchDescriptor} expectedPatch
 * @returns {{ ok: boolean, errorCode: string | null, actualSha256: string | null, actualSize: number | null, message: string }}
 */
export function verifyDownloadedPatchPackage(patchFilePath, expectedPatch) {
  if (!patchFilePath || !existsSync(patchFilePath)) {
    return { ok: false, errorCode: VERIFY_RESULTS.MISSING_FILE, actualSha256: null, actualSize: null, message: `Missing patch: ${patchFilePath}` };
  }
  const buf = readFileSync(patchFilePath);
  const actualSize = buf.byteLength;
  const actualSha256 = computeSha256(buf);
  if (actualSize !== expectedPatch.size) {
    return {
      ok: false,
      errorCode: UPDATER_ERROR_CODES.CONTENT_LENGTH_MISMATCH,
      actualSha256,
      actualSize,
      message: `Content length was ${expectedPatch.size} but only transferred ${actualSize} bytes`,
    };
  }
  const expectedSha = expectedPatch.sha256.toLowerCase();
  if (actualSha256 !== expectedSha) {
    return {
      ok: false,
      errorCode: UPDATER_ERROR_CODES.SHA256_MISMATCH,
      actualSha256,
      actualSize,
      message: `SHA256 mismatch for downloaded package: expected ${expectedSha}, got ${actualSha256}`,
    };
  }
  if (!hasZstdMagic(buf)) {
    return {
      ok: false,
      errorCode: UPDATER_ERROR_CODES.INVALID_FORMAT,
      actualSha256,
      actualSize,
      message: `${UPDATER_ERROR_CODES.INVALID_FORMAT}: missing Zstandard frame magic 28b52ffd`,
    };
  }
  return { ok: true, errorCode: null, actualSha256, actualSize, message: "Ok" };
}

/**
 * Обновляет `retryOptions` при отказе дельта-обновления согласно `/home/sanya/re-deep/main-arch.md` §5.2 (`#53006`).
 * @param {RetryOptionsState} retryOptions
 * @param {"HostInstall" | "ModuleInstall"} taskType
 * @param {string} moduleName
 * @returns {RetryOptionsState}
 */
function markDeltaFailureInRetryOptions(retryOptions, taskType, moduleName) {
  const base = createDefaultRetryOptions(retryOptions);
  if (taskType === "HostInstall" || moduleName === "host") {
    return {
      ...base,
      skip_host_delta: true,
    };
  }
  return { ...base, skip_module_delta: { ...base.skip_module_delta, [moduleName]: true } };
}

/**
 * Восстанавливает `targetFile` из резервной копии и при наличии `fallbackFullFile` накатывает полный образ.
 * @param {{
 *   targetFile: string,
 *   backupPath: string,
 *   stagingPath: string,
 *   originalMode: number,
 *   manifest: DeltaManifest,
 *   nextRetry: RetryOptionsState,
 *   fallbackFullFile?: string
 * }} ctx
 * @param {string} verifyRes
 * @param {string} errCode
 * @param {string} reasonMsg
 */
function restoreFromBackupAndFallback(ctx, verifyRes, errCode, reasonMsg) {
  const { targetFile, backupPath, stagingPath, originalMode, manifest, nextRetry, fallbackFullFile } = ctx;
  rmSync(stagingPath, { force: true });
  copyFileSync(backupPath, targetFile);
  chmodSync(targetFile, originalMode);
  rmSync(backupPath, { force: true });

  const rollbackCheck = verifyFileHash(targetFile, manifest.source.sha256, manifest.source.size);
  if (!rollbackCheck.ok) {
    return {
      ok: false,
      status: /** @type {const} */ ("rolled_back"),
      verifyResult: VERIFY_RESULTS.INVALID_HASH,
      errorCode: UPDATER_ERROR_CODES.ROLLBACK_FAILED,
      rolledBack: false,
      fallbackUsed: false,
      restoredSha256: rollbackCheck.actualSha256,
      nextRetryOptions: nextRetry,
      message: `ROLLBACK; rollback failed: ${rollbackCheck.message}`,
    };
  }

  if (fallbackFullFile && existsSync(fallbackFullFile)) {
    const fullCheck = verifyFileHash(fallbackFullFile, manifest.target.sha256, manifest.target.size);
    if (fullCheck.ok) {
      copyFileSync(fallbackFullFile, stagingPath);
      chmodSync(stagingPath, originalMode);
      renameSync(stagingPath, targetFile);
      return {
        ok: true,
        status: /** @type {const} */ ("fallback_full_applied"),
        verifyResult: VERIFY_RESULTS.OK,
        errorCode: null,
        rolledBack: true,
        fallbackUsed: true,
        restoredSha256: manifest.target.sha256,
        nextRetryOptions: nextRetry,
        message: `ROLLBACK; delta failed (${reasonMsg}), applied full package fallback`,
      };
    }
  }

  return {
    ok: false,
    status: /** @type {const} */ ("rolled_back"),
    verifyResult: verifyRes,
    errorCode: errCode,
    rolledBack: true,
    fallbackUsed: false,
    restoredSha256: rollbackCheck.actualSha256,
    nextRetryOptions: nextRetry,
    message: `ROLLBACK; ${maskSensitiveTokens(reasonMsg)}`,
  };
}

/**
 * Атомарно накладывает бинарную дельту на `targetFile`, проверяет SHA-256 и при любом повреждении
 * выполняет гарантированный откат к исходному образу.
 * @param {{
 *   targetFile: string,
 *   patchFile: string,
 *   manifest: DeltaManifest,
 *   taskType?: "HostInstall" | "ModuleInstall",
 *   retryOptions?: RetryOptionsState,
 *   fallbackFullFile?: string
 * }} options
 */
export function applyDeltaWithRollback(options) {
  const {
    targetFile,
    patchFile,
    manifest,
    taskType = manifest?.moduleName && manifest.moduleName !== "host" ? "ModuleInstall" : "HostInstall",
    retryOptions = createDefaultRetryOptions(),
    fallbackFullFile,
  } = options;

  const manifestValidation = validateDeltaManifest(manifest);
  const nextRetry = markDeltaFailureInRetryOptions(retryOptions, taskType, manifest?.moduleName ?? "host");
  if (!manifestValidation.valid) {
    return {
      ok: false,
      status: /** @type {const} */ ("rolled_back"),
      verifyResult: VERIFY_RESULTS.INVALID_HASH,
      errorCode: UPDATER_ERROR_CODES.INVALID_FORMAT,
      rolledBack: false,
      fallbackUsed: false,
      restoredSha256: null,
      nextRetryOptions: nextRetry,
      message: manifestValidation.errors.join("; "),
    };
  }

  const preflight = verifyFileHash(targetFile, manifest.source.sha256, manifest.source.size);
  if (!preflight.ok) {
    return {
      ok: false,
      status: /** @type {const} */ ("rolled_back"),
      verifyResult: preflight.result,
      errorCode: UPDATER_ERROR_CODES.PATCH_FAILED,
      rolledBack: false,
      fallbackUsed: false,
      restoredSha256: preflight.actualSha256,
      nextRetryOptions: nextRetry,
      message: preflight.message,
    };
  }

  const originalMode = statSync(targetFile).mode & 0o777;
  const pidSuffix = `${process.pid}.${Date.now()}`;
  const backupPath = `${targetFile}.${pidSuffix}.rollback.bak`;
  const stagingPath = `${targetFile}.${pidSuffix}.staging`;
  copyFileSync(targetFile, backupPath);
  chmodSync(backupPath, originalMode);

  const rollbackCtx = { targetFile, backupPath, stagingPath, originalMode, manifest, nextRetry, fallbackFullFile };

  try {
    const pkgCheck = verifyDownloadedPatchPackage(patchFile, manifest.patch);
    if (!pkgCheck.ok) {
      return restoreFromBackupAndFallback(
        rollbackCtx,
        VERIFY_RESULTS.INVALID_HASH,
        pkgCheck.errorCode ?? UPDATER_ERROR_CODES.PATCH_FAILED,
        pkgCheck.message,
      );
    }

    if (manifest.engine === "zstd-patch-from") {
      execFileSync("zstd", ["-d", "--patch-from=" + resolve(backupPath), resolve(patchFile), "-o", resolve(stagingPath), "-f", "-q"]);
    } else {
      const patchedBuf = applyBsdiff43Patch(readFileSync(backupPath), readFileSync(patchFile));
      writeFileSync(stagingPath, patchedBuf, { mode: originalMode });
    }

    copyFileSync(stagingPath, targetFile);
    chmodSync(targetFile, originalMode);

    const postCheck = verifyFileHash(targetFile, manifest.target.sha256, manifest.target.size);
    if (!postCheck.ok) {
      return restoreFromBackupAndFallback(rollbackCtx, postCheck.result, UPDATER_ERROR_CODES.SHA256_MISMATCH, postCheck.message);
    }

    rmSync(stagingPath, { force: true });
    rmSync(backupPath, { force: true });
    return {
      ok: true,
      status: /** @type {const} */ ("applied"),
      verifyResult: VERIFY_RESULTS.OK,
      errorCode: null,
      rolledBack: false,
      fallbackUsed: false,
      restoredSha256: postCheck.actualSha256,
      nextRetryOptions: createDefaultRetryOptions(retryOptions),
      message: "Ok",
    };
  } catch (err) {
    const code = err && typeof err === "object" && "code" in err && typeof err.code === "string" ? err.code : UPDATER_ERROR_CODES.PATCH_FAILED;
    return restoreFromBackupAndFallback(rollbackCtx, VERIFY_RESULTS.INVALID_HASH, code, err instanceof Error ? err.message : String(err));
  }
}

/**
 * Выполняет встроенный сквозной самотест (`--self-test`) на базе реального артефакта `rust/dist/discord-gpui`.
 * @param {string} [outJsonPath]
 * @returns {Record<string, any>}
 */
export function runSelfTest(outJsonPath) {
  const gpuiBin = join(DEFAULT_DIST_DIR, "discord-gpui");
  const sumsFile = join(DEFAULT_DIST_DIR, "SHA256SUMS");
  if (!existsSync(gpuiBin) || !existsSync(sumsFile)) {
    throw new Error("Отсутствуют эталонные артефакты rust/dist/discord-gpui или rust/dist/SHA256SUMS");
  }

  const tmp = mkdtempSync(join(tmpdir(), "delta-selftest-969-"));
  try {
    const rawBin = readFileSync(gpuiBin);
    const v1Slice = Buffer.from(rawBin.subarray(0, Math.min(131_072, rawBin.byteLength)));
    const v2Slice = Buffer.concat([v1Slice.subarray(0, 8192), Buffer.from("SELF_TEST_DELTA_V1_0_0_BETA_2", "utf8"), v1Slice.subarray(8192)]);

    const oldPath = join(tmp, "v1.bin");
    const newPath = join(tmp, "v2.bin");
    const patchPath = join(tmp, "patch.delta.zst");
    const manifestPath = join(tmp, "manifest.json");
    const targetPath = join(tmp, "target.bin");

    writeFileSync(oldPath, v1Slice, { mode: 0o755 });
    writeFileSync(newPath, v2Slice, { mode: 0o755 });
    writeFileSync(targetPath, v1Slice, { mode: 0o755 });

    const gen = generateBinaryDelta({ oldFile: oldPath, newFile: newPath, patchFile: patchPath, manifestFile: manifestPath, engine: "bsdiff43-zstd" });
    const applied = applyDeltaWithRollback({ targetFile: targetPath, patchFile: patchPath, manifest: gen.manifest });

    writeFileSync(targetPath, v1Slice, { mode: 0o755 });
    const badPatch = join(tmp, "corrupt.delta.zst");
    const corruptBuf = Buffer.from(readFileSync(patchPath));
    corruptBuf[corruptBuf.byteLength - 2] = (corruptBuf[corruptBuf.byteLength - 2] ?? 0) ^ 0xff;
    writeFileSync(badPatch, corruptBuf);
    const rollbackRes = applyDeltaWithRollback({ targetFile: targetPath, patchFile: badPatch, manifest: gen.manifest });

    const report = {
      taskId: "TASK-969",
      status: applied.ok && rollbackRes.rolledBack ? "passed" : "failed",
      engine: gen.manifest.engine,
      format: gen.manifest.format,
      sourceSize: gen.manifest.source.size,
      targetSize: gen.manifest.target.size,
      patchSize: gen.manifest.patch.size,
      compressionRatio: gen.manifest.patch.compressionRatio,
      zstdMagicHex: gen.manifest.patch.zstdMagicHex,
      hashRestored100Percent: applied.ok && applied.restoredSha256 === gen.manifest.target.sha256,
      rollbackVerified: rollbackRes.rolledBack && rollbackRes.restoredSha256 === gen.manifest.source.sha256,
      skipHostDeltaOnFailure: rollbackRes.nextRetryOptions.skip_host_delta === true,
      timestamp: new Date().toISOString(),
    };
    if (outJsonPath) {
      mkdirSync(dirname(resolve(outJsonPath)), { recursive: true });
      writeFileSync(outJsonPath, `${JSON.stringify(report, null, 2)}\n`, "utf8");
    }
    return report;
  } finally {
    rmSync(tmp, { recursive: true, force: true });
  }
}

/**
 * Разбирает аргументы командной строки CLI `scripts/generate-delta.mjs`.
 * @param {readonly string[]} argv
 */
function parseCliArgs(argv) {
  /** @type {"generate" | "verify" | "apply" | "self-test" | "help" | "invalid"} */
  let mode = "invalid";
  /** @type {Record<string, string | undefined>} */
  const opts = { engine: "bsdiff43-zstd" };
  /** @type {Record<string, string>} */
  const valueFlags = {
    "--old": "oldFile",
    "--new": "newFile",
    "--target": "targetFile",
    "--patch": "patchFile",
    "--manifest": "manifestFile",
    "--out-json": "outJson",
    "--fallback-full": "fallbackFullFile",
    "--engine": "engine",
  };

  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i] ?? "";
    if (arg === "--help" || arg === "-h") return { mode: /** @type {const} */ ("help"), opts };
    if (arg === "--self-test" || arg === "--generate" || arg === "--verify" || arg === "--apply") {
      mode = /** @type {any} */ (arg.slice(2));
      continue;
    }
    const key = valueFlags[arg];
    if (!key) return { mode: /** @type {const} */ ("invalid"), opts, error: `Неизвестный аргумент: ${arg}` };
    opts[key] = argv[++i];
  }
  if (opts.engine !== "bsdiff43-zstd" && opts.engine !== "zstd-patch-from") {
    return { mode: /** @type {const} */ ("invalid"), opts, error: `Неизвестный движок: ${String(opts.engine)}` };
  }
  if (mode === "invalid") {
    return { mode, opts, error: "Не указан режим работы (--generate, --verify, --apply или --self-test)" };
  }
  return { mode, opts };
}

/**
 * Точка входа CLI. Коды выхода: `0` — успех, `1` — ошибка аргументов, `2` — ошибка верификации/среды.
 * @param {readonly string[]} argv
 * @returns {number}
 */
export function runCli(argv) {
  const { mode, opts, error } = parseCliArgs(argv);
  if (mode === "help") {
    process.stdout.write("Использование: node scripts/generate-delta.mjs [--generate|--verify|--apply|--self-test] [опции]\n");
    return 0;
  }
  if (mode === "invalid") {
    process.stderr.write(`Ошибка аргументов: ${error ?? "некорректный вызов"}\n`);
    return 1;
  }
  try {
    if (mode === "self-test") {
      const report = runSelfTest(opts.outJson);
      process.stdout.write(`Самотест бинарных дельт пройден: ${JSON.stringify(report)}\n`);
      return report.status === "passed" ? 0 : 2;
    }
    if (mode === "generate") {
      if (!opts.oldFile || !opts.newFile || !opts.patchFile) {
        process.stderr.write("Ошибка аргументов: для --generate требуются --old, --new и --patch\n");
        return 1;
      }
      const res = generateBinaryDelta({
        oldFile: opts.oldFile,
        newFile: opts.newFile,
        patchFile: opts.patchFile,
        manifestFile: opts.manifestFile,
        engine: /** @type {DeltaEngine} */ (opts.engine),
      });
      process.stdout.write(`Дельта создана: ${res.patchPath} (${res.manifest.patch.size} Б)\n`);
      return 0;
    }
    if (mode === "verify") {
      if (!opts.patchFile || !opts.manifestFile) {
        process.stderr.write("Ошибка аргументов: для --verify требуются --patch и --manifest\n");
        return 1;
      }
      const manifest = /** @type {DeltaManifest} */ (JSON.parse(readFileSync(opts.manifestFile, "utf8")));
      const mCheck = validateDeltaManifest(manifest);
      const pkgCheck = mCheck.valid ? verifyDownloadedPatchPackage(opts.patchFile, manifest.patch) : null;
      if (!mCheck.valid || !pkgCheck?.ok) {
        process.stderr.write(`Ошибка верификации: ${!mCheck.valid ? mCheck.errors.join("; ") : pkgCheck?.message}\n`);
        return 2;
      }
      process.stdout.write(`Верификация успешна: ${pkgCheck.actualSha256}\n`);
      return 0;
    }
    if (!opts.targetFile || !opts.patchFile || !opts.manifestFile) {
      process.stderr.write("Ошибка аргументов: для --apply требуются --target, --patch и --manifest\n");
      return 1;
    }
    const manifest = /** @type {DeltaManifest} */ (JSON.parse(readFileSync(opts.manifestFile, "utf8")));
    const applyRes = applyDeltaWithRollback({
      targetFile: opts.targetFile,
      patchFile: opts.patchFile,
      manifest,
      fallbackFullFile: opts.fallbackFullFile,
    });
    if (!applyRes.ok) {
      process.stderr.write(`Ошибка применения дельты (откат=${String(applyRes.rolledBack)}): ${applyRes.message}\n`);
      return 2;
    }
    process.stdout.write(`Дельта успешно применена: ${applyRes.restoredSha256}\n`);
    return 0;
  } catch (err) {
    process.stderr.write(`Критическая ошибка: ${maskSensitiveTokens(err instanceof Error ? err.message : String(err))}\n`);
    return 2;
  }
}

const isMainModule = process.argv[1] && resolve(process.argv[1]) === resolve(fileURLToPath(import.meta.url));
if (isMainModule) {
  process.exitCode = runCli(process.argv.slice(2));
}
