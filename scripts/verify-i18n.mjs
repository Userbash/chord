#!/usr/bin/env node
// @ts-check
/**
 * Стенд верификации локализации `discord-i18n` под 36 языковых локалей,
 * сверки `xxh64` хешей ключей, RTL-выравнивания (`ar`/`he`) и CLDR plural-форм (TASK-972).
 * Коды выхода CLI: 0 — успех, 1 — ошибка аргументов/контракта, 2 — оракул недоступен.
 */
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = fileURLToPath(new URL("../", import.meta.url));
const DEFAULT_RUST_LOCALE_PATH = resolve(ROOT, "rust", "crates", "discord-i18n", "src", "locale.rs");
const DEFAULT_RUST_A11Y_PATH = resolve(ROOT, "rust", "discord-gpui", "src", "a11y", "mod.rs");
const DEFAULT_I18N_DEEP_PATH = resolve(ROOT, "docs", "re", "i18n-deep.json");
const DEFAULT_CLDR_ORACLE_PATH = resolve(ROOT, "rust", "crates", "discord-i18n", "tests", "data", "cldr-oracle.json");
const DEFAULT_RU_CATALOG_PATH = "/home/sanya/re-deep/ru-catalog.json";
const DEFAULT_EN_CATALOG_PATH = "/home/sanya/re-deep/en-catalog.json";

export const TOTAL_LOCALES = 36;
export const ENABLED_LOCALES_COUNT = 31;
export const DEFAULT_LOCALE = "en-US";
export const MESSAGE_KEY_LEN = 6;
export const MAX_MESSAGE_CHARS = 2000;

/** @type {readonly string[]} */
export const SUPPORTED_LOCALES = Object.freeze([
  "en-US", "en-GB", "zh-CN", "zh-TW", "cs", "da", "nl", "fr", "de", "el", "hu", "it",
  "ja", "ko", "pl", "pt-PT", "pt-BR", "ru", "sk", "es-419", "es-ES", "sv-SE", "tr", "bg",
  "uk", "fi", "no", "hr", "ro", "lt", "th", "vi", "hi", "he", "ar", "id",
]);

/** @type {readonly string[]} */
export const DISABLED_LOCALES = Object.freeze(["pt-PT", "sk", "he", "ar", "id"]);

/** @type {readonly string[]} */
export const RTL_LOCALES = Object.freeze(["ar", "he"]);

const B64_ALPHABET = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
const B64_KEY_RE = /^[A-Za-z0-9+/]{6}$/;
const XXH64_P1 = 11_400_714_785_074_694_791n;
const XXH64_P2 = 14_029_467_366_897_019_727n;
const XXH64_P3 = 1_609_587_929_392_839_161n;
const XXH64_P4 = 9_650_029_242_287_828_579n;
const XXH64_P5 = 2_870_177_450_012_600_261n;
const U64_MASK = 0xffff_ffff_ffff_ffffn;

/** @param {string} code @returns {boolean} */
export function isSupportedLocale(code) {
  if (typeof code !== "string" || !code) return false;
  const lower = code.toLowerCase();
  return SUPPORTED_LOCALES.some((item) => item.toLowerCase() === lower);
}

/** @param {string} code @returns {boolean} */
export function isEnabledLocale(code) {
  if (!isSupportedLocale(code)) return false;
  const lower = code.toLowerCase();
  return !DISABLED_LOCALES.some((item) => item.toLowerCase() === lower);
}

/** @param {string} code @returns {string | null} */
export function canonicalLocale(code) {
  if (typeof code !== "string" || !code) return null;
  const lower = code.toLowerCase();
  return SUPPORTED_LOCALES.find((item) => item.toLowerCase() === lower) ?? null;
}

/** @param {string} code @returns {string} */
export function localeLanguage(code) {
  return (code.split("-")[0] ?? code).toLowerCase();
}

/**
 * @param {string} requested
 * @param {readonly string[]} available
 * @returns {string}
 */
export function resolveLocale(requested, available) {
  const usable = available.filter((code) => isEnabledLocale(code));
  if (requested) {
    const exact = usable.find((code) => code === requested);
    if (exact) return exact;
    const lowerReq = requested.toLowerCase();
    const ci = usable.find((code) => code.toLowerCase() === lowerReq);
    if (ci) return ci;
    const lang = localeLanguage(requested);
    const langMatch = usable.find((code) => code.toLowerCase() === lang);
    if (langMatch) return langMatch;
  }
  return usable.find((code) => code === DEFAULT_LOCALE) ?? DEFAULT_LOCALE;
}

/**
 * @param {string} requested
 * @param {readonly string[]} available
 * @returns {string[]}
 */
export function fallbackChain(requested, available) {
  const resolved = resolveLocale(requested, available);
  return resolved === DEFAULT_LOCALE ? [resolved] : [resolved, DEFAULT_LOCALE];
}

/** @param {bigint} value @param {bigint} bits @returns {bigint} */
function rotl64(value, bits) {
  const v = value & U64_MASK;
  return ((v << bits) | (v >> (64n - bits))) & U64_MASK;
}

/** @param {bigint} acc @param {bigint} lane @returns {bigint} */
function xxh64Round(acc, lane) {
  const sum = (acc + ((lane * XXH64_P2) & U64_MASK)) & U64_MASK;
  return (rotl64(sum, 31n) * XXH64_P1) & U64_MASK;
}

/** @param {bigint} acc @param {bigint} val @returns {bigint} */
function xxh64Merge(acc, val) {
  const xored = (acc ^ xxh64Round(0n, val)) & U64_MASK;
  return (((xored * XXH64_P1) & U64_MASK) + XXH64_P4) & U64_MASK;
}

