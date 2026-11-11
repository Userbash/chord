// @ts-check
/**
 * Контрактные и модульные тесты стенда верификации авто-обновления через GitHub Releases API v3 (TASK-965):
 * - SUB-965-1: Семантическое версионирование SemVer 2.0.0 (парсинг, сравнение приоритетов,
 *   обработка префикса v, pre-release идентификаторов alpha/beta/rc, игнорирование build-метаданных +build,
 *   фильтрация каналов stable/beta и защита от даунгрейда).
 * - SUB-965-2: Криптографическая проверка подписи Ed25519 релизного манифеста и контрольных сумм
 *   артефактов rust/dist/SHA256SUMS (валидная подпись, искажение 1 бита в начале/середине/конце,
 *   неверный публичный ключ, неверная длина подписи, подмена SHA-256 архива).
 * - SUB-965-3: Поддержка ETag кэширования (If-None-Match -> HTTP 304 Not Modified с возвратом
 *   кэшированного релиза без повторного скачивания) и соблюдение лимита Rate-Limit 60 req/h
 *   (X-RateLimit-Limit: 60, X-RateLimit-Remaining: 0, X-RateLimit-Reset, HTTP 403/429 Retry-After,
 *   локальное скользящее окно 60 запросов в час).
 * - SUB-965-4: Сквозной HTTP-стенд GitHub Releases API v3 и CLI scripts/check-updates.mjs
 *   (коды выхода 0 / 1 / 2, маскирование токенов, обработка пустых релизов/draft/5xx).
 */
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, it } from "node:test";
import { fileURLToPath } from "node:url";

const ROOT = fileURLToPath(new URL("../../", import.meta.url));
const SCRIPT_PATH = join(ROOT, "scripts", "check-updates.mjs");
const DIST_DIR = join(ROOT, "rust", "dist");
const SHA256SUMS_PATH = join(DIST_DIR, "SHA256SUMS");
const TARBALL_NAME = "discord-gpui-v1.0.0-beta.1-linux-x86_64.tar.gz";

