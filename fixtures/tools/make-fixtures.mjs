// @ts-check
// Сборка курированных конформанс-фикстур: копирует минимальный срез из лаборатории
// (/home/sanya/re-deep или $RE_LAB) в fixtures/, вырезает значения чувствительных ключей
// (политика санитизации репозитория) и строит fixtures/MANIFEST.json с md5-провенансом.
// Точечная очистка: перезаписываются только копируемые пути (README.md и tools/ не трогаются).
//
// Запуск: node fixtures/tools/make-fixtures.mjs
// Проверка после сборки: node docs/rust-client/verify/verify-re.mjs
import { createHash } from "node:crypto";
import {
  cpSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const HERE = dirname(fileURLToPath(import.meta.url));
const REPO = resolve(HERE, "..", "..");
const SRC = process.env.RE_LAB ?? "/home/sanya/re-deep";
const DST = join(REPO, "fixtures");

/** Ключи, значения которых в репозиторий не попадают (вырезаются до REDACTED). */
const SENSITIVE_KEYS = [
  "analytics_token",
  "session_id",
  "auth_session_id_hash",
  "static_client_session_id",
  "qos_token",
  "resume_gateway_url",
];

/** Файловые фикстуры: [путь в репо, путь в лаборатории, назначение, тесты]. */
const FILES = [
  ["gateway/ready-stats.json", "gateway/ready-stats.json",
    "Счётчики READY: 40 ключей, 7 гильдий, 17 ЛС, 60 связей", "T-GW-04, T-DATA-02"],
  ["gateway/summary.json", "gateway/summary.json",
    "Сводка кадров одного сокета (op/t/s/размеры)", "T-GW-01, T-ENC-04"],
  ["gw-proto/facts-frozen.json", "gw-proto/facts-frozen.json",
    "Замороженные факты волны: ETF-теги (11), счётчики сокетов, READY-числа", "T-ENC-02, T-GW-08"],
  ["gw-proto/msgs-run2-95974_13072.jsonl", "gw-proto/msgs-run2-95974_13072.jsonl",
    "Декодированные кадры одного сокета (1544 всего в 7 файлах)", "T-ENC-04, T-GW-02"],
  ["rest/rest-static.json", "rest-static.json",
    "Статический реестр REST-эндпоинтов из веб-бандла (904)", "T-REST-01, T-REST-09"],
  ["i18n/i18n-stats.json", "i18n-stats.json",
    "Статистика i18n: 23 836 ru-ключей, рантайм-модули, локали", "T-I18N-01, T-I18N-07"],
  ["voice/rtc-frames.jsonl", "voice/rtc-frames.jsonl",
    "Кадры RTC-сигнализации (сессия 2)", "T-VOICE-01"],
  ["voice/rtc-summary.json", "voice/rtc-summary.json",
    "Сводка RTC-кадров (ops/ключи/длины)", "T-VOICE-01, T-VOICE-03"],
  ["voice/rtc1-frames.jsonl", "voice/rtc1-frames.jsonl",
    "Кадры RTC-сигнализации (сессия 1)", "T-VOICE-01"],
  ["voice/rtc1-summary.json", "voice/rtc1-summary.json",
    "Сводка RTC-кадров сессии 1", "T-VOICE-02, T-VOICE-03"],
  ["voice/session3.timing", "voice/session3.timing",
    "Тайминги сессии 3 (ротация медиа-сокета)", "T-VOICE-06"],
];

/** Каталожные фикстуры: [репо, лаборатория, назначение, тесты]. */
const DIRS = [
  ["gateway/by-type", "gateway/by-type",
    "Декодированные диспатчи по типам (164 файла, 12 типов)", "T-GW-08, T-DATA-06"],
];

/** @param {string} p @returns {string} */
const md5 = (p) => createHash("md5").update(readFileSync(p)).digest("hex");
/** @param {string} text @returns {string} */
const md5text = (text) => createHash("md5").update(text).digest("hex");

/** @param {string} text @returns {string} */
const sanitizeText = (text) => {
  let out = text;
  for (const key of SENSITIVE_KEYS) {
    out = out.replace(new RegExp(`("${key}":")([^"]*)(")`, "g"), "$1REDACTED$3");
  }
  return out.replace(/("email":")([^"@]*@[^"]*)(")/g, "$1REDACTED$3");
};

/** @param {string} dir */
const sanitizeTree = (dir) => {
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const p = join(dir, entry.name);
    if (entry.isDirectory()) sanitizeTree(p);
    else if (/\.(json|jsonl|md|txt)$/.test(entry.name)) {
      const text = readFileSync(p, "utf8");
      const clean = sanitizeText(text);
      if (clean !== text) writeFileSync(p, clean);
    }
  }
};

