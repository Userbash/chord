// @ts-check
/**
 * Event banner presentation helpers (RU).
 * Pure functions only: no DOM, no IPC, no side effects.
 */

/** @type {Readonly<Record<number, string>>} */
export const EVENT_ENTITY_LABELS_RU = Object.freeze({
  1: "Трибуна",
  2: "Голосовой канал",
  3: "Внешнее место",
});

/** @type {Readonly<Record<number, string>>} */
export const EVENT_STATUS_LABELS_RU = Object.freeze({
  1: "Запланировано",
  2: "Идёт сейчас",
  3: "Завершено",
  4: "Отменено",
});

/**
 * @typedef {object} EventSummaryInput
 * @property {string} [name]
 * @property {number} [entityType]
 * @property {number} [status]
 * @property {number | unknown} [userCount]
 * @property {string} [location]
 */

/**
 * @typedef {object} EventSummary
 * @property {string} title
 * @property {string} entityLabel
 * @property {string} statusLabel
 * @property {string} interestedLabel
 * @property {boolean} isActive
 */

/**
 * Normalize a count to a non-negative integer.
 * @param {unknown} value
 * @returns {number}
 */
function safeCount(value) {
  const numeric = Number(value);
  const truncated = Math.trunc(Number.isFinite(numeric) ? numeric : 0);
  return Math.max(0, truncated);
}

/**
 * Format an event into localized banner parts.
 * @param {EventSummaryInput} event
 * @returns {EventSummary}
 */
export function formatEventSummaryRu(event) {
  const source = event || {};
  const title = String(source.name ?? "").trim() || "Событие без названия";
  return {
    title,
    entityLabel: EVENT_ENTITY_LABELS_RU[/** @type {number} */ (source.entityType)] || "Неизвестно",
    statusLabel: EVENT_STATUS_LABELS_RU[/** @type {number} */ (source.status)] || "Запланировано",
    interestedLabel: `Интересуются: ${safeCount(source.userCount)}`,
    isActive: source.status === 2,
  };
}

/**
 * Toggle RSVP interest and derive the resulting attendee count.
 * @param {unknown} currentCount
 * @param {unknown} wasInterested
 * @returns {{ interested: boolean, userCount: number }}
 */
export function toggleEventRsvpState(currentCount, wasInterested) {
  const was = Boolean(wasInterested);
  return {
    interested: !was,
    userCount: Math.max(0, safeCount(currentCount) + (was ? -1 : 1)),
  };
}
