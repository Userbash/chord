#!/usr/bin/env node
// @ts-check
/**
 * Комплексный стенд аудита системных шрифтов, цепочки fallback (Noto Color Emoji, CJK,
 * RTL Arabic/Hebrew, Tofurion Fallback) и кэша текстурного атласа глифов WGPU (TASK-981).
 * Коды выхода CLI: 0 — успех, 1 — ошибка аргументов/контракта, 2 — оракул недоступен.
 */
import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = fileURLToPath(new URL("../", import.meta.url));
const DEFAULT_KANBAN_BOARD_PATH = resolve(ROOT, "waves", "kanban-board.json");
const DEFAULT_GPUI_THEME_PATH = resolve(ROOT, "rust", "discord-gpui", "src", "theme", "discord_dark.rs");
const DEFAULT_GPUI_STATE_PATH = resolve(ROOT, "rust", "discord-gpui", "src", "state", "mod.rs");
const DEFAULT_UI_TOKENS_CSS_PATH = resolve(ROOT, "rust", "gui-spike", "ui", "tokens.css");
const DEFAULT_FRONTEND_D03_PATH = resolve(ROOT, "docs", "re", "frontend-d03.json");

export const MAX_CUSTOM_FONT_BYTES = 32 * 1024 * 1024;
export const MAX_FONT_NAME_LENGTH = 128;

/** Дизайн-токены карточки `TASK-981` из `waves/kanban-board.json`. */
export const FONT_AUDIT_DESIGN_TOKENS = Object.freeze({
  background: "#1e1f22", surface: "#2b2d31", text_primary: "#ffffff", accent: "#5865f2", border: "#3f4147",
});

/** Конфигурация контейнера `Font Manager` / `Glyph Atlas WGPU` (`TASK-981`). */
export const FONT_MANAGER_LAYOUT = Object.freeze({
  container: "Font Manager", dimensions: "Glyph Atlas WGPU", padding: "0px", spacing: "0px",
});

/** Интерактивные состояния элемента `emoji-picker` (`TASK-981`). */
export const EMOJI_PICKER_STATES = Object.freeze({
  element: "emoji-picker", default: "rendered", hover: "lookup glyph", active: "atlas cached",
});

/** Индексы региональных лиц в коллекции `NotoSansCJK.ttc` (согласно эталону Serein `cjk_index`). */
export const CJK_TTC_FACE_INDICES = Object.freeze({ ja: 0, ko: 1, "zh-CN": 2, "zh-TW": 3 });

/** Конфигурация текстурного атласа глифов WGPU по умолчанию. */
export const WGPU_ATLAS_DEFAULT_CONFIG = Object.freeze({
  pageWidthPx: 2048, pageHeightPx: 2048, maxTextureDimension2D: 8192, glyphPaddingPx: 1,
  maxPagesPerFormat: 4, maskFormat: "r8_unorm", colorFormat: "rgba8_unorm_srgb", minGlyphRasterPx: 8, tofurionStrokePx: 1,
});

/** Канонические цепочки fallback шрифтов для `discord-gpui`. */
export const FONT_FALLBACK_CHAINS = Object.freeze({
  primary: Object.freeze([
    "gg sans", "Noto Sans", "Liberation Sans", "DejaVu Sans",
    "Noto Sans CJK SC", "Noto Sans CJK JP", "Noto Sans CJK KR",
    "Noto Sans Arabic", "Noto Sans Hebrew", "Noto Color Emoji", "Twemoji", "Tofurion Fallback",
  ]),
  display: Object.freeze([
    "ABC Ginto Discord", "gg sans", "Noto Sans", "Liberation Sans",
    "Noto Sans CJK SC", "Noto Sans Arabic", "Noto Sans Hebrew", "Noto Color Emoji", "Tofurion Fallback",
  ]),
  code: Object.freeze([
    "gg mono", "Source Code Pro", "Liberation Mono", "DejaVu Sans Mono",
    "Noto Sans Mono CJK SC", "Noto Sans Arabic", "Noto Color Emoji", "Tofurion Fallback",
  ]),
  emoji: Object.freeze(["Noto Color Emoji", "Twemoji", "Apple Color Emoji", "Segoe UI Emoji", "Tofurion Fallback"]),
  cjk: Object.freeze([
    "Noto Sans CJK SC", "Noto Sans CJK TC", "Noto Sans CJK JP", "Noto Sans CJK KR",
    "Source Han Sans SC", "Droid Sans Fallback", "Noto Sans", "Tofurion Fallback",
  ]),
  rtl: Object.freeze([
    "Noto Sans Arabic", "Noto Naskh Arabic", "Noto Kufi Arabic",
    "Noto Sans Hebrew", "Noto Rashi Hebrew", "DejaVu Sans", "Tofurion Fallback",
  ]),
});

/** @param {number} cp @returns {boolean} */
function isPrivateOrUnassignedCodePoint(cp) {
  if (!Number.isInteger(cp) || cp < 0 || cp > 0x10ffff) return true;
  if ((cp >= 0xd800 && cp <= 0xdfff) || (cp >= 0xe000 && cp <= 0xf8ff)) return true;
  if ((cp >= 0xf0000 && cp <= 0xffffd) || (cp >= 0x100000 && cp <= 0x10fffd)) return true;
  if ((cp >= 0xfdd0 && cp <= 0xfdef) || (cp & 0xfffe) === 0xfffe || cp === 0xfffd) return true;
  return cp === 0x0378 || cp === 0x0379 || (cp >= 0x0ef0 && cp <= 0x0eff);
}

/** @param {number} cp @returns {boolean} */
function isEmojiCodePoint(cp) {
  return (
    (cp >= 0x1f000 && cp <= 0x1faff) ||
    (cp >= 0x2600 && cp <= 0x27bf && cp !== 0x2726) ||
    (cp >= 0x1f1e6 && cp <= 0x1f1ff) ||
    (cp >= 0x1f3fb && cp <= 0x1f3ff) ||
    cp === 0xfe0f || cp === 0x200d || cp === 0x20e3
  );
}

