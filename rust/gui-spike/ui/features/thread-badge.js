// @ts-check

/**
 * Справочник RU-подписей для срока автоархивации ветки Discord.
 * Ключи — минуты, значения — человекочитаемая подпись.
 * @type {Readonly<Record<number, string>>}
 */
export const THREAD_ARCHIVE_DURATIONS_RU = Object.freeze({
  60: "1 час",
  1440: "24 часа",
  4320: "3 дня",
  10080: "1 неделя",
});

/** @type {readonly number[]} */
const ALLOWED_ARCHIVE_DURATIONS = [60, 1440, 4320, 10080];

/** @type {number} */
const DEFAULT_ARCHIVE_DURATION = 1440;

/**
 * Приводит срок автоархивации к допустимому значению Discord.
 * Неизвестное или нечисловое значение подменяется на 24 часа.
 * @param {unknown} minutes
 * @returns {number}
 */
export function normalizeArchiveDurationMinutes(minutes) {
  /** @type {number} */
  const parsed = Number(minutes);
  return ALLOWED_ARCHIVE_DURATIONS.includes(parsed) ? parsed : DEFAULT_ARCHIVE_DURATION;
}

/**
 * Форматирует счётчик непрочитанных сообщений для бейджа ветки.
 * @param {unknown} count
 * @returns {string}
 */
export function formatUnreadCountBadge(count) {
  const n = Math.max(0, Math.trunc(Number(count) || 0));
  if (n === 0) return "";
  if (n > 99) return "99+";
  return String(n);
}

/**
 * @typedef {object} ThreadStatusInput
 * @property {string} [name]
 * @property {number} [messageCount]
 * @property {number} [memberCount]
 * @property {number} [autoArchiveDuration]
 * @property {boolean} [archived]
 * @property {boolean} [locked]
 */

/**
 * @typedef {object} ThreadStatusSummary
 * @property {string} title
 * @property {string} archiveLabel
 * @property {string} stateLabel
 * @property {string} messageCountLabel
 */

/**
 * Собирает сводку состояния ветки для панели заголовка.
 * @param {ThreadStatusInput | null | undefined} thread
 * @returns {ThreadStatusSummary}
 */
export function formatThreadStatusSummaryRu(thread) {
  const rawName = thread?.name;
  const title =
    typeof rawName === "string" && rawName.trim() ? rawName.trim() : "Ветка без названия";

  return {
    title,
    // normalizeArchiveDurationMinutes отдаёт только ключи справочника; литерал
    // закрывает noUncheckedIndexedAccess, не меняя наблюдаемое поведение.
    archiveLabel:
      THREAD_ARCHIVE_DURATIONS_RU[normalizeArchiveDurationMinutes(thread?.autoArchiveDuration)] ??
      "24 часа",
    stateLabel: thread?.locked ? "Закрыта" : thread?.archived ? "В архиве" : "Активна",
    messageCountLabel: `Сообщений: ${Math.max(0, Math.trunc(Number(thread?.messageCount) || 0))}`,
  };
}
