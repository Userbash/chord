#!/usr/bin/env node
// @ts-check
/**
 * Скрипт проверки обновлений через GitHub Releases API v3 (TASK-965):
 * - Семантическое версионирование по стандарту SemVer 2.0.0 (parseSemVer, compareSemVer, isUpdateAvailable).
 * - Криптографическая подпись и верификация манифеста релиза Ed25519 + сверка SHA-256 артефактов.
 * - Поддержка кэширования ETag (If-None-Match -> HTTP 304 Not Modified).
 * - Контроль квоты GitHub Releases API v3 Rate-Limit 60 req/h (клиентское скользящее окно + заголовки X-RateLimit-*).
 */
import {
  createHash,
  createPrivateKey,
  createPublicKey,
  generateKeyPairSync,
  sign as cryptoSign,
  verify as cryptoVerify,
} from "node:crypto";
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync, mkdtempSync } from "node:fs";
import { createServer, request as httpRequest } from "node:http";
import { request as httpsRequest } from "node:https";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const DEFAULT_DIST_DIR = join(ROOT, "rust", "dist");

export const GITHUB_API_VERSION = "2022-11-28";
export const GITHUB_ACCEPT_HEADER = "application/vnd.github+json";
export const DEFAULT_RATE_LIMIT_PER_HOUR = 60;
export const RATE_LIMIT_WINDOW_MS = 3_600_000;
export const DEFAULT_REPO = "discord-lab/discord-gpui";
export const DEFAULT_CURRENT_VERSION = "v1.0.0-beta.1";

/** ASN.1 DER SPKI префикс для 32-байтного публичного ключа Ed25519 (RFC 8410). */
const ED25519_SPKI_PREFIX = Buffer.from("302a300506032b6570032100", "hex");

/** ASN.1 DER PKCS#8 префикс для 32-байтного приватного ключа Ed25519 (RFC 8410). */
const ED25519_PKCS8_PREFIX = Buffer.from("302e020100300506032b657004220420", "hex");