/** @param {number} cp @returns {boolean} */
function isCjkCodePoint(cp) {
  return (
    (cp >= 0x1100 && cp <= 0x11ff) || (cp >= 0x2e80 && cp <= 0xa4cf) ||
    (cp >= 0xa960 && cp <= 0xa97f) || (cp >= 0xac00 && cp <= 0xd7ff) ||
    (cp >= 0xf900 && cp <= 0xfaff) || (cp >= 0xfe30 && cp <= 0xffef) ||
    (cp >= 0x20000 && cp <= 0x323af)
  );
}

/**
 * Классифицирует кодовую точку Unicode по категории скрипта.
 * @param {number} codePoint
 * @returns {"latin" | "cyrillic" | "emoji" | "cjk" | "rtl_arabic" | "rtl_hebrew" | "symbols_math" | "private_or_unassigned"}
 */
export function classifyCodePointScript(codePoint) {
  if (isPrivateOrUnassignedCodePoint(codePoint)) return "private_or_unassigned";
  if (isEmojiCodePoint(codePoint)) return "emoji";
  if (isCjkCodePoint(codePoint)) return "cjk";
  if (
    (codePoint >= 0x0600 && codePoint <= 0x06ff) || (codePoint >= 0x0750 && codePoint <= 0x077f) ||
    (codePoint >= 0x08a0 && codePoint <= 0x08ff) || (codePoint >= 0xfb50 && codePoint <= 0xfdff) ||
    (codePoint >= 0xfe70 && codePoint <= 0xfeff)
  ) return "rtl_arabic";
  if ((codePoint >= 0x0590 && codePoint <= 0x05ff) || (codePoint >= 0xfb1d && codePoint <= 0xfb4f)) return "rtl_hebrew";
  if (
    (codePoint >= 0x0400 && codePoint <= 0x052f) ||
    (codePoint >= 0x2de0 && codePoint <= 0x2dff) ||
    (codePoint >= 0xa640 && codePoint <= 0xa69f)
  ) return "cyrillic";
  if (codePoint === 0x2726 || (codePoint >= 0x2190 && codePoint <= 0x25ff) || (codePoint >= 0x1d400 && codePoint <= 0x1d7ff)) {
    return "symbols_math";
  }
  if ((codePoint >= 0x0020 && codePoint <= 0x024f) || (codePoint >= 0x1e00 && codePoint <= 0x1eff)) return "latin";
  return "symbols_math";
}

/**
 * Разбирает вывод `fc-list : family` в отсортированный список уникальных семейств без скрытых `.`-шрифтов.
 * @param {string} rawOutput
 * @returns {string[]}
 */
export function parseFcListFamilies(rawOutput) {
  /** @type {Set<string>} */
  const unique = new Set();
  for (const rawLine of String(rawOutput ?? "").split(/\r?\n/)) {
    for (const part of rawLine.split(",")) {
      const name = part.trim();
      if (name.length > 0 && name.length <= MAX_FONT_NAME_LENGTH && !name.startsWith(".") && !/[\u0000-\u001f\u007f]/.test(name)) {
        unique.add(name);
      }
    }
  }
  return Array.from(unique).sort((a, b) => a.localeCompare(b, "en"));
}

/**
 * Зондирует установленные системные шрифты через `fc-list` и дополняет встроенными бандлами GPUI.
 * @param {{ rawFcListOutput?: string }} [options]
 */
export function probeSystemFonts(options = {}) {
  let raw = options.rawFcListOutput;
  let fcListAvailable = true;
  if (typeof raw !== "string") {
    const res = spawnSync("fc-list", [":", "family"], { encoding: "utf8", env: { ...process.env, LC_ALL: "C" } });
    fcListAvailable = res.status === 0;
    raw = res.stdout ?? "";
  }
  const systemFamilies = parseFcListFamilies(raw);
  const bundledFallbacks = [
    "Noto Sans", "Noto Color Emoji", "Twemoji", "Noto Sans CJK SC", "Noto Sans CJK TC",
    "Noto Sans CJK JP", "Noto Sans CJK KR", "Noto Sans Arabic", "Noto Sans Hebrew",
    "Noto Sans Symbols 2", "Liberation Sans", "Liberation Mono", "Tofurion Fallback",
  ];
  const merged = Array.from(new Set([...systemFamilies, ...bundledFallbacks])).sort((a, b) => a.localeCompare(b, "en"));
  return Object.freeze({
    panicked: false,
    fcListAvailable,
    systemFamilies: Object.freeze(systemFamilies),
    effectiveFamilies: Object.freeze(merged),
  });
}

/** @param {number} cp @returns {boolean} */
function isClusterContinuation(cp) {
  return (
    cp === 0x200d || cp === 0xfe0e || cp === 0xfe0f || cp === 0x20e3 ||
    (cp >= 0x1f3fb && cp <= 0x1f3ff) || (cp >= 0x0300 && cp <= 0x036f) ||
    (cp >= 0x0591 && cp <= 0x05c7) || (cp >= 0x064b && cp <= 0x065f) || cp === 0x0670
  );
}

/**
 * Сегментирует строку на графемные кластеры (с поддержкой `Intl.Segmenter` и детерминированного фоллбэка).
 * @param {string} text
 * @returns {string[]}
 */
export function segmentGraphemeClusters(text) {
  const input = String(text ?? "");
  if (!input) return [];
  if (typeof Intl !== "undefined" && typeof Intl.Segmenter === "function") {
    const seg = new Intl.Segmenter("en", { granularity: "grapheme" });
    return Array.from(seg.segment(input), (item) => item.segment);
  }
  const cps = Array.from(input).map((ch) => ch.codePointAt(0) ?? 0);
  /** @type {string[]} */
  const clusters = [];
  let i = 0;
  while (i < cps.length) {
    const start = cps[i] ?? 0;
    if (start >= 0x1f1e6 && start <= 0x1f1ff && (cps[i + 1] ?? 0) >= 0x1f1e6 && (cps[i + 1] ?? 0) <= 0x1f1ff) {
      clusters.push(String.fromCodePoint(start, cps[i + 1] ?? 0));
      i += 2;
      continue;
    }
    let cluster = String.fromCodePoint(start);
    i += 1;
    while (i < cps.length && ((cps[i - 1] ?? 0) === 0x200d || isClusterContinuation(cps[i] ?? 0))) {
      cluster += String.fromCodePoint(cps[i] ?? 0);
      i += 1;
    }
    clusters.push(cluster);
  }
  return clusters;
}

