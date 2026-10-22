#!/usr/bin/env node
// Генератор данных P7-01c RelativeTimeFormat (RW-28).
// Источник: Node 24 ICU (Intl.RelativeTimeFormat + Intl.PluralRules).
// Пишет данные прямо в крейт (`src/relative_data.rs`, `tests/data/relative-oracle.json`)
// и дублирует их в waves/RW-28/generated/ для аудита. Ручная правка запрещена.
import { readFileSync, writeFileSync, mkdirSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, "..", "..");
const localeSrc = readFileSync(join(root, "rust/crates/discord-i18n/src/locale.rs"), "utf8");

const list = (name) => {
  const m = localeSrc.match(new RegExp(`${name}: \\[&str; \\d+\\] = \\[([^\\]]+)\\]`));
  if (!m) throw new Error(`не найден ${name}`);
  return [...m[1].matchAll(/"([^"]+)"/g)].map((x) => x[1]);
};

const enabled = list("SUPPORTED_LOCALES").filter((c) => !list("DISABLED_LOCALES").includes(c));
if (enabled.length !== 31) throw new Error(`ожидалось 31 локаль, получено ${enabled.length}`);

const UNITS = ["year", "quarter", "month", "week", "day", "hour", "minute", "second"];
const STYLES = ["long", "short", "narrow"];
const CATS = ["zero", "one", "two", "few", "many", "other"];

const escapeTemplate = (s) => s.replaceAll("{", "{{").replaceAll("}", "}}");
const rs = (s) => JSON.stringify(s);

function templates(locale, unit, style, numeric, categorySamples) {
  const fmt = new Intl.RelativeTimeFormat(locale, { numeric, style });
  const out = { past: [], future: [] };
  for (const cat of CATS) {
    const raw = categorySamples[cat];
    if (raw === undefined) continue;
    const n = raw;
    for (const [dir, value] of [
      ["past", -n],
      ["future", n],
    ]) {
      const parts = fmt.formatToParts(value, unit);
      let hasInteger = false;
      let text = "";
      for (const p of parts) {
        if (p.type === "integer" || p.type === "group") {
          hasInteger = true;
          text += "{0}";
        } else {
          text += escapeTemplate(p.value);
        }
      }
      if (!hasInteger) throw new Error(`${locale} ${unit} ${style} ${numeric} ${value}: нет integer`);
      out[dir].push([cat, text]);
    }
  }
  return out;
}

function autoSpecial(locale, unit, style, value) {
  const parts = new Intl.RelativeTimeFormat(locale, { numeric: "auto", style }).formatToParts(
    value,
    unit,
  );
  if (parts.some((p) => p.type === "integer" || p.type === "group")) return null;
  return escapeTemplate(parts.map((p) => p.value).join(""));
}

function autoSpecials(locale, unit, style) {
  const out = [];
  for (let v = -10; v <= 10; v += 1) {
    const text = autoSpecial(locale, unit, style, v);
    if (text !== null) {
      if (v < -2 || v > 2) throw new Error(`спецформа вне [-2,2]: ${locale}/${unit}/${style}/${v}`);
      out.push([v, text]);
    }
  }
  return out;
}

const categorySamples = {};
for (const locale of enabled) {
  const pr = new Intl.PluralRules(locale);
  const samples = {};
  for (let n = 0; n <= 200; n += 1) {
    const cat = pr.select(n);
    if (samples[cat] === undefined) samples[cat] = n;
  }
  categorySamples[locale] = samples;
}

// В auto-режиме -1/0/1 дают спецформы без числа; для шаблонов берём число ≥2
// с той же категорией (plural-категория от величины не зависит).
function numericSample(locale, category, sample) {
  if (sample > 1) return sample;
  const pr = new Intl.PluralRules(locale);
  for (let n = 2; n <= 400; n += 1) if (pr.select(n) === category) return n;
  throw new Error(`${locale}: нет числа ≥2 для категории ${category}`);
}