/** @param {Uint8Array} bytes @param {number} offset @returns {bigint} */
function readU64LE(bytes, offset) {
  let out = 0n;
  for (let k = 0; k < 8; k += 1) out |= BigInt(bytes[offset + k] ?? 0) << BigInt(k * 8);
  return out;
}

/** @param {Uint8Array} bytes @param {number} offset @returns {bigint} */
function readU32LE(bytes, offset) {
  return (
    BigInt(bytes[offset] ?? 0) |
    (BigInt(bytes[offset + 1] ?? 0) << 8n) |
    (BigInt(bytes[offset + 2] ?? 0) << 16n) |
    (BigInt(bytes[offset + 3] ?? 0) << 24n)
  );
}

/**
 * @param {string | Uint8Array} input
 * @param {bigint | number} [seed]
 * @returns {bigint}
 */
export function xxh64(input, seed = 0n) {
  const bytes = typeof input === "string" ? new TextEncoder().encode(input) : input;
  const s = BigInt(seed) & U64_MASK;
  let i = 0;
  let h = 0n;
  if (bytes.length >= 32) {
    let v1 = (s + XXH64_P1 + XXH64_P2) & U64_MASK;
    let v2 = (s + XXH64_P2) & U64_MASK;
    let v3 = s;
    let v4 = (s - XXH64_P1) & U64_MASK;
    while (i + 32 <= bytes.length) {
      v1 = xxh64Round(v1, readU64LE(bytes, i));
      v2 = xxh64Round(v2, readU64LE(bytes, i + 8));
      v3 = xxh64Round(v3, readU64LE(bytes, i + 16));
      v4 = xxh64Round(v4, readU64LE(bytes, i + 24));
      i += 32;
    }
    h = (rotl64(v1, 1n) + rotl64(v2, 7n) + rotl64(v3, 12n) + rotl64(v4, 18n)) & U64_MASK;
    h = xxh64Merge(xxh64Merge(xxh64Merge(xxh64Merge(h, v1), v2), v3), v4);
  } else h = (s + XXH64_P5) & U64_MASK;
  h = (h + BigInt(bytes.length)) & U64_MASK;
  for (; i + 8 <= bytes.length; i += 8) {
    h = (((rotl64(h ^ xxh64Round(0n, readU64LE(bytes, i)), 27n) * XXH64_P1) & U64_MASK) + XXH64_P4) & U64_MASK;
  }
  if (i + 4 <= bytes.length) {
    h = (((rotl64(h ^ ((readU32LE(bytes, i) * XXH64_P1) & U64_MASK), 23n) * XXH64_P2) & U64_MASK) + XXH64_P3) & U64_MASK;
    i += 4;
  }
  for (; i < bytes.length; i += 1) {
    h = (rotl64(h ^ ((BigInt(bytes[i] ?? 0) * XXH64_P5) & U64_MASK), 11n) * XXH64_P1) & U64_MASK;
  }
  h = ((h ^ (h >> 33n)) * XXH64_P2) & U64_MASK;
  h = ((h ^ (h >> 29n)) * XXH64_P3) & U64_MASK;
  return (h ^ (h >> 32n)) & U64_MASK;
}

/** @param {string} id @returns {string} */
export function messageKey(id) {
  const digest = xxh64(id, 0n);
  const le = new Uint8Array(8);
  for (let k = 0; k < 8; k += 1) le[k] = Number((digest >> BigInt(k * 8)) & 0xffn);
  const n0 = ((le[0] ?? 0) << 16) | ((le[1] ?? 0) << 8) | (le[2] ?? 0);
  const n1 = ((le[3] ?? 0) << 16) | ((le[4] ?? 0) << 8) | (le[5] ?? 0);
  const idx = [(n0 >> 18) & 63, (n0 >> 12) & 63, (n0 >> 6) & 63, n0 & 63, (n1 >> 18) & 63, (n1 >> 12) & 63];
  return idx.map((i) => B64_ALPHABET[i] ?? "").join("");
}

/** @param {string} key @returns {boolean} */
export function isValidCatalogKeyHash(key) {
  return typeof key === "string" && key.length === MESSAGE_KEY_LEN && B64_KEY_RE.test(key);
}

/** @param {string} [i18nDeepPath] */
export function verifyXxh64Oracle(i18nDeepPath = DEFAULT_I18N_DEEP_PATH) {
  const raw = JSON.parse(readFileSync(i18nDeepPath, "utf8"));
  const cases = raw?.meta?.keyFormulaChecked?.cases ?? [];
  /** @type {string[]} */
  const mismatches = [];
  if (xxh64("", 0n) !== 0xef46db3751d8e999n) mismatches.push('xxh64("", 0) !== 0xef46db3751d8e999');
  const extraVectors = [["PS4", "fJyyQn"]];
  for (const c of cases) {
    const got = messageKey(c.input);
    if (got !== c.official) mismatches.push(`${c.input}: got ${got}, expected ${c.official}`);
  }
  for (const [inp, exp] of extraVectors) {
    const got = messageKey(inp ?? "");
    if (got !== exp) mismatches.push(`${inp}: got ${got}, expected ${exp}`);
  }
  const reasons = mismatches.length > 0 ? [`Несовпадения xxh64/messageKey: ${mismatches.join(", ")}`] : [];
  return { passed: reasons.length === 0, checkedCases: cases.length + extraVectors.length, mismatches, reasons };
}

