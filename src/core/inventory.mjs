// @ts-check
// Инвентарь символов/экспортов: форматы files/CSV. Контракт: tests/unit/inventory.test.mjs.

import { safeName } from "./csv.mjs";

/** @typedef {import("./types.js").ModuleInfo} ModuleInfo */
/** @typedef {import("./types.js").InventoryCounts} InventoryCounts */
/** @typedef {import("./types.js").InventoryJson} InventoryJson */

/**
 * Суммарные счётчики по модулям.
 * @param {readonly ModuleInfo[]} modules
 * @returns {InventoryCounts}
 */
export function inventoryCounts(modules) {
  return modules.reduce(
    (counts, module) => ({
      symbols: counts.symbols + module.symbols.length,
      exports: counts.exports + module.exports.length,
    }),
    { symbols: 0, exports: 0 },
  );
}

/**
 * Строки CSV инвентаря (паритет scripts/logger.mjs → writeInventory):
 * заголовок `process,pid,module,kind,type,name,address,offset`;
 * символы — `[label, pid, module, "symbol", "function", name, address, offset]`;
 * экспорты — `[label, pid, module, "export", type, name, address, offset]`,
 * где label = safeName(processName) (безопасное имя для файлов/таблиц).
 *
 * @param {string} processName
 * @param {number} pid
 * @param {readonly ModuleInfo[]} modules
 * @returns {(string | number)[][]}
 */
export function inventoryCsvRows(processName, pid, modules) {
  const label = safeName(processName);
  /** @type {(string | number)[][]} */
  const rows = [["process", "pid", "module", "kind", "type", "name", "address", "offset"]];
  for (const module of modules) {
    for (const symbol of module.symbols) {
      rows.push([label, pid, module.name, "symbol", "function", symbol.name, symbol.address, symbol.offset]);
    }
    for (const entry of module.exports) {
      rows.push([label, pid, module.name, "export", entry.type, entry.name, entry.address, entry.offset]);
    }
  }
  return rows;
}

/**
 * JSON-документ инвентаря: `{ pid, process, at, modules }`
 * (имя процесса сохраняется как есть; модули — без изменений).
 * @param {{ pid: number, processName: string, modules: ModuleInfo[], at: string }} args
 * @returns {InventoryJson}
 */
export function inventoryJson(args) {
  return { pid: args.pid, process: args.processName, at: args.at, modules: args.modules };
}
