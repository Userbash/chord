// @ts-check
// RED (M1): реализация — src/core/csv.mjs. Контракт: safeName, csv (паритет scripts/logger.mjs).
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { csv, safeName } from "../../src/core/csv.mjs";

describe("safeName", () => {
  it("оставляет буквы, цифры, точку, дефис и подчёркивание", () => {
    assert.equal(safeName("pid-123_discord_utils.node"), "pid-123_discord_utils.node");
  });

  it("заменяет пробелы, скобки и прочие символы на подчёркивания", () => {
    assert.equal(safeName("Discord (main)"), "Discord__main_");
  });

  it("кириллица заменяется на подчёркивания", () => {
    assert.equal(safeName("процесс"), "_______");
  });

  it("реальные имя процесса и имена модулей .node сохраняют разрешённые символы", () => {
    // Реальные значения из functions/pid-23548-Discord.csv (md5 d4b49d5bd833a2ca68f002e2addba401).
    assert.equal(safeName("Discord"), "Discord");
    assert.equal(safeName("updater.node"), "updater.node");
    assert.equal(safeName("discord_utils.node"), "discord_utils.node");
  });
});

describe("csv", () => {
  it("каждая ячейка в кавычках, числа приводятся к строкам", () => {
    assert.equal(csv([["process", "pid"], ["Discord", 7]]), '"process","pid"\n"Discord","7"');
  });

  it("внутренние кавычки удваиваются", () => {
    assert.equal(csv([['He said "hi"']]), '"He said ""hi"""');
  });

  it("запятые не ломают ячейку (они внутри кавычек)", () => {
    assert.equal(csv([["a,b", "c"]]), '"a,b","c"');
  });

  it("перенос строки внутри ячейки остаётся внутри кавычек", () => {
    assert.equal(csv([["a\nb"]]), '"a\nb"');
  });

  it("пустая таблица → пустая строка", () => {
    assert.equal(csv([]), "");
  });

  it("заголовок инвентаря и реальный формат строки со специальными символами", () => {
    // Заголовок сверялся с head -1 исходного CSV; quoting — с csv() в scripts/logger.mjs.
    assert.equal(csv([["process", "pid", "module", "kind", "type", "name", "address", "offset"]]),
      '"process","pid","module","kind","type","name","address","offset"');
    assert.equal(csv([["Discord", 23548, "updater.node", "symbol", "function", 'name, said "hi"', "0x0", "0x1"]]),
      '"Discord","23548","updater.node","symbol","function","name, said ""hi""","0x0","0x1"');
  });
});
