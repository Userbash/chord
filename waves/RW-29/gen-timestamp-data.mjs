#!/usr/bin/env node
// @ts-check
/**
 * Генератор данных P7-01e Discord-таймстемпов `<t:…>` (RW-29 / W29a).
 *
 * Источник — локальный Node 24 ICU (`Intl.DateTimeFormat`, `Intl.RelativeTimeFormat`),
 * сеть не используется. Все вызовы `Intl` идут с `timeZone: "UTC"`.
 *
 * Пишет:
 *   - `rust/crates/discord-i18n/src/timestamp_data.rs` — 31 локаль × 6 стилей;
 *   - `rust/crates/discord-i18n/tests/data/timestamp-oracle.json` — независимый оракул;
 *   - дубликаты обоих файлов в `waves/RW-29/generated/` для аудита.
 *
 * Ручная правка выходов запрещена. Запуск: `node waves/RW-29/gen-timestamp-data.mjs`.
 *
 * Детерминизм: в выходах нет времени генерации; строки зависят только от версии ICU,
 * поэтому на одном рантайме файлы воспроизводятся побайтово (проверяется повторным
 * запуском и сравнением md5).
 *
 * Нормализация: U+202F (NNBSP) и U+00A0 (NBSP) → U+0020 и в паттернах, и в оракуле —
 * ровно так же, как `render_pattern` в `format.rs`.
 */

import { readFileSync, writeFileSync, mkdirSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, "..", "..");
const crate = join(root, "rust/crates/discord-i18n");

const normalize = (/** @type {string} */ value) =>
  value.replaceAll("\u202f", " ").replaceAll("\u00a0", " ");

/* ------------------------------------------------------------------ */
/* 1. Список включённых локалей из locale.rs (источник истины).        */
/* ------------------------------------------------------------------ */

const localeSrc = readFileSync(join(crate, "src/locale.rs"), "utf8");
const rustList = (/** @type {string} */ name) => {
  const match = localeSrc.match(new RegExp(`${name}: \\[&str; \\d+\\] = \\[([^\\]]+)\\]`));
  if (!match) throw new Error(`в locale.rs не найден ${name}`);
  return [...match[1].matchAll(/"([^"]+)"/g)].map((item) => item[1]);
};

const disabled = rustList("DISABLED_LOCALES");
const enabled = rustList("SUPPORTED_LOCALES").filter((code) => !disabled.includes(code));
if (enabled.length !== 31) throw new Error(`ожидалось 31 включённая локаль, получено ${enabled.length}`);
for (const code of ["he", "ar", "id", "pt-PT", "sk"]) {
  if (enabled.includes(code)) throw new Error(`выключенная локаль попала в список: ${code}`);
}

/* ------------------------------------------------------------------ */
/* 2. Данные format.rs, которые использует рендер (для самопроверки).  */
/*    Генератор не пишет их, а сверяет с ними смоделированный рендер.  */
/* ------------------------------------------------------------------ */

const formatSrc = readFileSync(join(crate, "src/format_data.rs"), "utf8");
const statics = new Map();
for (const match of formatSrc.matchAll(
  /static (D_[A-Z0-9_]+): LocaleFormatData = LocaleFormatData \{([\s\S]*?)\n\};/g,
)) {
  statics.set(match[1], match[2]);
}
const staticForLocale = new Map();
for (const match of formatSrc.matchAll(/"([^"]+)" => &(D_[A-Z0-9_]+),/g)) {
  staticForLocale.set(match[1], match[2]);
}

/** @param {string} block @param {string} field @returns {string[]} */
const rustStringArray = (block, field) => {
  const match = block.match(new RegExp(`${field}: \\[([\\s\\S]*?)\\]`));
  if (!match) throw new Error(`в format_data.rs нет поля ${field}`);
  return [...match[1].matchAll(/"((?:[^"\\]|\\.)*)"/g)].map((item) => JSON.parse(`"${item[1]}"`));
};

/** @type {Record<string, {months_long: string[], weekdays_long: string[], day_periods: string[], year_offset: number}>} */
const formatData = {};
for (const locale of enabled) {
  const name = staticForLocale.get(locale);
  const block = name ? statics.get(name) : undefined;
  if (!block) throw new Error(`в format_data.rs нет данных для ${locale}`);
  const offset = block.match(/year_offset: (-?\d+)/);
  if (!offset) throw new Error(`в format_data.rs нет year_offset для ${locale}`);
  formatData[locale] = {
    months_long: rustStringArray(block, "months_long"),
    weekdays_long: rustStringArray(block, "weekdays_long"),
    day_periods: rustStringArray(block, "day_periods"),
    year_offset: Number(offset[1]),
  };
}

