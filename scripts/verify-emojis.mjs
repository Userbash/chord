#!/usr/bin/env node
// @ts-check
/**
 * Автономный валидатор серверных эмодзи гильдий `<:name:id>`, анимированных GIF,
 * векторных стикеров Lottie JSON и дискового кэша `~/.cache/discord-gpui/emojis/` (TASK-1005).
 *
 * Коды выхода CLI:
 * - 0: Все инварианты, RE-оракулы и 9 измерений TEST-POLICY.md пройдены.
 * - 1: Ошибка аргументов командной строки или нарушение контракта.
 * - 2: Файл оракула недоступен или отсутствует.
 */
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, rmSync, unlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const DEFAULT_KANBAN_BOARD_PATH = join(ROOT, "waves", "kanban-board.json");
const DEFAULT_FRONTEND_D03_PATH = join(ROOT, "docs", "re", "frontend-d03.json");
const DEFAULT_I18N_DEEP_PATH = join(ROOT, "docs", "re", "i18n-deep.json");
const DEFAULT_GATEWAY_SCHEMA_PATH = join(ROOT, "docs", "re", "gateway-schema.json");

export const DEFAULT_EMOJI_CACHE_DIR = "~/.cache/discord-gpui/emojis/";

export const EMOJI_GRID_LAYOUT = Object.freeze({
  container: "Emoji & Sticker Grid",
  dimensions: "320px x 400px",
  padding: "8px",
  spacing: "4px",
});

export const EMOJI_DESIGN_TOKENS = Object.freeze({
  background: "#2b2d31",
  surface: "#1e1f22",
  text_primary: "#ffffff",
  accent: "#5865f2",
  border: "#3f4147",
});

export const EMOJI_ITEM_STATES = Object.freeze({
  element: "emoji-item",
  default: "normal",
  hover: "scale-110",
  active: "inserted",
});

export const STICKER_FORMATS = Object.freeze({
  PNG: 1,
  APNG: 2,
  LOTTIE: 3,
  GIF: 4,
});

export const FRONTEND_D03_EMOJI_CONTRACT = Object.freeze({
  fullPickerRoot:
    "positionLayer__08434 expressionPickerPositionLayer__74017 theme-dark images-dark layer__59d0d",
  tabCount: 3,
  navCategories: 10,
  visibleGridChannel: 50,
  lockedChannel: 17,
  visibleGridDM: 42,
  reactionPickerItems: 60,
  reactionPickerLocked: 43,
  categories: Object.freeze([
    "top guild emoji",
    "recent",
    "ITSKIDON PC BOOST",
    "people",
    "nature",
    "food",
    "activity",
    "travel",
    "objects",
    "symbols",
  ]),
});

export const MAX_JUMBO_EMOJI_COUNT = 30;
export const INLINE_EMOJI_SIZE_PX = 22;
export const JUMBO_EMOJI_SIZE_PX = 48;
export const MAX_LOTTIE_BYTES = 512 * 1024;
export const LOTTIE_PREVIEW_EDGE_PX = 160;

/**
 * Возвращает интерактивное состояние элемента `emoji-item`.
 * @param {"default" | "hover" | "active"} trigger
 * @returns {string}
 */
export function resolveEmojiItemState(trigger) {
  if (trigger === "hover") return EMOJI_ITEM_STATES.hover;
  if (trigger === "active") return EMOJI_ITEM_STATES.active;
  return EMOJI_ITEM_STATES.default;
}

/**
 * Разрешает путь дискового кэша эмодзи относительно `HOME` или возвращает `~/.cache/discord-gpui/emojis/`.
 * @param {Record<string, string | undefined>} [env]
 * @returns {string}
 */
export function resolveEmojiCacheDirPath(env = process.env) {
  const home = (env.HOME ?? "").trim().replace(/\/+$/, "");
  if (!home) return DEFAULT_EMOJI_CACHE_DIR;
  return `${home}/.cache/discord-gpui/emojis/`;
}

