// @ts-check
// Мета-тест: переносимые фикстуры `tests/fixtures/**` обязаны быть отслеживаемы git.
// Краснеет, когда файл фикстур скрыт `.gitignore` или не добавлен в индекс: локально
// тесты его видят, а в чистом клоне/CI — нет. Инцидент CI (2026-10-18): паттерн
// `logs/` прятал `tests/fixtures/logs/*`, гейт падал с ENOENT.
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { readdir } from "node:fs/promises";
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

describe("fixtures-tracked: tests/fixtures целиком попадает в git", () => {
  it("каждый файл tests/fixtures отслеживается git (не скрыт .gitignore)", async (t) => {
    const inside = spawnSync("git", ["rev-parse", "--is-inside-work-tree"], {
      cwd: root,
      encoding: "utf8",
    });
    if (inside.status !== 0 || (inside.stdout ?? "").trim() !== "true") {
      t.skip("git недоступен — проверка отслеживания фикстур пропущена");
      return;
    }

    const ls = spawnSync("git", ["ls-files", "-z", "--", "tests/fixtures"], {
      cwd: root,
      encoding: "utf8",
    });
    assert.equal(ls.status, 0, `git ls-files завершился с ошибкой: ${ls.stderr ?? ""}`);
    const tracked = new Set((ls.stdout ?? "").split("\0").filter(Boolean));

    const untracked = (await listFiles(join(root, "tests", "fixtures")))
      .map(rel)
      .filter((p) => !tracked.has(p))
      .sort();

    assert.deepEqual(
      untracked,
      [],
      "файлы tests/fixtures не отслеживаются git — в чистом клоне (CI) их не будет",
    );
  });
});