/* ------------------------------------------------------------------ */
/* 3. Стили. Явные компоненты (а не dateStyle/timeStyle) фиксируют     */
/*    ширины полей, поэтому паттерн однозначен и не зависит от даты.   */
/* ------------------------------------------------------------------ */

const STYLES = /** @type {const} */ ([
  {
    letter: "t",
    name: "ShortTime",
    options: { hour: "numeric", minute: "2-digit" },
  },
  {
    letter: "T",
    name: "LongTime",
    options: { hour: "numeric", minute: "2-digit", second: "2-digit" },
  },
  {
    letter: "d",
    name: "ShortDate",
    options: { year: "numeric", month: "2-digit", day: "2-digit" },
  },
  {
    letter: "D",
    name: "LongDate",
    options: { year: "numeric", month: "long", day: "numeric" },
  },
  {
    letter: "f",
    name: "ShortDateTime",
    options: { year: "numeric", month: "2-digit", day: "2-digit", hour: "numeric", minute: "2-digit" },
  },
  {
    letter: "F",
    name: "LongDateTime",
    options: {
      year: "numeric",
      month: "long",
      day: "numeric",
      weekday: "long",
      hour: "numeric",
      minute: "2-digit",
    },
  },
]);

const MOMENTS = [
  Math.floor(Date.UTC(2021, 5, 12, 9, 1, 0) / 1000), // 1623488460
  Math.floor(Date.UTC(1969, 11, 31, 23, 59, 59) / 1000), // -1
  Math.floor(Date.UTC(2030, 0, 1, 0, 0, 0) / 1000), // 1893456000
];
const SAMPLE = MOMENTS[0];

/**
 * CLDR-паттерн из `formatToParts`: ширины полей берутся из опций, а не из значения,
 * поэтому паттерн годится для любой даты.
 * @param {string} locale
 * @param {(typeof STYLES)[number]} style
 */
function derivePattern(locale, style) {
  const options = /** @type {Intl.DateTimeFormatOptions} */ (style.options);
  const parts = new Intl.DateTimeFormat(locale, { ...options, timeZone: "UTC" }).formatToParts(
    SAMPLE * 1000,
  );
  const hasDayPeriod = parts.some((part) => part.type === "dayPeriod");
  let pattern = "";
  for (const part of parts) {
    if (part.type === "literal") {
      // Литералы остаются как в ICU (включая NNBSP/NBSP): нормализация — дело
      // `format.rs::render_pattern`, ровно как требует контракт §1.
      pattern += `'${part.value.replaceAll("'", "''")}'`;
      continue;
    }
    let token;
    switch (part.type) {
      case "year":
        token = "y";
        break;
      case "month":
        token = options.month === "2-digit" ? "MM" : options.month === "long" ? "MMMM" : "M";
        break;
      case "day":
        token = options.day === "2-digit" ? "dd" : "d";
        break;
      case "weekday":
        token = "EEEE";
        break;
      case "hour":
        token = hasDayPeriod
          ? part.value.length === 2
            ? "hh"
            : "h"
          : part.value.length === 2
            ? "HH"
            : "H";
        break;
      case "minute":
        token = "mm";
        break;
      case "second":
        token = "ss";
        break;
      case "dayPeriod":
        token = "a";
        break;
      default:
        throw new Error(`${locale}/${style.letter}: неожидаемая часть ${part.type}`);
    }
    pattern += token;
  }
  return pattern;
}

/**
 * Длинные названия дней недели именно в контексте даты (финский эссив
 * «lauantaina» против самостоятельного «lauantai»).
 * @param {string} locale
 */
function weekdayNamesInDate(locale) {
  return Array.from({ length: 7 }, (_, offset) => {
    const day = new Date(Date.UTC(2021, 5, 13 + offset, 12));
    const parts = new Intl.DateTimeFormat(locale, {
      weekday: "long",
      year: "numeric",
      month: "long",
      day: "numeric",
      timeZone: "UTC",
    }).formatToParts(day);
    const value = parts.find((part) => part.type === "weekday")?.value;
    if (!value) throw new Error(`${locale}: нет weekday в контексте даты`);
    return value;
  });
}