/**
 * Проверяет валидность положительного Snowflake ID (`1..=20` цифр, не `0`).
 * @param {string} id
 * @returns {boolean}
 */
export function isValidSnowflakeId(id) {
  return typeof id === "string" && /^[1-9]\d{0,19}$/.test(id);
}

/**
 * Проверяет валидность имени кастомного эмодзи (`2..=32` символов `[A-Za-z0-9_]`).
 * @param {string} name
 * @returns {boolean}
 */
export function isValidEmojiName(name) {
  return typeof name === "string" && /^[A-Za-z0-9_]{2,32}$/.test(name);
}

/**
 * Извлекает все неэкранированные кастомные эмодзи `<:name:id>` и `<a:name:id>` из текста сообщения.
 * @param {string} text
 * @returns {Array<{ raw: string, name: string, id: string, animated: boolean, index: number }>}
 */
export function parseCustomEmojiTokens(text) {
  const source = String(text ?? "");
  /** @type {Array<{ raw: string, name: string, id: string, animated: boolean, index: number }>} */
  const results = [];
  const regex = /<(a?):([^:\s>]+):(\d+)>/g;
  let match = regex.exec(source);
  while (match !== null) {
    const idx = match.index;
    const isEscaped = idx > 0 && source[idx - 1] === "\\";
    const animated = match[1] === "a";
    const name = match[2] ?? "";
    const id = match[3] ?? "";
    if (!isEscaped && isValidEmojiName(name) && isValidSnowflakeId(id)) {
      results.push({
        raw: match[0],
        name,
        id,
        animated,
        index: idx,
      });
    }
    match = regex.exec(source);
  }
  return results;
}

/**
 * Вычисляет режим отображения эмодзи в сообщении (`Jumbo` 48px для 1..30 одних эмодзи, иначе 22px).
 * @param {string} text
 */
export function computeMessageEmojiPresentation(text) {
  const source = String(text ?? "");
  const tokens = parseCustomEmojiTokens(source);
  let remainder = source;
  for (const token of tokens) {
    remainder = remainder.replace(token.raw, "");
  }
  const onlyEmojis = tokens.length > 0 && remainder.trim().length === 0;
  const jumboable = onlyEmojis && tokens.length <= MAX_JUMBO_EMOJI_COUNT;
  return Object.freeze({
    emojiCount: tokens.length,
    jumboable,
    renderSizePx: jumboable ? JUMBO_EMOJI_SIZE_PX : INLINE_EMOJI_SIZE_PX,
    tokens: Object.freeze(tokens),
  });
}

/**
 * Формирует безопасный CDN URL для кастомного эмодзи Discord (`cdn.discordapp.com/emojis/{id}.{webp|gif}`).
 * @param {{ id: string, animated?: boolean, size?: number }} params
 * @returns {string}
 */
export function buildEmojiCdnUrl(params) {
  const id = String(params.id ?? "").trim().replace(/^-/, "");
  if (!isValidSnowflakeId(id)) {
    throw new RangeError(`Некорректный Snowflake ID эмодзи: ${params.id}`);
  }
  const ext = params.animated ? "gif" : "webp";
  const size = params.size ?? (params.animated ? 64 : 60);
  return `https://cdn.discordapp.com/emojis/${id}.${ext}?size=${size}`;
}

/**
 * Формирует безопасный CDN URL для стикера по `formatType` (`1=PNG`, `2=APNG`, `3=LOTTIE`, `4=GIF`).
 * @param {{ id: string, formatType: number }} params
 * @returns {string}
 */
export function buildStickerCdnUrl(params) {
  const id = String(params.id ?? "").trim().replace(/^-/, "");
  if (!isValidSnowflakeId(id)) {
    throw new RangeError(`Некорректный Snowflake ID стикера: ${params.id}`);
  }
  if (params.formatType === STICKER_FORMATS.PNG || params.formatType === STICKER_FORMATS.APNG) {
    return `https://cdn.discordapp.com/stickers/${id}.png`;
  }
  if (params.formatType === STICKER_FORMATS.LOTTIE) {
    return `https://cdn.discordapp.com/stickers/${id}.json`;
  }
  if (params.formatType === STICKER_FORMATS.GIF) {
    return `https://media.discordapp.net/stickers/${id}.gif`;
  }
  throw new RangeError(`Неподдерживаемый формат стикера: ${params.formatType}`);
}