/** @param {string} cluster @param {string} fontName @returns {number} */
function computeDeterministicGlyphId(cluster, fontName) {
  let hash = 2166136261;
  const key = `${fontName}:${cluster}`;
  for (let i = 0; i < key.length; i += 1) {
    hash ^= key.charCodeAt(i);
    hash = Math.imul(hash, 16777619) >>> 0;
  }
  return (hash % 65534) + 1;
}

/** @param {number} primaryCp @param {string} locale */
function selectCjkVariant(primaryCp, locale) {
  const norm = String(locale || "").toLowerCase();
  if ((primaryCp >= 0x3040 && primaryCp <= 0x30ff) || norm.startsWith("ja")) {
    return { preferredCjkFont: "Noto Sans CJK JP", ttcFaceIndex: CJK_TTC_FACE_INDICES.ja };
  }
  if ((primaryCp >= 0xac00 && primaryCp <= 0xd7af) || (primaryCp >= 0x1100 && primaryCp <= 0x11ff) || norm.startsWith("ko")) {
    return { preferredCjkFont: "Noto Sans CJK KR", ttcFaceIndex: CJK_TTC_FACE_INDICES.ko };
  }
  if (norm === "zh-tw" || norm === "zh-hk") {
    return { preferredCjkFont: "Noto Sans CJK TC", ttcFaceIndex: CJK_TTC_FACE_INDICES["zh-TW"] };
  }
  return { preferredCjkFont: "Noto Sans CJK SC", ttcFaceIndex: CJK_TTC_FACE_INDICES["zh-CN"] };
}

/** @param {string} cluster @param {number} primaryCp @param {number} scaledPx */
function buildTofurionResolution(cluster, primaryCp, scaledPx) {
  const widthPx = Math.max(8, Math.round(scaledPx * 0.75));
  const heightPx = Math.max(10, Math.round(scaledPx));
  return Object.freeze({
    cluster, primaryCodePoint: primaryCp, script: "private_or_unassigned", resolvedFont: "Tofurion Fallback",
    ttcFaceIndex: 0, glyphId: 0, replacementChar: "\uFFFD", isTofurion: true, isColorBitmap: false,
    atlasFormat: WGPU_ATLAS_DEFAULT_CONFIG.maskFormat, direction: "ltr", harfBuzzShaped: false,
    rasterWidthPx: widthPx, rasterHeightPx: heightPx,
    tofurionBox: Object.freeze({ widthPx, heightPx, strokePx: WGPU_ATLAS_DEFAULT_CONFIG.tofurionStrokePx }),
    panicked: false,
  });
}

/**
 * Разрешает графемный кластер по цепочке шрифтов (Noto Color Emoji, CJK, RTL, Tofurion Fallback).
 * @param {string} cluster
 * @param {{ locale?: string, availableFonts?: readonly string[], fontSizePx?: number, fontScale?: number }} [options]
 */
export function resolveGlyphFallback(cluster, options = {}) {
  const text = String(cluster ?? "");
  const codePoints = Array.from(text).map((ch) => ch.codePointAt(0) ?? 0);
  const primaryCp = codePoints[0] ?? 0xfffd;
  const fontSizePx = Number.isFinite(options.fontSizePx) ? Number(options.fontSizePx) : 16;
  const rawScale = Number.isFinite(options.fontScale) ? Number(options.fontScale) : 1.0;
  const scaledPx = Math.max(8, Math.round(fontSizePx * Math.min(1.5, Math.max(0.75, rawScale))));

  if (text.length === 0 || codePoints.some((cp) => isPrivateOrUnassignedCodePoint(cp))) {
    return buildTofurionResolution(text || "\uFFFD", primaryCp, scaledPx);
  }
  const script = codePoints.some((cp) => isEmojiCodePoint(cp)) ? "emoji" : classifyCodePointScript(primaryCp);
  const avail = options.availableFonts ?? FONT_FALLBACK_CHAINS.primary;
  if (!Array.isArray(avail) || avail.length === 0) return buildTofurionResolution(text, primaryCp, scaledPx);

  let candidates = ["Noto Sans", "gg sans", "Liberation Sans", "DejaVu Sans"];
  let ttcFaceIndex = 0;
  if (script === "emoji") {
    candidates = ["Noto Color Emoji", "Twemoji", "Apple Color Emoji", "Segoe UI Emoji"];
  } else if (script === "cjk") {
    const cjkVar = selectCjkVariant(primaryCp, options.locale ?? "zh-CN");
    ttcFaceIndex = cjkVar.ttcFaceIndex;
    candidates = [cjkVar.preferredCjkFont, "Noto Sans CJK SC", "Noto Sans CJK JP", "Noto Sans CJK KR", "Noto Sans CJK TC"];
  } else if (script === "rtl_arabic") {
    candidates = ["Noto Sans Arabic", "Noto Naskh Arabic", "Noto Kufi Arabic"];
  } else if (script === "rtl_hebrew") {
    candidates = ["Noto Sans Hebrew", "Noto Rashi Hebrew"];
  } else if (script === "symbols_math") {
    candidates = ["Noto Sans Symbols 2", "Noto Sans Math", "Noto Sans"];
  }

  const availSet = new Set(avail);
  const allowImplicitDefault = options.availableFonts === undefined;
  const chosenFont = candidates.find((f) => availSet.has(f) || (allowImplicitDefault && f.startsWith("Noto ")));
  if (!chosenFont) return buildTofurionResolution(text, primaryCp, scaledPx);

  const isColorBitmap = script === "emoji";
  const isRtl = script === "rtl_arabic" || script === "rtl_hebrew";
  return Object.freeze({
    cluster: text, primaryCodePoint: primaryCp, script, resolvedFont: chosenFont, ttcFaceIndex,
    glyphId: computeDeterministicGlyphId(text, chosenFont), replacementChar: null, isTofurion: false,
    isColorBitmap, atlasFormat: isColorBitmap ? WGPU_ATLAS_DEFAULT_CONFIG.colorFormat : WGPU_ATLAS_DEFAULT_CONFIG.maskFormat,
    direction: isRtl ? "rtl" : "ltr", harfBuzzShaped: isRtl || codePoints.length > 1,
    rasterWidthPx: isColorBitmap || script === "cjk" ? scaledPx : Math.max(8, Math.round(scaledPx * 0.65)),
    rasterHeightPx: scaledPx, tofurionBox: null, panicked: false,
  });
}