/* ------------------------------------------------------------------ */
/* 4. Модель рендера `format.rs::render_pattern` (независимая от Rust). */
/* ------------------------------------------------------------------ */

/** @param {number} timestampSecs @param {number} yearOffset */
function dateParts(timestampSecs, yearOffset) {
  const date = new Date(timestampSecs * 1000);
  return {
    year: date.getUTCFullYear() + yearOffset,
    month: date.getUTCMonth() + 1,
    day: date.getUTCDate(),
    weekday: date.getUTCDay(),
    hour: date.getUTCHours(),
    minute: date.getUTCMinutes(),
    second: date.getUTCSeconds(),
  };
}

const pad2 = (/** @type {number} */ value) => String(value).padStart(2, "0");

/**
 * @param {string} pattern
 * @param {ReturnType<typeof dateParts>} parts
 * @param {{months_long: string[], weekdays_long: string[], day_periods: string[]}} data
 * @param {string[]} weekdays
 */
function renderPattern(pattern, parts, data, weekdays) {
  const chars = [...pattern];
  let out = "";
  let pos = 0;
  while (pos < chars.length) {
    if (chars[pos] === "'") {
      pos += 1;
      while (pos < chars.length) {
        if (chars[pos] === "'") {
          if (chars[pos + 1] === "'") {
            out += "'";
            pos += 2;
          } else {
            pos += 1;
            break;
          }
        } else {
          out += chars[pos];
          pos += 1;
        }
      }
      continue;
    }
    const char = chars[pos];
    if (/[A-Za-z]/.test(char)) {
      const start = pos;
      while (pos < chars.length && chars[pos] === char) pos += 1;
      const width = pos - start;
      let value;
      if (char === "y" && width === 1) value = String(parts.year);
      else if (char === "y" && width === 2) value = pad2(((parts.year % 100) + 100) % 100);
      else if (char === "y") value = String(parts.year).padStart(width, "0");
      else if (char === "M" && width === 1) value = String(parts.month);
      else if (char === "M" && width === 2) value = pad2(parts.month);
      else if (char === "M" && width === 3) throw new Error("MMM в timestamp-паттернах не используется");
      else if (char === "M") value = data.months_long[parts.month - 1];
      else if (char === "d" && width === 1) value = String(parts.day);
      else if (char === "d") value = pad2(parts.day);
      else if (char === "E") value = weekdays[parts.weekday];
      else if (char === "H" && width === 1) value = String(parts.hour);
      else if (char === "H") value = pad2(parts.hour);
      else if (char === "h" && width === 1) value = String(((parts.hour + 11) % 12) + 1);
      else if (char === "h") value = pad2(((parts.hour + 11) % 12) + 1);
      else if (char === "m" && width === 1) value = String(parts.minute);
      else if (char === "m") value = pad2(parts.minute);
      else if (char === "s" && width === 1) value = String(parts.second);
      else if (char === "s") value = pad2(parts.second);
      else if (char === "a") value = data.day_periods[parts.hour >= 12 ? 1 : 0];
      else throw new Error(`неподдерживаемый токен ${char.repeat(width)}`);
      out += value;
    } else {
      out += char;
      pos += 1;
    }
  }
  return normalize(out);
}

/**
 * @param {string} locale
 * @param {(typeof STYLES)[number]} style
 * @param {number} timestampSecs
 */
function icuFormat(locale, style, timestampSecs) {
  return normalize(
    new Intl.DateTimeFormat(locale, {
      .../** @type {Intl.DateTimeFormatOptions} */ (style.options),
      timeZone: "UTC",
    }).format(timestampSecs * 1000),
  );
}

/* ------------------------------------------------------------------ */
/* 5. Генерация паттернов + полная самопроверка матрицы 31×6×3.        */
/* ------------------------------------------------------------------ */

/** @type {Record<string, {patterns: string[], weekdays: string[]}>} */
const localeData = {};
for (const locale of enabled) {
  localeData[locale] = {
    patterns: STYLES.map((style) => derivePattern(locale, style)),
    weekdays: weekdayNamesInDate(locale),
  };
}

