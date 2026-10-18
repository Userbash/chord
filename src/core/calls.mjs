// @ts-check
// Записи вызовов Frida и сводка по ним. Контракт: tests/unit/calls.test.mjs
// (паритет scripts/logger.mjs hook + scripts/summarize-calls.sh).

/** @typedef {import("./types.js").CallRecord} CallRecord */
/** @typedef {import("./types.js").CallsSummary} CallsSummary */

/**
 * Является ли значение записью вызова: объект с `type === "call"`.
 * Именно такой payload шлёт агент scripts/frida-log-calls.js.
 * @param {unknown} value
 * @returns {value is CallRecord}
 */
export function isCallPayload(value) {
  return typeof value === "object" && value !== null && "type" in value && value.type === "call";
}

/**
 * Строка журнала для записи вызова: JSON + `\n`.
 * Не-вызов (isCallPayload === false) → `null` (строка не пишется).
 * @param {unknown} payload
 * @returns {string | null}
 */
export function callLine(payload) {
  return isCallPayload(payload) ? `${JSON.stringify(payload)}\n` : null;
}

/**
 * Разбирает содержимое журнала calls-*.jsonl:
 * пустые и битые строки пропускаются, не-вызовы отбрасываются.
 * @param {string} text
 * @returns {CallRecord[]}
 */
export function parseCallLog(text) {
  /** @type {CallRecord[]} */
  const records = [];
  for (const line of text.split(/\r?\n/)) {
    if (line.trim() === "") continue;
    try {
      /** @type {unknown} */
      const value = JSON.parse(line);
      if (isCallPayload(value)) records.push(value);
    } catch {
      // Частично записанные или повреждённые строки не являются записями.
    }
  }
  return records;
}

/**
 * Сводка как в scripts/summarize-calls.sh:
 * группировка по тройке `(module, name, address)`, сортировка по ключу
 * (лексикографически), CSV без внешних кавычек, но с удвоением внутренних:
 *
 * ```
 * calls,module,name,address
 * 3,discord_utils.node,Task::Run,0x1f00
 * ```
 *
 * `total` — число записей, `unique` — число уникальных функций.
 * @param {readonly CallRecord[]} records
 * @returns {CallsSummary}
 */
export function summarizeCallLines(records) {
  /** @type {Map<string, { module: string, name: string, address: string, count: number }>} */
  const counts = new Map();
  for (const record of records) {
    const key = JSON.stringify([record.module, record.name, record.address]);
    const entry = counts.get(key);
    if (entry) entry.count++;
    else counts.set(key, { module: record.module, name: record.name, address: record.address, count: 1 });
  }

  /** @param {string} a @param {string} b */
  const compare = (a, b) => (a < b ? -1 : a > b ? 1 : 0);
  const entries = [...counts.values()].sort((a, b) =>
    compare(a.module, b.module) || compare(a.name, b.name) || compare(a.address, b.address),
  );
  /** @param {string} value */
  const escape = (value) => value.replaceAll('"', '""');
  const lines = ["calls,module,name,address", ...entries.map(({ count, module, name, address }) =>
    [String(count), module, name, address].map(escape).join(","),
  )];
  return { total: records.length, unique: entries.length, csv: lines.join("\n") };
}