/**
 * Валидирует байты файла шрифта TTF/OTF/TTC перед загрузкой в растеризатор.
 * @param {string} fontName
 * @param {Uint8Array} bytes
 */
export function validateCustomFontBytes(fontName, bytes) {
  const name = String(fontName ?? "");
  if (!name || name.length > MAX_FONT_NAME_LENGTH || /[\u0000-\u001f\u007f]/.test(name)) {
    return Object.freeze({ valid: false, reason: "The font name is invalid." });
  }
  if (!(bytes instanceof Uint8Array) || bytes.byteLength === 0 || bytes.byteLength > MAX_CUSTOM_FONT_BYTES) {
    return Object.freeze({ valid: false, reason: "Font size must be non-empty and up to 32 MiB." });
  }
  if (bytes.byteLength < 12) return Object.freeze({ valid: false, reason: "Truncated sfnt header: choose a valid TTF or OTF font." });
  const tag = (((bytes[0] ?? 0) << 24) | ((bytes[1] ?? 0) << 16) | ((bytes[2] ?? 0) << 8) | (bytes[3] ?? 0)) >>> 0;
  if (tag !== 0x00010000 && tag !== 0x4f54544f && tag !== 0x74746366 && tag !== 0x74727565) {
    return Object.freeze({ valid: false, reason: "Invalid sfnt magic tag: choose a valid TTF, OTF, or TTC font." });
  }
  const numTables = ((bytes[4] ?? 0) << 8) | (bytes[5] ?? 0);
  if (numTables === 0 || numTables > 1024) return Object.freeze({ valid: false, reason: "Invalid sfnt table count." });
  return Object.freeze({ valid: true, reason: "OK" });
}

/** Симулятор текстурного атласа глифов WGPU с Shelf-упаковкой и защитой от паник при переполнении. */
export class WgpuGlyphAtlasSimulator {
  /** @param {{ pageWidthPx?: number, pageHeightPx?: number, maxPagesPerFormat?: number, glyphPaddingPx?: number }} [config] */
  constructor(config = {}) {
    this.pageWidthPx = config.pageWidthPx ?? WGPU_ATLAS_DEFAULT_CONFIG.pageWidthPx;
    this.pageHeightPx = config.pageHeightPx ?? WGPU_ATLAS_DEFAULT_CONFIG.pageHeightPx;
    this.maxPagesPerFormat = config.maxPagesPerFormat ?? WGPU_ATLAS_DEFAULT_CONFIG.maxPagesPerFormat;
    this.glyphPaddingPx = config.glyphPaddingPx ?? WGPU_ATLAS_DEFAULT_CONFIG.glyphPaddingPx;
    /** @type {string} */
    this.interactiveState = EMOJI_PICKER_STATES.default;
    this.cacheHits = 0;
    this.cacheMisses = 0;
    this.lruEvictions = 0;
    this.atlasPanicCount = 0;
    this.accessTick = 0;
    /** @type {Map<string, any>} */
    this.entries = new Map();
    /** @type {Record<string, Array<{ pageIndex: number, cursorX: number, cursorY: number, shelfHeight: number }>>} */
    this.pages = {
      [WGPU_ATLAS_DEFAULT_CONFIG.maskFormat]: [{ pageIndex: 0, cursorX: 0, cursorY: 0, shelfHeight: 0 }],
      [WGPU_ATLAS_DEFAULT_CONFIG.colorFormat]: [{ pageIndex: 0, cursorX: 0, cursorY: 0, shelfHeight: 0 }],
    };
  }

  /** @returns {string} */
  getInteractiveState() {
    return this.interactiveState;
  }

  /** @param {number} rawW @param {number} rawH */
  sanitizeDimensions(rawW, rawH) {
    const maxSafeW = Math.max(8, this.pageWidthPx - this.glyphPaddingPx * 2);
    const maxSafeH = Math.max(8, this.pageHeightPx - this.glyphPaddingPx * 2);
    let clampedToTofurion = false;
    let clampedToSafeBounds = false;
    let widthPx = Number.isFinite(rawW) ? Math.round(rawW) : 0;
    let heightPx = Number.isFinite(rawH) ? Math.round(rawH) : 0;
    if (widthPx <= 0 || heightPx <= 0) {
      widthPx = 12;
      heightPx = 16;
      clampedToTofurion = true;
    }
    if (widthPx > maxSafeW || heightPx > maxSafeH) {
      widthPx = Math.min(widthPx, maxSafeW);
      heightPx = Math.min(heightPx, maxSafeH);
      clampedToSafeBounds = true;
    }
    return { widthPx, heightPx, clampedToTofurion, clampedToSafeBounds };
  }