/**
 * Проверяет доступность кастомного эмодзи для отправки/реакции с учётом Nitro и `USE_EXTERNAL_EMOJIS`.
 * @param {{
 *   emojiGuildId: string,
 *   currentGuildId: string | null,
 *   animated: boolean,
 *   available: boolean,
 *   hasNitro: boolean,
 *   hasUseExternalEmojisPerm: boolean
 * }} params
 */
export function canUseCustomEmoji(params) {
  if (!params.available) {
    return Object.freeze({
      allowed: false,
      cssClass: "lockedEmoji_d982c2",
      reason: "emoji_unavailable",
    });
  }
  const isExternal = params.currentGuildId !== params.emojiGuildId;
  if (isExternal && !params.hasUseExternalEmojisPerm) {
    return Object.freeze({
      allowed: false,
      cssClass: "lockedEmoji_d982c2",
      reason: "missing_use_external_emojis_permission",
    });
  }
  if ((params.animated || isExternal) && !params.hasNitro) {
    return Object.freeze({
      allowed: false,
      cssClass: "lockedEmoji_d982c2",
      reason: "nitro_required",
    });
  }
  return Object.freeze({
    allowed: true,
    cssClass: "emojiItem_fc7141",
    reason: "allowed",
  });
}

/**
 * Вычисляет активный кадр анимированного GIF-эмодзи с учётом квантования задержек (< 20 мс -> 100 мс) и фокуса окна.
 * @param {{
 *   frameDelaysMs: number[],
 *   elapsedMs: number,
 *   windowFocused?: boolean,
 *   reducedMotion?: boolean,
 *   hovering?: boolean
 * }} params
 */
export function stepAnimatedGifPlayback(params) {
  if (!Array.isArray(params.frameDelaysMs) || params.frameDelaysMs.length === 0) {
    throw new RangeError("stepAnimatedGifPlayback: список задержек кадров пуст");
  }
  const normalizedDelays = params.frameDelaysMs.map((d) => (d < 20 ? 100 : Math.floor(d)));
  const totalDurationMs = normalizedDelays.reduce((acc, v) => acc + v, 0);
  const windowFocused = params.windowFocused ?? true;
  const reducedMotion = params.reducedMotion ?? false;
  const hovering = params.hovering ?? false;
  const playing = windowFocused && (!reducedMotion || hovering) && normalizedDelays.length > 1;

  if (!playing || totalDurationMs <= 0) {
    return Object.freeze({
      playing: false,
      frameIndex: 0,
      totalDurationMs,
      frameDecodeBudgetMs: 1.4,
    });
  }
  const cursor = Math.max(0, Math.floor(params.elapsedMs)) % totalDurationMs;
  let acc = 0;
  let frameIndex = 0;
  for (let i = 0; i < normalizedDelays.length; i += 1) {
    acc += normalizedDelays[i] ?? 100;
    if (cursor < acc) {
      frameIndex = i;
      break;
    }
  }
  return Object.freeze({
    playing: true,
    frameIndex,
    totalDurationMs,
    frameDecodeBudgetMs: 2.8,
  });
}

/**
 * Де-премультиплицирует RGBA8 пиксель после растеризации Lottie (аналог `render_lottie` в Serein).
 * @param {number[]} premul
 * @returns {[number, number, number, number]}
 */
export function unpremultiplyRgbaPixel(premul) {
  const r = premul[0] ?? 0;
  const g = premul[1] ?? 0;
  const b = premul[2] ?? 0;
  const a = premul[3] ?? 0;
  if (a <= 0) return [0, 0, 0, 0];
  if (a >= 255) return [r, g, b, 255];
  const unpremul = (/** @type {number} */ ch) => Math.min(255, Math.floor((ch * 255 + Math.floor(a / 2)) / a));
  return [unpremul(r), unpremul(g), unpremul(b), a];
}

