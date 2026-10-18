// @ts-check
// Разбор argv командной строки discord-lab.
// Контракт зафиксирован тестами tests/unit/args.test.mjs (паритет с scripts/logger.mjs).

/** @typedef {import("./types.js").ParsedArgs} ParsedArgs */

const RESERVED = new Set(["_", "modules", "__proto__", "constructor", "prototype"]);

/**
 * Разбирает argv в объект аргументов.
 *
 * Семантика (текущая, зафиксирована тестами):
 * - позиционные токены → `result._` (в порядке появления);
 * - `--key value` → `result.key = "value"` (значение из следующего токена,
 *   если он не начинается с `--`);
 * - `--key=value` → `result.key = "value"` (в т.ч. `--key=` → пустая строка);
 * - `--flag` (без значения) → `result.flag = true`;
 * - `--module X` (повторяемый) → `result.modules.push("X")`, сам ключ `module`
 *   в результат не попадает.
 * - служебные ключи `_`, `modules` и ключи прототипа игнорируются после обычного
 *   поглощения значения; это намеренное отклонение от `scripts/logger.mjs`, чтобы
 *   аргументы не могли повредить позиционные данные, список модулей или прототип.
 *
 * Числовые значения остаются строками — приведение делает вызывающий код.
 *
 * @param {readonly string[]} argv
 * @returns {ParsedArgs}
 */
export function parseArgs(argv) {
  /** @type {ParsedArgs} */
  const parsed = { _: [], modules: [] };
  for (let index = 0; index < argv.length; index += 1) {
    const token = argv[index] ?? "";
    if (!token.startsWith("--")) {
      parsed._.push(token);
      continue;
    }

    const [key = "", inline] = token.slice(2).split("=");
    /** @type {string | boolean | undefined} */
    let value = inline;
    const next = argv[index + 1];
    if (value === undefined && next !== undefined && !next.startsWith("--")) {
      value = next;
      index += 1;
    }
    if (value === undefined) value = true;

    if (key === "module") parsed.modules.push(String(value));
    else if (!RESERVED.has(key)) parsed[key] = value;
  }
  return parsed;
}