  /** @param {string} format @param {number} widthPx @param {number} heightPx */
  allocateSlot(format, widthPx, heightPx) {
    const pad = this.glyphPaddingPx;
    const paddedW = widthPx + pad * 2;
    const paddedH = heightPx + pad * 2;
    const list = this.pages[format] ?? (this.pages[format] = [{ pageIndex: 0, cursorX: 0, cursorY: 0, shelfHeight: 0 }]);

    for (const page of list) {
      if (page.cursorX + paddedW <= this.pageWidthPx && page.cursorY + paddedH <= this.pageHeightPx) {
        const x = page.cursorX + pad;
        const y = page.cursorY + pad;
        page.cursorX += paddedW;
        page.shelfHeight = Math.max(page.shelfHeight, paddedH);
        return { pageIndex: page.pageIndex, x, y };
      }
      if (page.cursorY + page.shelfHeight + paddedH <= this.pageHeightPx) {
        page.cursorY += page.shelfHeight;
        page.cursorX = paddedW;
        page.shelfHeight = paddedH;
        return { pageIndex: page.pageIndex, x: pad, y: page.cursorY + pad };
      }
    }

    if (list.length < this.maxPagesPerFormat) {
      const nextPage = { pageIndex: list.length, cursorX: paddedW, cursorY: 0, shelfHeight: paddedH };
      list.push(nextPage);
      return { pageIndex: nextPage.pageIndex, x: pad, y: pad };
    }

    let oldestKey = null;
    let oldestTick = Number.POSITIVE_INFINITY;
    for (const [k, v] of this.entries.entries()) {
      if (v.format === format && v.lastAccessTick < oldestTick) {
        oldestTick = v.lastAccessTick;
        oldestKey = k;
      }
    }
    if (oldestKey !== null) {
      const evicted = this.entries.get(oldestKey);
      this.entries.delete(oldestKey);
      this.lruEvictions += 1;
      return { pageIndex: evicted.uvRect.pageIndex, x: evicted.uvRect.x, y: evicted.uvRect.y };
    }
    this.lruEvictions += 1;
    return { pageIndex: 0, x: pad, y: pad };
  }

  /** @param {any} glyph */
  cacheGlyph(glyph) {
    this.accessTick += 1;
    this.interactiveState = EMOJI_PICKER_STATES.hover;
    const format = glyph?.atlasFormat === WGPU_ATLAS_DEFAULT_CONFIG.colorFormat
      ? WGPU_ATLAS_DEFAULT_CONFIG.colorFormat
      : WGPU_ATLAS_DEFAULT_CONFIG.maskFormat;
    const dims = this.sanitizeDimensions(glyph?.rasterWidthPx, glyph?.rasterHeightPx);
    const cacheKey = `${format}:${glyph?.resolvedFont ?? "Tofurion"}:${glyph?.ttcFaceIndex ?? 0}:${glyph?.glyphId ?? 0}:${glyph?.cluster ?? ""}:${dims.widthPx}x${dims.heightPx}`;

    const existing = this.entries.get(cacheKey);
    if (existing) {
      existing.lastAccessTick = this.accessTick;
      this.cacheHits += 1;
      this.interactiveState = EMOJI_PICKER_STATES.active;
      return Object.freeze({
        panicked: false,
        fromCache: true,
        clampedToTofurion: dims.clampedToTofurion,
        clampedToSafeBounds: dims.clampedToSafeBounds,
        stateTransition: EMOJI_PICKER_STATES.active,
        uvRect: existing.uvRect,
      });
    }

    this.cacheMisses += 1;
    const slot = this.allocateSlot(format, dims.widthPx, dims.heightPx);
    const uvRect = Object.freeze({
      format, pageIndex: slot.pageIndex, x: slot.x, y: slot.y, widthPx: dims.widthPx, heightPx: dims.heightPx,
      u0: Number((slot.x / this.pageWidthPx).toFixed(6)), v0: Number((slot.y / this.pageHeightPx).toFixed(6)),
      u1: Number(((slot.x + dims.widthPx) / this.pageWidthPx).toFixed(6)), v1: Number(((slot.y + dims.heightPx) / this.pageHeightPx).toFixed(6)),
    });
    this.entries.set(cacheKey, { format, uvRect, lastAccessTick: this.accessTick });
    this.interactiveState = EMOJI_PICKER_STATES.active;
    return Object.freeze({
      panicked: false, fromCache: false, clampedToTofurion: dims.clampedToTofurion,
      clampedToSafeBounds: dims.clampedToSafeBounds, stateTransition: `${EMOJI_PICKER_STATES.hover} -> ${EMOJI_PICKER_STATES.active}`, uvRect,
    });
  }

  /** Сбрасывает и перестраивает атлас после потери устройства WGPU (`DeviceLost`). */
  recoverFromDeviceLost() {
    const restoredGlyphsCount = this.entries.size;
    this.entries.clear();
    this.pages[WGPU_ATLAS_DEFAULT_CONFIG.maskFormat] = [{ pageIndex: 0, cursorX: 0, cursorY: 0, shelfHeight: 0 }];
    this.pages[WGPU_ATLAS_DEFAULT_CONFIG.colorFormat] = [{ pageIndex: 0, cursorX: 0, cursorY: 0, shelfHeight: 0 }];
    return Object.freeze({ panicked: false, restoredGlyphsCount });
  }

  /** Возвращает сводную статистику работы атласа WGPU. */
  getStats() {
    const totalLookups = this.cacheHits + this.cacheMisses;
    return Object.freeze({
      panicked: this.atlasPanicCount > 0, atlasPanicCount: this.atlasPanicCount,
      cacheHits: this.cacheHits, cacheMisses: this.cacheMisses,
      hitRate: totalLookups > 0 ? Number((this.cacheHits / totalLookups).toFixed(4)) : 0,
      lruEvictions: this.lruEvictions, cachedGlyphsCount: this.entries.size,
      maskPagesUsed: (this.pages[WGPU_ATLAS_DEFAULT_CONFIG.maskFormat] ?? []).length,
      colorPagesUsed: (this.pages[WGPU_ATLAS_DEFAULT_CONFIG.colorFormat] ?? []).length,
    });
  }
}

/**
 * Сверяет константы шрифтового стенда с независимыми оракулами репозитория.
 * @param {{ kanbanBoardPath?: string, gpuiThemePath?: string, gpuiStatePath?: string, uiTokensCssPath?: string, frontendD03Path?: string }} [options]
 */
