#!/usr/bin/env node
// Генератор данных P7-01d Intl.DurationFormat (RW-30).
//
// Источник: Node 24 ICU (Intl.DurationFormat + Intl.PluralRules + Intl.NumberFormat).
// Пишет данные прямо в крейт (`src/duration_data.rs`, `tests/data/duration-oracle.json`)
// и дублирует их в `waves/RW-30/generated/` для аудита.
//
// Модель (заморожена контрактом `waves/RW-30/TZ-W30a.md` §1):
//   вывод = unit1 [join2 | middle/last] unit2 …, где текст единицы — шаблон
//   `{0}` + плюральная категория; нули опускаются, все нули → "".
// Гипотеза регулярной склейки проверяется на 200 случайных длительностях на
// локаль/стиль (при расхождении — падение генератора, а не «подгонка» данных).
//
// Ручная правка сгенерированных файлов запрещена: FIXED seed ⇒ те же md5.
import { readFileSync, writeFileSync, mkdirSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { createHash } from "node:crypto";

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, "..", "..");
const crate = join(root, "rust/crates/discord-i18n");
const localeSrc = readFileSync(join(crate, "src/locale.rs"), "utf8");

if (typeof Intl.DurationFormat !== "function") {
  throw new Error("Intl.DurationFormat недоступен: нужен Node 24 с полным ICU");
}

const list = (name) => {
  const m = localeSrc.match(new RegExp(`${name}: \\[&str; \\d+\\] = \\[([^\\]]+)\\]`));
  if (!m) throw new Error(`не найден ${name}`);
  return [...m[1].matchAll(/"([^"]+)"/g)].map((x) => x[1]);
};

const enabled = list("SUPPORTED_LOCALES").filter((c) => !list("DISABLED_LOCALES").includes(c));
if (enabled.length !== 31) throw new Error(`ожидалось 31 локаль, получено ${enabled.length}`);

// Порядок единиц — контракт `format_duration`: years → milliseconds.
const UNITS = [
  ["year", "years"],
  ["month", "months"],
  ["week", "weeks"],
  ["day", "days"],
  ["hour", "hours"],
  ["minute", "minutes"],
  ["second", "seconds"],
  ["millisecond", "milliseconds"],
];
const STYLES = ["long", "short", "narrow"];
const CATS = ["zero", "one", "two", "few", "many", "other"];
// Значения из ТЗ §2: покрытие CLDR-категорий, проверяется Intl.PluralRules.
const SAMPLE_VALUES = [1, 2, 5, 11, 21, 101];
// Нормализация как в `format.rs::render_pattern`: NBSP/NNBSP → обычный пробел.
const normalize = (value) => value.replaceAll("\u202f", " ").replaceAll("\u00a0", " ");

const rs = (value) =>
  `"${[...value]
    .map((ch) => {
      const cp = ch.codePointAt(0);
      if (ch === '"') return '\\"';
      if (ch === "\\") return "\\\\";
      if (cp === 0x0a) return "\\n";
      if (cp === 0x0d) return "\\r";
      if (cp === 0x09) return "\\t";
      if (cp < 0x20 || cp === 0x7f) return `\\u{${cp.toString(16)}}`;
      return ch;
    })
    .join("")}"`;

const durationObject = (unitField, value) => ({ [unitField]: value });
const durationFrom = (fields) =>
  Object.fromEntries(fields.map(([unitField, value]) => [unitField, value]));

function formatter(locale, style) {
  // display: "auto" опускает нули (в ICU 78 это и так дефолт — фиксируем явно,
  // чтобы контракт не зависел от изменения дефолта).
  return new Intl.DurationFormat(locale, { style, display: "auto" });
}

