#!/usr/bin/env node
// @ts-check
/**
 * Generate the two independent i18n oracles for RW-24 / P7-01.
 *
 * Source is the local Node ICU (`Intl.*`), offline, timeZone UTC. No network.
 * Outputs:
 *   - rust/crates/discord-i18n/src/format_data.rs
 *   - rust/crates/discord-i18n/tests/data/cldr-oracle.json
 *
 * Usage: node waves/RW-24/gen-format-data.mjs
 */
import { writeFileSync, mkdirSync } from "node:fs";
import { dirname, join } from "node:path";

const LOCALES = [
  "en-US", "en-GB", "zh-CN", "zh-TW", "cs", "da", "nl", "fr", "de", "el",
  "hu", "it", "ja", "ko", "pl", "pt-PT", "pt-BR", "ru", "sk", "es-419",
  "es-ES", "sv-SE", "tr", "bg", "uk", "fi", "no", "hr", "ro", "lt", "th",
  "vi", "hi", "he", "ar", "id",
];
const STYLES = /** @type {const} */ (["short", "medium", "long", "full"]);
const SAMPLE = new Date(Date.UTC(2024, 3, 5, 14, 7, 9));
const SAMPLE2 = new Date(Date.UTC(2024, 0, 2, 9, 7, 9));
const TIMESTAMP = Math.floor(SAMPLE.getTime() / 1000);
const TIMESTAMP2 = Math.floor(SAMPLE2.getTime() / 1000);
const PLURAL_VALUES = [
  0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 19, 20, 21, 22, 23,
  24, 25, 30, 40, 50, 60, 70, 80, 90, 100, 101, 102, 103, 110, 111, 112, 120,
  200, 800, 1000, 1001, 10_000, 100_000, 1_000_000, 2_000_000, 10_000_000,
  0.1, 0.5, 1.1, 1.5, 1.9, 2.1, 2.5, 10.5, 1_000_000.5,
];
const NUMBER_VALUES = [0, 1, 1.5, 1234, 1234567, 1234567.89];

/** @param {string} locale @param {string} style */
function datePattern(locale, style) {
  const parts = new Intl.DateTimeFormat(locale, { dateStyle: style, timeZone: "UTC" })
    .formatToParts(SAMPLE);
  let out = "";
  for (const part of parts) {
    if (part.type === "literal") {
      out += `'${part.value.replace(/'/g, "''")}'`;
      continue;
    }
    let token = "";
    if (part.type === "year") token = part.value.length === 2 ? "yy" : "y";
    else if (part.type === "month") {
      if (style === "short" || /^\d+$/.test(part.value)) {
        token = part.value.length === 2 ? "MM" : "M";
      } else token = style === "medium" ? "MMM" : "MMMM";
    } else if (part.type === "day") token = part.value.length === 2 ? "dd" : "d";
    else if (part.type === "weekday") token = "EEEE";
    else if (part.type === "era") token = "G";
    if (token) out += token;
    else if (part.type !== "literal") out += `'${part.value.replace(/'/g, "''")}'`;
  }
  return out;
}

/** @param {string} locale @param {string} style */
function timePattern(locale, style) {
  const parts = new Intl.DateTimeFormat(locale, { timeStyle: style, timeZone: "UTC" })
    .formatToParts(SAMPLE2);
  const hasDayPeriod = parts.some((p) => p.type === "dayPeriod");
  let out = "";
  for (const part of parts) {
    if (part.type === "literal") {
      out += `'${part.value.replace(/'/g, "''")}'`;
      continue;
    }
    let token = "";
    if (part.type === "hour") {
      token = hasDayPeriod
        ? part.value.length === 2 ? "hh" : "h"
        : part.value.length === 2 ? "HH" : "H";
    } else if (part.type === "minute") token = part.value.length === 2 ? "mm" : "m";
    else if (part.type === "second") token = part.value.length === 2 ? "ss" : "s";
    else if (part.type === "dayPeriod") token = "a";
    else if (part.type === "timeZoneName") token = part.value.length <= 4 ? "z" : "zzzz";
    if (token) out += token;
    else if (part.type !== "literal") out += `'${part.value.replace(/'/g, "''")}'`;
  }
  return out;
}

/** @param {string} locale @param {number} month @param {string} width */
function monthPart(locale, month, width) {
  const d = new Date(Date.UTC(2024, month, 15, 12));
  return new Intl.DateTimeFormat(locale, { month: width, day: "numeric", timeZone: "UTC" })
    .formatToParts(d).find((p) => p.type === "month")?.value ?? "";
}