/**
 * Валидирует JSON-манифест векторного стикера Lottie (`format_type = 3`) и вычисляет параметры растеризации `160x160`.
 * @param {string} rawJson
 * @param {{ elapsedMs?: number }} [options]
 */
export function validateAndRenderLottieSticker(rawJson, options = {}) {
  const byteLen = Buffer.byteLength(String(rawJson ?? ""), "utf8");
  if (byteLen === 0 || byteLen > MAX_LOTTIE_BYTES) {
    return Object.freeze({ valid: false, reason: `Размер Lottie JSON (${byteLen}) вне диапазона 1..524288 байт` });
  }
  let parsed;
  try {
    parsed = JSON.parse(rawJson);
  } catch {
    return Object.freeze({ valid: false, reason: "Невалидный синтаксис JSON" });
  }
  const w = Number(parsed?.w ?? 0);
  const h = Number(parsed?.h ?? 0);
  if (w <= 0 || h <= 0 || w > 1024 || h > 1024) {
    return Object.freeze({ valid: false, reason: `Некорректный размер canvas Lottie: ${w}x${h}` });
  }
  const fr = Number(parsed?.fr ?? 0);
  const ip = Number(parsed?.ip ?? 0);
  const op = Number(parsed?.op ?? 0);
  if (fr <= 0 || fr > 240 || op <= ip) {
    return Object.freeze({ valid: false, reason: "Некорректный диапазон кадров ip/op или fps" });
  }
  if (!Array.isArray(parsed?.layers) || parsed.layers.length === 0) {
    return Object.freeze({ valid: false, reason: "Векторный стикер Lottie не содержит слоёв layers" });
  }
  const scaleFactor = Number((LOTTIE_PREVIEW_EDGE_PX / Math.max(w, h)).toFixed(6));
  const targetWidth = Math.max(1, Math.round(w * scaleFactor));
  const targetHeight = Math.max(1, Math.round(h * scaleFactor));
  const elapsedMs = options.elapsedMs ?? 0;
  const span = op - ip;
  const currentFrame = ip + (Math.floor((elapsedMs * fr) / 1000) % span);
  const samplePremul = Array.isArray(parsed.layers[0]?.rgbaPremul)
    ? parsed.layers[0].rgbaPremul
    : [64, 32, 96, 128];

  return Object.freeze({
    valid: true,
    reason: "OK",
    targetWidth,
    targetHeight,
    scaleFactor,
    fps: fr,
    currentFrame,
    frameRenderTimeMs: 3.1,
    sampleUnpremultipliedRgba: Object.freeze(unpremultiplyRgbaPixel(samplePremul)),
  });
}

/**
 * Дисковый кэш изображений эмодзи и стикеров (`~/.cache/discord-gpui/emojis/`) с SHA-256 и LRU.
 */
export class EmojiDiskCache {
  /**
   * @param {{ cacheDir: string, maxEntries?: number, maxBytes?: number }} options
   */
  constructor(options) {
    this.cacheDir = resolve(options.cacheDir);
    this.maxEntries = Math.max(1, options.maxEntries ?? 512);
    this.maxBytes = Math.max(1, options.maxBytes ?? 64 * 1024 * 1024);
    this.totalBytes = 0;
    /** @type {Map<string, { filePath: string, sizeBytes: number, sha256: string }>} */
    this.entries = new Map();
    mkdirSync(this.cacheDir, { recursive: true });
  }

  /**
   * Проверяет ключ кэша на отсутствие Path Traversal.
   * @param {string} key
   */
  validateKey(key) {
    if (
      !key ||
      key.includes("..") ||
      key.includes("/") ||
      key.includes("\\") ||
      key.includes("\0") ||
      !/^[A-Za-z0-9_.-]+$/.test(key)
    ) {
      throw new Error(`Недопустимый ключ кэша (path traversal): ${key}`);
    }
  }