const rows = [];
let cases = [];
for (const locale of enabled) {
  for (const unit of UNITS) {
    for (const style of STYLES) {
      const always = templates(locale, unit, style, "always", categorySamples[locale]);
      for (const v of [-6, 6, -5, 5]) {
        const a = new Intl.RelativeTimeFormat(locale, { numeric: "always", style }).format(v, unit);
        if (a !== new Intl.RelativeTimeFormat(locale, { numeric: "auto", style }).format(v, unit)) {
          throw new Error(`auto|v|>2 расходятся: ${locale}/${unit}/${style}/${v}`);
        }
      }
      rows.push({
        locale,
        unit,
        style,
        past: always.past,
        future: always.future,
        auto: autoSpecials(locale, unit, style),
      });
    }
  }
}
if (rows.length !== 744) throw new Error(`строк ${rows.length}, ожидалось 744`);
// Порядок строк — контракт binary_search в relative.rs: (locale, unit, style) побайтово.
const cmp = (x, y) => (x < y ? -1 : x > y ? 1 : 0);
rows.sort((a, b) => cmp(a.locale, b.locale) || cmp(a.unit, b.unit) || cmp(a.style, b.style));

const mkCases = (locales, units, styles, numerics, values) => {
  const out = [];
  for (const locale of locales)
    for (const unit of units)
      for (const style of styles)
        for (const numeric of numerics)
          for (const value of values) {
            const fmt = new Intl.RelativeTimeFormat(locale, { numeric, style });
            out.push({ locale, unit, style, numeric, value, expected: fmt.format(value, unit) });
          }
  return out;
};

const core = ["en-US", "ru", "de", "fr", "ja", "zh-CN", "es-ES", "pl", "tr", "uk"];
cases = cases.concat(mkCases(core, UNITS, STYLES, ["always", "auto"], [-2, -1, 0, 1, 2]));
cases = cases.concat(
  mkCases(enabled, ["year", "day", "hour", "minute"], ["long"], ["always", "auto"], [-2, -1, 0, 2]),
);

const cats = (pairs) =>
  `&[${pairs.map(([cat, text]) => `(${rs(cat)}, ${rs(text)})`).join(", ")}]`;
const opt = (v) => (v === null ? "None" : `Some(${rs(v)})`);
const lines = [
  "//! Сгенерированные данные `Intl.RelativeTimeFormat` (P7-01c).",
  "//!",
  `//! Источник: Node ${process.version} / ICU, скрипт \`waves/RW-28/gen-relative-data.mjs\`.`,
  "//! **Ручная правка запрещена.** Строки: 31 локаль × 8 единиц × 3 стиля = 744.",
  "",
  "pub(crate) struct RelativeRow {",
  "    pub locale: &'static str,",
  "    pub unit: &'static str,",
  "    pub style: &'static str,",
  "    pub past: &'static [(&'static str, &'static str)],",
  "    pub future: &'static [(&'static str, &'static str)],",
  "    pub auto: &'static [(i32, &'static str)],",
  "}",
  "",
  "#[rustfmt::skip]",
  "pub(crate) static RELATIVE_ROWS: &[RelativeRow] = &[",
];
for (const r of rows) {
  lines.push("    RelativeRow {");
  lines.push(`        locale: ${rs(r.locale)},`);
  lines.push(`        unit: ${rs(r.unit)},`);
  lines.push(`        style: ${rs(r.style)},`);
  lines.push(`        past: ${cats(r.past)},`);
  lines.push(`        future: ${cats(r.future)},`);
  lines.push(`        auto: &[${r.auto.map(([v, t]) => `(${v}, ${rs(t)})`).join(", ")}],`);
  lines.push("    },");
}
lines.push("];", "");

mkdirSync(join(here, "generated"), { recursive: true });
mkdirSync(join(root, "rust/crates/discord-i18n/tests/data"), { recursive: true });
writeFileSync(join(here, "generated/relative_data.rs"), lines.join("\n"));
writeFileSync(
  join(here, "generated/relative-oracle.json"),
  `${JSON.stringify({ locales: enabled, cases }, null, 0)}\n`,
);
writeFileSync(join(root, "rust/crates/discord-i18n/src/relative_data.rs"), lines.join("\n"));
writeFileSync(
  join(root, "rust/crates/discord-i18n/tests/data/relative-oracle.json"),
  `${JSON.stringify({ locales: enabled, cases }, null, 0)}\n`,
);
console.log(
  `rows=${rows.length} cases=${cases.length} locales=${enabled.length} categories=${JSON.stringify(
    Object.fromEntries(enabled.slice(0, 4).map((l) => [l, Object.keys(categorySamples[l])])),
  )}`,
);