/** @param {string} locale @param {number} offset @param {string} width */
function weekdayPart(locale, offset, width) {
  const d = new Date(Date.UTC(2024, 3, 7 + offset, 12));
  return new Intl.DateTimeFormat(locale, { weekday: width, timeZone: "UTC" })
    .formatToParts(d).find((p) => p.type === "weekday")?.value ?? "";
}

/** @param {string} locale @param {number} hour */
function dayPeriod(locale, hour) {
  const d = new Date(Date.UTC(2024, 3, 5, hour, 7));
  return new Intl.DateTimeFormat(locale, { hour: "numeric", minute: "numeric", timeZone: "UTC" })
    .formatToParts(d).find((p) => p.type === "dayPeriod")?.value ?? "";
}

/** @param {string} locale @param {string} style */
function timeZoneName(locale, style) {
  return new Intl.DateTimeFormat(locale, { timeStyle: style, timeZone: "UTC" })
    .formatToParts(SAMPLE).find((p) => p.type === "timeZoneName")?.value ?? "";
}

/** @param {string} locale */
function era(locale) {
  return new Intl.DateTimeFormat(locale, { dateStyle: "full", timeZone: "UTC" })
    .formatToParts(SAMPLE).find((p) => p.type === "era")?.value ?? "";
}

/** @param {string} locale @param {string} style */
function year(locale, style) {
  return new Intl.DateTimeFormat(locale, { dateStyle: style, timeZone: "UTC" })
    .formatToParts(SAMPLE).find((p) => p.type === "year")?.value ?? "";
}

/** @param {string} locale */
function numberData(locale) {
  const parts = new Intl.NumberFormat(locale).formatToParts(1234567890.5);
  const group = parts.find((p) => p.type === "group")?.value ?? "";
  const decimal = parts.find((p) => p.type === "decimal")?.value ?? ".";
  const integers = parts.filter((p) => p.type === "integer").map((p) => p.value);
  let primary = 3;
  let secondary = 3;
  if (integers.length > 1) {
    primary = integers[integers.length - 1].length;
    secondary = integers[integers.length - 2].length;
  }
  return { group, decimal, primary, secondary };
}

const data = Object.fromEntries(LOCALES.map((locale) => {
  const datePatterns = Object.fromEntries(STYLES.map((s) => [s, datePattern(locale, s)]));
  const timePatterns = Object.fromEntries(STYLES.map((s) => [s, timePattern(locale, s)]));
  const monthsShort = Array.from({ length: 12 }, (_, m) => monthPart(locale, m, "short"));
  const monthsLong = Array.from({ length: 12 }, (_, m) => monthPart(locale, m, "long"));
  const weekdaysShort = Array.from({ length: 7 }, (_, i) => weekdayPart(locale, i, "short"));
  const weekdaysLong = Array.from({ length: 7 }, (_, i) => weekdayPart(locale, i, "long"));
  const eraValue = era(locale);
  const yearValue = Number.parseInt(year(locale, "long"), 10);
  return [locale, {
    datePatterns, timePatterns, monthsShort, monthsLong, weekdaysShort, weekdaysLong,
    dayPeriods: [dayPeriod(locale, 9), dayPeriod(locale, 21)],
    timezoneShort: timeZoneName(locale, "long"),
    timezoneLong: timeZoneName(locale, "full"),
    era: eraValue,
    yearOffset: eraValue ? yearValue - 2024 : 0,
    ...numberData(locale),
  }];
}));

const root = process.cwd();
const rustPath = join(root, "rust/crates/discord-i18n/src/format_data.rs");
const oraclePath = join(root, "rust/crates/discord-i18n/tests/data/cldr-oracle.json");
mkdirSync(dirname(oraclePath), { recursive: true });