/** Скан собранных фикстур: токены/чувствительные значения не проходят. @param {string} dir */
const scanTree = (dir) => {
  /** @type {string[]} */
  const problems = [];
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const p = join(dir, entry.name);
    if (entry.isDirectory() && entry.name !== "tools") problems.push(...scanTree(p));
    else if (entry.isDirectory()) continue;
    else if (p.endsWith("MANIFEST.json") || p.endsWith("README.md")) continue;
    else {
      const t = readFileSync(p, "utf8");
      if (/[A-Za-z0-9_-]{20,}\.[A-Za-z0-9_-]{4,}\.[A-Za-z0-9_-]{15,}/.test(t)) {
        problems.push(`${p}: токен-подобная строка`);
      }
      for (const key of SENSITIVE_KEYS) {
        if (new RegExp(`"${key}":"(?!REDACTED)`).test(t)) problems.push(`${p}: ${key} без REDACTED`);
      }
    }
  }
  return problems;
};

// ── Сборка (точечная очистка: README.md и tools/ сохраняются) ───────────────
for (const [relRepo, relLab] of [...FILES.map((f) => f.slice(0, 2)), ...DIRS.map((d) => d.slice(0, 2))]) {
  const target = join(DST, relRepo);
  rmSync(target, { recursive: true, force: true });
  mkdirSync(relRepo.includes("/") ? dirname(target) : target, { recursive: true });
  cpSync(join(SRC, relLab), target, { recursive: true });
}

// Санитизация ДО подсчёта md5: манифест фиксирует уже очищенные копии.
sanitizeTree(DST);

/** @type {any[]} */
const fixtures = [];
for (const [relRepo, relLab, purpose, usedBy] of FILES) {
  const target = join(DST, relRepo);
  fixtures.push({
    path: relRepo, source: relLab, kind: "file",
    bytes: statSync(target).size, md5: md5(target), purpose, usedBy,
  });
}
for (const [relRepo, relLab, purpose, usedBy] of DIRS) {
  const target = join(DST, relRepo);
  const names = readdirSync(target).sort();
  /** @type {string[]} */
  const lines = [];
  let bytes = 0;
  for (const name of names) {
    const p = join(target, name);
    const size = statSync(p).size;
    bytes += size;
    lines.push(`${name}\t${size}\t${md5(p)}`);
  }
  fixtures.push({
    path: relRepo, source: relLab, kind: "dir", files: names.length, bytes,
    md5: `index:${md5text(lines.join("\n"))}`, purpose, usedBy,
  });
}

const manifest = {
  $comment:
    "Курированные конформанс-фикстуры для Rust-клиента (сырьё волны R7, санитизированные). " +
    "Полная лаборатория — /home/sanya/re-deep; сборка — fixtures/tools/make-fixtures.mjs; " +
    "сверка — docs/rust-client/verify/verify-re.mjs.",
  labRoot: SRC,
  fixtures,
};
writeFileSync(join(DST, "MANIFEST.json"), JSON.stringify(manifest, null, 2) + "\n");

const problems = scanTree(DST);
const total = fixtures.reduce((s, f) => s + f.bytes, 0);
console.log(`фикстур: ${fixtures.length}, суммарно ${(total / 1024 / 1024).toFixed(2)} МБ, из ${SRC}`);
for (const f of fixtures) {
  console.log(`  ${f.kind === "dir" ? `${f.files} файлов ` : ""}${(f.bytes / 1024).toFixed(1)} КБ  ${f.path}  ${f.md5.slice(0, 16)}…`);
}
if (problems.length > 0) {
  console.error(`\nСКАН САНИТИЗАЦИИ НЕ ПРОЙДЕН (${problems.length}):`);
  for (const p of problems.slice(0, 20)) console.error(`  ${p}`);
  process.exit(1);
}
console.log("\nсанитизация: чисто (токенов и чувствительных значений нет)");
