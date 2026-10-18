// @ts-check
// RED (M1): реализация — src/core/inventory.mjs. Контракт: форматы инвентаря
// (паритет scripts/logger.mjs → writeInventory: JSON + CSV).
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { inventoryCounts, inventoryCsvRows, inventoryJson } from "../../src/core/inventory.mjs";

/** @typedef {import("../../src/core/types.js").ModuleInfo} ModuleInfo */

/** @type {ModuleInfo[]} */
const modules = [
  {
    name: "discord_utils.node",
    path: "/app/modules/discord_utils.node",
    base: "0x1000",
    size: 4096,
    symbols: [
      { name: "Task::Run", address: "0x1100", offset: "0x100" },
      { name: "Task::Poll", address: "0x1200", offset: "0x200" },
    ],
    exports: [
      { name: "Discord_Utils", type: "function", address: "0x2000", offset: "0x1000" },
    ],
    error: null,
  },
];

// Источник: functions/pid-23548-Discord.json (md5 323890fd6ab57b1d1f822b0a84713fdc).
// Фрагмент сверялся jq: .modules[0] | {name,path,base,size,symbols:(.symbols[:3]),exports:(.exports[:2]),error}.
const realInventory = JSON.parse(readFileSync(fileURLToPath(new URL("../fixtures/inventory/pid-23548.sample.json", import.meta.url)), "utf8"));
/** @type {ModuleInfo[]} */
const realModules = realInventory.modules;
const realCsvSample = readFileSync(fileURLToPath(new URL("../fixtures/inventory/pid-23548.sample.csv", import.meta.url)), "utf8").trimEnd().split("\n");

describe("inventoryCounts", () => {
  it("считает реальные записи фрагмента (jq: [.modules[] | [.symbols|length,.exports|length]])", () => {
    assert.deepEqual(inventoryCounts(realModules), { symbols: 3, exports: 2 });
  });
  it("суммирует символы и экспорты", () => {
    assert.deepEqual(inventoryCounts(modules), { symbols: 2, exports: 1 });
  });

  it("пустой список → нули", () => {
    assert.deepEqual(inventoryCounts([]), { symbols: 0, exports: 0 });
  });
});

describe("inventoryCsvRows", () => {
  it("CSV-фрагмент взят из реального файла: заголовок и экспорт присутствуют", () => {
    // md5 исходника d4b49d5bd833a2ca68f002e2addba401; awk 'NR<=16 || NR==189' подтвердил строки.
    assert.equal(realCsvSample[0], '"process","pid","module","kind","type","name","address","offset"');
    assert.equal(realCsvSample.length, 17);
    assert.ok(realCsvSample.some((line) => line.includes('","export","function","__externref_table_dealloc",')));
  });

  it("строит реальные строки по jq-оракулу, включая exports и offsets", () => {
    assert.deepEqual(inventoryCsvRows("Discord", 23548, realModules), [
      ["process", "pid", "module", "kind", "type", "name", "address", "offset"],
      ["Discord", 23548, "updater.node", "symbol", "function", "getenv", "0x0", "0xffff80f7eb000000"],
      ["Discord", 23548, "updater.node", "symbol", "function", "dl_iterate_phdr", "0x0", "0xffff80f7eb000000"],
      ["Discord", 23548, "updater.node", "symbol", "function", "free", "0x0", "0xffff80f7eb000000"],
      ["Discord", 23548, "updater.node", "export", "function", "__externref_table_dealloc", "0x7f081528c2fb", "0x28c2fb"],
      ["Discord", 23548, "updater.node", "export", "function", "__externref_drop_slice", "0x7f081528c197", "0x28c197"],
    ]);
  });

  it("заголовок, затем строки символов и экспортов", () => {
    const rows = inventoryCsvRows("pid-7", 7, modules);
    assert.deepEqual(rows, [
      ["process", "pid", "module", "kind", "type", "name", "address", "offset"],
      ["pid-7", 7, "discord_utils.node", "symbol", "function", "Task::Run", "0x1100", "0x100"],
      ["pid-7", 7, "discord_utils.node", "symbol", "function", "Task::Poll", "0x1200", "0x200"],
      ["pid-7", 7, "discord_utils.node", "export", "function", "Discord_Utils", "0x2000", "0x1000"],
    ]);
  });

  it("имя процесса безопасно для таблиц и файлов (safeName)", () => {
    const rows = inventoryCsvRows("Discord (main)", 7, modules);
    assert.equal(rows[1]?.[0], "Discord__main_");
    assert.equal(rows[1]?.[1], 7);
  });

  it("без модулей — только заголовок", () => {
    assert.deepEqual(inventoryCsvRows("pid-1", 1, []), [
      ["process", "pid", "module", "kind", "type", "name", "address", "offset"],
    ]);
  });
});

describe("inventoryJson", () => {
  it("сохраняет структуру реального JSON {pid, process, at, modules}", () => {
    assert.deepEqual(inventoryJson({ pid: realInventory.pid, processName: realInventory.process,
      modules: realModules, at: realInventory.at }), realInventory);
  });

  it("документ { pid, process, at, modules } (имя процесса как есть)", () => {
    const at = "2026-10-18T12:00:00.000Z";
    assert.deepEqual(inventoryJson({ pid: 7, processName: "Discord (main)", modules, at }), {
      pid: 7,
      process: "Discord (main)",
      at,
      modules,
    });
  });
});