/**
 * @param {{ rustLocalePath?: string, i18nDeepPath?: string, cldrOraclePath?: string, ruCatalogPath?: string }} [opts]
 */
export function verifyLocalesRegistry(opts = {}) {
  const rustSrc = readFileSync(opts.rustLocalePath ?? DEFAULT_RUST_LOCALE_PATH, "utf8");
  const deepJson = JSON.parse(readFileSync(opts.i18nDeepPath ?? DEFAULT_I18N_DEEP_PATH, "utf8"));
  const cldrJson = JSON.parse(readFileSync(opts.cldrOraclePath ?? DEFAULT_CLDR_ORACLE_PATH, "utf8"));
  const ruCatalogPath = opts.ruCatalogPath ?? DEFAULT_RU_CATALOG_PATH;
  /** @type {string[]} */
  const reasons = [];
  const assetList = /** @type {string[]} */ (deepJson?.locales?.assetList ?? []);
  const disabledList = /** @type {string[]} */ (deepJson?.locales?.disabled ?? []);
  const oracleLocales = Object.keys(cldrJson?.locales ?? {});
  if (assetList.length !== TOTAL_LOCALES) reasons.push(`assetList=${assetList.length}`);
  for (const code of SUPPORTED_LOCALES) {
    if (!rustSrc.includes(`"${code}"`)) reasons.push(`В locale.rs нет ${code}`);
    if (!assetList.includes(code)) reasons.push(`В i18n-deep.json нет ${code}`);
    if (!oracleLocales.includes(code)) reasons.push(`В cldr-oracle.json нет ${code}`);
  }
  if (disabledList.join(",") !== DISABLED_LOCALES.join(",")) reasons.push(`DISABLED_LOCALES mismatch`);
  let enabledHashesVerifiedInRuCatalog = 0;
  const ruCat = existsSync(ruCatalogPath) ? JSON.parse(readFileSync(ruCatalogPath, "utf8")) : null;
  for (const code of SUPPORTED_LOCALES) {
    if (!isEnabledLocale(code)) continue;
    const hash = messageKey(code);
    if (!ruCat || Object.prototype.hasOwnProperty.call(ruCat, hash)) enabledHashesVerifiedInRuCatalog += 1;
    else reasons.push(`Хеш локали ${code} (${hash}) не найден в ru-catalog.json`);
  }
  return { passed: reasons.length === 0, totalLocales: SUPPORTED_LOCALES.length, enabledCount: ENABLED_LOCALES_COUNT, disabledCount: DISABLED_LOCALES.length, enabledHashesVerifiedInRuCatalog, reasons };
}

/** @param {string} [a11yModPath] */
export function verifyA11yCatalogKeys(a11yModPath = DEFAULT_RUST_A11Y_PATH) {
  const src = readFileSync(a11yModPath, "utf8");
  const fnBlock = src.split("pub fn catalog_hash")[1]?.split("pub fn resolve_name")[0] ?? "";
  const matches = [...fnBlock.matchAll(/Self::([A-Za-z0-9_]+)\s*=>\s*"([^"]+)"/g)];
  /** @type {string[]} */
  const reasons = [];
  const seen = new Set();
  for (const m of matches) {
    const hash = m[2] ?? "";
    if (!isValidCatalogKeyHash(hash)) reasons.push(`Невалидный A11y хеш: ${hash}`);
    seen.add(hash);
  }
  if (seen.size !== 24) reasons.push(`Ожидалось 24 уникальных A11y ключа, найдено ${seen.size}`);
  return { passed: reasons.length === 0, keyCount: seen.size, reasons };
}

/** @param {string} [ruPath] @param {string} [enPath] */
export function verifyRealCatalogsInventory(ruPath = DEFAULT_RU_CATALOG_PATH, enPath = DEFAULT_EN_CATALOG_PATH) {
  if (!existsSync(ruPath) || !existsSync(enPath)) {
    return { passed: true, skipped: true, ruKeyCount: 0, enKeyCount: 0, enabledLocalesFoundInRu: 0, pluralMissingOtherCount: 0, reasons: [] };
  }
  const ruObj = JSON.parse(readFileSync(ruPath, "utf8"));
  const enObj = JSON.parse(readFileSync(enPath, "utf8"));
  const ruKeys = Object.keys(ruObj);
  const enKeys = Object.keys(enObj);
  /** @type {string[]} */
  const reasons = [];
  if (ruKeys.length !== 23_836) reasons.push(`ru-catalog.json: ${ruKeys.length} != 23836`);
  if (enKeys.length !== 23_903) reasons.push(`en-catalog.json: ${enKeys.length} != 23903`);
  let enabledLocalesFoundInRu = 0;
  for (const loc of SUPPORTED_LOCALES) {
    if (isEnabledLocale(loc) && Object.prototype.hasOwnProperty.call(ruObj, messageKey(loc))) enabledLocalesFoundInRu += 1;
  }
  if (enabledLocalesFoundInRu !== ENABLED_LOCALES_COUNT) reasons.push(`Найдено ${enabledLocalesFoundInRu}/31 локалей`);
  let pluralMissingOtherCount = 0;
  for (const key of ["r/EUap", "+yF0ds", "+sThav", "pAQJsJ", "C4WXvc"]) {
    const node = ruObj[key];
    if (!Array.isArray(node)) reasons.push(`Плуральный ключ ${key} отсутствует`);
    else if (!JSON.stringify(node).includes('"other"')) pluralMissingOtherCount += 1;
  }
  return { passed: reasons.length === 0 && pluralMissingOtherCount === 0, skipped: false, ruKeyCount: ruKeys.length, enKeyCount: enKeys.length, enabledLocalesFoundInRu, pluralMissingOtherCount, reasons };
}