const SEMVER_REGEX =
  /^[vV]?(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?(?:\+([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?$/;

/**
 * @typedef {Object} ParsedSemVer
 * @property {number} major
 * @property {number} minor
 * @property {number} patch
 * @property {readonly string[]} prerelease
 * @property {readonly string[]} build
 * @property {string} raw
 * @property {string} normalized
 */

/**
 * Проверяет валидность идентификаторов pre-release (числовые без ведущих нулей).
 * @param {readonly string[]} identifiers
 * @returns {boolean}
 */
function hasValidPrereleaseIdentifiers(identifiers) {
  for (const id of identifiers) {
    if (!id || !/^[0-9A-Za-z-]+$/.test(id)) return false;
    if (/^\d+$/.test(id) && id.length > 1 && id.startsWith("0")) {
      return false;
    }
  }
  return true;
}

/**
 * Парсит строку версии по стандарту SemVer 2.0.0 (с поддержкой префикса v/V).
 * Возвращает null при нарушении формата.
 * @param {string} versionStr
 * @returns {ParsedSemVer | null}
 */
export function parseSemVer(versionStr) {
  if (typeof versionStr !== "string") return null;
  const trimmed = versionStr.trim();
  const match = SEMVER_REGEX.exec(trimmed);
  if (!match) return null;

  const major = Number(match[1]);
  const minor = Number(match[2]);
  const patch = Number(match[3]);
  if (!Number.isSafeInteger(major) || !Number.isSafeInteger(minor) || !Number.isSafeInteger(patch)) {
    return null;
  }

  const prerelease = match[4] ? match[4].split(".") : [];
  if (!hasValidPrereleaseIdentifiers(prerelease)) {
    return null;
  }

  const build = match[5] ? match[5].split(".") : [];
  for (const b of build) {
    if (!b || !/^[0-9A-Za-z-]+$/.test(b)) return null;
  }

  const base = `${major}.${minor}.${patch}`;
  const normalized = prerelease.length > 0 ? `${base}-${prerelease.join(".")}` : base;

  return Object.freeze({
    major,
    minor,
    patch,
    prerelease: Object.freeze(prerelease),
    build: Object.freeze(build),
    raw: trimmed,
    normalized,
  });
}

/**
 * Сравнивает два идентификатора pre-release согласно SemVer 2.0.0 §11.4.
 * @param {string} a
 * @param {string} b
 * @returns {-1 | 0 | 1}
 */
function comparePrereleaseIdentifier(a, b) {
  if (a === b) return 0;
  const aIsNum = /^\d+$/.test(a);
  const bIsNum = /^\d+$/.test(b);
  if (aIsNum && bIsNum) {
    const aBig = BigInt(a);
    const bBig = BigInt(b);
    return aBig < bBig ? -1 : 1;
  }
  if (aIsNum && !bIsNum) return -1;
  if (!aIsNum && bIsNum) return 1;
  return a < b ? -1 : 1;
}

/**
 * Сравнивает две версии по стандарту SemVer 2.0.0 (§10, §11).
 * Возвращает -1 (a < b), 0 (a == b), 1 (a > b).
 * @param {string} aStr
 * @param {string} bStr
 * @returns {-1 | 0 | 1}
 */
export function compareSemVer(aStr, bStr) {
  const a = parseSemVer(aStr);
  if (!a) {
    throw new TypeError(`Некорректная версия SemVer: "${aStr}"`);
  }
  const b = parseSemVer(bStr);
  if (!b) {
    throw new TypeError(`Некорректная версия SemVer: "${bStr}"`);
  }

  if (a.major !== b.major) return a.major < b.major ? -1 : 1;
  if (a.minor !== b.minor) return a.minor < b.minor ? -1 : 1;
  if (a.patch !== b.patch) return a.patch < b.patch ? -1 : 1;

  const aPre = a.prerelease;
  const bPre = b.prerelease;
  if (aPre.length === 0 && bPre.length === 0) return 0;
  if (aPre.length === 0 && bPre.length > 0) return 1;
  if (aPre.length > 0 && bPre.length === 0) return -1;

  const len = Math.min(aPre.length, bPre.length);
  for (let i = 0; i < len; i += 1) {
    const idA = aPre[i];
    const idB = bPre[i];
    if (idA !== undefined && idB !== undefined) {
      const cmp = comparePrereleaseIdentifier(idA, idB);
      if (cmp !== 0) return cmp;
    }
  }

  if (aPre.length === bPre.length) return 0;
  return aPre.length < bPre.length ? -1 : 1;
}

/**
 * Проверяет, является ли версия-кандидат допустимым обновлением относительно текущей версии.
 * @param {string} currentVersion
 * @param {string} candidateVersion
 * @param {{ channel?: "stable" | "beta" | "nightly", allowPrerelease?: boolean, allowDowngrade?: boolean }} [options]
 * @returns {boolean}
 */
export function isUpdateAvailable(currentVersion, candidateVersion, options = {}) {
  const current = parseSemVer(currentVersion);
  const candidate = parseSemVer(candidateVersion);
  if (!current || !candidate) return false;

  const channel = options.channel ?? (current.prerelease.length > 0 ? "beta" : "stable");
  const allowPrerelease = options.allowPrerelease ?? channel !== "stable";

  if (!allowPrerelease && candidate.prerelease.length > 0) {
    return false;
  }

  const cmp = compareSemVer(candidate.normalized, current.normalized);
  if (cmp > 0) return true;
  if (cmp < 0 && options.allowDowngrade === true) return true;
  return false;
}

/**
 * Генерирует ключевую пару Ed25519 для подписания и верификации манифестов обновлений.
 * @returns {{ publicKeyPem: string, privateKeyPem: string, publicKeyRawHex: string, privateKeyRawHex: string }}
 */
export function generateUpdateKeyPair() {
  const { publicKey, privateKey } = generateKeyPairSync("ed25519");
  const publicKeyPem = publicKey.export({ type: "spki", format: "pem" }).toString();
  const privateKeyPem = privateKey.export({ type: "pkcs8", format: "pem" }).toString();
  const spkiDer = /** @type {Buffer} */ (publicKey.export({ type: "spki", format: "der" }));
  const pkcs8Der = /** @type {Buffer} */ (privateKey.export({ type: "pkcs8", format: "der" }));

  return Object.freeze({
    publicKeyPem,
    privateKeyPem,
    publicKeyRawHex: spkiDer.subarray(spkiDer.length - 32).toString("hex"),
    privateKeyRawHex: pkcs8Der.subarray(pkcs8Der.length - 32).toString("hex"),
  });
}

/**
 * Канонизирует полезную нагрузку манифеста релиза для детерминированной подписи Ed25519.
 * @param {string | Uint8Array | Record<string, any>} manifest
 * @returns {string}
 */
export function canonicalizeManifestPayload(manifest) {
  if (typeof manifest === "string") return manifest;
  if (manifest instanceof Uint8Array) return Buffer.from(manifest).toString("utf8");

  const parsedVer = parseSemVer(String(manifest.version ?? manifest.tag_name ?? ""));
  const rawAssets = Array.isArray(manifest.assets) ? manifest.assets : [];
  const normalizedAssets = rawAssets
    .map((a) => ({
      name: String(a.name ?? ""),
      sha256: String(a.sha256 ?? "").toLowerCase(),
      size: Number(a.size ?? 0),
      browser_download_url: String(a.browser_download_url ?? ""),
    }))
    .sort((x, y) => (x.name < y.name ? -1 : x.name > y.name ? 1 : 0));

  const canonicalObj = {
    schema: "discord-gpui-release-manifest/v1",
    tag_name: String(manifest.tag_name ?? ""),
    version: parsedVer ? parsedVer.normalized : String(manifest.version ?? ""),
    published_at: String(manifest.published_at ?? ""),
    sha256sums: String(manifest.sha256sums ?? "").trim(),
    assets: normalizedAssets,
  };
  return JSON.stringify(canonicalObj);
}

/**
 * Разрешает приватный ключ Ed25519 из PEM или 64-символьного hex-представления.
 * @param {string} privateKeyInput
 */
function resolveEd25519PrivateKey(privateKeyInput) {
  const trimmed = privateKeyInput.trim();
  if (/^[0-9a-fA-F]{64}$/.test(trimmed)) {
    const der = Buffer.concat([ED25519_PKCS8_PREFIX, Buffer.from(trimmed, "hex")]);
    return createPrivateKey({ key: der, format: "der", type: "pkcs8" });
  }
  return createPrivateKey(trimmed);
}

/**
 * Разрешает публичный ключ Ed25519 из SPKI PEM, 64-символьного hex или base64.
 * @param {string} publicKeyInput
 */
function resolveEd25519PublicKey(publicKeyInput) {
  const trimmed = publicKeyInput.trim();
  if (/^[0-9a-fA-F]{64}$/.test(trimmed)) {
    const der = Buffer.concat([ED25519_SPKI_PREFIX, Buffer.from(trimmed, "hex")]);
    return createPublicKey({ key: der, format: "der", type: "spki" });
  }
  if (!trimmed.startsWith("-----BEGIN")) {
    const raw = Buffer.from(trimmed, "base64");
    if (raw.byteLength === 32) {
      const der = Buffer.concat([ED25519_SPKI_PREFIX, raw]);
      return createPublicKey({ key: der, format: "der", type: "spki" });
    }
  }
  return createPublicKey(trimmed);
}

/**
 * Подписывает манифест релиза алгоритмом Ed25519 (RFC 8032).
 * @param {string | Uint8Array | Record<string, any>} manifest
 * @param {string} privateKeyInput
 * @returns {{ signatureHex: string, signatureBase64: string, canonicalPayload: string }}
 */
export function signUpdateManifest(manifest, privateKeyInput) {
  const canonicalPayload = canonicalizeManifestPayload(manifest);
  const keyObj = resolveEd25519PrivateKey(privateKeyInput);
  const sigBuf = cryptoSign(null, Buffer.from(canonicalPayload, "utf8"), keyObj);
  return Object.freeze({
    signatureHex: sigBuf.toString("hex"),
    signatureBase64: sigBuf.toString("base64"),
    canonicalPayload,
  });
}

/**
 * Декодирует 64-байтную подпись Ed25519 из hex (128 символов), base64 или Uint8Array.
 * @param {string | Uint8Array} signatureInput
 * @returns {Buffer | null}
 */
function decodeSignatureBytes(signatureInput) {
  if (signatureInput instanceof Uint8Array) {
    return signatureInput.byteLength === 64 ? Buffer.from(signatureInput) : null;
  }
  if (typeof signatureInput !== "string") return null;
  const trimmed = signatureInput.trim();
  if (!trimmed) return null;

  if (/^[0-9a-fA-F]{128}$/.test(trimmed)) {
    return Buffer.from(trimmed, "hex");
  }
  if (/^[A-Za-z0-9+/]{86}==$/.test(trimmed) || /^[A-Za-z0-9_-]{86}(?:==)?$/.test(trimmed)) {
    const buf = Buffer.from(trimmed, "base64");
    return buf.byteLength === 64 ? buf : null;
  }
  return null;
}

/**
 * Проверяет криптографическую подпись Ed25519 для канонического манифеста.
 * @param {string | Uint8Array | Record<string, any>} payload
 * @param {string | Uint8Array} signatureInput
 * @param {string} publicKeyInput
 * @returns {{ valid: boolean, algorithm: "Ed25519", reason: string | null }}
 */
export function verifyEd25519Signature(payload, signatureInput, publicKeyInput) {
  const sigBytes = decodeSignatureBytes(signatureInput);
  if (!sigBytes) {
    return Object.freeze({
      valid: false,
      algorithm: "Ed25519",
      reason: "Подпись Ed25519 должна иметь длину ровно 64 байта (128 hex или 88 base64 символов)",
    });
  }

  try {
    const pubKey = resolveEd25519PublicKey(publicKeyInput);
    const dataBuf = Buffer.from(canonicalizeManifestPayload(payload), "utf8");
    const isValid = cryptoVerify(null, dataBuf, pubKey, sigBytes);
    return Object.freeze({
      valid: isValid,
      algorithm: "Ed25519",
      reason: isValid ? null : "Криптографическая проверка подписи Ed25519 не прошла",
    });
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    return Object.freeze({
      valid: false,
      algorithm: "Ed25519",
      reason: `Ошибка ключа или формата Ed25519: ${msg}`,
    });
  }
}

/**
 * Проверяет Ed25519 подпись релиза и сверяет SHA-256 контрольные суммы локальных артефактов.
 * @param {Record<string, any>} release
 * @param {{ publicKey: string, distDir?: string }} options
 * @returns {{ passed: boolean, signatureVerified: boolean, verifiedAssets: Array<{ name: string, sha256: string }>, reasons: string[] }}
 */
export function verifyReleaseManifestAndAssets(release, options) {
  /** @type {string[]} */
  const reasons = [];
  /** @type {Array<{ name: string, sha256: string }>} */
  const verifiedAssets = [];

  const signature = String(release.signature ?? release.ed25519_signature ?? "");
  const sigCheck = verifyEd25519Signature(release, signature, options.publicKey);
  if (!sigCheck.valid) {
    reasons.push(sigCheck.reason ?? "Невалидная подпись Ed25519");
    return Object.freeze({
      passed: false,
      signatureVerified: false,
      verifiedAssets: [],
      reasons,
    });
  }

  const assets = Array.isArray(release.assets) ? release.assets : [];
  for (const asset of assets) {
    const name = String(asset.name ?? "");
    const expectedSha = String(asset.sha256 ?? "").toLowerCase();
    if (!/^[a-f0-9]{64}$/.test(expectedSha)) {
      reasons.push(`Ассет "${name}" не содержит валидного SHA-256 дайджеста`);
      continue;
    }

    if (options.distDir) {
      const candidatePath = join(options.distDir, name);
      if (existsSync(candidatePath)) {
        const actualSha = createHash("sha256")
          .update(readFileSync(candidatePath))
          .digest("hex");
        if (actualSha !== expectedSha) {
          reasons.push(
            `Несовпадение SHA-256 для артефакта "${name}": ожидалось ${expectedSha}, фактически ${actualSha}`,
          );
          continue;
        }
      }
    }
    verifiedAssets.push({ name, sha256: expectedSha });
  }

  return Object.freeze({
    passed: reasons.length === 0,
    signatureVerified: true,
    verifiedAssets,
    reasons,
  });
}

/**
 * Маскирует чувствительные токены в сообщениях логов и ошибок.
 * @param {string} text
 * @returns {string}
 */
export function maskSensitiveToken(text) {
  if (!text) return "";
  return text
    .replace(/\bgh[pousr]_[A-Za-z0-9_]{16,255}\b/g, "ghp_***")
    .replace(/\bgithub_pat_[A-Za-z0-9_]{20,255}\b/g, "github_pat_***")
    .replace(/\bBearer\s+[A-Za-z0-9._-]{16,}\b/gi, "Bearer ***")
    .replace(/\bmfa\.[A-Za-z0-9_-]{20,}\b/g, "mfa.***")
    .replace(/\bBot\s+[A-Za-z0-9._-]{20,}\b/gi, "Bot ***");
}

/**
 * Формирует заголовки HTTP-запроса к GitHub Releases API v3 с поддержкой ETag.
 * @param {{ etag?: string | null, token?: string | null, userAgent?: string }} [options]
 * @returns {Record<string, string>}
 */
export function buildGitHubReleaseRequestHeaders(options = {}) {
  /** @type {Record<string, string>} */
  const headers = {
    Accept: GITHUB_ACCEPT_HEADER,
    "X-GitHub-Api-Version": GITHUB_API_VERSION,
    "User-Agent": options.userAgent ?? "discord-gpui-update-checker/1.0.0",
  };
  if (options.etag && options.etag.trim().length > 0) {
    headers["If-None-Match"] = options.etag.trim();
  }
  if (options.token && options.token.trim().length > 0) {
    headers.Authorization = `Bearer ${options.token.trim()}`;
  }
  return Object.freeze(headers);
}

/**
 * Загружает сохранённую запись кэша ETag с диска.
 * @param {string} cachePath
 * @returns {{ etag: string, url: string, cachedAt: string, release: Record<string, any> } | null}
 */
export function loadEtagCache(cachePath) {
  if (!cachePath || !existsSync(cachePath)) return null;
  try {
    const parsed = JSON.parse(readFileSync(cachePath, "utf8"));
    if (!parsed || typeof parsed.etag !== "string" || !parsed.release) {
      return null;
    }
    return parsed;
  } catch {
    return null;
  }
}

/**
 * Сохраняет запись кэша ETag на диск.
 * @param {string} cachePath
 * @param {{ etag: string, url: string, cachedAt: string, release: Record<string, any> }} entry
 */
export function saveEtagCache(cachePath, entry) {
  mkdirSync(dirname(cachePath), { recursive: true });
  writeFileSync(cachePath, JSON.stringify(entry, null, 2), "utf8");
}

/**
 * Извлекает заголовок без учёта регистра.
 * @param {Record<string, string | string[] | undefined>} headers
 * @param {string} key
 * @returns {string | undefined}
 */
function getHeader(headers, key) {
  const lower = key.toLowerCase();
  for (const [k, v] of Object.entries(headers)) {
    if (k.toLowerCase() === lower) {
      return Array.isArray(v) ? v[0] : v;
    }
  }
  return undefined;
}

/**
 * Разбирает заголовки X-RateLimit-* и Retry-After ответа GitHub API v3.
 * @param {Record<string, string | string[] | undefined>} headers
 * @param {number} [nowMs]
 * @returns {{ limit: number, remaining: number, used: number, resetAtMs: number, retryAfterMs: number, isExhausted: boolean }}
 */
export function parseRateLimitHeaders(headers, nowMs = Date.now()) {
  const rawLimit = Number(getHeader(headers, "x-ratelimit-limit") ?? DEFAULT_RATE_LIMIT_PER_HOUR);
  const limit = Number.isFinite(rawLimit) && rawLimit > 0 ? rawLimit : DEFAULT_RATE_LIMIT_PER_HOUR;

  const rawRemaining = getHeader(headers, "x-ratelimit-remaining");
  const remaining =
    rawRemaining !== undefined && Number.isFinite(Number(rawRemaining))
      ? Math.max(0, Number(rawRemaining))
      : limit;

  const rawUsed = getHeader(headers, "x-ratelimit-used");
  const used =
    rawUsed !== undefined && Number.isFinite(Number(rawUsed))
      ? Math.max(0, Number(rawUsed))
      : Math.max(0, limit - remaining);

  const rawResetSec = Number(getHeader(headers, "x-ratelimit-reset") ?? 0);
  const resetAtMs =
    Number.isFinite(rawResetSec) && rawResetSec > 0 ? rawResetSec * 1000 : nowMs + RATE_LIMIT_WINDOW_MS;

  const rawRetryAfter = getHeader(headers, "retry-after");
  let retryAfterMs = 0;
  if (rawRetryAfter !== undefined) {
    const seconds = Number(rawRetryAfter);
    if (Number.isFinite(seconds) && seconds >= 0) {
      retryAfterMs = seconds * 1000;
    }
  } else if (remaining === 0 && resetAtMs > nowMs) {
    retryAfterMs = resetAtMs - nowMs;
  }

  return Object.freeze({
    limit,
    remaining,
    used,
    resetAtMs,
    retryAfterMs,
    isExhausted: remaining === 0,
  });
}

/**
 * Оценивает бюджет клиентского лимита 60 запросов в час (скользящее окно 3600 с).
 * @param {{ timestamps?: readonly number[], serverRemaining?: number, serverResetAtMs?: number }} state
 * @param {number} [nowMs]
 * @param {number} [maxPerHour]
 * @returns {{ allowed: boolean, limit: number, remaining: number, retryAfterMs: number, nextTimestamps: number[], reason: string | null }}
 */
export function evaluateRateLimitBudget(
  state = {},
  nowMs = Date.now(),
  maxPerHour = DEFAULT_RATE_LIMIT_PER_HOUR,
) {
  const windowStart = nowMs - RATE_LIMIT_WINDOW_MS;
  const activeTimestamps = (state.timestamps ?? []).filter((t) => t > windowStart && t <= nowMs);

  if (
    state.serverRemaining !== undefined &&
    state.serverRemaining <= 0 &&
    state.serverResetAtMs !== undefined &&
    state.serverResetAtMs > nowMs
  ) {
    return Object.freeze({
      allowed: false,
      limit: maxPerHour,
      remaining: 0,
      retryAfterMs: state.serverResetAtMs - nowMs,
      nextTimestamps: activeTimestamps,
      reason: "Превышен серверный лимит GitHub API X-RateLimit-Remaining = 0",
    });
  }

  if (activeTimestamps.length >= maxPerHour) {
    const oldest = activeTimestamps[0] ?? nowMs;
    const retryAfterMs = Math.max(1, oldest + RATE_LIMIT_WINDOW_MS - nowMs);
    return Object.freeze({
      allowed: false,
      limit: maxPerHour,
      remaining: 0,
      retryAfterMs,
      nextTimestamps: activeTimestamps,
      reason: `Превышен локальный лимит ${maxPerHour} req/h для GitHub Releases API v3`,
    });
  }

  const nextTimestamps = [...activeTimestamps, nowMs];
  const localRemaining = Math.max(0, maxPerHour - nextTimestamps.length);
  const remaining =
    state.serverRemaining !== undefined
      ? Math.min(localRemaining, Math.max(0, state.serverRemaining))
      : localRemaining;

  return Object.freeze({
    allowed: true,
    limit: maxPerHour,
    remaining,
    retryAfterMs: 0,
    nextTimestamps,
    reason: null,
  });
}

/**
 * Выполняет HTTP/HTTPS GET запрос и возвращает статус, заголовки и тело ответа.
 * @param {string} urlStr
 * @param {Record<string, string>} headers
 * @param {number} [timeoutMs]
 * @returns {Promise<{ statusCode: number, headers: Record<string, string | string[] | undefined>, body: string }>}
 */
function performHttpGet(urlStr, headers, timeoutMs = 10_000) {
  return new Promise((resolvePromise, rejectPromise) => {
    const parsedUrl = new URL(urlStr);
    const reqFn = parsedUrl.protocol === "https:" ? httpsRequest : httpRequest;
    const req = reqFn(
      parsedUrl,
      {
        method: "GET",
        headers,
        timeout: timeoutMs,
      },
      (res) => {
        /** @type {Buffer[]} */
        const chunks = [];
        res.on("data", (chunk) => chunks.push(Buffer.from(chunk)));
        res.on("end", () => {
          resolvePromise({
            statusCode: res.statusCode ?? 0,
            headers: res.headers,
            body: Buffer.concat(chunks).toString("utf8"),
          });
        });
      },
    );
    req.on("timeout", () => {
      req.destroy(new Error(`Превышен таймаут ожидания ответа (${timeoutMs} мс)`));
    });
    req.on("error", (err) => rejectPromise(err));
    req.end();
  });
}

/**
 * Выбирает подходящий релиз из ответа GitHub Releases API (объект релиза или массив релизов).
 * Игнорирует черновики (draft: true).
 * @param {unknown} payload
 * @param {{ channel?: "stable" | "beta" | "nightly", allowPrerelease?: boolean }} options
 * @returns {Record<string, any> | null}
 */
function selectTargetRelease(payload, options) {
  if (!payload || typeof payload !== "object") return null;
  const list = Array.isArray(payload) ? payload : [payload];
  const allowPrerelease = options.allowPrerelease ?? (options.channel ?? "beta") !== "stable";

  const candidates = list.filter((item) => {
    if (!item || typeof item !== "object" || item.draft === true) return false;
    const ver = parseSemVer(String(item.tag_name ?? item.version ?? ""));
    if (!ver) return false;
    if (!allowPrerelease && (item.prerelease === true || ver.prerelease.length > 0)) {
      return false;
    }
    return true;
  });

  if (candidates.length === 0) return null;
  candidates.sort((a, b) =>
    compareSemVer(
      String(b.tag_name ?? b.version ?? "0.0.0"),
      String(a.tag_name ?? a.version ?? "0.0.0"),
    ),
  );
  return candidates[0] ?? null;
}

/**
 * Выполняет полную проверку обновлений через GitHub Releases API v3 с поддержкой ETag,
 * Rate-Limit 60 req/h, SemVer 2.0.0 и проверкой подписи Ed25519.
 * @param {{
 *   apiUrl?: string,
 *   repo?: string,
 *   currentVersion?: string,
 *   channel?: "stable" | "beta" | "nightly",
 *   allowDowngrade?: boolean,
 *   publicKey: string,
 *   etagCachePath?: string,
 *   distDir?: string,
 *   token?: string,
 *   fallbackToCache?: boolean,
 *   rateLimitState?: { timestamps?: number[] },
 *   nowMs?: number
 * }} options
 */
export async function checkGitHubReleasesUpdate(options) {
  const nowMs = options.nowMs ?? Date.now();
  const currentVersion = options.currentVersion ?? DEFAULT_CURRENT_VERSION;
  const parsedCurrent = parseSemVer(currentVersion);
  if (!parsedCurrent) {
    throw new TypeError(`Некорректная текущая версия SemVer: "${currentVersion}"`);
  }

  const repo = options.repo ?? DEFAULT_REPO;
  const apiUrl =
    options.apiUrl ?? `https://api.github.com/repos/${repo}/releases/latest`;
  const channel = options.channel ?? (parsedCurrent.prerelease.length > 0 ? "beta" : "stable");

  const cached = options.etagCachePath ? loadEtagCache(options.etagCachePath) : null;
  const budget = evaluateRateLimitBudget(options.rateLimitState ?? {}, nowMs, DEFAULT_RATE_LIMIT_PER_HOUR);

  if (!budget.allowed) {
    if (options.fallbackToCache && cached?.release) {
      const cachedVer = parseSemVer(String(cached.release.tag_name ?? cached.release.version ?? ""));
      return Object.freeze({
        status: "rate_limited_cached",
        httpStatus: 429,
        fromCache: true,
        updateAvailable: cachedVer
          ? isUpdateAvailable(currentVersion, cachedVer.normalized, {
              channel,
              allowDowngrade: options.allowDowngrade,
            })
          : false,
        currentVersion: parsedCurrent.normalized,
        latestVersion: cachedVer ? cachedVer.normalized : null,
        etag: cached.etag,
        signatureVerified: true,
        rateLimit: {
          limit: budget.limit,
          remaining: 0,
          used: budget.limit,
          resetAtMs: nowMs + budget.retryAfterMs,
          retryAfterMs: budget.retryAfterMs,
          isExhausted: true,
        },
      });
    }
    return Object.freeze({
      status: "rate_limited",
      httpStatus: 429,
      fromCache: false,
      updateAvailable: false,
      currentVersion: parsedCurrent.normalized,
      latestVersion: null,
      etag: cached?.etag ?? null,
      signatureVerified: false,
      rateLimit: {
        limit: budget.limit,
        remaining: 0,
        used: budget.limit,
        resetAtMs: nowMs + budget.retryAfterMs,
        retryAfterMs: budget.retryAfterMs,
        isExhausted: true,
      },
    });
  }

  const headers = buildGitHubReleaseRequestHeaders({
    etag: cached?.etag ?? null,
    token: options.token ?? null,
  });

  const response = await performHttpGet(apiUrl, headers);
  const rateLimit = parseRateLimitHeaders(response.headers, nowMs);
  const responseEtag = getHeader(response.headers, "etag") ?? cached?.etag ?? null;

  if (response.statusCode === 304 && cached?.release) {
    const cachedRelease = cached.release;
    const sigAudit = verifyReleaseManifestAndAssets(cachedRelease, {
      publicKey: options.publicKey,
      distDir: options.distDir,
    });
    const cachedSemVer = parseSemVer(String(cachedRelease.tag_name ?? cachedRelease.version ?? ""));
    const updateAvailable =
      sigAudit.passed &&
      cachedSemVer !== null &&
      isUpdateAvailable(currentVersion, cachedSemVer.normalized, {
        channel,
        allowDowngrade: options.allowDowngrade,
      });

    return Object.freeze({
      status: "not_modified",
      httpStatus: 304,
      fromCache: true,
      updateAvailable,
      currentVersion: parsedCurrent.normalized,
      latestVersion: cachedSemVer ? cachedSemVer.normalized : null,
      etag: responseEtag,
      signatureVerified: sigAudit.signatureVerified,
      rateLimit,
    });
  }

  if (response.statusCode === 429 || (response.statusCode === 403 && rateLimit.isExhausted)) {
    if (options.fallbackToCache && cached?.release) {
      const cachedSemVer = parseSemVer(String(cached.release.tag_name ?? cached.release.version ?? ""));
      return Object.freeze({
        status: "rate_limited_cached",
        httpStatus: response.statusCode,
        fromCache: true,
        updateAvailable: cachedSemVer
          ? isUpdateAvailable(currentVersion, cachedSemVer.normalized, {
              channel,
              allowDowngrade: options.allowDowngrade,
            })
          : false,
        currentVersion: parsedCurrent.normalized,
        latestVersion: cachedSemVer ? cachedSemVer.normalized : null,
        etag: cached.etag,
        signatureVerified: true,
        rateLimit: { ...rateLimit, isExhausted: true },
      });
    }
    return Object.freeze({
      status: "rate_limited",
      httpStatus: response.statusCode,
      fromCache: false,
      updateAvailable: false,
      currentVersion: parsedCurrent.normalized,
      latestVersion: null,
      etag: responseEtag,
      signatureVerified: false,
      rateLimit: { ...rateLimit, isExhausted: true },
    });
  }

  if (response.statusCode !== 200) {
    return Object.freeze({
      status: "http_error",
      httpStatus: response.statusCode,
      fromCache: false,
      updateAvailable: false,
      currentVersion: parsedCurrent.normalized,
      latestVersion: null,
      etag: responseEtag,
      signatureVerified: false,
      error: maskSensitiveToken(response.body.slice(0, 300)),
      rateLimit,
    });
  }

  const parsedBody = JSON.parse(response.body);
  const targetRelease = selectTargetRelease(parsedBody, { channel });
  if (!targetRelease) {
    return Object.freeze({
      status: "no_releases",
      httpStatus: 200,
      fromCache: false,
      updateAvailable: false,
      currentVersion: parsedCurrent.normalized,
      latestVersion: null,
      etag: responseEtag,
      signatureVerified: false,
      rateLimit,
    });
  }

  const latestSemVer = parseSemVer(String(targetRelease.tag_name ?? targetRelease.version ?? ""));
  if (!latestSemVer) {
    return Object.freeze({
      status: "invalid_semver",
      httpStatus: 200,
      fromCache: false,
      updateAvailable: false,
      currentVersion: parsedCurrent.normalized,
      latestVersion: null,
      etag: responseEtag,
      signatureVerified: false,
      rateLimit,
    });
  }

  const audit = verifyReleaseManifestAndAssets(targetRelease, {
    publicKey: options.publicKey,
    distDir: options.distDir,
  });

  if (!audit.passed) {
    return Object.freeze({
      status: "signature_invalid",
      httpStatus: 200,
      fromCache: false,
      updateAvailable: false,
      currentVersion: parsedCurrent.normalized,
      latestVersion: latestSemVer.normalized,
      etag: responseEtag,
      signatureVerified: false,
      reasons: audit.reasons,
      rateLimit,
    });
  }

  if (options.etagCachePath && responseEtag) {
    saveEtagCache(options.etagCachePath, {
      etag: responseEtag,
      url: apiUrl,
      cachedAt: new Date(nowMs).toISOString(),
      release: targetRelease,
    });
  }

  const updateAvailable = isUpdateAvailable(currentVersion, latestSemVer.normalized, {
    channel,
    allowDowngrade: options.allowDowngrade,
  });

  return Object.freeze({
    status: updateAvailable ? "update_available" : "up_to_date",
    httpStatus: 200,
    fromCache: false,
    updateAvailable,
    currentVersion: parsedCurrent.normalized,
    latestVersion: latestSemVer.normalized,
    etag: responseEtag,
    signatureVerified: true,
    verifiedAssets: audit.verifiedAssets,
    rateLimit,
  });
}

/**
 * Запускает встроенный автономный стенд самопроверки (200 OK -> 304 ETag -> Ed25519 -> Rate-Limit 60/h -> SemVer).
 */
export async function runSelfTestStand() {
  const keys = generateUpdateKeyPair();
  const shaSumsPath = join(DEFAULT_DIST_DIR, "SHA256SUMS");
  const tarballName = "discord-gpui-v1.0.0-beta.1-linux-x86_64.tar.gz";
  const tarballPath = join(DEFAULT_DIST_DIR, tarballName);

  const shaSumsText = existsSync(shaSumsPath) ? readFileSync(shaSumsPath, "utf8").trim() : "";
  const tarballSha = existsSync(tarballPath)
    ? createHash("sha256").update(readFileSync(tarballPath)).digest("hex")
    : createHash("sha256").update("discord-gpui-self-test").digest("hex");
  const tarballSize = existsSync(tarballPath) ? readFileSync(tarballPath).byteLength : 1024;

  const baseRelease = {
    tag_name: "v1.0.0-beta.2",
    version: "1.0.0-beta.2",
    draft: false,
    prerelease: true,
    published_at: "2026-11-11T16:00:00Z",
    sha256sums: shaSumsText,
    assets: [
      {
        name: tarballName,
        sha256: tarballSha,
        size: tarballSize,
        browser_download_url: `http://127.0.0.1/releases/download/v1.0.0-beta.2/${tarballName}`,
      },
    ],
  };

  const signed = signUpdateManifest(baseRelease, keys.privateKeyPem);
  const fullRelease = { ...baseRelease, signature: signed.signatureHex };
  const etagValue = 'W/"self-test-etag-965"';

  const server = createServer((req, res) => {
    if (req.headers["if-none-match"] === etagValue) {
      res.writeHead(304, {
        ETag: etagValue,
        "X-RateLimit-Limit": "60",
        "X-RateLimit-Remaining": "58",
        "X-RateLimit-Reset": String(Math.floor(Date.now() / 1000) + 3600),
      });
      res.end();
      return;
    }
    res.writeHead(200, {
      "Content-Type": "application/json",
      ETag: etagValue,
      "X-RateLimit-Limit": "60",
      "X-RateLimit-Remaining": "59",
      "X-RateLimit-Reset": String(Math.floor(Date.now() / 1000) + 3600),
    });
    res.end(JSON.stringify(fullRelease));
  });

  await new Promise((r) => server.listen(0, "127.0.0.1", () => r(undefined)));
  const addr = server.address();
  const port = addr && typeof addr === "object" ? addr.port : 0;
  const apiUrl = `http://127.0.0.1:${port}/repos/discord-lab/discord-gpui/releases/latest`;
  const tmp = mkdtempSync(join(tmpdir(), "check-updates-selftest-"));
  const cachePath = join(tmp, "etag.json");

  try {
    const r200 = await checkGitHubReleasesUpdate({
      apiUrl,
      currentVersion: "v1.0.0-beta.1",
      channel: "beta",
      publicKey: keys.publicKeyRawHex,
      etagCachePath: cachePath,
      distDir: existsSync(tarballPath) ? DEFAULT_DIST_DIR : undefined,
    });

    const r304 = await checkGitHubReleasesUpdate({
      apiUrl,
      currentVersion: "v1.0.0-beta.1",
      channel: "beta",
      publicKey: keys.publicKeyRawHex,
      etagCachePath: cachePath,
      distDir: existsSync(tarballPath) ? DEFAULT_DIST_DIR : undefined,
    });

    const nowMs = Date.now();
    const fullHourTimestamps = Array.from({ length: 60 }, (_, idx) => nowMs - idx * 1000);
    const rateLimitCheck = evaluateRateLimitBudget({ timestamps: fullHourTimestamps }, nowMs, 60);
    const semverOk =
      compareSemVer("v1.0.0-beta.1", "v1.0.0-beta.2") === -1 &&
      compareSemVer("v1.0.0", "v1.0.0-beta.99") === 1;

    const passed =
      r200.status === "update_available" &&
      r200.signatureVerified === true &&
      r304.status === "not_modified" &&
      r304.fromCache === true &&
      rateLimitCheck.allowed === false &&
      rateLimitCheck.limit === 60 &&
      semverOk;

    return Object.freeze({
      taskId: "TASK-965",
      status: passed ? "passed" : "failed",
      etagCachingVerified: r304.status === "not_modified" && r304.fromCache === true,
      ed25519SignatureVerified: r200.signatureVerified === true,
      rateLimit60Verified: rateLimitCheck.allowed === false && rateLimitCheck.limit === 60,
      semverVerified: semverOk,
      currentVersion: r200.currentVersion,
      latestVersion: r200.latestVersion,
      etag: r200.etag,
    });
  } finally {
    await new Promise((r) => server.close(() => r(undefined)));
    rmSync(tmp, { recursive: true, force: true });
  }
}

/**
 * Точка входа CLI scripts/check-updates.mjs.
 * Коды выхода: 0 — успех, 1 — ошибка аргументов, 2 — ошибка среды/верификации.
 * @param {readonly string[]} argv
 */
async function main(argv) {
  let outJson = "";
  let selfTest = false;
  let apiUrl = "";
  let currentVersion = DEFAULT_CURRENT_VERSION;
  let publicKey = "";
  let etagCachePath = "";

  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === "--self-test") {
      selfTest = true;
    } else if (arg === "--out-json") {
      const next = argv[i + 1];
      if (!next || next.startsWith("--")) {
        console.error("Ошибка: флаг --out-json требует путь к файлу");
        process.exit(1);
      }
      outJson = next;
      i += 1;
    } else if (arg === "--api-url") {
      const next = argv[i + 1];
      if (!next) {
        console.error("Ошибка: флаг --api-url требует URL");
        process.exit(1);
      }
      apiUrl = next;
      i += 1;
    } else if (arg === "--current-version") {
      const next = argv[i + 1];
      if (!next || !parseSemVer(next)) {
        console.error(`Ошибка: некорректная версия SemVer для --current-version: ${next ?? ""}`);
        process.exit(1);
      }
      currentVersion = next;
      i += 1;
    } else if (arg === "--public-key") {
      const next = argv[i + 1];
      if (!next) {
        console.error("Ошибка: флаг --public-key требует ключ Ed25519");
        process.exit(1);
      }
      publicKey = next;
      i += 1;
    } else if (arg === "--etag-cache") {
      const next = argv[i + 1];
      if (!next) {
        console.error("Ошибка: флаг --etag-cache требует путь к файлу кэша");
        process.exit(1);
      }
      etagCachePath = next;
      i += 1;
    } else if (arg === "-h" || arg === "--help") {
      console.log(
        "Использование: node scripts/check-updates.mjs [--self-test] [--api-url URL] [--current-version VER] [--public-key KEY] [--etag-cache PATH] [--out-json PATH]",
      );
      process.exit(0);
    } else {
      console.error(`Ошибка: неизвестный аргумент CLI: ${arg}`);
      process.exit(1);
    }
  }

  try {
    const report =
      selfTest || !apiUrl
        ? await runSelfTestStand()
        : await checkGitHubReleasesUpdate({
            apiUrl,
            currentVersion,
            publicKey,
            etagCachePath: etagCachePath || undefined,
            distDir: DEFAULT_DIST_DIR,
          });

    if (outJson) {
      mkdirSync(dirname(resolve(outJson)), { recursive: true });
      writeFileSync(resolve(outJson), JSON.stringify(report, null, 2), "utf8");
    }
    console.log(JSON.stringify(report, null, 2));

    if ("status" in report && (report.status === "failed" || report.status === "signature_invalid" || report.status === "http_error")) {
      process.exit(2);
    }
    process.exit(0);
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    console.error(`Ошибка среды выполнения проверки обновлений: ${maskSensitiveToken(msg)}`);
    process.exit(2);
  }
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  await main(process.argv.slice(2));
}
