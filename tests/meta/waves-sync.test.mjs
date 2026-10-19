// @ts-check
// Мета-тест (зелёный): журнал волн `waves/` — состояние и события восстановимы.
// Проверяет формат `db.jsonl` (схема v1), наличие README/STATE и артефактов принятых
// волн, а также существование файлов, на которые ссылается поле `evidence`.
// Краснеет при расползании журнала (см. waves/README.md).
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { readFile, readdir, stat } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../../", import.meta.url));
const wavesDir = join(root, "waves");

/** Словарь событий журнала (waves/README.md, схема v1). */
const EVENTS = new Set([
  "wave-planned",
  "red",
  "green",
  "gate",
  "review",
  "accepted",
  "commit",
  "note",
  "push",
]);

/**
 * @typedef {{
 *   v?: number, ts?: string, event?: string, wave?: string, card?: string,
 *   summary?: string, evidence?: string, gate?: string, commit?: string,
 * }} WaveEvent
 */

/**
 * Читает `waves/db.jsonl` в массив событий; на невалидной строке падает с её номером.
 * @returns {Promise<WaveEvent[]>}
 */
async function readDb() {
  const raw = await readFile(join(wavesDir, "db.jsonl"), "utf8");
  /** @type {WaveEvent[]} */
  const events = [];
  for (const [i, line] of raw.split("\n").entries()) {
    const text = line.trim();
    if (text === "") continue;
    /** @type {WaveEvent | null} */
    let ev = null;
    try {
      ev = JSON.parse(text);
    } catch (error) {
      assert.fail(
        `db.jsonl: строка ${i + 1} — не JSON: ${error instanceof Error ? error.message : String(error)}`,
      );
    }
    assert.ok(ev !== null, `db.jsonl: строка ${i + 1} не разобралась`);
    events.push(ev);
  }
  return events;
}

describe("waves-sync: журнал волн восстановим", () => {
  it("README.md, STATE.md и db.jsonl на месте", async () => {
    for (const file of ["README.md", "STATE.md", "db.jsonl"]) {
      const info = await stat(join(wavesDir, file)).catch(() => null);
      assert.ok(info?.isFile(), `waves/${file} отсутствует`);
    }
  });

  it("каждая строка db.jsonl — событие схемы v1, evidence существует", async () => {
    const events = await readDb();
    assert.ok(events.length >= 14, `событий: ${events.length} — ожидалось ≥ 14`);
    for (const [i, ev] of events.entries()) {
      const at = `строка ${i + 1}`;
      assert.equal(ev.v, 1, `${at}: v != 1`);
      assert.match(ev.ts ?? "", /^\d{4}-\d{2}-\d{2}/, `${at}: ts не ISO-дата`);
      assert.ok(EVENTS.has(ev.event ?? ""), `${at}: событие «${ev.event}» вне словаря`);
      if (ev.wave !== undefined) assert.match(ev.wave, /^RW-\d{2}$/, `${at}: wave`);
      if (ev.event === "red" || ev.event === "accepted") {
        assert.ok((ev.evidence ?? "").length > 0, `${at}: у события нет evidence`);
      }
      if ((ev.evidence ?? "").startsWith("waves/")) {
        const found = await stat(join(root, ev.evidence ?? "")).catch(() => null);
        assert.ok(found !== null, `${at}: evidence «${ev.evidence}» не найден`);
      }
    }
  });

  it("принятые волны имеют папки с артефактами, STATE ссылается на журнал", async () => {
    const accepted = new Set(
      (await readDb()).filter((ev) => ev.event === "accepted").map((ev) => ev.wave),
    );
    for (let n = 1; n <= 7; n++) {
      const wave = `RW-0${n}`;
      assert.ok(accepted.has(wave), `${wave}: нет события accepted`);
      const entries = await readdir(join(wavesDir, wave)).catch(() => null);
      assert.ok(entries !== null, `waves/${wave}/ отсутствует`);
      assert.ok(entries.length > 0, `waves/${wave}/ пуст`);
    }
    const state = await readFile(join(wavesDir, "STATE.md"), "utf8");
    assert.ok(state.includes("db.jsonl"), "STATE.md должен ссылаться на db.jsonl");
  });
});