const q = (/** @type {string} */ s) => JSON.stringify(s);
const ident = (/** @type {string} */ s) => `D_${s.replace(/[^A-Za-z0-9]/g, "_").toUpperCase()}`;
let rs = `//! Generated CLDR-like formatting data for the 36 Discord locales.\n//!\n//! Source: Node \`Intl.DateTimeFormat\` / \`Intl.NumberFormat\` with \`timeZone: \"UTC\"\`\n//! (offline ICU), sample 2024-04-05T14:07:09Z. Do not edit by hand: regenerate with\n//! \`waves/RW-24/gen-format-data.mjs\`.\n\n#[derive(Clone, Copy)]\npub(crate) struct LocaleFormatData {\n    pub date_patterns: [&'static str; 4],\n    pub time_patterns: [&'static str; 4],\n    pub months_short: [&'static str; 12],\n    pub months_long: [&'static str; 12],\n    pub weekdays_short: [&'static str; 7],\n    pub weekdays_long: [&'static str; 7],\n    pub day_periods: [&'static str; 2],\n    pub timezone_short: &'static str,\n    pub timezone_long: &'static str,\n    pub era: &'static str,\n    pub year_offset: i32,\n    pub group: &'static str,\n    pub decimal: &'static str,\n    pub group_primary: u8,\n    pub group_secondary: u8,\n}\n\n`;
for (const locale of LOCALES) {
  const d = data[locale];
  rs += `static ${ident(locale)}: LocaleFormatData = LocaleFormatData {\n`;
  rs += `    date_patterns: [${STYLES.map((s) => q(d.datePatterns[s])).join(", ")}],\n`;
  rs += `    time_patterns: [${STYLES.map((s) => q(d.timePatterns[s])).join(", ")}],\n`;
  rs += `    months_short: [${d.monthsShort.map(q).join(", ")}],\n`;
  rs += `    months_long: [${d.monthsLong.map(q).join(", ")}],\n`;
  rs += `    weekdays_short: [${d.weekdaysShort.map(q).join(", ")}],\n`;
  rs += `    weekdays_long: [${d.weekdaysLong.map(q).join(", ")}],\n`;
  rs += `    day_periods: [${d.dayPeriods.map(q).join(", ")}],\n`;
  rs += `    timezone_short: ${q(d.timezoneShort)},\n`;
  rs += `    timezone_long: ${q(d.timezoneLong)},\n`;
  rs += `    era: ${q(d.era)},\n`;
  rs += `    year_offset: ${d.yearOffset},\n`;
  rs += `    group: ${q(d.group)},\n`;
  rs += `    decimal: ${q(d.decimal)},\n`;
  rs += `    group_primary: ${d.primary},\n`;
  rs += `    group_secondary: ${d.secondary},\n`;
  rs += `};\n\n`;
}
rs += `pub(crate) fn data(locale: &str) -> &'static LocaleFormatData {\n    match locale {\n`;
for (const locale of LOCALES) rs += `        ${q(locale)} => &${ident(locale)},\n`;
rs += `        _ => &${ident("en-US")},\n    }\n}\n`;
writeFileSync(rustPath, rs);

const oracle = {
  generatedAt: new Date().toISOString(),
  source: "Node Intl (offline ICU), timeZone UTC",
  timestamp: TIMESTAMP,
  timestamp2: TIMESTAMP2,
  locales: Object.fromEntries(LOCALES.map((locale) => {
    const cardinal = Object.fromEntries(PLURAL_VALUES.map((v) => [
      String(v), new Intl.PluralRules(locale, { type: "cardinal" }).select(v),
    ]));
    const ordinal = Object.fromEntries(PLURAL_VALUES.map((v) => [
      String(v), new Intl.PluralRules(locale, { type: "ordinal" }).select(v),
    ]));
    const number = Object.fromEntries(NUMBER_VALUES.map((v) => [
      String(v), new Intl.NumberFormat(locale).format(v),
    ]));
    const dates = Object.fromEntries(STYLES.map((s) => [
      s, new Intl.DateTimeFormat(locale, { dateStyle: s, timeZone: "UTC" }).format(SAMPLE),
    ]));
    const times = Object.fromEntries(STYLES.map((s) => [
      s, new Intl.DateTimeFormat(locale, { timeStyle: s, timeZone: "UTC" }).format(SAMPLE),
    ]));
    const dates2 = Object.fromEntries(STYLES.map((s) => [
      s, new Intl.DateTimeFormat(locale, { dateStyle: s, timeZone: "UTC" }).format(SAMPLE2),
    ]));
    const times2 = Object.fromEntries(STYLES.map((s) => [
      s, new Intl.DateTimeFormat(locale, { timeStyle: s, timeZone: "UTC" }).format(SAMPLE2),
    ]));
    return [locale, { plural: { cardinal, ordinal }, number, dates, times, dates2, times2 }];
  })),
};
writeFileSync(oraclePath, `${JSON.stringify(oracle, null, 1)}\n`);
console.log(`wrote ${rustPath}`);
console.log(`wrote ${oraclePath}`);
