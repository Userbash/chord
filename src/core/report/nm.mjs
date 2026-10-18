// @ts-check
// Разбор nm. Контракт: tests/unit/nm.test.mjs (паритет extract-static-functions.sh).

/** @typedef {import("../types.js").NmEntry} NmEntry */

/**
 * Разбирает вывод `nm --format=posix` (строки `name type value [size]`):
 * - строки короче 3 полей пропускаются (есть и служебные строки вроде заголовков);
 * - берутся только типы функций `T`, `t`, `W`, `w`;
 * - отсутствующий size → пустая строка.
 * @param {string} text
 * @returns {NmEntry[]}
 */
export function parseNmPosix(text) {
  /** @type {NmEntry[]} */
  const entries = [];
  for (const line of text.split(/\r?\n/)) {
    const fields = line.trim().split(/\s+/);
    if (fields.length < 3) continue;
    const type = fields[1];
    if (!type || !/^[TtWw]$/.test(type)) continue;
    // Older fixtures use `value type name`; native nm emits `name type value`.
    const legacyOrder = /^[0-9a-fA-F]+$/.test(fields[0] ?? "");
    const name = legacyOrder ? fields[2] : fields[0];
    const address = legacyOrder ? fields[0] : fields[2];
    const size = fields[3] ?? "";
    if (!name || !address) continue;
    entries.push({ name, type, address, size: size ?? "" });
  }
  return entries;
}

/**
 * CSV таблицы функций модуля (паритет `<module>.csv`):
 * строки `name,type,address,size` через `\n`, без финального `\n`.
 * @param {readonly NmEntry[]} entries
 * @returns {string}
 */
export function staticFunctionsCsv(entries) {
  return entries.map(({ name, type, address, size }) => `${name},${type},${address},${size}`).join("\n");
}