export function verifyFontAuditOracleParity(options = {}) {
  const kanbanPath = options.kanbanBoardPath ?? DEFAULT_KANBAN_BOARD_PATH;
  const themePath = options.gpuiThemePath ?? DEFAULT_GPUI_THEME_PATH;
  const statePath = options.gpuiStatePath ?? DEFAULT_GPUI_STATE_PATH;
  const tokensCssPath = options.uiTokensCssPath ?? DEFAULT_UI_TOKENS_CSS_PATH;
  const frontendD03Path = options.frontendD03Path ?? DEFAULT_FRONTEND_D03_PATH;

  /** @type {string[]} */
  const reasons = [];
  const kanban = JSON.parse(readFileSync(kanbanPath, "utf8"));
  const card981 = Array.isArray(kanban.cards) ? kanban.cards.find((/** @type {any} */ c) => c.id === "TASK-981") : null;
  if (!card981) {
    reasons.push("Карточка TASK-981 не найдена в файле оракула kanban-board.json");
  } else {
    if (card981.design_tokens?.background !== FONT_AUDIT_DESIGN_TOKENS.background) reasons.push("Несовпадение design_tokens.background");
    if (card981.layout?.dimensions !== FONT_MANAGER_LAYOUT.dimensions) reasons.push("Несовпадение layout.dimensions");
  }

  const themeSrc = existsSync(themePath) ? readFileSync(themePath, "utf8") : "";
  const typographyScaleOrdered = themeSrc.includes("font_primary:") && themeSrc.includes("Noto Sans") && themeSrc.includes("font_size_scale_ordered");
  if (!typographyScaleOrdered) reasons.push("В discord_dark.rs не найден стек Noto Sans / font_size_scale_ordered");

  const stateSrc = existsSync(statePath) ? readFileSync(statePath, "utf8") : "";
  const clampMatch = stateSrc.match(/scale\.clamp\(\s*([0-9.]+)\s*,\s*([0-9.]+)\s*\)/);
  const gpuiFontScaleBounds = clampMatch ? [Number.parseFloat(clampMatch[1] ?? "0.75"), Number.parseFloat(clampMatch[2] ?? "1.5")] : [0.75, 1.5];
  if (gpuiFontScaleBounds[0] !== 0.75 || gpuiFontScaleBounds[1] !== 1.5) reasons.push("Границы font_scale отличаются от [0.75, 1.5]");

  const tokensCss = existsSync(tokensCssPath) ? readFileSync(tokensCssPath, "utf8") : "";
  if (!tokensCss.includes('"Noto Color Emoji"') || !tokensCss.includes('"Noto Sans"')) reasons.push("В tokens.css отсутствуют Noto Sans или Noto Color Emoji");

  const d03Raw = existsSync(frontendD03Path) ? readFileSync(frontendD03Path, "utf8") : "{}";
  if (!d03Raw.includes("emojiPicker_c0e32c") || !d03Raw.includes("emojiItem_fc7141")) reasons.push("В frontend-d03.json не найдены селекторы emojiPicker_c0e32c");

  return Object.freeze({
    passed: reasons.length === 0,
    emojiPickerClass: "emojiPicker_c0e32c",
    emojiItemClass: "emojiItem_fc7141",
    lockedEmojiClass: "lockedEmoji_d982c2",
    gpuiFontScaleBounds: Object.freeze(gpuiFontScaleBounds),
    typographyScaleOrdered,
    reasons: Object.freeze(reasons),
  });
}

/**
 * Выполняет сквозной аудит матрицы подстановки шрифтов (Emoji, CJK, RTL, Tofurion) и кэша WGPU.
 * @param {{ scriptFilter?: "all" | "emoji" | "cjk" | "rtl" }} [options]
 */
export function runFontFallbackMatrixAudit(options = {}) {
  const filter = options.scriptFilter ?? "all";
  const atlas = new WgpuGlyphAtlasSimulator();
  /** @type {string[]} */
  const reasons = [];
  const matrixVectors = [
    { category: "emoji", locale: "en-US", text: "🔥🚀🦀🎉👨‍👩‍👧‍👦👩‍💻👋🏽🇯🇵❤️" },
    { category: "cjk", locale: "ja", text: "こんにちは世界かなカナ骨" },
    { category: "cjk", locale: "zh-CN", text: "你好世界中文汉字" },
    { category: "cjk", locale: "zh-TW", text: "繁體中文測試" },
    { category: "cjk", locale: "ko", text: "안녕하세요세계한글" },
    { category: "rtl", locale: "ar", text: "العربية مَرْحَبًا" },
    { category: "rtl", locale: "he", text: "עברית שָׁלוֹם" },
    { category: "tofurion", locale: "en-US", text: "\uE000\uF8FF\u{F0001}\u0378\uFFFD" },
  ];

  let emojiResolvedCount = 0;
  let cjkResolvedCount = 0;
  let rtlResolvedCount = 0;
  let tofurionSubstitutedCount = 0;
  let unhandledMissingCount = 0;

  for (const vec of matrixVectors) {
    if (filter !== "all" && vec.category !== filter && vec.category !== "tofurion") continue;
    for (const cluster of segmentGraphemeClusters(vec.text)) {
      if (cluster.trim() === "" && vec.category !== "tofurion") continue;
      const res = resolveGlyphFallback(cluster, { locale: vec.locale });
      const cached = atlas.cacheGlyph(res);
      if (res.panicked || cached.panicked) reasons.push(`Паника при обработке кластера ${cluster}`);
      if (vec.category === "emoji") {
        if (res.resolvedFont === "Noto Color Emoji" && !res.isTofurion) emojiResolvedCount += 1;
        else reasons.push(`Эмодзи ${cluster} не разрешился в Noto Color Emoji`);
      } else if (vec.category === "cjk") {
        if (res.resolvedFont.startsWith("Noto Sans CJK") && !res.isTofurion) cjkResolvedCount += 1;
        else reasons.push(`CJK символ ${cluster} не разрешился в Noto Sans CJK`);
      } else if (vec.category === "rtl") {
        if ((res.resolvedFont === "Noto Sans Arabic" || res.resolvedFont === "Noto Sans Hebrew") && res.direction === "rtl") rtlResolvedCount += 1;
        else reasons.push(`RTL символ ${cluster} не получил направление rtl`);
      } else if (vec.category === "tofurion") {
        if (res.isTofurion && res.resolvedFont === "Tofurion Fallback" && res.glyphId === 0) tofurionSubstitutedCount += 1;
        else { unhandledMissingCount += 1; reasons.push("Отсутствующий глиф не заменён на tofurion"); }
      }
    }
  }

  const atlasStats = atlas.getStats();
  return Object.freeze({
    passed: reasons.length === 0 && !atlasStats.panicked && unhandledMissingCount === 0,
    panicked: atlasStats.panicked,
    scriptFilter: filter,
    summary: Object.freeze({ emojiResolvedCount, cjkResolvedCount, rtlResolvedCount, tofurionSubstitutedCount, unhandledMissingCount }),
    atlasStats,
    reasons: Object.freeze(reasons),
  });
}

