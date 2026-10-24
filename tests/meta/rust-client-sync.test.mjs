// @ts-check
// Мета-тест (красный → зелёный): план Rust-клиента не расползается. Проверяет: пакет
// `docs/rust-client/` (концепт, каталог функций, архитектура, план, тест-план, структура,
// трассировка, верификация, индекс) на месте; Mermaid-графы есть; ID функций (`F-*`) и тестов
// (`T-*`) уникальны и в достаточном числе; волны P0–P8 описаны; ссылки из
// README.md/TASKS.md/TDD-PLAN.md/docs/map.md/docs/re живые; курированные фикстуры (`fixtures/`),
// каркас (`rust/`) и отчёт верификации (`verify/report.json`) согласованы. Краснеет при потере
// файла, обнулении каталога, дубле ID, снятой ссылке или расхождении фикстур/каркаса.
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { readFile, stat } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../../", import.meta.url));
const DIR = "docs/rust-client";

/** Состав пакета: индекс + восемь документов. */
const DOCS = [
  "README.md", "CONCEPT.md", "FEATURES.md", "ARCHITECTURE.md", "PLAN.md", "TEST-PLAN.md",
  "PROJECT-STRUCTURE.md", "TRACEABILITY.md", "VERIFICATION.md", "CODE-STANDARD.md",
  "MUTATION-LEDGER.md", "SECURITY.md",
];

/** Минимумы — защита от «уполовинивания» плана. */
const MIN_FEATURES = 100;
const MIN_TESTS = 110;
const MIN_CARDS = 60;

/** Группы каталога функций (FEATURES.md). */
const FEATURE_GROUPS = [
  "GW", "ENC", "AUTH", "API", "DATA", "MSG", "SRCH",
  "PRES", "VOICE", "MEDIA", "UI", "I18N", "SYS", "SEC",
];

/** Группы каталога тестов (TEST-PLAN.md). */
const TEST_GROUPS = [
  "ENC", "GW", "REST", "DATA", "MSG", "SRCH", "VOICE",
  "MEDIA", "UI", "I18N", "SYS", "SEC", "HARN",
];

/** Крейты каркаса (`rust/crates/*`). */
const CRATES = [
  "discord-codec", "discord-core", "discord-store", "discord-voice", "discord-media",
  "discord-platform", "discord-i18n", "discord-ui", "discord-app", "discord-conformance",
  "discord-perf", "discord-updater",
];

/**
 * Читает файл относительно корня проекта.
 * @param {string} path путь от корня
 * @returns {Promise<string>}
 */
const read = (path) => readFile(join(root, path), "utf8");

/**
 * Вытаскивает ID строк таблицы по шаблону (первая колонка).
 * @param {string} text содержимое markdown
 * @param {RegExp} re выражение с группой 1 — ID
 * @returns {string[]}
 */
function tableIds(text, re) {
  return [...text.matchAll(re)].map((m) => m[1] ?? "");
}

/**
 * Проверяет: непусто, не меньше минимума, нет дублей.
 * @param {string[]} ids найденные ID
 * @param {number} min минимум
 * @param {string} label подпись для сообщения
 */
function assertUniqueEnough(ids, min, label) {
  assert.ok(ids.length >= min, `${label}: найдено ${ids.length} — ожидалось ≥ ${min}`);
  const dup = ids.filter((id, i) => ids.indexOf(id) !== i);
  assert.deepEqual([...new Set(dup)], [], `${label}: дубли ID`);
}

