// @ts-check
// Мета-тест (зелёный): связность «TDD-PLAN.md §8 ↔ тестовые файлы ↔ модули ↔ экспорты».
// Краснеет, когда план и тесты разъезжаются: тест без строки, строка без файла,
// экспорт модуля без теста (или наоборот).
//
// Формат §8 (см. TDD-PLAN.md): одна строка таблицы = один тестовый файл:
//   | `tests/unit/x.test.mjs` | `src/core/x.mjs` | `fnA`, `fnB` |
// Модули и имена — в косых кавычках; для строк без модуля — прочерк.
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { readFile, readdir, stat } from "node:fs/promises";
import { join, relative, sep } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../../", import.meta.url));

/**
 * Рекурсивный список файлов каталога.
 * @param {string} dir
 * @returns {Promise<string[]>}
 */
async function listFiles(dir) {
  /** @type {string[]} */
  const out = [];
  for (const entry of await readdir(dir, { withFileTypes: true })) {
    const full = join(dir, entry.name);
    if (entry.isDirectory()) out.push(...(await listFiles(full)));
    else if (entry.isFile()) out.push(full);
  }
  return out;
}

/**
 * Путь относительно корня проекта (всегда со слэшами).
 * @param {string} p
 * @returns {string}
 */
const rel = (p) => relative(root, p).split(sep).join("/");

/**
 * @typedef {object} PlanRow
 * @property {string} test
 * @property {string[]} modules
 * @property {string[]} tokens
 */

/**
 * Разбирает строки таблицы §8.
 * @param {string} markdown
 * @returns {PlanRow[]}
 */
function parsePlanRows(markdown) {
  const start = markdown.indexOf("## 8.");
  const end = markdown.indexOf("## 9.");
  assert.ok(start >= 0 && end > start, "TDD-PLAN.md: не найден §8 (или §9 после него)");
  /** @type {PlanRow[]} */
  const rows = [];
  for (const line of markdown.slice(start, end).split("\n")) {
    const m = /^\|\s*`(tests\/[^`]+\.mjs)`\s*\|([^|]*)\|([^|]*)\|\s*$/.exec(line.trim());
    if (!m) continue;
    const test = m[1];
    if (!test) continue;
    /** @param {string} cell @returns {string[]} */
    const backticks = (cell) => [...cell.matchAll(/`([^`]+)`/g)].map((x) => x[1] ?? "");
    rows.push({
      test,
      modules: backticks(m[2] ?? "").filter((t) => t.startsWith("src/") || t.startsWith("bin/")),
      tokens: backticks(m[3] ?? ""),
    });
  }
  return rows;
}

/**
 * Имена экспортируемых функций модуля (по исходному тексту).
 * @param {string} text
 * @returns {Set<string>}
 */
function functionExportsOf(text) {
  const names = new Set();
  for (const m of text.matchAll(/^export\s+(?:async\s+)?function\s+([A-Za-z_$][\w$]*)/gm)) {
    const name = m[1];
    if (name) names.add(name);
  }
  return names;
}

/**
 * Все именованные экспорты модуля (функции, константы, классы).
 * @param {string} text
 * @returns {Set<string>}
 */
function allExportsOf(text) {
  const names = functionExportsOf(text);
  for (const m of text.matchAll(/^export\s+(?:const|let|var|class)\s+([A-Za-z_$][\w$]*)/gm)) {
    const name = m[1];
    if (name) names.add(name);
  }
  return names;
}

/** @param {string} t @returns {boolean} */
const isIdentifier = (t) => /^[A-Za-z_$][\w$]*$/.test(t);

const planPromise = readFile(join(root, "TDD-PLAN.md"), "utf8").then(parsePlanRows);