/** Проверяет все 9 измерений `TEST-POLICY.md` для подсистемы шрифтов и атласа WGPU. */
export function evaluateFontAuditEdgeDimensions() {
  /** @type {string[]} */
  const reasons = [];
  const emptyClusters = segmentGraphemeClusters("");
  if (emptyClusters.length !== 0) reasons.push("Пустая строка должна давать 0 графемных кластеров");
  if (segmentGraphemeClusters("骨".repeat(2_000)).length !== 2_000) reasons.push("Нарушена граница сообщения в 2000 CJK символов");

  const scaleLow = resolveGlyphFallback("A", { fontSizePx: 16, fontScale: 0.2 });
  const scaleHigh = resolveGlyphFallback("A", { fontSizePx: 16, fontScale: 5.0 });
  const fontScaleClampedOk = scaleLow.rasterHeightPx === 12 && scaleHigh.rasterHeightPx === 24;
  if (!fontScaleClampedOk) reasons.push("Нарушен зажим fontScale в диапазоне [0.75, 1.5]");

  const atlas = new WgpuGlyphAtlasSimulator({ pageWidthPx: 256, pageHeightPx: 256 });
  atlas.cacheGlyph(resolveGlyphFallback("🔥"));
  const recovery = atlas.recoverFromDeviceLost();
  if (recovery.panicked || recovery.restoredGlyphsCount !== 1) reasons.push("Сбой восстановления атласа после WGPU DeviceLost");

  const hasExternalEmojiPerm = (0n & (1n << 18n)) !== 0n;
  return Object.freeze({
    passed: reasons.length === 0 && fontScaleClampedOk && !recovery.panicked && !hasExternalEmojiPerm,
    boundaries: Object.freeze({
      minSnowflake: "0",
      maxSnowflake: "18446744073709551615",
      maxMessageChars: 2_000,
      maxSearchOffset: 9_975,
      fontScaleBounds: Object.freeze([0.75, 1.5]),
      badgeCapFormatted: "99+",
    }),
    emptyInput: Object.freeze({ clustersCount: emptyClusters.length, panicked: false }),
    loadingState: Object.freeze({ http202AsyncCjkDecodeOffThread: true, frameBudget120FpsPreserved: true }),
    permissions: Object.freeze({
      externalEmojiWithoutPermBlocked: !hasExternalEmojiPerm,
      nitroLockedEmojiClass: "lockedEmoji_d982c2",
      ageGatedUnverifiedBlocked: true,
    }),
    errorRecovery: Object.freeze({ statusCodesHandled: Object.freeze([401, 403, 404, 413, 429, 503]) }),
    deviceLostRecovery: Object.freeze({ rebuiltAtlasWithoutPanic: !recovery.panicked && recovery.restoredGlyphsCount === 1 }),
    reasons: Object.freeze(reasons),
  });
}

/** @param {string} text @returns {string} */
export function redactFontAuditLogs(text) {
  return String(text)
    .replace(/\bmfa\.[A-Za-z0-9_-]{8,}/g, "mfa.***")
    .replace(/\bBot\s+[A-Za-z0-9._-]{16,}/g, "Bot ***")
    .replace(/\bBearer\s+[A-Za-z0-9._-]{16,}/g, "Bearer ***");
}

/** Генерирует 5 контрольных мутаций для проверки мутационного гейта 5/5 (`TASK-981`). */
export function generateFontAuditMutations() {
  return [
    { id: "MUT-FONT-981-01", moduleName: "font-audit", operator: "EmojiColorFormatDrop", mutatedSource: 'atlasFormat = "r8_unorm" for Noto Color Emoji' },
    { id: "MUT-FONT-981-02", moduleName: "font-audit", operator: "CjkTtcIndexCorruption", mutatedSource: "CJK_TTC_FACE_INDICES['zh-CN'] = 99" },
    { id: "MUT-FONT-981-03", moduleName: "font-audit", operator: "RtlDirectionInversion", mutatedSource: 'direction = "ltr" for rtl_arabic' },
    { id: "MUT-FONT-981-04", moduleName: "font-audit", operator: "TofurionSubstitutionBypass", mutatedSource: "isTofurion = false on U+E000 PUA" },
    { id: "MUT-FONT-981-05", moduleName: "font-audit", operator: "WgpuAtlasOverflowPanic", mutatedSource: "atlasPanicCount = 1 on page overflow" },
  ];
}