// Числовой шаблон единицы: numeric-части (integer/group/…) заменяются одним `{0}`.
function unitTemplate(fmt, field, value, where) {
  const parts = fmt.formatToParts(durationObject(field, value));
  let out = "";
  let placeholder = false;
  for (const part of parts) {
    if (part.type === "integer" || part.type === "group") {
      if (!placeholder) {
        out += "{0}";
        placeholder = true;
      }
      continue;
    }
    out += part.value;
  }
  if (!placeholder) throw new Error(`${where}: нет integer/group в formatToParts`);
  if (out.includes("{0}{0}")) throw new Error(`${where}: два числовых плейсхолдера`);
  return out;
}

// Числовая часть вывода (integer+group) — независимая сверка группировки.
function numericText(fmt, field, value) {
  return fmt
    .formatToParts(durationObject(field, value))
    .filter((part) => part.type === "integer" || part.type === "group")
    .map((part) => part.value)
    .join("");
}

function unassignedLiterals(parts) {
  // Литералы разделителей не имеют атрибута `unit`; литералы внутри шаблона единицы — имеют.
  return parts.filter((part) => part.unit === undefined).map((part) => part.value);
}

function extractJoins(locale, style) {
  const fmt = formatter(locale, style);
  const pair = fmt.formatToParts(durationFrom([["days", 2], ["hours", 3]]));
  const triple = fmt.formatToParts(durationFrom([["days", 2], ["hours", 3], ["minutes", 4]]));
  const pairSeps = unassignedLiterals(pair);
  const tripleSeps = unassignedLiterals(triple);
  if (pairSeps.length > 1) {
    throw new Error(`${locale}/${style}: >1 разделителя в паре: ${JSON.stringify(pairSeps)}`);
  }
  if (tripleSeps.length > 2) {
    throw new Error(`${locale}/${style}: >2 разделителей в тройке: ${JSON.stringify(tripleSeps)}`);
  }
  const joins = {
    join2: pairSeps[0] ?? "",
    joinMiddle: tripleSeps[0] ?? "",
    joinLast: tripleSeps[1] ?? "",
  };
  for (const [name, value] of Object.entries(joins)) {
    if (value.includes("{") || value.includes("}")) {
      throw new Error(`${locale}/${style}: ${name} содержит плейсхолдер: ${JSON.stringify(value)}`);
    }
  }
  return joins;
}