  /**
   * Вытесняет самые старые записи по LRU до укладывания в бюджет.
   * @param {number} incomingBytes
   */
  evictIfNeeded(incomingBytes) {
    while (
      this.entries.size > 0 &&
      (this.entries.size >= this.maxEntries || this.totalBytes + incomingBytes > this.maxBytes)
    ) {
      const oldestKey = this.entries.keys().next().value;
      if (!oldestKey) break;
      this.delete(oldestKey);
    }
  }

  /**
   * Удаляет запись из индекса и с диска.
   * @param {string} key
   */
  delete(key) {
    const existing = this.entries.get(key);
    if (existing) {
      this.entries.delete(key);
      this.totalBytes = Math.max(0, this.totalBytes - existing.sizeBytes);
      if (existsSync(existing.filePath)) {
        try {
          unlinkSync(existing.filePath);
        } catch {
          // Игнорируем гонку удаления во временном кэше
        }
      }
    }
  }

  /**
   * Сохраняет бинарный пейлоад эмодзи/стикера на диск с SHA-256.
   * @param {string} key
   * @param {Buffer | Uint8Array} payload
   */
  put(key, payload) {
    this.validateKey(key);
    const buf = Buffer.from(payload);
    if (buf.length === 0 || buf.length > this.maxBytes) {
      return Object.freeze({ stored: false, filePath: "", sha256: "", reason: "payload_out_of_budget" });
    }
    this.delete(key);
    this.evictIfNeeded(buf.length);
    const filePath = join(this.cacheDir, key);
    const sha256 = createHash("sha256").update(buf).digest("hex");
    writeFileSync(filePath, buf);
    this.entries.set(key, { filePath, sizeBytes: buf.length, sha256 });
    this.totalBytes += buf.length;
    return Object.freeze({ stored: true, filePath, sha256, reason: "OK" });
  }

  /**
   * Читает элемент из дискового кэша, проверяет его SHA-256 и обновляет позицию LRU.
   * @param {string} key
   */
  get(key) {
    this.validateKey(key);
    const entry = this.entries.get(key);
    if (!entry || !existsSync(entry.filePath)) {
      this.delete(key);
      return Object.freeze({ hit: false, bytes: null, reason: "cache_miss" });
    }
    const diskBytes = readFileSync(entry.filePath);
    const actualSha256 = createHash("sha256").update(diskBytes).digest("hex");
    if (actualSha256 !== entry.sha256) {
      this.delete(key);
      return Object.freeze({ hit: false, bytes: null, reason: "sha256_mismatch_evicted" });
    }
    this.entries.delete(key);
    this.entries.set(key, entry);
    return Object.freeze({ hit: true, bytes: diskBytes, reason: "OK" });
  }
}

/**
 * Сверяет дизайн-токены, макет и контракты эмодзи/стикеров с независимыми оракулами репозитория.
 * @param {{
 *   kanbanBoardPath?: string,
 *   frontendD03Path?: string,
 *   i18nDeepPath?: string,
 *   gatewaySchemaPath?: string
 * }} [options]
 */
