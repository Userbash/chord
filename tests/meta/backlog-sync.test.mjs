// @ts-check
// Мета-тест (красный → зелёный): бэклог переноса функций синхронен с документацией
// проекта. Проверяет: `docs/feature-backlog.md` существует; волны R4/R5/R6/D на месте;
// у карточек уникальные ID и заполненные поля; README.md и TASKS.md ссылаются на бэклог;
// `docs/map.md` содержит Mermaid-граф с референсами и бэклогом. Краснеет при расхождении
// (документ потерян, ID продублирован, колонка опустела, ссылка снята).
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../../", import.meta.url));
const BACKLOG = "docs/feature-backlog.md";

/** Минимум карточек по волнам — защита от «уполовинивания» бэклога. */
const WAVE_MINIMUMS = { R4: 12, R5: 11, R6: 10, D: 8 };

/**
 * Разбирает карточки бэклога: строки таблиц, начинающиеся с ID волны
 * (`R4-01`, `R5-11`, `R6-10`, `D-08`).
 * @param {string} text содержимое `docs/feature-backlog.md`
 * @returns {Array<{ id: string, wave: string, cells: string[] }>}
 */
function parseItems(text) {
  /** @type {Array<{ id: string, wave: string, cells: string[] }>} */
  const items = [];
  for (const line of text.split("\n")) {
    const m = /^\|\s*((R[456]|D)-\d{2})\s*\|/.exec(line);
    const id = m?.[1];
    const wave = m?.[2];
    if (!id || !wave) continue;
    const cells = line
      .replace(/^\|/, "")
      .replace(/\|$/, "")
      .split("|")
      .map((cell) => cell.trim());
    items.push({ id, wave, cells });
  }
  return items;
}

describe("backlog-sync: бэклог переноса ↔ docs ↔ README/TASKS", () => {
  it("документы существуют: бэклог, два анализа, карта", async () => {
    for (const file of [
      BACKLOG,
      "docs/vesktop-analysis.md",
      "docs/discord-qt-analysis.md",
      "docs/map.md",
    ]) {
      const text = await readFile(join(root, file), "utf8").catch(() => "");
      assert.ok(text.length > 0, `${file}: документ отсутствует или пуст`);
    }
  });

  it("волны R4/R5/R6/D присутствуют и карточек не меньше минимума", async () => {
    const text = await readFile(join(root, BACKLOG), "utf8");
    for (const wave of Object.keys(WAVE_MINIMUMS)) {
      assert.ok(text.includes(`## ${wave}`), `${BACKLOG}: нет раздела «## ${wave}»`);
    }
    const items = parseItems(text);
    const counts = new Map();
    for (const item of items) counts.set(item.wave, (counts.get(item.wave) ?? 0) + 1);
    for (const [wave, min] of Object.entries(WAVE_MINIMUMS)) {
      const count = counts.get(wave) ?? 0;
      assert.ok(count >= min, `${BACKLOG}: волна ${wave} — ${count} карточек, ожидалось ≥ ${min}`);
    }
  });

  it("у каждой карточки уникальный ID и заполненные поля (≥ 6 колонок)", async () => {
    const text = await readFile(join(root, BACKLOG), "utf8");
    const items = parseItems(text);
    const ids = items.map((item) => item.id);
    const dup = [...new Set(ids.filter((id, i) => ids.indexOf(id) !== i))];
    assert.deepEqual(dup, [], `${BACKLOG}: дубли ID`);
    for (const item of items) {
      assert.ok(item.cells.length >= 6, `${item.id}: колонок ${item.cells.length}, ожидалось ≥ 6`);
      for (const [i, cell] of item.cells.entries()) {
        assert.ok(cell.length > 0, `${item.id}: пустая колонка №${i + 1}`);
      }
    }
  });

  it("README.md и TASKS.md ссылаются на бэклог и анализы", async () => {
    const readme = await readFile(join(root, "README.md"), "utf8");
    const tasks = await readFile(join(root, "TASKS.md"), "utf8");
    assert.ok(readme.includes(BACKLOG), `README.md: нет ссылки на ${BACKLOG}`);
    assert.ok(
      readme.includes("docs/vesktop-analysis.md"),
      "README.md: нет ссылки на docs/vesktop-analysis.md",
    );
    assert.ok(
      readme.includes("docs/discord-qt-analysis.md"),
      "README.md: нет ссылки на docs/discord-qt-analysis.md",
    );
    assert.ok(tasks.includes(BACKLOG), `TASKS.md: нет ссылки на ${BACKLOG}`);
  });

  it("docs/map.md содержит Mermaid-граф с референсами и бэклогом", async () => {
    const map = await readFile(join(root, "docs/map.md"), "utf8");
    assert.ok(map.includes("```mermaid"), "docs/map.md: нет Mermaid-блока");
    assert.ok(map.includes("Vesktop"), "docs/map.md: нет узла про Vesktop");
    assert.ok(map.includes("Бэклог переноса"), "docs/map.md: нет узла про бэклог переноса");
  });
});