// Детерминированный PRNG (mulberry32): повторный запуск даёт те же md5.
function mulberry32(seed) {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

const categoriesByLocale = new Map();
for (const locale of enabled) {
  const pr = new Intl.PluralRules(locale);
  const samples = new Map();
  let extra = 0;
  for (const value of SAMPLE_VALUES) {
    const cat = pr.select(value);
    if (!samples.has(cat)) samples.set(cat, value);
  }
  // Дополняем категории, недостижимые значениями из ТЗ (романские `many` = 10^6),
  // иначе длительность 1 000 000 единиц получила бы неверную форму.
  const all = new Set();
  for (let n = 0; n <= 2000; n += 1) all.add(pr.select(n));
  for (const n of [1_000_000, 2_000_000, 10_000_000]) all.add(pr.select(n));
  for (const cat of all) {
    if (samples.has(cat)) continue;
    let found = null;
    for (let n = 0; n <= 2_000_000 && found === null; n += 1) {
      if (pr.select(n) === cat) found = n;
    }
    for (const n of [1_000_000, 2_000_000, 10_000_000]) {
      if (found === null && pr.select(n) === cat) found = n;
    }
    if (found === null) throw new Error(`${locale}: нет сэмпла для категории ${cat}`);
    samples.set(cat, found);
    extra += 1;
  }
  // `other` недостижим для целых в части локалей (pl: one/few/many); это нормально —
  // нулевые единицы не форматируются, дробных значений в контракте нет.
  if (samples.size === 0) throw new Error(`${locale}: PluralRules не даёт категорий`);
  categoriesByLocale.set(locale, samples);
  if (extra > 0) {
    console.log(`  ${locale}: +${extra} категорий (сэмплы ${JSON.stringify([...samples])})`);
  }
}

const rows = [];
const joinRows = [];
let checkedDurations = 0;

for (const locale of enabled) {
  const numberFormat = new Intl.NumberFormat(locale, { maximumFractionDigits: 0 });
  for (const style of STYLES) {
    const fmt = formatter(locale, style);
    const joins = extractJoins(locale, style);
    joinRows.push({ locale, style, ...joins });

    const samples = categoriesByLocale.get(locale);
    for (const [unit, field] of UNITS) {
      const templates = new Map();
      for (const [cat, value] of samples) {
        const where = `${locale}/${style}/${unit}/${cat}(${value})`;
        const template = unitTemplate(fmt, field, value, where);
        // Самопроверка: шаблон + группировка NumberFormat воспроизводят вывод ICU
        // (для сэмплов ≤ 101 / 10^6 группировка NumberFormat совпадает с DurationFormat).
        const rebuilt = template.replace("{0}", numberFormat.format(value));
        const actual = fmt.format(durationObject(field, value));
        if (rebuilt !== actual) {
          throw new Error(`${where}: шаблон не воспроизводит вывод: ${JSON.stringify({ rebuilt, actual })}`);
        }
        const previous = templates.get(cat);
        if (previous !== undefined && previous !== template) {
          throw new Error(`${where}: шаблон категории нестабилен: ${JSON.stringify([previous, template])}`);
        }
        templates.set(cat, template);
      }
      for (const cat of CATS) {
        if (!templates.has(cat)) templates.set(cat, templates.get("other") ?? templates.get(CATS.find((name) => templates.has(name))));
      }
      rows.push({
        locale,
        style,
        unit,
        patterns: CATS.map((cat) => [cat, templates.get(cat)]),
      });
    }

    // Группировка за пределами 4 цифр должна совпадать с NumberFormat (включая
    // индийские primary=3/secondary=2): иначе ожидания для u64::MAX были бы выдуманы.
    for (const field of ["days", "milliseconds"]) {
      for (const value of [10_000, 100_000, 1_000_000, 100_000_000]) {
        const icu = numericText(fmt, field, value);
        const expected = numberFormat.format(value);
        if (icu !== expected) {
          throw new Error(
            `${locale}/${style}/${field}/${value}: группировка ${JSON.stringify(icu)} != ${JSON.stringify(expected)}`,
          );
        }
      }
    }

    // Проверка гипотезы регулярной склейки на 200 случайных длительностях.
    const rnd = mulberry32(0x5eed1234 ^ style.length ^ [...locale].reduce((a, c) => a * 31 + c.codePointAt(0), 7));
    for (let iteration = 0; iteration < 200; iteration += 1) {
      const count = 1 + Math.floor(rnd() * UNITS.length);
      const chosen = [];
      while (chosen.length < count) {
        const candidate = Math.floor(rnd() * UNITS.length);
        if (!chosen.includes(candidate)) chosen.push(candidate);
      }
      chosen.sort((a, b) => a - b);
      const fields = chosen.map((index) => [
        UNITS[index][1],
        1 + Math.floor(rnd() * 1_000_000),
      ]);
      const duration = durationFrom(fields);
      const actual = fmt.format(duration);
      let expected;
      if (fields.length === 1) {
        expected = fmt.format(duration);
      } else {
        const texts = fields.map(([field, value]) => fmt.format(durationObject(field, value)));
        expected = texts[0];
        for (let i = 1; i < texts.length; i += 1) {
          expected += i === texts.length - 1 ? joins.joinLast : joins.joinMiddle;
          expected += texts[i];
        }
        if (fields.length === 2) expected = texts[0] + joins.join2 + texts[1];
      }
      if (actual !== expected) {
        throw new Error(
          `${locale}/${style}: гипотеза склейки нарушена на ${JSON.stringify(duration)}: ` +
            `${JSON.stringify({ expected, actual })}`,
        );
      }
      checkedDurations += 1;
    }
  }
}

if (rows.length !== 744) throw new Error(`строк ${rows.length}, ожидалось 744`);
if (joinRows.length !== 93) throw new Error(`join-строк ${joinRows.length}, ожидалось 93`);

// Порядок строк — контракт binary_search: (locale, style, unit) побайтово.
const cmp = (x, y) => (x < y ? -1 : x > y ? 1 : 0);
rows.sort((a, b) => cmp(a.locale, b.locale) || cmp(a.style, b.style) || cmp(a.unit, b.unit));
joinRows.sort((a, b) => cmp(a.locale, b.locale) || cmp(a.style, b.style));

const dataLines = [
  "//! Сгенерированные данные `Intl.DurationFormat` (P7-01d).",
  "//!",
  `//! Источник: Node ${process.version} / ICU, скрипт \`waves/RW-30/gen-duration-data.mjs\`.`,
  "//! **Ручная правка запрещена.** Строки: 31 локаль × 3 стиля × 8 единиц = 744,",
  "//! паттерны: по 6 CLDR-категорий (zero/one/two/few/many/other) на строку.",
  "//! Отсутствующие у локали категории заполнены формой `other` (lookup их не выбирает).",
  "",
  "pub(crate) struct DurationRow {",
  "    pub locale: &'static str,",
  "    pub style: &'static str,",
  "    pub unit: &'static str,",
  '    pub patterns: &\'static [(&\'static str, &\'static str); 6],',
  "}",
  "",
  "pub(crate) struct DurationJoin {",
  "    pub locale: &'static str,",
  "    pub style: &'static str,",
  "    pub join2: &'static str,",
  "    pub join_middle: &'static str,",
  "    pub join_last: &'static str,",
  "}",
  "",
  "#[rustfmt::skip]",
  "pub(crate) static DURATION_ROWS: &[DurationRow] = &[",
];
for (const row of rows) {
  const patterns = row.patterns.map(([cat, text]) => `(${rs(cat)}, ${rs(text)})`).join(", ");
  dataLines.push("    DurationRow {");
  dataLines.push(`        locale: ${rs(row.locale)},`);
  dataLines.push(`        style: ${rs(row.style)},`);
  dataLines.push(`        unit: ${rs(row.unit)},`);
  dataLines.push(`        patterns: &[${patterns}],`);
  dataLines.push("    },");
}
dataLines.push("];", "");
dataLines.push("#[rustfmt::skip]");
dataLines.push("pub(crate) static DURATION_JOINS: &[DurationJoin] = &[");
for (const row of joinRows) {
  dataLines.push("    DurationJoin {");
  dataLines.push(`        locale: ${rs(row.locale)},`);
  dataLines.push(`        style: ${rs(row.style)},`);
  dataLines.push(`        join2: ${rs(row.join2)},`);
  dataLines.push(`        join_middle: ${rs(row.joinMiddle)},`);
  dataLines.push(`        join_last: ${rs(row.joinLast)},`);
  dataLines.push("    },");
}
dataLines.push("];", "");
const dataText = dataLines.join("\n");

// ---------------------------------------------------------------------------
// Оракул: 2000+ случайных длительностей (фиксированный seed) + краевые случаи.
// ---------------------------------------------------------------------------
const rnd = mulberry32(0x0d15ea5e);
const cases = [];
const addCase = (locale, style, duration, expected) => {
  cases.push({ locale, style, d: duration, expected: normalize(expected) });
};
const randomValue = () => 1 + Math.floor(rnd() * 1_000_000);

// Случайные: 24 на локаль/стиль = 2232 (> 2000), все 31×3 комбинации покрыты.
let randomCases = 0;
for (let iteration = 0; iteration < 24; iteration += 1) {
  for (const locale of enabled) {
    for (const style of STYLES) {
      const count = 1 + Math.floor(rnd() * UNITS.length);
      const chosen = [];
      while (chosen.length < count) {
        const candidate = Math.floor(rnd() * UNITS.length);
        if (!chosen.includes(candidate)) chosen.push(candidate);
      }
      chosen.sort((a, b) => a - b);
      const duration = {};
      for (const index of chosen) duration[UNITS[index][1]] = randomValue();
      const fmt = formatter(locale, style);
      addCase(locale, style, duration, fmt.format(duration));
      randomCases += 1;
    }
  }
}

// Краевые: все нули, u64::MAX, одиночные единицы 1/1000, значения 21/101.
const U64_MAX = 18_446_744_073_709_551_615n;
const U64_MAX_TEXT = U64_MAX.toString();
for (const locale of enabled) {
  const pr = new Intl.PluralRules(locale);
  const numberFormat = new Intl.NumberFormat(locale, { maximumFractionDigits: 0 });
  const maxGrouped = numberFormat.format(U64_MAX);
  // Категория u64::MAX: правила CLDR для целых зависят от i mod 10/100/1000/10^6,
  // поэтому берём точный остаток (f64-округление 2^64 его искажает).
  const remainder = Number(U64_MAX % 1_000_000n);
  const maxCategory = pr.select(remainder);
  if (pr.select(U64_MAX_TEXT) !== maxCategory) {
    throw new Error(`${locale}: категория u64::MAX нестабильна (${maxCategory} vs ${pr.select(U64_MAX_TEXT)})`);
  }
  for (const style of STYLES) {
    const fmt = formatter(locale, style);
    addCase(locale, style, {}, fmt.format(durationFrom(UNITS.map(([, field]) => [field, 0]))));
    const maxRow = rows.find((row) => row.locale === locale && row.style === style && row.unit === "day");
    const maxTemplate = maxRow.patterns.find(([cat]) => cat === maxCategory)[1];
    addCase(locale, style, { days: U64_MAX_TEXT }, maxTemplate.replace("{0}", maxGrouped));
    for (const [unit, field] of UNITS) {
      for (const value of [1, 1000]) {
        addCase(locale, style, durationObject(field, value), fmt.format(durationObject(field, value)));
      }
    }
    for (const value of [21, 101]) {
      addCase(locale, style, { days: value }, fmt.format({ days: value }));
    }
  }
}

if (randomCases < 2000) throw new Error(`случайных кейсов ${randomCases}, нужно ≥ 2000`);
const localesInCases = new Set(cases.map((item) => item.locale));
if (localesInCases.size !== 31) throw new Error(`локалей в оракуле ${localesInCases.size}`);

const oracle = {
  generatedBy: "waves/RW-30/gen-duration-data.mjs",
  node: process.version,
  normalization: "U+202F/U+00A0 → U+0020",
  locales: enabled,
  styles: STYLES,
  units: UNITS.map(([unit]) => unit),
  categories: CATS,
  counts: {
    rows: rows.length,
    joins: joinRows.length,
    patterns: rows.length * CATS.length,
    randomCases,
    edgeCases: cases.length - randomCases,
    cases: cases.length,
  },
  cases,
};
const oracleText = `${JSON.stringify(oracle)}\n`;

mkdirSync(join(here, "generated"), { recursive: true });
mkdirSync(join(crate, "tests/data"), { recursive: true });
for (const [relative, text] of [
  ["generated/duration_data.rs", dataText],
  ["generated/duration-oracle.json", oracleText],
]) {
  writeFileSync(join(here, relative), text);
}
writeFileSync(join(crate, "src/duration_data.rs"), dataText);
writeFileSync(join(crate, "tests/data/duration-oracle.json"), oracleText);

const md5 = (text) => createHash("md5").update(text).digest("hex");
console.log(
  `rows=${rows.length} joins=${joinRows.length} patterns=${rows.length * CATS.length} ` +
    `cases=${cases.length} random=${randomCases} joinChecks=${checkedDurations} ` +
    `locales=${enabled.length} node=${process.version}`,
);
console.log(`md5 duration_data.rs=${md5(dataText)} duration-oracle.json=${md5(oracleText)}`);