let checked = 0;
for (const locale of enabled) {
  const data = formatData[locale];
  const days = localeData[locale].weekdays;
  STYLES.forEach((style, index) => {
    const pattern = localeData[locale].patterns[index];
    for (const timestamp of MOMENTS) {
      const expected = icuFormat(locale, style, timestamp);
      const actual = renderPattern(pattern, dateParts(timestamp, data.year_offset), data, days);
      checked += 1;
      if (expected !== actual) {
        throw new Error(
          `самопроверка не прошла: ${locale}/${style.letter}/${timestamp}\n` +
            `  pattern  = ${JSON.stringify(pattern)}\n` +
            `  ICU      = ${JSON.stringify(expected)}\n` +
            `  рендер   = ${JSON.stringify(actual)}`,
        );
      }
    }
  });
  for (const pattern of localeData[locale].patterns) {
    if (/[{}]/.test(pattern)) throw new Error(`${locale}: плейсхолдер в паттерне ${pattern}`);
  }
}
if (checked !== 31 * 6 * 3) throw new Error(`сверено ${checked} строк вместо 558`);
if (!enabled.some((locale) => localeData[locale].patterns.some((p) => /[\u00a0\u202f]/.test(p)))) {
  throw new Error("в паттернах нет NBSP/NNBSP — нормализация в format.rs не проверяется");
}

/* ------------------------------------------------------------------ */
/* 6. R-ладдер и относительный оракул.                                */
/* ------------------------------------------------------------------ */

const DAY = 86_400n;
const LADDER = [
  { unit: "year", seconds: 365n * DAY },
  { unit: "month", seconds: 30n * DAY },
  { unit: "week", seconds: 7n * DAY },
  { unit: "day", seconds: DAY },
  { unit: "hour", seconds: 3600n },
  { unit: "minute", seconds: 60n },
  { unit: "second", seconds: 1n },
];

/** @param {bigint} delta */
function ladder(delta) {
  const magnitude = delta < 0n ? -delta : delta;
  for (const step of LADDER) {
    if (magnitude >= step.seconds) {
      return { unit: step.unit, value: delta / step.seconds };
    }
  }
  return { unit: "second", value: delta };
}

/** @param {string} locale @param {bigint} delta */
function icuRelative(locale, delta) {
  const { unit, value } = ladder(delta);
  const formatter = new Intl.RelativeTimeFormat(locale, { numeric: "auto", style: "long" });
  const expected = normalize(formatter.format(Number(value), /** @type {Intl.RelativeTimeFormatUnit} */ (unit)));
  return { unit, value, expected };
}

const RELATIVE_LOCALES = ["en-US", "ru", "de", "ja"];
const RELATIVE_MAGNITUDES = [
  30n,
  5n * 60n,
  3n * 3600n,
  2n * DAY,
  3n * 7n * DAY,
  5n * 30n * DAY,
  2n * 365n * DAY,
];
const NOW = 1_800_000_000;

const relativeCases = [];
for (const locale of RELATIVE_LOCALES) {
  for (const magnitude of RELATIVE_MAGNITUDES) {
    for (const delta of [magnitude, -magnitude]) {
      const { unit, value, expected } = icuRelative(locale, delta);
      relativeCases.push({
        locale,
        delta: Number(delta),
        unit,
        value: Number(value),
        expected,
      });
    }
  }
  const zero = icuRelative(locale, 0n);
  relativeCases.push({ locale, delta: 0, unit: zero.unit, value: Number(zero.value), expected: zero.expected });
}

const LADDER_DELTAS = [
  0n,
  59n,
  -59n,
  60n,
  -60n,
  119n,
  3599n,
  3600n,
  -3600n,
  7199n,
  86_399n,
  DAY,
  2n * DAY,
  604_799n,
  7n * DAY,
  3n * 7n * DAY,
  2_591_999n,
  30n * DAY,
  5n * 30n * DAY,
  31_535_999n,
  365n * DAY,
  -365n * DAY,
  2n * 365n * DAY,
  -2n * 365n * DAY,
];
const ladderCases = LADDER_DELTAS.map((delta) => {
  const { unit, value, expected } = icuRelative("en-US", delta);
  return {
    delta: Number(delta),
    now: NOW,
    timestamp: NOW + Number(delta),
    unit,
    value: Number(value),
    expected,
  };
});