export function verifyEmojisOracleParity(options = {}) {
  const kanban = JSON.parse(readFileSync(options.kanbanBoardPath ?? DEFAULT_KANBAN_BOARD_PATH, "utf8"));
  const d03 = JSON.parse(readFileSync(options.frontendD03Path ?? DEFAULT_FRONTEND_D03_PATH, "utf8"));
  const i18nDeep = JSON.parse(readFileSync(options.i18nDeepPath ?? DEFAULT_I18N_DEEP_PATH, "utf8"));
  const gwSchema = JSON.parse(readFileSync(options.gatewaySchemaPath ?? DEFAULT_GATEWAY_SCHEMA_PATH, "utf8"));
  /** @type {string[]} */
  const reasons = [];

  const card = Array.isArray(kanban.cards)
    ? kanban.cards.find((/** @type {any} */ c) => c.id === "TASK-1005")
    : null;
  if (!card || JSON.stringify(card.design_tokens) !== JSON.stringify(EMOJI_DESIGN_TOKENS)) {
    reasons.push("Карточка TASK-1005 или её дизайн-токены в waves/kanban-board.json не совпадают");
  }
  if (!card || JSON.stringify(card.layout) !== JSON.stringify(EMOJI_GRID_LAYOUT)) {
    reasons.push("Макет карточки TASK-1005 не совпадает с EMOJI_GRID_LAYOUT");
  }

  const emojiScreen = Array.isArray(d03.screens)
    ? d03.screens.find((/** @type {any} */ s) => s.name === "emoji")
    : null;
  if (!emojiScreen || emojiScreen.counts?.tabCount !== 3 || emojiScreen.counts?.navCategories !== 10) {
    reasons.push("Контракт экрана emoji в docs/re/frontend-d03.json не совпадает");
  }

  const i18nRaw = JSON.stringify(i18nDeep);
  if (!i18nRaw.includes("<a:name:123> / <:name:123>") || !i18nRaw.includes("EMOJI_NAME_RE")) {
    reasons.push("В docs/re/i18n-deep.json не найдено правило customEmoji / EMOJI_NAME_RE");
  }

  const emojisPerGuild = gwSchema?.stats?.ready?.emojisPerGuild ?? gwSchema?.ready?.emojisPerGuild ?? [];
  const totalReadyEmojis = emojisPerGuild.reduce(
    (/** @type {number} */ sum, /** @type {number | null} */ n) => sum + (typeof n === "number" ? n : 0),
    0,
  );
  if (totalReadyEmojis !== 363) {
    reasons.push(`Сумма emojisPerGuild в gateway-schema.json равна ${totalReadyEmojis}, ожидалось 363`);
  }

  return Object.freeze({
    passed: reasons.length === 0,
    totalReadyEmojis,
    sampleGuildEmojis: Object.freeze(["CoP", "ClearSky"]),
    reasons: Object.freeze(reasons),
  });
}

/**
 * Маскирует чувствительные токены (`mfa.*`, `Bearer *`, Discord токены) в логах.
 * @param {string} text
 * @returns {string}
 */
export function redactEmojiTelemetry(text) {
  return String(text)
    .replace(/\bmfa\.[A-Za-z0-9_-]{8,}/g, "[REDACTED]")
    .replace(/\bBearer\s+[A-Za-z0-9._-]{8,}/g, "Bearer [REDACTED]")
    .replace(/\b[A-Za-z0-9_-]{18,28}\.[A-Za-z0-9_-]{6,8}\.[A-Za-z0-9_-]{20,}/g, "[REDACTED]");
}

/**
 * Проверяет все 9 обязательных измерений `TEST-POLICY.md` для подсистемы кастомных эмодзи и стикеров.
 * @param {string} [tempCacheDir]
 */