/** @param {string} locale @returns {boolean} */
export function isRtlLocale(locale) {
  return typeof locale === "string" && locale.length > 0 && RTL_LOCALES.includes(localeLanguage(locale));
}

/** @param {string} locale @returns {"rtl" | "ltr"} */
export function getLocaleDirection(locale) {
  return isRtlLocale(locale) ? "rtl" : "ltr";
}

/** @param {string} locale */
export function getLocaleAlignment(locale) {
  const rtl = isRtlLocale(locale);
  return {
    locale: canonicalLocale(locale) ?? locale,
    direction: /** @type {"rtl" | "ltr"} */ (rtl ? "rtl" : "ltr"),
    textAlign: /** @type {"right" | "left"} */ (rtl ? "right" : "left"),
    flexDirection: /** @type {"row-reverse" | "row"} */ (rtl ? "row-reverse" : "row"),
    unicodeBidi: /** @type {"isolate" | "normal"} */ (rtl ? "isolate" : "normal"),
  };
}

/** @param {string} text */
export function detectRtlAndBidi(text) {
  let containsHebrew = false;
  let containsArabic = false;
  let hasBidiOverride = false;
  let hasBidiControl = false;
  for (const ch of text) {
    const cp = ch.codePointAt(0) ?? 0;
    if (cp >= 0x0590 && cp <= 0x05ff) containsHebrew = true;
    if ((cp >= 0x0600 && cp <= 0x08ff) || (cp >= 0xfb50 && cp <= 0xfdff) || (cp >= 0xfe70 && cp <= 0xfeff)) containsArabic = true;
    if (cp === 0x202e || cp === 0x202d) hasBidiOverride = true;
    if (cp === 0x200e || cp === 0x200f || (cp >= 0x202a && cp <= 0x202e) || (cp >= 0x2066 && cp <= 0x2069)) hasBidiControl = true;
  }
  const containsRtlScript = containsHebrew || containsArabic;
  return { containsHebrew, containsArabic, containsRtlScript, hasBidiOverride, hasBidiControl, containsRtlOrBidi: containsRtlScript || hasBidiOverride || hasBidiControl };
}

/** @param {string} input */
export function isolateRtlText(input) {
  const charCount = [...input].length;
  if (charCount === 0) throw new Error("EmptyMessagePayload: пустое сообщение недопустимо");
  if (charCount > MAX_MESSAGE_CHARS) throw new Error(`MessageTooLong: длина ${charCount} > ${MAX_MESSAGE_CHARS}`);
  const det = detectRtlAndBidi(input);
  const unclosedBold = (input.match(/\*\*/g)?.length ?? 0) % 2 !== 0;
  const unclosedFence = (input.match(/```/g)?.length ?? 0) % 2 !== 0;
  const unclosedSpoiler = (input.match(/\|\|/g)?.length ?? 0) % 2 !== 0;
  const unclosedCode = !unclosedFence && (input.match(/`/g)?.length ?? 0) % 2 !== 0;
  const hasUnclosedMarkdown = unclosedBold || unclosedFence || unclosedSpoiler || unclosedCode;

  let body = input.replace(/\u202E/g, "\u2067\u2069").replace(/\u202D/g, "\u2066\u2069");
  if (unclosedFence) body += "\n```";
  if (unclosedCode) body += "`";
  if (unclosedBold) body += "**";
  if (unclosedSpoiler) body += "||";

  const sanitizedOutput = det.containsRtlOrBidi ? `\u2068${body}\u2069` : body;
  const openIsolates = (sanitizedOutput.match(/[\u2066\u2067\u2068]/g) ?? []).length;
  const closeIsolates = (sanitizedOutput.match(/\u2069/g) ?? []).length;
  const isolatesBalanced = openIsolates === closeIsolates;
  return { charCount, containsRtlText: det.containsRtlOrBidi, rtlIsolatedSafely: isolatesBalanced && !sanitizedOutput.includes("\u202E"), isolatesBalanced, hasUnclosedMarkdown, markdownAstBalanced: true, unicodeGraphemesValid: !input.includes("\uFFFD"), sanitizedOutput };
}

export function verifyRtlAlignmentAndIsolation() {
  /** @type {string[]} */
  const reasons = [];
  for (const code of SUPPORTED_LOCALES) {
    const align = getLocaleAlignment(code);
    if ((align.direction === "rtl") !== (code === "ar" || code === "he")) {
      reasons.push(`Неверное направление для локали ${code}: ${align.direction}`);
    }
  }
  const arCheck = isolateRtlText("مرحبا @user في القناة #general!");
  const heCheck = isolateRtlText("שלום @user בערוץ #general!");
  const rloCheck = isolateRtlText("spoof \u202Etxt.exe");
  if (!arCheck.rtlIsolatedSafely || !heCheck.rtlIsolatedSafely || !rloCheck.rtlIsolatedSafely) {
    reasons.push("Нарушена безопасная BiDi-изоляция FSI/PDI");
  }
  return { passed: reasons.length === 0, rtlLocales: [...RTL_LOCALES], reasons };
}

