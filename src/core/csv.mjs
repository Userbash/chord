// @ts-check
// CSV-хелперы инвентаря. Контракт: tests/unit/csv.test.mjs (паритет scripts/logger.mjs).

/**
 * Приводит строку к безопасному имени файла:
 * всё, кроме `[A-Za-z0-9._-]`, заменяется на `_`.
 * @param {string} value
 * @returns {string}
 */
export function safeName(value) {
  return value.replaceAll(/[^A-Za-z0-9._-]/g, "_");
}

/**
 * Сериализует таблицу в CSV: каждая ячейка — в кавычках, внутренние кавычки
 * удваиваются (`"` → `""`). Строки соединяются символом `\n` (без финального `\n`).
 * @param {readonly (readonly (string | number)[])[]} rows
 * @returns {string}
 */
export function csv(rows) {
  return rows
    .map((row) => row.map((value) => `"${String(value).replaceAll('"', '""')}"`).join(","))
    .join("\n");
}