describe("plan-sync: §8 ↔ тестовые файлы ↔ модули", () => {
  it("таблица §8 разбирается и не пуста", async () => {
    const rows = await planPromise;
    assert.ok(
      rows.length >= 19,
      `§8: разобрано ${rows.length} строк — ожидалось ≥ 19 (таблица повреждена?)`,
    );
    const dup = rows.map((r) => r.test).filter((t, i, a) => a.indexOf(t) !== i);
    assert.deepEqual(dup, [], "§8: один тестовый файл описан несколькими строками");
  });

  it("каждая строка §8 указывает на существующий тест; токены встречаются в нём", async () => {
    const rows = await planPromise;
    /** @type {string[]} */
    const missingTest = [];
    /** @type {string[]} */
    const missingTokens = [];
    for (const row of rows) {
      let text;
      try {
        text = await readFile(join(root, row.test), "utf8");
      } catch {
        missingTest.push(row.test);
        continue;
      }
      for (const token of row.tokens) {
        if (!text.includes(token)) missingTokens.push(`${row.test}: \`${token}\``);
      }
    }
    assert.deepEqual(missingTest, [], "§8 ссылается на несуществующие тестовые файлы");
    assert.deepEqual(missingTokens, [], "токены из §8 не встречаются в своих тестах");
  });

  it("каждый тестовый файл (unit/meta/smoke/e2e) описан в §8 ровно один раз", async () => {
    const rows = await planPromise;
    const actual = (await listFiles(join(root, "tests")))
      .map(rel)
      .filter((p) => p.endsWith(".test.mjs"));
    const listed = new Set(rows.map((r) => r.test));
    const unlisted = actual.filter((p) => !listed.has(p));
    const unknown = [...listed].filter((p) => !actual.includes(p));
    assert.deepEqual(unlisted, [], "тестовые файлы без строки в §8");
    assert.deepEqual(unknown, [], "§8 описывает файлы, которых нет");
  });

  it("модули из §8 существуют; каждый src/**/*.mjs приписан к строке", async () => {
    const rows = await planPromise;
    const modules = new Set(rows.flatMap((r) => r.modules));
    /** @type {string[]} */
    const missing = [];
    for (const modulePath of modules) {
      const exists = await stat(join(root, modulePath)).then(
        () => true,
        () => false,
      );
      if (!exists) missing.push(modulePath);
    }
    assert.deepEqual(missing, [], "§8 ссылается на несуществующие модули");

    const srcFiles = (await listFiles(join(root, "src")))
      .map(rel)
      .filter((p) => p.endsWith(".mjs"));
    const unattached = srcFiles.filter((p) => !modules.has(p));
    assert.deepEqual(unattached, [], "src-модули без строки в §8 (нет теста?)");

    const binFiles = (await listFiles(join(root, "bin")))
      .map(rel)
      .filter((p) => p.endsWith(".mjs"));
    const binUnattached = binFiles.filter((p) => !modules.has(p));
    assert.deepEqual(binUnattached, [], "bin-скрипты без строки в §8");
  });

  it("строки с одним src-модулем: токены покрывают все экспортируемые функции", async () => {
    const rows = await planPromise;
    /** @type {string[]} */
    const problems = [];
    for (const row of rows) {
      const srcModules = row.modules.filter((m) => m.startsWith("src/"));
      if (srcModules.length !== 1) continue;
      if (row.tokens.length === 0) continue;
      if (!row.tokens.every(isIdentifier)) continue; // не-идентификаторы (`--help`) — свободный текст
      const modulePath = srcModules[0];
      if (!modulePath) continue;
      const text = await readFile(join(root, modulePath), "utf8");
      const functions = [...functionExportsOf(text)].sort();
      const allowed = allExportsOf(text);
      const actual = [...new Set(row.tokens)].sort();
      const unknown = actual.filter((t) => !allowed.has(t));
      const missing = functions.filter((f) => !actual.includes(f));
      if (unknown.length > 0 || missing.length > 0) {
        problems.push(
          `${row.test} ↔ ${modulePath}: лишние [${unknown.join(", ")}], не покрыты [${missing.join(", ")}]`,
        );
      }
    }
    assert.deepEqual(problems, [], "расхождение токенов §8 и экспортов модулей");
  });
});