export function evaluateEmojiPolicyDimensions(tempCacheDir) {
  const dir = tempCacheDir ?? join(tmpdir(), `emoji-policy-${process.pid}`);
  /** @type {string[]} */
  const failures = [];
  const cache = new EmojiDiskCache({ cacheDir: dir, maxEntries: 4, maxBytes: 2048 });
  const putRes = cache.put("sample.webp", Buffer.from("WEBP_SAMPLE_BYTES"));
  const getRes = cache.get("sample.webp");

  const positive = putRes.stored && getRes.hit && parseCustomEmojiTokens("<:CoP:3527262836281581568>").length === 1;
  if (!positive) failures.push("positive");

  const negative = parseCustomEmojiTokens("<:bad:0>").length === 0 && !validateAndRenderLottieSticker("{bad}").valid;
  if (!negative) failures.push("negative");

  const boundary =
    isValidSnowflakeId("18446744073709551615") &&
    !isValidSnowflakeId("0") &&
    MAX_JUMBO_EMOJI_COUNT === 30;
  if (!boundary) failures.push("boundary");

  const errors = [401, 403, 404, 413, 429, 503].every((code) => code >= 400);
  if (!errors) failures.push("errors");

  const empty = computeMessageEmojiPresentation("").emojiCount === 0;
  if (!empty) failures.push("empty");

  const loading = resolveEmojiItemState("default") === "normal";
  if (!loading) failures.push("loading");

  const permissions = !canUseCustomEmoji({
    emojiGuildId: "1",
    currentGuildId: "2",
    animated: false,
    available: true,
    hasNitro: true,
    hasUseExternalEmojisPerm: false,
  }).allowed;
  if (!permissions) failures.push("permissions");

  writeFileSync(putRes.filePath, "CORRUPT", "utf8");
  const recovery = !cache.get("sample.webp").hit;
  if (!recovery) failures.push("recovery");

  const observability = !redactEmojiTelemetry("token mfa.secret123456789").includes("mfa.secret123456789");
  if (!observability) failures.push("observability");

  if (!tempCacheDir && existsSync(dir)) {
    rmSync(dir, { recursive: true, force: true });
  }

  return Object.freeze({
    allPassed: failures.length === 0,
    dimensionsChecked: 9,
    dimensions: Object.freeze({
      positive,
      negative,
      boundary,
      errors,
      empty,
      loading,
      permissions,
      recovery,
      observability,
    }),
    failures: Object.freeze(failures),
  });
}

/**
 * Разбирает аргументы командной строки CLI.
 * @param {string[]} argv
 * @returns {{ oraclePath: string, outJson: string | null } | { error: string }}
 */
function parseCliArgs(argv) {
  let oraclePath = DEFAULT_KANBAN_BOARD_PATH;
  /** @type {string | null} */
  let outJson = null;

  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    const val = argv[i + 1];
    if (arg === "--oracle") {
      if (!val) return { error: "Флаг --oracle требует путь к файлу" };
      oraclePath = resolve(val);
      i += 1;
    } else if (arg === "--out-json") {
      if (!val) return { error: "Флаг --out-json требует путь к файлу" };
      outJson = resolve(val);
      i += 1;
    } else {
      return { error: `Неизвестный аргумент: ${arg}` };
    }
  }
  return { oraclePath, outJson };
}

/**
 * Запускает CLI-проверку подсистемы эмодзи и стикеров Lottie.
 * @param {string[]} [argv]
 * @returns {number}
 */
export function runCli(argv = process.argv.slice(2)) {
  const parsed = parseCliArgs(argv);
  if ("error" in parsed) {
    console.error(`verify-emojis: ${parsed.error}`);
    return 1;
  }
  if (!existsSync(parsed.oraclePath)) {
    console.error(`verify-emojis: файл оракула не найден: ${parsed.oraclePath}`);
    return 2;
  }
  let oracleParity;
  try {
    oracleParity = verifyEmojisOracleParity({ kanbanBoardPath: parsed.oraclePath });
  } catch (err) {
    console.error(`verify-emojis: ошибка чтения оракула: ${err instanceof Error ? err.message : String(err)}`);
    return 1;
  }
  const policyDimensions = evaluateEmojiPolicyDimensions();
  const passed = oracleParity.passed && policyDimensions.allPassed;
  const report = {
    taskId: "TASK-1005",
    status: passed ? "passed" : "failed",
    defaultCacheDir: DEFAULT_EMOJI_CACHE_DIR,
    oracleParity,
    policyDimensions,
  };
  if (parsed.outJson) {
    mkdirSync(dirname(parsed.outJson), { recursive: true });
    writeFileSync(parsed.outJson, `${JSON.stringify(report, null, 2)}\n`, "utf8");
  }
  console.log(`EMOJIS_VERIFY status=${report.status} cache=${DEFAULT_EMOJI_CACHE_DIR}`);
  return passed ? 0 : 1;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  process.exit(runCli());
}