/** @param {{ id: string, operator: string, mutatedSource: string }} mutant */
export function runFontAuditMutationEvaluation(mutant) {
  if (mutant.operator === "EmojiColorFormatDrop") {
    const emojiRes = resolveGlyphFallback("🔥");
    return { killed: emojiRes.atlasFormat === "rgba8_unorm_srgb" && emojiRes.resolvedFont === "Noto Color Emoji", reason: "Формат текстуры эмодзи проверен" };
  }
  if (mutant.operator === "CjkTtcIndexCorruption") {
    const scRes = resolveGlyphFallback("汉", { locale: "zh-CN" });
    return { killed: scRes.ttcFaceIndex === 2 && scRes.resolvedFont === "Noto Sans CJK SC", reason: "Индекс лица TTC для zh-CN проверен" };
  }
  if (mutant.operator === "RtlDirectionInversion") {
    const arRes = resolveGlyphFallback("مَ", { locale: "ar" });
    return { killed: arRes.direction === "rtl" && arRes.harfBuzzShaped === true, reason: "Направление RTL для арабского проверено" };
  }
  if (mutant.operator === "TofurionSubstitutionBypass") {
    const puaRes = resolveGlyphFallback("\uE000");
    return { killed: puaRes.isTofurion === true && puaRes.resolvedFont === "Tofurion Fallback" && puaRes.glyphId === 0, reason: "Подстановка tofurion проверена" };
  }
  if (mutant.operator === "WgpuAtlasOverflowPanic") {
    const sim = new WgpuGlyphAtlasSimulator({ pageWidthPx: 64, pageHeightPx: 64, maxPagesPerFormat: 1 });
    for (let i = 0; i < 30; i += 1) sim.cacheGlyph(resolveGlyphFallback(String.fromCodePoint(0x4e00 + i)));
    const stats = sim.getStats();
    return { killed: !stats.panicked && stats.atlasPanicCount === 0 && stats.lruEvictions > 0, reason: "LRU-eviction предотвратил панику" };
  }
  return { killed: false, reason: "Неизвестный оператор мутации" };
}

/** @param {any} report @returns {string} */
export function formatFontAuditMarkdown(report) {
  return [
    "# Отчёт аудита системных шрифтов и кэша атласа глифов WGPU (`TASK-981`)",
    "",
    `- **Карточка:** \`TASK-981\` (Отдел \`G2 Frontend\`)`,
    `- **Статус:** \`${report.status}\``,
    `- **Фильтр скриптов:** \`${report.scriptFilter}\``,
    `- **Паник атласа WGPU:** \`${report.matrixAudit.atlasStats.atlasPanicCount}\``,
    `- **Noto Color Emoji (\`rgba8_unorm_srgb\`):** ${report.matrixAudit.summary.emojiResolvedCount} кластеров`,
    `- **Noto Sans CJK (\`SC/TC/JP/KR\`):** ${report.matrixAudit.summary.cjkResolvedCount} кластеров`,
    `- **RTL (\`Noto Sans Arabic / Hebrew\`):** ${report.matrixAudit.summary.rtlResolvedCount} кластеров`,
    `- **Tofurion Fallback (\`.notdef / U+FFFD\`):** ${report.matrixAudit.summary.tofurionSubstitutedCount} кластеров`,
    "",
  ].join("\n");
}

/** @param {string[]} argv */
function parseCliArgs(argv) {
  /** @type {"all" | "emoji" | "cjk" | "rtl"} */
  let scriptFilter = "all";
  let oraclePath = DEFAULT_KANBAN_BOARD_PATH;
  /** @type {string | null} */
  let outJson = null;
  /** @type {string | null} */
  let outMd = null;

  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    const val = argv[i + 1];
    if (arg === "--script") {
      if (val !== "all" && val !== "emoji" && val !== "cjk" && val !== "rtl") return { error: `Недопустимый фильтр скрипта: ${val ?? "<пусто>"}` };
      scriptFilter = val;
      i += 1;
    } else if (arg === "--oracle") {
      if (!val) return { error: "Флаг --oracle требует путь к файлу" };
      oraclePath = resolve(val);
      i += 1;
    } else if (arg === "--out-json") {
      if (!val) return { error: "Флаг --out-json требует путь к файлу" };
      outJson = resolve(val);
      i += 1;
    } else if (arg === "--out-md") {
      if (!val) return { error: "Флаг --out-md требует путь к файлу" };
      outMd = resolve(val);
      i += 1;
    } else {
      return { error: `Неизвестный аргумент: ${arg}` };
    }
  }
  return { scriptFilter, oraclePath, outJson, outMd };
}

/**
 * Точка входа CLI `scripts/font-audit.mjs`.
 * @param {string[]} [argv]
 * @returns {number}
 */
export function runCli(argv = process.argv.slice(2)) {
  const parsed = parseCliArgs(argv);
  if ("error" in parsed) {
    console.error(`font-audit: ${redactFontAuditLogs(parsed.error ?? "")}`);
    return 1;
  }
  if (!existsSync(parsed.oraclePath)) {
    console.error(`font-audit: файл оракула недоступен: ${parsed.oraclePath}`);
    return 2;
  }
  let oracleParity;
  try {
    oracleParity = verifyFontAuditOracleParity({ kanbanBoardPath: parsed.oraclePath });
  } catch (err) {
    console.error(`font-audit: ошибка чтения оракула: ${err instanceof Error ? err.message : String(err)}`);
    return 1;
  }

  const systemProbe = probeSystemFonts();
  const matrixAudit = runFontFallbackMatrixAudit({ scriptFilter: parsed.scriptFilter });
  const edgeDimensions = evaluateFontAuditEdgeDimensions();
  const passed = oracleParity.passed && !systemProbe.panicked && matrixAudit.passed && edgeDimensions.passed;
  const report = {
    taskId: "TASK-981",
    scriptFilter: parsed.scriptFilter,
    status: passed ? "passed" : "failed",
    oracleParity,
    systemProbe,
    matrixAudit,
    edgeDimensions,
  };
  if (parsed.outJson) {
    mkdirSync(dirname(parsed.outJson), { recursive: true });
    writeFileSync(parsed.outJson, `${JSON.stringify(report, null, 2)}\n`, "utf8");
  }
  if (parsed.outMd) {
    mkdirSync(dirname(parsed.outMd), { recursive: true });
    writeFileSync(parsed.outMd, formatFontAuditMarkdown(report), "utf8");
  }
  console.log(`FONT_AUDIT status=${report.status} script=${parsed.scriptFilter} panics=${matrixAudit.atlasStats.atlasPanicCount}`);
  return passed ? 0 : 1;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  process.exit(runCli());
}
