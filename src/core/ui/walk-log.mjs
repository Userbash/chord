// @ts-check
// Журнал UI-прогона: прогресс, записи шагов, ошибки консоли.
// Контракт: tests/unit/walk-log.test.mjs (паритет ui-walk-settings2).

import { isBlacklisted } from "./blacklist.mjs";

/** @typedef {import("../types.js").VisitedState} VisitedState */
/** @typedef {import("../types.js").WalkRecord} WalkRecord */
/** @typedef {import("../types.js").ConsoleIssues} ConsoleIssues */
/** @typedef {import("../types.js").CdpEvent} CdpEvent */
/** @typedef {import("../types.js").NavItem} NavItem */

/**
 * Восстанавливает прогресс прогона из содержимого журнала ui-walk-*.jsonl.
 *
 * Правила:
 * - пустые строки пропускаются;
 * - битая JSON-строка пропускается, разбор продолжается со следующей
 *   (не роняем весь прогон из-за одного повреждения);
 * - каждая валидная запись добавляет `label` в набор и обновляет `maxStep`;
 * - отсутствие журнала (пустой текст) → пустой набор, maxStep 0.
 * @param {string} text
 * @returns {VisitedState}
 */
export function parseVisited(text) {
  /** @type {Set<string>} */
  const labels = new Set();
  let maxStep = 0;
  for (const line of text.split("\n")) {
    if (!line.trim()) continue;
    try {
      const record = /** @type {Record<string, unknown>} */ (JSON.parse(line));
      if (typeof record.step === "number" && Number.isFinite(record.step)) {
        maxStep = Math.max(maxStep, record.step);
      }
      if (typeof record.label === "string") labels.add(record.label);
    } catch {
      // Повреждённая строка не мешает восстановить остальные записи.
    }
  }
  return { labels, maxStep };
}

/**
 * Новая запись шага со значениями по умолчанию:
 * `{ step, label, clicked: false, screenshot: null, consoleErrors: 0,
 *    consoleErrorTexts: [], topFunctions: [], error: null }`.
 * @param {number} step
 * @param {string} label
 * @returns {WalkRecord}
 */
export function buildWalkRecord(step, label) {
  return {
    step,
    label,
    clicked: false,
    screenshot: null,
    consoleErrors: 0,
    consoleErrorTexts: [],
    topFunctions: [],
    error: null,
  };
}

/**
 * Сводка ошибок консоли за шаг (паритет ui-walk-settings2):
 * считаются события `Runtime.exceptionThrown` и `Runtime.consoleAPICalled`
 * с `params.type === "error"`;
 * `texts` — не больше 3, каждая = `JSON.stringify(params).slice(0, 220)`.
 * @param {readonly CdpEvent[]} events
 * @returns {ConsoleIssues}
 */
export function consoleIssues(events) {
  const issues = events.filter((event) =>
    event.method === "Runtime.exceptionThrown" ||
    (event.method === "Runtime.consoleAPICalled" && event.params?.type === "error"),
  );
  return {
    count: issues.length,
    texts: issues.slice(0, 3).map((event) => JSON.stringify(event.params).slice(0, 220)),
  };
}

/**
 * Нужно ли прервать прогон по тексту ошибки шага: соответствует ли он
 * `/таймаут|закрыто/` (соединение с CDP потеряно). `null` → false.
 * @param {string | null} error
 * @returns {boolean}
 */
export function shouldBail(error) {
  return error !== null && /таймаут|закрыто/.test(error);
}

/**
 * Первый по порядку пункт, который ещё не посещён и не запрещён.
 * @param {readonly NavItem[]} items
 * @param {ReadonlySet<string>} visited
 * @param {(label: string) => boolean} [blacklist] по умолчанию isBlacklisted
 * @returns {NavItem | undefined}
 */
export function pickNextItem(items, visited, blacklist = isBlacklisted) {
  return items.find((item) => !visited.has(item.text) && !blacklist(item.text));
}