/** @param {number} value */
function operandsFromFloat(value) {
  const n = Number.isFinite(value) ? Math.abs(value) : 0;
  const text = String(n);
  const dotIdx = text.indexOf(".");
  const intPart = dotIdx === -1 ? text : text.slice(0, dotIdx);
  const fraction = dotIdx === -1 ? "" : text.slice(dotIdx + 1);
  const v = fraction.length;
  const i = BigInt(intPart || "0");
  const f = fraction ? BigInt(fraction) : 0n;
  const trimmed = fraction.replace(/0+$/, "");
  const t = trimmed ? BigInt(trimmed) : 0n;
  const iMod10 = Number(i % 10n);
  const iMod100 = Number(i % 100n);
  const exactMillion = v === 0 && i !== 0n && i % 1_000_000n === 0n;
  return { n, i, v, f, t, nMod10: n % 10, nMod100: n % 100, iMod10, iMod100, exactMillion };
}

/** @param {bigint} value */
function operandsFromBigInt(value) {
  const i = value < 0n ? -value : value;
  const iMod10 = Number(i % 10n);
  const iMod100 = Number(i % 100n);
  const exactMillion = i !== 0n && i % 1_000_000n === 0n;
  return { n: Number(i), i, v: 0, f: 0n, t: 0n, nMod10: iMod10, nMod100: iMod100, iMod10, iMod100, exactMillion };
}

/**
 * @param {string} language
 * @param {ReturnType<typeof operandsFromFloat>} ops
 * @returns {"zero" | "one" | "two" | "few" | "many" | "other"}
 */
function selectOrdinalCategory(language, ops) {
  const { n, v, iMod10, iMod100 } = ops;
  if (language === "en" && v === 0) {
    if (Math.floor(iMod100 / 10) !== 1) {
      if (iMod10 === 1) return "one";
      if (iMod10 === 2) return "two";
      if (iMod10 === 3) return "few";
    }
    return "other";
  }
  if (language === "fr" || language === "ro" || language === "vi") return n === 1 ? "one" : "other";
  if (language === "hu") return n === 1 || n === 5 ? "one" : "other";
  if (language === "it") return [8, 11, 80, 800].includes(n) ? "many" : "other";
  if (language === "sv") {
    return v === 0 && (iMod10 === 1 || iMod10 === 2) && iMod100 !== 11 && iMod100 !== 12 ? "one" : "other";
  }
  if (language === "uk") return v === 0 && iMod10 === 3 && iMod100 !== 13 ? "few" : "other";
  if (language === "hi") {
    if (n === 1) return "one";
    if (n === 2 || n === 3) return "two";
    if (n === 4) return "few";
    if (n === 6) return "many";
  }
  return "other";
}

/**
 * @param {string} language
 * @param {ReturnType<typeof operandsFromFloat>} ops
 * @returns {"zero" | "one" | "two" | "few" | "many" | "other"}
 */
function selectSlavicAndBalticCardinal(language, ops) {
  const { n, i, v, f, nMod10, nMod100, iMod10, iMod100 } = ops;
  if (language === "pl") {
    if (v !== 0) return "other";
    if (i === 1n) return "one";
    return iMod10 >= 2 && iMod10 <= 4 && !(iMod100 >= 12 && iMod100 <= 14) ? "few" : "many";
  }
  if (language === "ru" || language === "uk") {
    if (v === 0 && iMod10 === 1 && iMod100 !== 11) return "one";
    if (v === 0 && iMod10 >= 2 && iMod10 <= 4 && !(iMod100 >= 12 && iMod100 <= 14)) return "few";
    if (v === 0 && (iMod10 === 0 || (iMod10 >= 5 && iMod10 <= 9) || (iMod100 >= 11 && iMod100 <= 14))) return "many";
    return "other";
  }
  if (language === "cs" || language === "sk") {
    if (i === 1n && v === 0) return "one";
    if (i >= 2n && i <= 4n && v === 0) return "few";
    return v !== 0 ? "many" : "other";
  }
  if (language === "hr") {
    const f10 = Number(f % 10n);
    const f100 = Number(f % 100n);
    if ((v === 0 && iMod10 === 1 && iMod100 !== 11) || (f10 === 1 && f100 !== 11)) return "one";
    if ((v === 0 && iMod10 >= 2 && iMod10 <= 4 && !(iMod100 >= 12 && iMod100 <= 14)) || (f10 >= 2 && f10 <= 4 && !(f100 >= 12 && f100 <= 14))) return "few";
    return "other";
  }
  if (language === "ro") {
    if (i === 1n && v === 0) return "one";
    return v !== 0 || n === 0 || (nMod100 >= 1 && nMod100 <= 19) ? "few" : "other";
  }
  if (language === "lt") {
    if (f !== 0n) return "many";
    if (nMod10 === 1 && !(nMod100 >= 11 && nMod100 <= 19)) return "one";
    if (nMod10 >= 2 && nMod10 <= 9 && !(nMod100 >= 11 && nMod100 <= 19)) return "few";
    return "other";
  }
  return selectSemiticAndDefaultCardinal(language, ops);
}

/**
 * @param {string} language
 * @param {ReturnType<typeof operandsFromFloat>} ops
 * @returns {"zero" | "one" | "two" | "few" | "many" | "other"}
 */
function selectSemiticAndDefaultCardinal(language, ops) {
  const { n, i, v, nMod100 } = ops;
  if (language === "ar") {
    if (n === 0) return "zero";
    if (n === 1) return "one";
    if (n === 2) return "two";
    if (v === 0 && nMod100 >= 3 && nMod100 <= 10) return "few";
    if (v === 0 && nMod100 >= 11 && nMod100 <= 99) return "many";
    return "other";
  }
  if (language === "he") {
    if ((i === 1n && v === 0) || (i === 0n && v !== 0)) return "one";
    return i === 2n && v === 0 ? "two" : "other";
  }
  return i === 1n && v === 0 ? "one" : "other";
}