describe("TASK-965: Стенд верификации авто-обновления через GitHub Releases API v3", () => {
  it("SUB-965-1: парсит и сравнивает версии по стандарту SemVer 2.0.0 (позитивные, негативные и граничные случаи)", async () => {
    const {
      parseSemVer,
      compareSemVer,
      isUpdateAvailable,
    } = await import("../../scripts/check-updates.mjs");

    // 1. Позитивный парсинг с префиксом v, pre-release и build-метаданными
    const parsed = parseSemVer("v1.0.0-beta.1+20261111.build005");
    assert.ok(parsed, "Версия v1.0.0-beta.1+20261111.build005 должна парситься");
    assert.equal(parsed.major, 1);
    assert.equal(parsed.minor, 0);
    assert.equal(parsed.patch, 0);
    assert.deepEqual(parsed.prerelease, ["beta", "1"]);
    assert.deepEqual(parsed.build, ["20261111", "build005"]);
    assert.equal(parsed.normalized, "1.0.0-beta.1");

    // 2. Граничные значения SemVer (0.0.0, большие числа, порядок pre-release по SemVer 2.0.0 §11)
    assert.equal(compareSemVer("0.0.0", "0.0.0"), 0);
    assert.equal(compareSemVer("v1.0.0-beta.1", "1.0.0-beta.1"), 0);
    assert.equal(
      compareSemVer("1.0.0-beta.1+build.1", "1.0.0-beta.1+build.999"),
      0,
      "Build-метаданные (+...) не должны влиять на приоритет версий по SemVer 2.0.0 §10",
    );

    const orderedChain = [
      "0.0.0",
      "0.1.0-alpha",
      "0.1.0-alpha.1",
      "0.1.0-alpha.beta",
      "0.1.0-beta",
      "0.1.0-beta.2",
      "0.1.0-beta.11",
      "0.1.0-rc.1",
      "0.1.0",
      "1.0.0-alpha.1",
      "1.0.0-beta.1",
      "1.0.0-beta.2",
      "1.0.0-beta.10",
      "1.0.0-rc.1",
      "1.0.0",
      "1.0.1",
      "1.1.0",
      "2.0.0",
    ];
    for (let i = 0; i < orderedChain.length - 1; i += 1) {
      const lower = orderedChain[i];
      const higher = orderedChain[i + 1];
      assert.ok(lower && higher);
      assert.equal(
        compareSemVer(lower, higher),
        -1,
        `Ожидалось ${lower} < ${higher}`,
      );
      assert.equal(
        compareSemVer(higher, lower),
        1,
        `Ожидалось ${higher} > ${lower}`,
      );
    }

    // 3. Негативные сценарии SemVer: ведущие нули, пустые строки, мусорные символы
    for (const invalidVer of [
      "",
      "v",
      "1.0",
      "1.0.0.0",
      "01.0.0",
      "1.01.0",
      "1.0.01",
      "1.0.0-01",
      "1.0.0-",
      "1.0.0+",
      "1.0.0-beta..1",
      "not-a-version",
    ]) {
      assert.equal(
        parseSemVer(invalidVer),
        null,
        `Некорректная строка "${invalidVer}" должна отвергаться парсером SemVer`,
      );
      assert.throws(
        () => compareSemVer(invalidVer, "1.0.0"),
        /Некорректная версия SemVer/i,
      );
    }

    // 4. Политика каналов обновлений (stable vs beta) и защита от даунгрейда
    assert.equal(
      isUpdateAvailable("v1.0.0-beta.1", "v1.0.0-beta.2", { channel: "beta" }),
      true,
      "На канале beta переход 1.0.0-beta.1 -> 1.0.0-beta.2 должен определяться как обновление",
    );
    assert.equal(
      isUpdateAvailable("v1.0.0", "v1.1.0-beta.1", { channel: "stable" }),
      false,
      "На канале stable pre-release 1.1.0-beta.1 должен игнорироваться",
    );
    assert.equal(
      isUpdateAvailable("v1.0.0", "v1.1.0", { channel: "stable" }),
      true,
      "На канале stable релиз 1.1.0 > 1.0.0 должен приниматься",
    );
    assert.equal(
      isUpdateAvailable("v1.0.0-beta.2", "v1.0.0-beta.1", { channel: "beta" }),
      false,
      "Даунгрейд 1.0.0-beta.2 -> 1.0.0-beta.1 должен блокироваться по умолчанию",
    );
    assert.equal(
      isUpdateAvailable("v1.0.0-beta.2", "v1.0.0-beta.1", {
        channel: "beta",
        allowDowngrade: true,
      }),
      true,
      "При явном флаге allowDowngrade откат версии разрешён",
    );
  });

  it("SUB-965-2: проверяет Ed25519 подпись манифеста и SHA-256 контрольные суммы артефактов (вкл. мутации битов)", async () => {
    const {
      generateUpdateKeyPair,
      canonicalizeManifestPayload,
      signUpdateManifest,
      verifyEd25519Signature,
      verifyReleaseManifestAndAssets,
    } = await import("../../scripts/check-updates.mjs");

    const keys = generateUpdateKeyPair();
    const otherKeys = generateUpdateKeyPair();

    // Проверка длины ключей Ed25519 (32 байта = 64 hex символа публичного ключа)
    assert.equal(keys.publicKeyRawHex.length, 64);

    const realSumsText = readFileSync(SHA256SUMS_PATH, "utf8");
    const tarballShaMatch = new RegExp(`^([a-f0-9]{64})\\s+\\*?${TARBALL_NAME.replaceAll(".", "\\.")}$`, "m").exec(realSumsText);
    assert.ok(tarballShaMatch?.[1], "В rust/dist/SHA256SUMS должен быть хеш релизного архива");
    const realTarballSha = tarballShaMatch[1];

    const manifest = {
      tag_name: "v1.0.0-beta.1",
      version: "1.0.0-beta.1",
      published_at: "2026-11-11T12:00:00Z",
      sha256sums: realSumsText.trim(),
      assets: [
        {
          name: TARBALL_NAME,
          sha256: realTarballSha,
          size: readFileSync(join(DIST_DIR, TARBALL_NAME)).byteLength,
          browser_download_url: `https://github.com/discord-lab/discord-gpui/releases/download/v1.0.0-beta.1/${TARBALL_NAME}`,
        },
      ],
    };

    const canonical = canonicalizeManifestPayload(manifest);
    const signed = signUpdateManifest(manifest, keys.privateKeyPem);
    assert.equal(signed.signatureHex.length, 128, "Подпись Ed25519 должна быть ровно 64 байта (128 hex символов)");

    // 1. Позитивная проверка как по PEM, так и по raw 32-byte hex ключу и base64 подписи
    const verifyPem = verifyEd25519Signature(canonical, signed.signatureHex, keys.publicKeyPem);
    assert.equal(verifyPem.valid, true, `Ожидалась валидная подпись: ${verifyPem.reason}`);
    assert.equal(verifyPem.algorithm, "Ed25519");

    const verifyRawHex = verifyEd25519Signature(
      canonical,
      signed.signatureBase64,
      keys.publicKeyRawHex,
    );
    assert.equal(verifyRawHex.valid, true, "Должна поддерживаться проверка по raw-hex ключу и base64 подписи");

    // 2. Мутации подписи: инверсия 1 бита в начале (байт 0), середине (байт 31) и конце (байт 63)
    const sigBytes = Buffer.from(signed.signatureHex, "hex");
    for (const byteIdx of [0, 31, 63]) {
      const mutated = Buffer.from(sigBytes);
      mutated[byteIdx] = (mutated[byteIdx] ?? 0) ^ 0x01;
      const res = verifyEd25519Signature(canonical, mutated.toString("hex"), keys.publicKeyRawHex);
      assert.equal(
        res.valid,
        false,
        `Искажение бита в байте ${byteIdx} подписи Ed25519 обязано детектироваться`,
      );
    }

    // 3. Негативные сценарии: чужой публичный ключ, подмена текста манифеста, усечённая/пустая подпись
    const wrongKeyRes = verifyEd25519Signature(
      canonical,
      signed.signatureHex,
      otherKeys.publicKeyRawHex,
    );
    assert.equal(wrongKeyRes.valid, false, "Подпись от другого ключа Ed25519 должна отвергаться");

    const tamperedManifest = { ...manifest, tag_name: "v1.0.0-beta.2" };
    const tamperedRes = verifyEd25519Signature(
      canonicalizeManifestPayload(tamperedManifest),
      signed.signatureHex,
      keys.publicKeyRawHex,
    );
    assert.equal(tamperedRes.valid, false, "Изменение tag_name в манифесте должно инвалидировать подпись");

    for (const badSig of ["", "abcd", "00".repeat(63), "00".repeat(65), "zz".repeat(64)]) {
      const badRes = verifyEd25519Signature(canonical, badSig, keys.publicKeyRawHex);
      assert.equal(badRes.valid, false, `Некорректная подпись "${badSig.slice(0, 16)}" должна отвергаться`);
    }

    // 4. Комплексная проверка релиза и реального артефакта в rust/dist/
    const fullCheck = verifyReleaseManifestAndAssets(
      { ...manifest, signature: signed.signatureHex },
      { publicKey: keys.publicKeyRawHex, distDir: DIST_DIR },
    );
    assert.equal(fullCheck.passed, true, `Проверка манифеста и артефактов упала: ${fullCheck.reasons.join("; ")}`);
    assert.equal(fullCheck.signatureVerified, true);
    assert.equal(fullCheck.verifiedAssets.length, 1);
    assert.equal(fullCheck.verifiedAssets[0]?.name, TARBALL_NAME);

    // 5. Негативный сценарий: подмена SHA-256 хеша ассета (даже если переподписано атакующим без ключа или хеш не бьётся с файлом)
    const badHashManifest = {
      ...manifest,
      assets: [
        {
          ...manifest.assets[0],
          sha256: "f".repeat(64),
        },
      ],
    };
    const badHashSigned = signUpdateManifest(badHashManifest, keys.privateKeyPem);
    const badHashCheck = verifyReleaseManifestAndAssets(
      { ...badHashManifest, signature: badHashSigned.signatureHex },
      { publicKey: keys.publicKeyRawHex, distDir: DIST_DIR },
    );
    assert.equal(badHashCheck.passed, false, "Несовпадение SHA-256 файла на диске должно приводить к отказу");
    assert.match(badHashCheck.reasons.join("; "), /SHA-256/i);
  });

  it("SUB-965-3: реализует ETag кэширование (If-None-Match / 304 Not Modified) и лимит Rate-Limit 60 req/h", async () => {
    const {
      DEFAULT_RATE_LIMIT_PER_HOUR,
      buildGitHubReleaseRequestHeaders,
      parseRateLimitHeaders,
      evaluateRateLimitBudget,
      loadEtagCache,
      saveEtagCache,
    } = await import("../../scripts/check-updates.mjs");

    assert.equal(DEFAULT_RATE_LIMIT_PER_HOUR, 60, "Лимит GitHub API v3 без токена должен быть 60 req/h");

    // 1. Формирование заголовков запроса с ETag (If-None-Match)
    const headersWithEtag = buildGitHubReleaseRequestHeaders({
      etag: 'W/"a1b2c3d4e5"',
      userAgent: "discord-gpui-updater/1.0.0",
    });
    assert.equal(headersWithEtag.Accept, "application/vnd.github+json");
    assert.equal(headersWithEtag["X-GitHub-Api-Version"], "2022-11-28");
    assert.equal(headersWithEtag["If-None-Match"], 'W/"a1b2c3d4e5"');
    assert.equal(headersWithEtag["User-Agent"], "discord-gpui-updater/1.0.0");

    // 2. Парсинг заголовков X-RateLimit-* и Retry-After
    const nowMs = 1_762_860_000_000;
    const resetSec = Math.floor((nowMs + 900_000) / 1000);
    const parsedLimit = parseRateLimitHeaders(
      {
        "x-ratelimit-limit": "60",
        "x-ratelimit-remaining": "0",
        "x-ratelimit-used": "60",
        "x-ratelimit-reset": String(resetSec),
      },
      nowMs,
    );
    assert.equal(parsedLimit.limit, 60);
    assert.equal(parsedLimit.remaining, 0);
    assert.equal(parsedLimit.used, 60);
    assert.equal(parsedLimit.isExhausted, true);
    assert.ok(parsedLimit.retryAfterMs > 0 && parsedLimit.retryAfterMs <= 900_000);

    // 3. Граничные испытания клиентского бюджета 60 запросов в час (1..60 разрешены, 61-й блокируется)
    /** @type {number[]} */
    let timestamps = [];
    for (let i = 1; i <= 60; i += 1) {
      const stepTime = nowMs + i * 1000;
      const verdict = evaluateRateLimitBudget({ timestamps }, stepTime, 60);
      assert.equal(verdict.allowed, true, `Запрос #${i} из 60 должен быть разрешён`);
      assert.equal(verdict.remaining, 60 - i);
      timestamps = verdict.nextTimestamps;
    }

    // 61-й запрос внутри того же часового окна (3600 с) должен быть заблокирован
    const blocked61 = evaluateRateLimitBudget({ timestamps }, nowMs + 61_000, 60);
    assert.equal(blocked61.allowed, false, "61-й запрос за час обязан блокироваться Rate-Limiter'ом");
    assert.equal(blocked61.remaining, 0);
    assert.ok(blocked61.retryAfterMs > 3_500_000, "retryAfterMs должен указывать время до освобождения слота");

    // Спустя 1 час + 2 секунды окно очищается и запрос снова разрешён
    const afterWindow = evaluateRateLimitBudget({ timestamps }, nowMs + 3_602_000, 60);
    assert.equal(afterWindow.allowed, true, "После истечения 3600 секунд окно квоты должно восстановиться");

    // 4. Запись и чтение файла кэша ETag на диске
    const tmp = mkdtempSync(join(tmpdir(), "etag-cache-965-"));
    try {
      const cacheFile = join(tmp, "github-releases-etag.json");
      assert.equal(loadEtagCache(cacheFile), null, "Для несуществующего файла кэш должен возвращать null");

      const sampleEntry = {
        etag: '"etag-v1.0.0-beta.2"',
        url: "https://api.github.com/repos/discord-lab/discord-gpui/releases/latest",
        cachedAt: "2026-11-11T16:00:00.000Z",
        release: {
          tag_name: "v1.0.0-beta.2",
          version: "1.0.0-beta.2",
        },
      };
      saveEtagCache(cacheFile, sampleEntry);
      const loaded = loadEtagCache(cacheFile);
      assert.deepEqual(loaded, sampleEntry);
    } finally {
      rmSync(tmp, { recursive: true, force: true });
    }
  });

  it("SUB-965-4: выполняет сквозной цикл проверки обновлений через локальный HTTP-стенд GitHub Releases API v3 (200 -> 304 ETag -> 429 Rate-Limit -> CLI)", async () => {
    const {
      generateUpdateKeyPair,
      signUpdateManifest,
      checkGitHubReleasesUpdate,
    } = await import("../../scripts/check-updates.mjs");

    const keys = generateUpdateKeyPair();
    const realSumsText = readFileSync(SHA256SUMS_PATH, "utf8");
    const tarballBytes = readFileSync(join(DIST_DIR, TARBALL_NAME));
    const tarballSha = createHash("sha256").update(tarballBytes).digest("hex");

    const releasePayloadBase = {
      tag_name: "v1.0.0-beta.2",
      version: "1.0.0-beta.2",
      name: "Discord GPUI v1.0.0-beta.2",
      draft: false,
      prerelease: true,
      published_at: "2026-11-11T16:00:00Z",
      sha256sums: realSumsText.trim(),
      assets: [
        {
          name: TARBALL_NAME,
          sha256: tarballSha,
          size: tarballBytes.byteLength,
          browser_download_url: `http://127.0.0.1/download/${TARBALL_NAME}`,
        },
      ],
    };
    const signed = signUpdateManifest(releasePayloadBase, keys.privateKeyPem);
    const signedRelease = {
      ...releasePayloadBase,
      signature: signed.signatureHex,
    };

    const expectedEtag = 'W/"release-v1.0.0-beta.2-etag"';
    /** @type {Array<{ method: string, url: string, ifNoneMatch: string | undefined }>} */
    const requestLog = [];
    let simulateMode = "normal"; // "normal" | "rate_limited" | "tampered_sig" | "empty_draft"

    const server = createServer((req, res) => {
      const ifNoneMatch = /** @type {string | undefined} */ (req.headers["if-none-match"]);
      requestLog.push({
        method: req.method ?? "GET",
        url: req.url ?? "/",
        ifNoneMatch,
      });

      if (simulateMode === "rate_limited") {
        res.writeHead(429, {
          "Content-Type": "application/json",
          "X-RateLimit-Limit": "60",
          "X-RateLimit-Remaining": "0",
          "X-RateLimit-Reset": String(Math.floor(Date.now() / 1000) + 1200),
          "Retry-After": "1200",
        });
        res.end(JSON.stringify({ message: "API rate limit exceeded" }));
        return;
      }

      if (simulateMode === "empty_draft") {
        res.writeHead(200, {
          "Content-Type": "application/json",
          "X-RateLimit-Limit": "60",
          "X-RateLimit-Remaining": "58",
        });
        res.end(JSON.stringify([{ ...signedRelease, draft: true }]));
        return;
      }

      if (simulateMode === "tampered_sig") {
        res.writeHead(200, {
          "Content-Type": "application/json",
          ETag: '"tampered-etag"',
          "X-RateLimit-Limit": "60",
          "X-RateLimit-Remaining": "57",
        });
        res.end(
          JSON.stringify({
            ...signedRelease,
            signature: "00".repeat(64),
          }),
        );
        return;
      }

      if (ifNoneMatch === expectedEtag) {
        res.writeHead(304, {
          ETag: expectedEtag,
          "X-RateLimit-Limit": "60",
          "X-RateLimit-Remaining": "59",
          "X-RateLimit-Reset": String(Math.floor(Date.now() / 1000) + 3600),
        });
        res.end();
        return;
      }

      res.writeHead(200, {
        "Content-Type": "application/json",
        ETag: expectedEtag,
        "X-RateLimit-Limit": "60",
        "X-RateLimit-Remaining": "59",
        "X-RateLimit-Reset": String(Math.floor(Date.now() / 1000) + 3600),
      });
      res.end(JSON.stringify(signedRelease));
    });

    await new Promise((resolve) => server.listen(0, "127.0.0.1", () => resolve(undefined)));
    const addr = server.address();
    assert.ok(addr && typeof addr === "object");
    const apiUrl = `http://127.0.0.1:${addr.port}/repos/discord-lab/discord-gpui/releases/latest`;

    const tmp = mkdtempSync(join(tmpdir(), "update-stand-965-"));
    const etagCachePath = join(tmp, "etag-cache.json");

    try {
      // Шаг 1: Первый запрос -> 200 OK, валидация Ed25519 подписи, сохранение ETag в кэш
      const firstCheck = await checkGitHubReleasesUpdate({
        apiUrl,
        currentVersion: "v1.0.0-beta.1",
        channel: "beta",
        publicKey: keys.publicKeyRawHex,
        etagCachePath,
        distDir: DIST_DIR,
      });

      assert.equal(firstCheck.status, "update_available");
      assert.equal(firstCheck.httpStatus, 200);
      assert.equal(firstCheck.fromCache, false);
      assert.equal(firstCheck.updateAvailable, true);
      assert.equal(firstCheck.latestVersion, "1.0.0-beta.2");
      assert.equal(firstCheck.signatureVerified, true);
      assert.equal(firstCheck.etag, expectedEtag);
      assert.equal(firstCheck.rateLimit.limit, 60);
      assert.equal(firstCheck.rateLimit.remaining, 59);

      // Шаг 2: Повторный запрос с тем же etagCachePath -> отправляет If-None-Match и получает 304 Not Modified
      const secondCheck = await checkGitHubReleasesUpdate({
        apiUrl,
        currentVersion: "v1.0.0-beta.1",
        channel: "beta",
        publicKey: keys.publicKeyRawHex,
        etagCachePath,
        distDir: DIST_DIR,
      });

      assert.equal(requestLog[1]?.ifNoneMatch, expectedEtag, "Второй запрос должен передать заголовок If-None-Match");
      assert.equal(secondCheck.status, "not_modified");
      assert.equal(secondCheck.httpStatus, 304);
      assert.equal(secondCheck.fromCache, true);
      assert.equal(secondCheck.updateAvailable, true);
      assert.equal(secondCheck.latestVersion, "1.0.0-beta.2");
      assert.equal(secondCheck.signatureVerified, true);

      // Шаг 3: Ответ 429 Rate Limit Exceeded с fallback на кэш и без кэша
      simulateMode = "rate_limited";
      const fallbackCheck = await checkGitHubReleasesUpdate({
        apiUrl,
        currentVersion: "v1.0.0-beta.1",
        channel: "beta",
        publicKey: keys.publicKeyRawHex,
        etagCachePath,
        fallbackToCache: true,
      });
      assert.equal(fallbackCheck.status, "rate_limited_cached");
      assert.equal(fallbackCheck.fromCache, true);
      assert.equal(fallbackCheck.rateLimit.isExhausted, true);
      assert.ok(fallbackCheck.rateLimit.retryAfterMs > 0);

      const noFallbackCheck = await checkGitHubReleasesUpdate({
        apiUrl,
        currentVersion: "v1.0.0-beta.1",
        channel: "beta",
        publicKey: keys.publicKeyRawHex,
        etagCachePath: join(tmp, "empty-cache.json"),
        fallbackToCache: false,
      });
      assert.equal(noFallbackCheck.status, "rate_limited");
      assert.equal(noFallbackCheck.httpStatus, 429);
      assert.equal(noFallbackCheck.rateLimit.isExhausted, true);

      // Шаг 4: Негативный сценарий — искажённая Ed25519 подпись от сервера блокирует обновление
      simulateMode = "tampered_sig";
      const tamperedCheck = await checkGitHubReleasesUpdate({
        apiUrl,
        currentVersion: "v1.0.0-beta.1",
        channel: "beta",
        publicKey: keys.publicKeyRawHex,
        etagCachePath: join(tmp, "tampered-cache.json"),
      });
      assert.equal(tamperedCheck.status, "signature_invalid");
      assert.equal(tamperedCheck.signatureVerified, false);
      assert.equal(tamperedCheck.updateAvailable, false);

      // Шаг 5: Пустой список релизов (только draft: true)
      simulateMode = "empty_draft";
      const emptyDraftCheck = await checkGitHubReleasesUpdate({
        apiUrl,
        currentVersion: "v1.0.0-beta.1",
        channel: "beta",
        publicKey: keys.publicKeyRawHex,
        etagCachePath: join(tmp, "draft-cache.json"),
      });
      assert.equal(emptyDraftCheck.status, "no_releases");
      assert.equal(emptyDraftCheck.updateAvailable, false);

      // Шаг 6: Проверка CLI scripts/check-updates.mjs --self-test --out-json
      const outJsonPath = join(tmp, "cli-report.json");
      const cliRun = spawnSync(
        process.execPath,
        [SCRIPT_PATH, "--self-test", "--out-json", outJsonPath],
        { cwd: ROOT, encoding: "utf8" },
      );
      assert.equal(cliRun.status, 0, `CLI завершился с ошибкой: ${cliRun.stderr}`);
      assert.ok(existsSync(outJsonPath), "CLI должен записать JSON-отчёт");
      const cliReport = JSON.parse(readFileSync(outJsonPath, "utf8"));
      assert.equal(cliReport.taskId, "TASK-965");
      assert.equal(cliReport.status, "passed");
      assert.equal(cliReport.etagCachingVerified, true);
      assert.equal(cliReport.ed25519SignatureVerified, true);
      assert.equal(cliReport.rateLimit60Verified, true);
      assert.equal(cliReport.semverVerified, true);

      // Код выхода 1 при неизвестном флаге
      const badCli = spawnSync(process.execPath, [SCRIPT_PATH, "--unknown-flag"], {
        cwd: ROOT,
        encoding: "utf8",
      });
      assert.equal(badCli.status, 1);
    } finally {
      await new Promise((resolve) => server.close(() => resolve(undefined)));
      rmSync(tmp, { recursive: true, force: true });
    }
  });

  it("SUB-965-5: проверяет статус up_to_date, блокировку 61-го запроса до сети и маскирование токенов", async () => {
    const {
      generateUpdateKeyPair,
      signUpdateManifest,
      checkGitHubReleasesUpdate,
      maskSensitiveToken,
    } = await import("../../scripts/check-updates.mjs");

    // 1. Маскирование секретных токенов GitHub/Discord в логах ошибок
    const fakeGhp = "ghp_" + "A1b2C3d4E5f6G7h8I9j0K1l2M3n4O5p6Q7r8";
    const fakeBearer = "Bearer " + "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.secretPayloadToken12345";
    const masked = maskSensitiveToken(`Auth failed: ${fakeGhp} and ${fakeBearer}`);
    assert.ok(masked.includes("ghp_***"), "Токен ghp_ должен маскироваться в ghp_***");
    assert.ok(masked.includes("Bearer ***"), "Токен Bearer должен маскироваться в Bearer ***");
    assert.ok(!masked.includes(fakeGhp), "Сырой токен ghp_ не должен оставаться в тексте ошибки");

    // 2. Блокировка 61-го запроса за час локальным Rate-Limiter'ом без обращения к серверу
    const keys = generateUpdateKeyPair();
    const nowMs = 1_762_860_000_000;
    const sixtyTimestamps = Array.from({ length: 60 }, (_, idx) => nowMs - idx * 1000);
    const localBlocked = await checkGitHubReleasesUpdate({
      apiUrl: "http://127.0.0.1:1/unreachable-should-not-be-called",
      currentVersion: "v1.0.0-beta.1",
      channel: "beta",
      publicKey: keys.publicKeyRawHex,
      rateLimitState: { timestamps: sixtyTimestamps },
      nowMs,
    });
    assert.equal(localBlocked.status, "rate_limited");
    assert.equal(localBlocked.httpStatus, 429);
    assert.equal(localBlocked.rateLimit.isExhausted, true);
    assert.equal(localBlocked.rateLimit.remaining, 0);

    // 3. Проверка статуса up_to_date, когда установленная версия равна или новее релиза
    const releaseObj = {
      tag_name: "v1.0.0-beta.1",
      version: "1.0.0-beta.1",
      draft: false,
      prerelease: true,
      published_at: "2026-11-11T12:00:00Z",
      sha256sums: "",
      assets: [],
    };
    const sig = signUpdateManifest(releaseObj, keys.privateKeyPem);
    const srv = createServer((_req, res) => {
      res.writeHead(200, { "Content-Type": "application/json", ETag: '"etag-same"' });
      res.end(JSON.stringify({ ...releaseObj, signature: sig.signatureHex }));
    });
    await new Promise((r) => srv.listen(0, "127.0.0.1", () => r(undefined)));
    try {
      const addr = srv.address();
      assert.ok(addr && typeof addr === "object");
      const upToDateRes = await checkGitHubReleasesUpdate({
        apiUrl: `http://127.0.0.1:${addr.port}/repos/discord-lab/discord-gpui/releases/latest`,
        currentVersion: "v1.0.0-beta.1",
        channel: "beta",
        publicKey: keys.publicKeyRawHex,
      });
      assert.equal(upToDateRes.status, "up_to_date");
      assert.equal(upToDateRes.updateAvailable, false);
      assert.equal(upToDateRes.signatureVerified, true);
    } finally {
      await new Promise((r) => srv.close(() => r(undefined)));
    }
  });
});