const I64_MIN = -(2n ** 63n);
const I64_MAX = 2n ** 63n - 1n;
const saturatingSubtract = (/** @type {bigint} */ left, /** @type {bigint} */ right) => {
  const result = left - right;
  return result < I64_MIN ? I64_MIN : result > I64_MAX ? I64_MAX : result;
};
const extremeCases = [
  { timestamp: I64_MIN, now: I64_MAX },
  { timestamp: I64_MAX, now: I64_MIN },
].map(({ timestamp, now }) => {
  const { unit, value, expected } = icuRelative("en-US", saturatingSubtract(timestamp, now));
  return {
    locale: "en-US",
    style: "R",
    timestamp: timestamp.toString(),
    now: now.toString(),
    unit,
    value: value.toString(),
    expected,
  };
});

/* ------------------------------------------------------------------ */
/* 7. Запись `timestamp_data.rs`.                                     */
/* ------------------------------------------------------------------ */

const rustString = (/** @type {string} */ value) => JSON.stringify(value);
// Таблица сортируется по коду локали: `timestamp.rs` ищет строку binary_search'ем.
// Порядок `SUPPORTED_LOCALES` в locale.rs для этого не обязан быть отсортированным.
const sortedLocales = [...enabled].sort();
if (new Set(sortedLocales).size !== sortedLocales.length) {
  throw new Error("дубликаты локалей в locale.rs");
}

let rust = `//! Сгенерированные CLDR-паттерны Discord-таймстемпов \`<t:…>\` (P7-01e).
//!
//! Источник: Node ${process.version} / ICU ${process.versions.icu}, \`Intl.DateTimeFormat\`
//! с \`timeZone: "UTC"\`, скрипт \`waves/RW-29/gen-timestamp-data.mjs\`.
//! **Ручная правка запрещена.** 31 включённая локаль × 6 стилей (\`t T d D f F\`) = 186.

/// Данные одной локали: паттерны в порядке \`t T d D f F\` и длинные названия дней
/// недели в контексте даты (в финском эссив \`lauantaina\`, а не \`lauantai\`).
pub(crate) struct TimestampLocaleData {
    pub locale: &'static str,
    pub patterns: [&'static str; 6],
    pub weekdays_long: [&'static str; 7],
}

#[rustfmt::skip]
pub(crate) static TIMESTAMP_LOCALES: &[TimestampLocaleData] = &[
`;
for (const locale of sortedLocales) {
  const data = localeData[locale];
  rust += `    TimestampLocaleData {
        locale: ${rustString(locale)},
        patterns: [${data.patterns.map(rustString).join(", ")}],
        weekdays_long: [${data.weekdays.map(rustString).join(", ")}],
    },
`;
}
rust += `];
`;

/* ------------------------------------------------------------------ */
/* 8. Запись оракула и дубликатов.                                    */
/* ------------------------------------------------------------------ */

const cases = [];
for (const locale of sortedLocales) {
  STYLES.forEach((style, index) => {
    for (const timestamp of MOMENTS) {
      cases.push({
        locale,
        style: style.letter,
        timestamp,
        expected: icuFormat(locale, style, timestamp),
      });
    }
  });
}
if (cases.length !== 31 * 6 * 3) throw new Error(`кейсов ${cases.length} вместо 558`);

const oracle = {
  source: {
    runtime: `Node ${process.version}`,
    icu: process.versions.icu,
    formatter: "Intl.DateTimeFormat / Intl.RelativeTimeFormat, timeZone: \"UTC\"",
    generator: "waves/RW-29/gen-timestamp-data.mjs",
    normalization: "U+202F/U+00A0 -> U+0020",
  },
  styles: STYLES.map((style) => style.letter),
  locales: sortedLocales,
  moments: MOMENTS,
  cases,
  relative: {
    locales: RELATIVE_LOCALES,
    now: NOW,
    cases: relativeCases,
  },
  ladder: ladderCases,
  extreme: extremeCases,
};

const oracleText = `${JSON.stringify(oracle, null, 2)}\n`;

const outputs = [
  [join(crate, "src/timestamp_data.rs"), rust],
  [join(crate, "tests/data/timestamp-oracle.json"), oracleText],
  [join(here, "generated/timestamp_data.rs"), rust],
  [join(here, "generated/timestamp-oracle.json"), oracleText],
];
for (const [path, text] of outputs) {
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, text);
}

console.log(
  `ok: ${enabled.length} локалей × ${STYLES.length} стилей, ` +
    `${cases.length} строк матрицы, ${relativeCases.length} R-кейсов, ` +
    `${ladderCases.length} границ ладдера, ${extremeCases.length} экстремумов`,
);
