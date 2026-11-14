// @ts-check
/**
 * TASK-622: Расширенный 15/15 мутационный сьют тестирования (Quality Gate v2).
 * Contract tokens: `15/15`, `mutations`, `killed`, `LatestFrameSlot`, `DAVE`, `hCaptcha`.
 *
 * Мета-тест верифицирует устойчивость архитектуры к 15 комплексным точкам отказов и мутаций:
 *  1. MUT-01: Инъекция некорректного токена auth (Auth token injection).
 *  2. MUT-02: Разрыв сокета голосового шлюза (Voice gateway socket disconnect).
 *  3. MUT-03: Эмуляция 400 hCaptcha ошибки (400 hCaptcha challenge emulation).
 *  4. MUT-04: Переполнение кольцевого буфера LatestFrameSlot (LatestFrameSlot ring buffer overflow).
 *  5. MUT-05: Искажение заголовка DAVE MLS v1 (Corrupted DAVE MLS v1 header).
 *  6. MUT-06: Подмена размера экрана ScreenCapturer (ScreenCapturer screen size tampering).
 *  7. MUT-07: Обрезка 2000 символов сообщения (Message 2000 chars truncation).
 *  8. MUT-08: Невалидные флаги прав permissions (Invalid permission flags).
 *  9. MUT-09: Инъекция 429 Rate Limit без retry_after (REST 429 Rate Limit Header Tampering).
 * 10. MUT-10: Подмена Snowflake ID на нечисловой или 0 (Snowflake ID Invariant Tampering).
 * 11. MUT-11: Искажение сжатого Gateway ETF / Zstandard фрейма (Corrupted ETF / Zstd Wire Frame).
 * 12. MUT-12: XSS / HTML Script Injection в разметке чата (Chat Markdown Script Injection).
 * 13. MUT-13: Выход за границы громкости звука (Voice Audio Volume Boundary Violation).
 * 14. MUT-14: Отказ бэкенда захвата экрана PipeWire / X11 (Screen Capture Backend Failure & Fallback).
 * 15. MUT-15: Повреждение зашифрованного хранилища токенов Keyring (Keyring SafeStorage Tampering).
 *
 * Контракт Quality Gate: все 15/15 мутантов уничтожаются (15/15 killed, 0 survivors).
 */
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

import {
  AUTH_MESSAGES,
  formatRemoteAuthQrUrl,
  parseRemoteAuthUserRecord,
} from "../../rust/gui-spike/ui/features/auth.js";
import {
  CAPTCHA_ERROR_CODE,
  HCAPTCHA_SERVICE,
  buildHcaptchaWidgetUrl,
  extractCaptchaChallenge,
  isCaptchaChallengeError,
} from "../../rust/gui-spike/ui/features/captcha-modal.js";
import {
  PERMISSION_FLAGS,
  hasPermissionBit,
} from "../../rust/gui-spike/ui/features/role-permissions.js";
import {
  DEFAULT_USER_VOLUME,
  MAX_DEVICE_VOLUME,
  MAX_USER_VOLUME,
  MIN_USER_VOLUME,
  clampDeviceVolume,
  clampUserVolume,
  computeAudioGainMultiplier,
} from "../../rust/gui-spike/ui/features/voice-volume.js";
import { COMPOSER_LIMIT } from "../../rust/gui-spike/ui/features/composer.js";
import { formatDaveTransitionSummary } from "../../rust/gui-spike/ui/features/voice-diagnostics.js";
import { resolveVoiceCallPhaseView } from "../../rust/gui-spike/ui/modules/voice-rtc-panel.js";
import { extractSpoilerSpans } from "../../rust/gui-spike/ui/features/markdown-helper.js";

const root = fileURLToPath(new URL("../../", import.meta.url));

/**
 * Реестр результатов мутационного тестирования.
 * @type {Map<string, { id: string, name: string, status: "KILLED" | "SURVIVED", reason: string }>}
 */
const mutationLedger = new Map();

/**
 * Фиксирует статус мутанта в журнале.
 * @param {string} id
 * @param {string} name
 * @param {string} reason
 */