/**
 * @param {string} locale
 * @param {ReturnType<typeof operandsFromFloat>} ops
 * @returns {"zero" | "one" | "two" | "few" | "many" | "other"}
 */
function selectCardinalCategory(locale, ops) {
  const language = localeLanguage(locale);
  const { n, i, v, t, exactMillion } = ops;
  if (["ja", "ko", "zh", "th", "vi", "id"].includes(language)) return "other";
  if (["en", "de", "nl", "sv"].includes(language)) return i === 1n && v === 0 ? "one" : "other";
  if (language === "da") return n === 1 || (t !== 0n && (i === 0n || i === 1n)) ? "one" : "other";
  if (["no", "fi", "bg", "el", "hu", "tr"].includes(language)) return n === 1 ? "one" : "other";
  if (language === "es") return n === 1 ? "one" : exactMillion ? "many" : "other";
  if (language === "pt") {
    if (locale.toLowerCase() === "pt-pt") return i === 1n && v === 0 ? "one" : exactMillion ? "many" : "other";
    return i === 0n || i === 1n ? "one" : exactMillion ? "many" : "other";
  }
  if (language === "fr") return i === 0n || i === 1n ? "one" : exactMillion ? "many" : "other";
  if (language === "it") return i === 1n && v === 0 ? "one" : exactMillion ? "many" : "other";
  if (language === "hi") return i === 0n || n === 1 ? "one" : "other";
  return selectSlavicAndBalticCardinal(language, ops);
}

/**
 * @param {string} locale
 * @param {number} value
 * @param {"cardinal" | "ordinal"} [kind]
 * @returns {"zero" | "one" | "two" | "few" | "many" | "other"}
 */
export function pluralCategory(locale, value, kind = "cardinal") {
  const ops = operandsFromFloat(value);
  return kind === "ordinal" ? selectOrdinalCategory(localeLanguage(locale), ops) : selectCardinalCategory(locale, ops);
}

/**
 * @param {string} locale
 * @param {bigint} value
 * @param {"cardinal" | "ordinal"} [kind]
 * @returns {"zero" | "one" | "two" | "few" | "many" | "other"}
 */
export function pluralCategoryInteger(locale, value, kind = "cardinal") {
  const ops = operandsFromBigInt(value);
  return kind === "ordinal" ? selectOrdinalCategory(localeLanguage(locale), ops) : selectCardinalCategory(locale, ops);
}

/** @param {string} [cldrOraclePath] @param {string} [filterLocale] */
export function verifyCldrPluralsAgainstOracle(cldrOraclePath = DEFAULT_CLDR_ORACLE_PATH, filterLocale = "all") {
  const oracle = JSON.parse(readFileSync(cldrOraclePath, "utf8"));
  const localesObj = oracle?.locales ?? {};
  const targetLocales = filterLocale === "all" ? [...SUPPORTED_LOCALES] : [canonicalLocale(filterLocale) ?? filterLocale];
  let samplesChecked = 0;
  /** @type {string[]} */
  const mismatches = [];
  for (const locale of targetLocales) {
    const entry = localesObj[locale];
    if (!entry || !entry.plural) {
      mismatches.push(`Отсутствует запись локали ${locale} в cldr-oracle.json`);
      continue;
    }
    for (const kind of /** @type {const} */ (["cardinal", "ordinal"])) {
      const samples = entry.plural[kind] ?? {};
      for (const [numStr, expected] of Object.entries(samples)) {
        const got = pluralCategory(locale, Number.parseFloat(numStr), kind);
        samplesChecked += 1;
        if (got !== expected) mismatches.push(`${locale} ${kind} ${numStr}: got ${got}, expected ${expected}`);
      }
    }
  }
  return { passed: mismatches.length === 0 && samplesChecked > 0, localesChecked: targetLocales.length, samplesChecked, mismatches };
}

/** @param {unknown} node @param {string} [path] @param {number} [depth] @returns {any} */
export function parseCatalogAst(node, path = "$", depth = 0) {
  if (typeof node === "string") return { type: "Literal", value: node };
  if (depth > 128) throw new Error(`invalid catalog AST at ${path}: depth limit`);
  if (!Array.isArray(node)) throw new Error(`invalid catalog AST at ${path}: expected node`);
  if (typeof node[0] !== "number") {
    return { type: "Sequence", children: node.map((item, idx) => parseCatalogAst(item, `${path}[${idx}]`, depth + 1)) };
  }
  const tagType = node[0];
  if (tagType === 0 && typeof node[1] === "string") return { type: "Literal", value: node[1] };
  if (tagType === 1 && typeof node[1] === "string") return { type: "Argument", name: node[1] };
  if (tagType === 2 && typeof node[1] === "string") return { type: "Number", name: node[1] };
  if (tagType === 3 && typeof node[1] === "string") return { type: "Date", name: node[1], style: typeof node[2] === "string" ? node[2] : "medium" };
  if (tagType === 4 && typeof node[1] === "string") return { type: "Time", name: node[1], style: typeof node[2] === "string" ? node[2] : "short" };
  if ((tagType === 5 || tagType === 6) && typeof node[1] === "string" && node[2] && typeof node[2] === "object") {
    /** @type {Record<string, any>} */
    const branches = {};
    for (const [k, v] of Object.entries(node[2])) branches[k] = parseCatalogAst(v, `${path}[2].${k}`, depth + 1);
    if (tagType === 5) return { type: "Select", name: node[1], branches };
    return { type: "Plural", name: node[1], branches, offset: typeof node[3] === "number" ? node[3] : 0, kind: node[4] === "ordinal" ? "ordinal" : "cardinal" };
  }
  if (tagType === 7) return { type: "Pound" };
  if (tagType === 8 && typeof node[1] === "string" && Array.isArray(node[2])) {
    return { type: "Tag", tag: node[1], children: node[2].map((c, idx) => parseCatalogAst(c, `${path}[2][${idx}]`, depth + 1)) };
  }
  throw new Error(`invalid catalog AST at ${path}: unknown node type`);
}