describe("rust-client-sync: план Rust-клиента цел и синхронен", () => {
  it("пакет docs/rust-client на месте и непуст", async () => {
    for (const file of DOCS) {
      const info = await stat(join(root, DIR, file)).catch(() => null);
      assert.ok(info?.isFile(), `${DIR}/${file}: файл отсутствует`);
      assert.ok(
        (info?.size ?? 0) > 400,
        `${DIR}/${file}: подозрительно мал (${info?.size ?? 0} Б)`,
      );
    }
  });

  it("индекс ссылается на все документы пакета", async () => {
    const text = await read(`${DIR}/README.md`);
    for (const file of DOCS.slice(1)) {
      assert.ok(text.includes(file), `README.md пакета не упоминает ${file}`);
    }
  });

  it("графы Mermaid есть; карта проекта получила слой 8", async () => {
    const arch = await read(`${DIR}/ARCHITECTURE.md`);
    const plan = await read(`${DIR}/PLAN.md`);
    assert.ok(arch.includes("```mermaid"), "ARCHITECTURE.md без Mermaid-графа");
    assert.match(
      arch,
      /flowchart|sequenceDiagram|erDiagram/,
      "ARCHITECTURE.md без типов диаграмм (flowchart/sequence/er)",
    );
    assert.ok(plan.includes("```mermaid"), "PLAN.md без Mermaid-графа (волновая карта)");
    const map = await read("docs/map.md");
    assert.ok(map.includes("Rust-клиент"), "docs/map.md без узла Rust-клиента");
    assert.ok(map.includes("S8"), "docs/map.md без слоя S8");
    assert.ok(map.includes("```mermaid"), "docs/map.md без Mermaid-блока");
  });

  it("каталог функций: ≥100 уникальных F-ID и все 14 групп", async () => {
    const text = await read(`${DIR}/FEATURES.md`);
    const ids = tableIds(text, /^\|\s*(F-[A-Z0-9]+-\d{2})\s*\|/gm);
    assertUniqueEnough(ids, MIN_FEATURES, "FEATURES");
    for (const group of FEATURE_GROUPS) {
      assert.ok(
        ids.some((id) => id.startsWith(`F-${group}-`)),
        `FEATURES: нет ни одной функции группы F-${group}`,
      );
    }
  });

  it("тест-план: ≥110 уникальных T-ID, все группы, манифест фикстур", async () => {
    const text = await read(`${DIR}/TEST-PLAN.md`);
    const ids = tableIds(text, /^\|\s*(T-[A-Z0-9]+-\d{2})\s*\|/gm);
    assertUniqueEnough(ids, MIN_TESTS, "TEST-PLAN");
    for (const group of TEST_GROUPS) {
      assert.ok(
        ids.some((id) => id.startsWith(`T-${group}-`)),
        `TEST-PLAN: нет ни одного теста группы T-${group}`,
      );
    }
    assert.ok(text.includes("/home/sanya/re-deep"), "TEST-PLAN без манифеста фикстур лаборатории");
    assert.ok(text.includes("L1") && text.includes("L5"), "TEST-PLAN без уровней L1/L5");
  });

  it("тест-план: расширенные уровни L0–L7, приёмка и нефункциональный контур", async () => {
    const text = await read(`${DIR}/TEST-PLAN.md`);
    for (const level of ["L0", "L1", "L2", "L3", "L4", "L5", "L6", "L7"]) {
      assert.ok(text.includes(level), `TEST-PLAN без уровня ${level}`);
    }
    assert.ok(text.includes("Приёмочн"), "TEST-PLAN без приёмочного уровня");
    assert.ok(text.includes("Нефункциональн"), "TEST-PLAN без нефункционального уровня");
    assert.ok(text.includes("Mutation"), "TEST-PLAN без mutation-контура");
    assert.ok(text.includes("Матрица выбора"), "TEST-PLAN без матрицы выбора уровней");
    assert.ok(text.includes("оракул"), "TEST-PLAN без требований к оракулу");
  });

  it("mutation-ledger: сводка по волнам, классы мутантов и разбор survivors", async () => {
    const text = await read(`${DIR}/MUTATION-LEDGER.md`);
    assert.ok(text.includes("## 1. Сводка"), "MUTATION-LEDGER без сводки");
    assert.ok(text.includes("RW-35"), "MUTATION-LEDGER без последней волны");
    assert.ok(text.includes("эквивалент"), "MUTATION-LEDGER без эквивалентных мутантов");
    assert.ok(text.includes("survivor"), "MUTATION-LEDGER без разбора survivors");
    assert.ok(text.includes("INCONCLUSIVE"), "MUTATION-LEDGER без INCONCLUSIVE");
    assert.ok(text.includes("restore"), "MUTATION-LEDGER без проверки restore");
  });

  it("security: модель угроз, границы доверия и чек-листы", async () => {
    const text = await read(`${DIR}/SECURITY.md`);
    assert.ok(text.includes("Модель угроз"), "SECURITY без модели угроз");
    assert.ok(text.includes("Границы доверия"), "SECURITY без границ доверия");
    assert.ok(text.includes("FFI"), "SECURITY без FFI-чек-листа");
    assert.ok(text.includes("Секреты"), "SECURITY без политики секретов");
    assert.ok(text.includes("Supply chain"), "SECURITY без supply-chain политики");
  });

  it("план: сквозной контур качества Q-01…Q-14", async () => {
    const text = await read(`${DIR}/PLAN.md`);
    const ids = tableIds(text, /^\|\s*(Q-\d{2})\s*\|/gm);
    assertUniqueEnough(ids, 14, "PLAN/Q");
    assert.ok(text.includes("Сквозной контур качества"), "PLAN без раздела качества");
  });

  it("план: волны P0–P8, ≥60 карточек, уникальные ID", async () => {
    const text = await read(`${DIR}/PLAN.md`);
    const ids = tableIds(text, /^\|\s*(P(?:45|[0-8])-\d{2})\s*\|/gm);
    assertUniqueEnough(ids, MIN_CARDS, "PLAN");
    for (const phase of ["## P0", "## P1", "## P2", "## P3", "## P4", "## P4.5", "## P5", "## P6", "## P7", "## P8"]) {
      assert.ok(text.includes(phase), `PLAN: нет раздела «${phase}»`);
    }
    assert.ok(text.includes("TEST-PLAN.md"), "PLAN без ссылок на тест-план");
    assert.ok(text.includes("P0-01") && text.includes("P8-05"), "PLAN без крайних карточек P0-01/P8-05");
  });

  it("репозиторные доки ссылаются на план (живые ссылки)", async () => {
    const readme = await read("README.md");
    const tasks = await read("TASKS.md");
    const tdd = await read("TDD-PLAN.md");
    const reIndex = await read("docs/re/README.md");
    assert.ok(readme.includes("docs/rust-client/"), "README.md без ссылки на docs/rust-client/");
    assert.ok(tasks.includes("docs/rust-client/"), "TASKS.md без ссылки на docs/rust-client/");
    assert.ok(tasks.includes("## RC"), "TASKS.md без раздела §RC");
    assert.ok(tdd.includes("docs/rust-client/"), "TDD-PLAN.md без ссылки на docs/rust-client/");
    assert.ok(reIndex.includes("rust-client"), "docs/re/README.md без ссылки на потребителя");
    const concept = await read(`${DIR}/CONCEPT.md`);
    assert.ok(
      concept.includes("Electron") && concept.includes("Chromium"),
      "CONCEPT.md без цели «без Electron/Chromium»",
    );
  });

  it("RC-07: фикстуры и каркас на месте и согласованы", async () => {
    const manifestRaw = await read("fixtures/MANIFEST.json");
    const manifest = /** @type {any} */ (JSON.parse(manifestRaw));
    const fixtures = manifest.fixtures ?? [];
    assert.ok(fixtures.length >= 12, `фикстуры: ${fixtures.length} — ожидалось ≥ 12`);
    for (const f of fixtures) {
      assert.ok(f.path && f.source && f.md5 && Number.isInteger(f.bytes), `фикстура без провенанса: ${f.path}`);
      assert.ok(f.usedBy, `фикстура ${f.path} без привязки к тестам (usedBy)`);
      const info = await stat(join(root, "fixtures", f.path)).catch(() => null);
      assert.ok(info, `фикстура отсутствует: fixtures/${f.path}`);
    }
    const fxReadme = await stat(join(root, "fixtures/README.md")).catch(() => null);
    assert.ok(fxReadme?.isFile(), "fixtures/README.md отсутствует");
    const builder = await stat(join(root, "fixtures/tools/make-fixtures.mjs")).catch(() => null);
    assert.ok((builder?.size ?? 0) > 1_000, "fixtures/tools/make-fixtures.mjs отсутствует/подозрительно мал");

    const workspace = await read("rust/Cargo.toml");
    assert.ok(workspace.includes("[workspace]") && workspace.includes('members = ["crates/*"]'), "rust/Cargo.toml без workspace/members");
    for (const crate of CRATES) {
      const cargo = await stat(join(root, "rust/crates", crate, "Cargo.toml")).catch(() => null);
      assert.ok(cargo?.isFile && cargo.size > 50, `rust/crates/${crate}/Cargo.toml отсутствует`);
      const srcFile = crate === "discord-app" ? "src/main.rs" : "src/lib.rs";
      const src = await stat(join(root, "rust/crates", crate, srcFile)).catch(() => null);
      assert.ok(src?.isFile && src.size > 200, `rust/crates/${crate}/${srcFile} отсутствует/пуст`);
    }
  });

  it("RC-08: отчёт верификации сырья — полный", async () => {
    const script = await stat(join(root, DIR, "verify/verify-re.mjs")).catch(() => null);
    assert.ok((script?.size ?? 0) > 2_000, "verify/verify-re.mjs отсутствует/подозрительно мал");
    const report = /** @type {any} */ (JSON.parse(await read(`${DIR}/verify/report.json`)));
    assert.ok(report.total >= 20, `verify/report.json: проверок ${report.total} — ожидалось ≥ 20`);
    assert.equal(report.passed, report.total, `verify/report.json: пройдено ${report.passed}/${report.total}`);
    const failed = (report.results ?? []).filter((/** @type {any} */ r) => !r.ok);
    assert.deepEqual(failed, [], "verify/report.json содержит непройденные проверки");
    assert.ok(String(report.lab).includes("re-deep"), "verify/report.json без ссылки на лабораторию");
  });

  it("трассировка и строение описывают все группы", async () => {
    const trace = await read(`${DIR}/TRACEABILITY.md`);
    for (const group of FEATURE_GROUPS) {
      assert.ok(trace.includes(`F-${group}`), `TRACEABILITY.md: нет ссылки на группу F-${group}`);
    }
    for (const group of TEST_GROUPS) {
      assert.ok(trace.includes(`T-${group}`), `TRACEABILITY.md: нет ссылки на группу T-${group}`);
    }
    const structure = await read(`${DIR}/PROJECT-STRUCTURE.md`);
    assert.ok(structure.includes("fixtures/") && structure.includes("rust/"), "PROJECT-STRUCTURE.md без карты fixtures/rust");
    assert.ok(structure.includes("```mermaid"), "PROJECT-STRUCTURE.md без Mermaid-графа");
    const map = await read("docs/map.md");
    for (const node of ["rc6", "rc7", "rc8"]) {
      assert.ok(map.includes(node), `docs/map.md без узла ${node}`);
    }
  });
});