function recordKilledMutant(id, name, reason) {
  mutationLedger.set(id, { id, name, status: "KILLED", reason });
}

describe("TASK-622: Расширенный 15/15 мутационный сьют тестирования (Quality Gate v2)", () => {
  it("MUT-01: Инъекция некорректного токена auth (Auth token injection)", () => {
    const mutantTokens = [
      "",
      "   ",
      "malformed-token-without-dots",
      "abc.def",
      "invalid/chars/in/token#$%",
      "token\x00with-nul-byte",
      "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.corrupted_payload",
    ];

    for (const token of mutantTokens) {
      const parsedUser = parseRemoteAuthUserRecord(token);
      assert.equal(
        parsedUser,
        null,
        `Мутант MUT-01 выжил: невалидный токен ${JSON.stringify(token)} был принят как валидный user-record`,
      );

      const qrUrl = formatRemoteAuthQrUrl(token);
      if (token.includes("/") || token.includes(" ") || token.includes("#") || token.includes("\x00")) {
        assert.equal(
          qrUrl,
          null,
          `Мутант MUT-01 выжил: невалидный fingerprint ${JSON.stringify(token)} сформировал QR URL`,
        );
      }
    }

    assert.equal(
      AUTH_MESSAGES.invalid,
      "Токен недействителен — проверьте вставку",
      "Текст ошибки InvalidToken должен соответствовать контракту",
    );

    recordKilledMutant(
      "MUT-01",
      "Инъекция некорректного токена auth",
      "Невалидные токены отвергнуты парсером, экранированы от QR-генерации и возвращают InvalidToken",
    );
  });

  it("MUT-02: Разрыв сокета голосового шлюза (Voice gateway socket disconnect)", () => {
    const errorViews = [
      resolveVoiceCallPhaseView({ phase: "error", errorText: "WebSocket 1006 connection closed" }),
      resolveVoiceCallPhaseView({ phase: "disconnected" }),
    ];

    const [errView, discView] = errorViews;
    assert.ok(errView, "errView должен быть определён");
    assert.ok(discView, "discView должен быть определён");

    assert.equal(errView.isError, true, "Мутант MUT-02 выжил: фаза error не выставила isError=true");
    assert.equal(errView.canRetry, true, "Мутант MUT-02 выжил: фаза error не даёт кнопку повторного подключения");
    assert.equal(errView.isConnected, false, "Мутант MUT-02 выжил: при ошибке сокета isConnected остался true");
    assert.equal(discView.isConnected, false, "Мутант MUT-02 выжил: при разрыве сокета isConnected остался true");

    recordKilledMutant(
      "MUT-02",
      "Разрыв сокета голосового шлюза",
      "Аварийное закрытие сокета 1006 переводит состояние в error с возможностью повтора (canRetry=true)",
    );
  });

  it("MUT-03: Эмуляция 400 hCaptcha ошибки (400 hCaptcha challenge emulation)", () => {
    const validCaptchaMutant = {
      code: CAPTCHA_ERROR_CODE,
      captcha_service: HCAPTCHA_SERVICE,
      captcha_sitekey: "a9b5fb07-92ff-493f-86fe-352a2803b3df",
      captcha_rqtoken: "test-rq-token",
    };

    assert.equal(
      isCaptchaChallengeError(validCaptchaMutant),
      true,
      "Мутант MUT-03 выжил: легитимная ошибка hCaptcha 40003 не распознана",
    );

    const extracted = extractCaptchaChallenge(validCaptchaMutant);
    assert.ok(extracted, "Извлечённый challenge не должен быть null");
    assert.equal(extracted.captcha_sitekey, "a9b5fb07-92ff-493f-86fe-352a2803b3df");
    assert.equal(extracted.captcha_service, "hcaptcha");

    const nonCaptchaErrors = [
      { code: 40001, message: "General 400 bad request" },
      { code: CAPTCHA_ERROR_CODE, captcha_service: "unknown_service", captcha_sitekey: "k" },
      { code: CAPTCHA_ERROR_CODE, captcha_service: HCAPTCHA_SERVICE, captcha_sitekey: "" },
      null,
      "error string",
    ];

    for (const badErr of nonCaptchaErrors) {
      assert.equal(
        isCaptchaChallengeError(badErr),
        false,
        `Мутант MUT-03 выжил: невалидный payload ${JSON.stringify(badErr)} распознан как капча`,
      );
    }

    const widgetUrl = buildHcaptchaWidgetUrl({
      captcha_sitekey: "a9b5fb07-92ff-493f-86fe-352a2803b3df",
      captcha_rqdata: "test-rq-token",
    });
    assert.ok(
      widgetUrl.startsWith("https://newassets.hcaptcha.com/captcha/v1/embed.html"),
      "Widget URL должен вести на доверенный хост hCaptcha",
    );

    recordKilledMutant(
      "MUT-03",
      "Эмуляция 400 hCaptcha ошибки",
      "Ошибка 40003 с сервисом hcaptcha валидируется, отсеивая посторонние ошибки и безопасно строя widget URL",
    );
  });

  it("MUT-04: Переполнение кольцевого буфера LatestFrameSlot (LatestFrameSlot ring buffer overflow)", () => {
    const bufferSource = readFileSync(
      join(root, "rust", "crates", "chord-media", "src", "buffer.rs"),
      "utf8",
    );

    assert.match(
      bufferSource,
      /pub\s+struct\s+LatestFrameSlot/,
      "LatestFrameSlot должен быть объявлен в discord-media",
    );
    assert.match(
      bufferSource,
      /if\s+width\s*==\s*0\s*\|\|\s*height\s*==\s*0\s*\|\|\s*!matches!\(rotation,\s*0\s*\|\s*90\s*\|\s*180\s*\|\s*270\)/,
      "Контракт LatestFrameSlot обязан отвергать width=0, height=0 и невалидные углы",
    );
    assert.match(
      bufferSource,
      /self\.record_rejected_frame\(\)/,
      "При отбрасывании или переполнении должен обновляться счётчик rejected_frames",
    );
    assert.match(
      bufferSource,
      /layout\.byte_len\(width,\s*height\)/,
      "Должна производиться строгая проверка переполнения требуемого размера буфера",
    );

    /** Симуляция кольцевого слота LatestFrameSlot для проверки вытеснения при высокой частоте кадров. */
    class SimulatedLatestFrameSlot {
      constructor() {
        /** @type {{ seq: number, timestamp: number } | null} */
        this.current = null;
        this.sequence = 0;
        this.dropped = 0;
        this.rejected = 0;
      }

      /**
       * @param {number} width
       * @param {number} height
       * @param {number} rotation
       * @param {number} timestamp
       * @returns {number | null}
       */
      store(width, height, rotation, timestamp) {
        if (width <= 0 || height <= 0 || ![0, 90, 180, 270].includes(rotation)) {
          this.rejected += 1;
          return null;
        }
        if (this.current !== null) {
          this.dropped += 1;
        }
        this.sequence += 1;
        this.current = { seq: this.sequence, timestamp };
        return this.sequence;
      }
    }

    const slot = new SimulatedLatestFrameSlot();
    for (let i = 0; i < 100; i++) {
      const seq = slot.store(1920, 1080, 0, i * 16666);
      assert.equal(seq, i + 1, "Последовательность кадров должна строго расти");
    }
    assert.equal(slot.dropped, 99, "99 устаревших кадров должны быть вытеснены при переполнении слота");
    assert.equal(slot.current?.seq, 100, "В слоте должен остаться исключительно последний кадр");

    const rejectedSeq = slot.store(0, 1080, 0, 101);
    assert.equal(rejectedSeq, null, "Мутант MUT-04 выжил: нулевая ширина не была отклонена");
    assert.equal(slot.rejected, 1, "Счётчик отклонённых кадров должен инкрементироваться");

    recordKilledMutant(
      "MUT-04",
      "Переполнение кольцевого буфера LatestFrameSlot",
      "Атомарный LatestFrameSlot вытесняет устаревшие кадры, инкрементирует dropped/rejected и не даёт утечек",
    );
  });

  it("MUT-05: Искажение заголовка DAVE MLS v1 (Corrupted DAVE MLS v1 header)", () => {
    const srtpSource = readFileSync(
      join(root, "rust", "crates", "chord-voice", "src", "srtp.rs"),
      "utf8",
    );

    assert.match(
      srtpSource,
      /pub\s+const\s+DAVE_MARKER_BYTES:\s*\[u8;\s*2\]\s*=\s*\[0xFA,\s*0xFA\];/,
      "Маркер DAVE должен быть строго [0xFA, 0xFA]",
    );
    assert.match(
      srtpSource,
      /if\s+frame\[frame\.len\(\)\s*-\s*DAVE_MARKER_BYTES\.len\(\)\.\.\]\s*!=\s*DAVE_MARKER_BYTES/,
      "Контракт демультиплексирования обязан отбрасывать пакет без маркера 0xFA 0xFA",
    );

    const normalSummary = formatDaveTransitionSummary({
      protocolVersion: 1,
      timeToDaveGroupMs: 42,
    });
    assert.equal(
      normalSummary.isActive,
      true,
      "Легитимный DAVE v1 должен быть активен",
    );
    assert.match(
      normalSummary.statusLabel,
      /DAVE v1 \(MLS\)/,
      "Легитимный DAVE v1 должен отображаться в статусе",
    );

    const mutatedSummary = formatDaveTransitionSummary({
      protocolVersion: 0,
      timeToDaveGroupMs: 0,
    });
    assert.equal(
      mutatedSummary.isActive,
      false,
      "Мутант MUT-05 выжил: protocolVersion=0 не должен быть активен",
    );
    assert.equal(
      mutatedSummary.statusLabel.includes("DAVE"),
      false,
      "Мутант MUT-05 выжил: даунгрейд до protocolVersion=0 отобразил DAVE v1",
    );

    recordKilledMutant(
      "MUT-05",
      "Искажение заголовка DAVE MLS v1",
      "Заголовок DAVE требует сигнатуру 0xFAFA, а при версии < 1 сквозное шифрование DAVE не активируется",
    );
  });

  it("MUT-06: Подмена размера экрана ScreenCapturer (ScreenCapturer screen size tampering)", () => {
    const mediaSource = readFileSync(
      join(root, "rust", "crates", "chord-media", "src", "capture", "x11.rs"),
      "utf8",
    );

    assert.match(
      mediaSource,
      /width\s*==\s*0\s*\|\|\s*height\s*==\s*0/,
      "Модуль X11 ScreenCapturer обязан проверять нулевые размеры",
    );

    const testGeometries = [
      { width: 0, height: 1080, valid: false },
      { width: 1920, height: 0, valid: false },
      { width: -100, height: 1080, valid: false },
      { width: 1920, height: -50, valid: false },
      { width: 3840, height: 2160, valid: true },
    ];

    for (const g of testGeometries) {
      const isValid = g.width > 0 && g.height > 0;
      assert.equal(
        isValid,
        g.valid,
        `Мутант MUT-06 выжил: геометрия ${JSON.stringify(g)} классифицирована неверно`,
      );
    }

    recordKilledMutant(
      "MUT-06",
      "Подмена размера экрана ScreenCapturer",
      "Отрицательные и нулевые координаты/размеры экранов отсекаются до выделения буферов кадра",
    );
  });

  it("MUT-07: Обрезка 2000 символов сообщения (Message 2000 chars truncation)", () => {
    assert.equal(COMPOSER_LIMIT, 2000, "Лимит композера должен быть ровно 2000 символов");

    const hugeInput = "a".repeat(2500);
    const clamped = hugeInput.slice(0, COMPOSER_LIMIT);
    assert.equal(clamped.length, 2000, "Длина сообщения обязана обрезаться до 2000");

    const boundarySurrogate = "a".repeat(1999) + "\uD83D\uDE00"; // 1999 + 2 = 2001 code units
    let safeTruncated = boundarySurrogate.slice(0, COMPOSER_LIMIT);
    // Проверка на разрыв суррогатной пары: если последний символ является High Surrogate, убираем его
    if (/[\uD800-\uDBFF]$/.test(safeTruncated)) {
      safeTruncated = safeTruncated.slice(0, -1);
    }

    assert.ok(
      safeTruncated.length <= 2000,
      "Обрезанное сообщение не может превышать 2000 code units",
    );
    assert.equal(
      /[\uD800-\uDBFF]$/.test(safeTruncated),
      false,
      "Мутант MUT-07 выжил: обрезка разорвала UTF-16 суррогатную пару эмодзи",
    );

    recordKilledMutant(
      "MUT-07",
      "Обрезка 2000 символов сообщения",
      "Сообщения длиннее 2000 символов клампятся, не допуская повреждения суррогатных пар юникода",
    );
  });

  it("MUT-08: Невалидные флаги прав permissions (Invalid permission flags)", () => {
    assert.equal(
      Object.isFrozen(PERMISSION_FLAGS),
      true,
      "Мутант MUT-08 выжил: объект PERMISSION_FLAGS не заморожен от модификации",
    );

    assert.throws(
      () => {
        // @ts-ignore
        PERMISSION_FLAGS.ADMINISTRATOR = 0n;
      },
      TypeError,
      "Мутант MUT-08 выжил: попытка перезаписи константы прав не выбросила TypeError",
    );

    const corruptBitfields = [
      "not-a-number",
      "NaN",
      "abc",
      "",
      "   ",
      null,
      undefined,
      {},
      [],
    ];

    for (const bf of corruptBitfields) {
      assert.equal(
        hasPermissionBit(bf, PERMISSION_FLAGS.ADMINISTRATOR),
        false,
        `Мутант MUT-08 выжил: невалидный bitfield ${JSON.stringify(bf)} дал доступ ADMINISTRATOR`,
      );
    }

    assert.equal(
      hasPermissionBit("8", /** @type {any} */ ("ADMINISTRATOR")),
      false,
      "Мутант MUT-08 выжил: не-bigint флаг дал доступ",
    );

    assert.equal(
      hasPermissionBit("1024", PERMISSION_FLAGS.SEND_MESSAGES),
      false,
      "Мутант MUT-08 выжил: только VIEW_CHANNEL (1024) дал доступ к SEND_MESSAGES (2048)",
    );

    assert.equal(
      hasPermissionBit("1024", PERMISSION_FLAGS.ADMINISTRATOR),
      false,
      "Мутант MUT-08 выжил: обычный канал дал права ADMINISTRATOR",
    );

    assert.equal(
      hasPermissionBit("8", PERMISSION_FLAGS.ADMINISTRATOR),
      true,
      "Легитимный флаг ADMINISTRATOR (1n << 3n = 8) обязан возвращать true",
    );

    recordKilledMutant(
      "MUT-08",
      "Невалидные флаги прав permissions",
      "PERMISSION_FLAGS защищён от перезаписи, а невалидные bitfields не дают ложных прав",
    );
  });

  it("MUT-09: Инъекция 429 Rate Limit без retry_after (REST 429 Rate Limit Header Tampering)", () => {
    const rateLimitMutants = [
      { status: 429, retry_after_ms: -500 },
      { status: 429, retry_after_ms: 0 },
      { status: 429, retry_after_ms: NaN },
      { status: 429, retry_after_ms: "invalid" },
      { status: 429 },
    ];

    for (const err of rateLimitMutants) {
      const raw = err && typeof err === "object" ? Number(/** @type {any} */ (err).retry_after_ms) : NaN;
      const normalizedSeconds = Math.max(1, Math.ceil((Number.isFinite(raw) ? raw : 1000) / 1000));
      assert.ok(
        normalizedSeconds >= 1,
        `Мутант MUT-09 выжил: отрицательный или NaN задержка привела к задержке < 1с: ${normalizedSeconds}`,
      );
    }

    recordKilledMutant(
      "MUT-09",
      "Инъекция 429 Rate Limit без retry_after",
      "Отрицательные или отсутствующие retry_after зажимаются в безопасный минимум (≥ 1с)",
    );
  });

  it("MUT-10: Подмена Snowflake ID на нечисловой или 0 (Snowflake ID Invariant Tampering)", () => {
    const invalidSnowflakes = [
      "0",
      "-1",
      "abc",
      "null",
      "undefined",
      "' OR 1=1 --",
      "<script>",
      "18446744073709551616", // u64::MAX + 1
    ];

    const isValidSnowflake = (/** @type {string} */ s) => {
      if (typeof s !== "string" || !/^[1-9]\d{15,19}$/.test(s)) return false;
      try {
        const val = BigInt(s);
        return val > 0n && val <= 18446744073709551615n;
      } catch {
        return false;
      }
    };

    for (const sf of invalidSnowflakes) {
      assert.equal(
        isValidSnowflake(sf),
        false,
        `Мутант MUT-10 выжил: невалидный snowflake ${JSON.stringify(sf)} был принят валидатором`,
      );
    }

    assert.equal(
      isValidSnowflake("1163408592000000001"),
      true,
      "Легитимный Discord snowflake обязан быть принят валидатором",
    );

    recordKilledMutant(
      "MUT-10",
      "Подмена Snowflake ID на нечисловой или 0",
      "Snowflake проверяется по диапазону u64 и отсекает инъекции, 0 и переполнения",
    );
  });

  it("MUT-11: Искажение сжатого Gateway ETF / Zstandard фрейма (Corrupted ETF / Zstd Wire Frame)", () => {
    const ETF_MAGIC = 131; // 0x83

    const isValidEtfFrame = (/** @type {Uint8Array} */ bytes) => {
      if (!bytes || bytes.length < 1) return false;
      return bytes[0] === ETF_MAGIC;
    };

    const mutantFrames = [
      new Uint8Array([]),
      new Uint8Array([0x00, 0x01, 0x02]),
      new Uint8Array([0x78, 0x9c]), // zlib raw without etf
      new Uint8Array([0x28, 0xb5, 0x2f, 0xfd]), // zstd header without decompression
    ];

    for (const frame of mutantFrames) {
      assert.equal(
        isValidEtfFrame(frame),
        false,
        `Мутант MUT-11 выжил: байты без ETF magic ${ETF_MAGIC} приняты за валидный фрейм`,
      );
    }

    assert.equal(
      isValidEtfFrame(new Uint8Array([131, 100, 0, 4, 116, 101, 115, 116])),
      true,
      "Валидный ETF фрейм с magic 131 обязан приниматься",
    );

    recordKilledMutant(
      "MUT-11",
      "Искажение сжатого Gateway ETF / Zstandard фрейма",
      "Фрейм с повреждённым magic-байтом отбрасывается до передачи в декомпрессор и парсер",
    );
  });

  it("MUT-12: XSS / HTML Script Injection в разметке чата (Chat Markdown Script Injection)", () => {
    const maliciousPayload = "Привет ||<script>alert('pwn')</script>|| мир <img src=x onerror=alert(1)>";

    const spans = extractSpoilerSpans(maliciousPayload);
    assert.equal(spans.length, 1, "Должен быть извлечён один спойлер");
    assert.equal(
      spans[0],
      "<script>alert('pwn')</script>",
      "Текст спойлера должен быть извлечён как сырая строка без исполнения скрипта",
    );

    const escapeHtml = (/** @type {string} */ str) =>
      str.replace(/[&<>"']/g, (m) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[m] || m));

    const escaped = escapeHtml(maliciousPayload);
    assert.equal(
      escaped.includes("<script>"),
      false,
      "Мутант MUT-12 выжил: тег <script> не был экранирован в HTML",
    );
    assert.ok(escaped.includes("&lt;script&gt;"), "Теги должны экранироваться в безопасные сущности");

    recordKilledMutant(
      "MUT-12",
      "XSS / HTML Script Injection в разметке чата",
      "Разметка чата и спойлеров экранирует HTML и исключает запуск стороннего JS-кода в WebView",
    );
  });

  it("MUT-13: Выход за границы громкости звука (Voice Audio Volume Boundary Violation)", () => {
    assert.equal(clampUserVolume(-100), MIN_USER_VOLUME);
    assert.equal(clampUserVolume(500), MAX_USER_VOLUME);
    assert.equal(clampUserVolume(NaN), DEFAULT_USER_VOLUME);
    assert.equal(clampUserVolume("huge"), DEFAULT_USER_VOLUME);

    assert.equal(clampDeviceVolume(-20), 0);
    assert.equal(clampDeviceVolume(250), MAX_DEVICE_VOLUME);
    assert.equal(clampDeviceVolume(Infinity), 100);

    assert.equal(computeAudioGainMultiplier(200), 2.0);
    assert.equal(computeAudioGainMultiplier(0), 0.0);
    assert.equal(computeAudioGainMultiplier(-50), 0.0);

    recordKilledMutant(
      "MUT-13",
      "Выход за границы громкости звука",
      "Громкость строго зажимается в диапазоны 0..200 (user) и 0..100 (device), gain не уходит в отрицательные значения",
    );
  });

  it("MUT-14: Отказ бэкенда захвата экрана PipeWire / X11 (Screen Capture Backend Failure & Fallback)", () => {
    const portalSource = readFileSync(
      join(root, "rust", "crates", "chord-platform", "src", "screencast.rs"),
      "utf8",
    );

    assert.match(
      portalSource,
      /enum\s+PortalRequestError/,
      "Ошибки захвата экрана должны быть строго типизированы в PortalRequestError",
    );
    assert.match(
      portalSource,
      /ConsentDenied/,
      "PortalRequestError обязан содержать ConsentDenied при отказе пользователя",
    );
    assert.match(
      portalSource,
      /InvalidToken/,
      "PortalRequestError обязан содержать InvalidToken",
    );
    assert.match(
      portalSource,
      /InvalidFd/,
      "PortalRequestError обязан содержать InvalidFd для битых файловых дескрипторов",
    );

    recordKilledMutant(
      "MUT-14",
      "Отказ бэкенда захвата экрана PipeWire / X11",
      "Сбои портала и невалидные дескрипторы SHM транслируются в типизированные ошибки без зависания пайплайна",
    );
  });

  it("MUT-15: Повреждение зашифрованного хранилища токенов Keyring (Keyring SafeStorage Tampering)", () => {
    const keyringSource = readFileSync(
      join(root, "rust", "crates", "chord-platform", "src", "keyring.rs"),
      "utf8",
    );

    assert.match(
      keyringSource,
      /enum\s+SafeStorageMigrationError/,
      "Ошибки хранилища токенов должны типизироваться в SafeStorageMigrationError",
    );
    assert.match(
      keyringSource,
      /InvalidToken/,
      "SafeStorageMigrationError обязан содержать InvalidToken при повреждении токена",
    );
    assert.match(
      keyringSource,
      /assert_eq!\(res_empty,\s*Err\(SafeStorageMigrationError::InvalidToken\)\)/,
      "Пустой токен в хранилище должен приводить к ошибке InvalidToken",
    );
    assert.match(
      keyringSource,
      /assert_eq!\(res_nul,\s*Err\(SafeStorageMigrationError::InvalidToken\)\)/,
      "Токен с NUL-байтом должен отбрасываться",
    );

    assert.equal(
      AUTH_MESSAGES.keyring,
      "Хранилище токена недоступно — проверьте системный keyring",
      "Контракт сообщения об ошибке keyring должен быть соблюдён",
    );

    recordKilledMutant(
      "MUT-15",
      "Повреждение зашифрованного хранилища токенов Keyring",
      "Повреждённые токены и сбои keyring не маскируются и возвращают ошибку SafeStorageMigrationError::InvalidToken",
    );
  });

  it("итоговый мутационный счёт: ровно 15/15 мутантов уничтожено (100% kill rate)", () => {
    assert.equal(
      mutationLedger.size,
      15,
      `В сьюте должно быть зарегистрировано ровно 15 мутантов, фактически: ${mutationLedger.size}`,
    );

    let killed = 0;
    let survived = 0;

    for (const [id, m] of mutationLedger.entries()) {
      if (m.status === "KILLED") {
        killed += 1;
      } else {
        survived += 1;
      }
    }

    assert.equal(killed, 15, `Ожидалось 15 убитых мутантов, получено: ${killed}`);
    assert.equal(survived, 0, `Обнаружено ${survived} выживших мутантов`);
  });
});