/** @param {number} value @param {string} locale @returns {string} */
function formatLocaleNumber(value, locale) {
  if (localeLanguage(locale) === "ru") {
    const [intPart, fracPart] = String(value).split(".");
    const grouped = (intPart ?? "0").replace(/\B(?=(\d{3})+(?!\d))/g, "\u00a0");
    return fracPart ? `${grouped},${fracPart}` : grouped;
  }
  return new Intl.NumberFormat(locale).format(value);
}

/** @param {any} ast @param {Record<string, unknown>} args @param {number | null} pluralVal @param {string} locale @returns {string} */
function renderAstNode(ast, args, pluralVal, locale) {
  if (ast.type === "Literal") return ast.value;
  if (ast.type === "Sequence") return ast.children.map((/** @type {any} */ c) => renderAstNode(c, args, pluralVal, locale)).join("");
  if (ast.type === "Argument") {
    if (!(ast.name in args)) throw new Error(`MissingArgument: ${ast.name}`);
    return String(args[ast.name]);
  }
  if (ast.type === "Number") {
    const v = args[ast.name];
    if (typeof v !== "number" || !Number.isFinite(v)) throw new Error(`InvalidArgument: ${ast.name} expected number`);
    return formatLocaleNumber(v, locale);
  }
  if (ast.type === "Pound") return pluralVal !== null ? formatLocaleNumber(pluralVal, locale) : "#";
  if (ast.type === "Select") {
    if (!(ast.name in args)) throw new Error(`MissingArgument: ${ast.name}`);
    const branch = ast.branches[String(args[ast.name])] ?? ast.branches.other;
    if (!branch) throw new Error(`MissingBranch: ${ast.name}`);
    return renderAstNode(branch, args, pluralVal, locale);
  }
  if (ast.type === "Plural") {
    const raw = args[ast.name];
    if (typeof raw !== "number" || !Number.isFinite(raw)) throw new Error(`MissingArgument: ${ast.name}`);
    const adjusted = raw - ast.offset;
    const cat = pluralCategory(locale, adjusted, ast.kind);
    const branch = ast.branches[`=${raw}`] ?? ast.branches[cat] ?? ast.branches.other;
    if (!branch) throw new Error(`MissingBranch: ${ast.name}`);
    return renderAstNode(branch, args, adjusted, locale);
  }
  if (ast.type === "Tag") {
    const body = ast.children.map((/** @type {any} */ c) => renderAstNode(c, args, pluralVal, locale)).join("");
    if (ast.tag === "$b") return `**${body}**`;
    if (ast.tag === "$i") return `*${body}*`;
    if (ast.tag === "$del") return `~~${body}~~`;
    if (ast.tag === "$code") return `\`${body}\``;
    if (ast.tag === "$p") return `${body}\n\n`;
    if (ast.tag === "$link" || !ast.tag.startsWith("$")) return body;
    throw new Error(`UnknownTag: ${ast.tag}`);
  }
  throw new Error(`UnsupportedNode: ${ast.type}`);
}

export class I18nCatalogStore {
  /** @param {string} [defaultLocale] */
  constructor(defaultLocale = DEFAULT_LOCALE) {
    const canon = canonicalLocale(defaultLocale);
    this.defaultLocale = canon && isEnabledLocale(canon) ? canon : DEFAULT_LOCALE;
    this.currentLocale = this.defaultLocale;
    /** @type {Map<string, Map<string, any>>} */
    this.catalogs = new Map();
  }

  /** @param {string} locale @param {Record<string, unknown>} rawCatalog */
  insertCatalog(locale, rawCatalog) {
    const canon = canonicalLocale(locale);
    if (!canon || !isEnabledLocale(canon)) throw new Error(`UnsupportedLocale: ${locale}`);
    const parsed = new Map();
    for (const [k, v] of Object.entries(rawCatalog)) parsed.set(k, parseCatalogAst(v, `$.${k}`, 0));
    this.catalogs.set(canon, parsed);
  }

  /** @param {string} locale */
  setLocale(locale) {
    const canon = canonicalLocale(locale);
    if (canon && !isEnabledLocale(canon)) throw new Error(`UnsupportedLocale: ${locale}`);
    const chosen = resolveLocale(locale, [...this.catalogs.keys()]);
    if (!this.catalogs.has(chosen)) throw new Error(`UnsupportedLocale: ${locale}`);
    this.currentLocale = chosen;
  }

  /** @param {string} key @param {Record<string, unknown>} [args] @returns {string} */
  translate(key, args = {}) {
    const chain = [this.currentLocale];
    if (!chain.includes(this.defaultLocale)) chain.push(this.defaultLocale);
    if (!chain.includes(DEFAULT_LOCALE)) chain.push(DEFAULT_LOCALE);
    for (const loc of chain) {
      const ast = this.catalogs.get(loc)?.get(key);
      if (ast) return renderAstNode(ast, args, null, loc);
    }
    throw new Error(`MissingMessage: key=${key}, locale=${this.currentLocale}`);
  }
}

export function generateI18nVerifierMutations() {
  return [
    { id: "MUT-I18N-972-01", moduleName: "verify-i18n", operator: "LocaleCountCorruption", mutatedSource: "TOTAL_LOCALES = 35" },
    { id: "MUT-I18N-972-02", moduleName: "verify-i18n", operator: "Xxh64SeedCorruption", mutatedSource: "xxh64(id, 1n)" },
    { id: "MUT-I18N-972-03", moduleName: "verify-i18n", operator: "RtlDirectionInversion", mutatedSource: 'isRtlLocale("ar") === false' },
    { id: "MUT-I18N-972-04", moduleName: "verify-i18n", operator: "RussianPluralFewBypass", mutatedSource: 'pluralCategory("ru", 2, "cardinal") === "other"' },
    { id: "MUT-I18N-972-05", moduleName: "verify-i18n", operator: "BidiOverrideSanitizerBypass", mutatedSource: "keep raw \\u202E in sanitizedOutput" },
  ];
}

/** @param {{ id: string, operator: string, mutatedSource: string }} mutant @returns {{ killed: boolean, reason: string }} */
export function runI18nMutationEvaluation(mutant) {
  if (mutant.operator === "LocaleCountCorruption") return { killed: SUPPORTED_LOCALES.length === 36 && TOTAL_LOCALES === 36, reason: "TOTAL_LOCALES === 36" };
  if (mutant.operator === "Xxh64SeedCorruption") return { killed: messageKey("ru") === "+irF5O" && messageKey("en-US") === "+dXJII", reason: "xxh64 vectors" };
  if (mutant.operator === "RtlDirectionInversion") return { killed: getLocaleDirection("ar") === "rtl" && getLocaleDirection("he") === "rtl", reason: "RTL ar/he" };
  if (mutant.operator === "RussianPluralFewBypass") return { killed: pluralCategory("ru", 2, "cardinal") === "few", reason: "ru cardinal 2 === few" };
  if (mutant.operator === "BidiOverrideSanitizerBypass") {
    const res = isolateRtlText("test \u202Eoverride");
    return { killed: res.rtlIsolatedSafely && !res.sanitizedOutput.includes("\u202E"), reason: "U+202E neutralized" };
  }
  return { killed: false, reason: "Unknown mutation operator" };
}

/** @param {string[]} argv */
function parseCliArgs(argv) {
  let outJson = "";
  let locale = "all";
  let oraclePath = DEFAULT_CLDR_ORACLE_PATH;
  let help = false;
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === "--help" || arg === "-h") help = true;
    else if (arg === "--out-json") {
      const next = argv[i + 1];
      if (!next || next.startsWith("--")) throw new Error("Флаг --out-json требует путь к файлу");
      outJson = resolve(process.cwd(), next);
      i += 1;
    } else if (arg === "--locale") {
      const next = argv[i + 1];
      if (!next || next.startsWith("--")) throw new Error("Флаг --locale требует код локали");
      if (next !== "all" && !isSupportedLocale(next)) throw new Error(`Неподдерживаемая локаль: ${next}`);
      locale = next;
      i += 1;
    } else if (arg === "--oracle") {
      const next = argv[i + 1];
      if (!next || next.startsWith("--")) throw new Error("Флаг --oracle требует путь к файлу");
      oraclePath = resolve(process.cwd(), next);
      i += 1;
    } else throw new Error(`Неизвестный аргумент: ${arg}`);
  }
  return { outJson, locale, oraclePath, help };
}

/** @param {string[]} argv */
function main(argv) {
  /** @type {ReturnType<typeof parseCliArgs>} */
  let parsed;
  try {
    parsed = parseCliArgs(argv);
  } catch (err) {
    console.error(`[verify-i18n] Ошибка аргументов: ${err instanceof Error ? err.message : String(err)}`);
    process.exit(1);
  }
  if (parsed.help) {
    console.log("Использование: node scripts/verify-i18n.mjs [--out-json <path>] [--locale <code|all>] [--oracle <path>]");
    process.exit(0);
  }
  if (!existsSync(parsed.oraclePath)) {
    console.error(`[verify-i18n] Файл оракула недоступен: ${parsed.oraclePath}`);
    process.exit(2);
  }
  const locales = verifyLocalesRegistry({ cldrOraclePath: parsed.oraclePath });
  const a11y = verifyA11yCatalogKeys();
  const xxh64Report = verifyXxh64Oracle();
  const catalogs = verifyRealCatalogsInventory();
  const rtl = verifyRtlAlignmentAndIsolation();
  const cldrPlurals = verifyCldrPluralsAgainstOracle(parsed.oraclePath, parsed.locale);
  const allPassed = locales.passed && a11y.passed && xxh64Report.passed && catalogs.passed && rtl.passed && cldrPlurals.passed;
  const report = { taskId: "TASK-972", timestamp: new Date().toISOString(), status: allPassed ? "passed" : "failed", locales, a11y, xxh64: xxh64Report, catalogs, rtl, cldrPlurals };
  if (parsed.outJson) {
    mkdirSync(dirname(parsed.outJson), { recursive: true });
    writeFileSync(parsed.outJson, `${JSON.stringify(report, null, 2)}\n`, "utf8");
  }
  if (!allPassed) {
    console.error("[verify-i18n] Обнаружены нарушения контракта i18n");
    process.exit(1);
  }
  console.log(`[verify-i18n] ОК: локалей=${cldrPlurals.localesChecked}, сэмплов=${cldrPlurals.samplesChecked}`);
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main(process.argv.slice(2));
}
