// @ts-check
/**
 * T-230 (FE-P2-25): форматтер активности присутствия и выбор первичной
 * активности для строки участника.
 *
 * Чистые функции без DOM: UI-слой получает готовую русскую строку и не
 * дублирует правила приоритета. Тип активности — числовой enum Discord
 * (`ActivityType`), кастомный статус ограничен 128 символами.
 */

/**
 * @typedef {{
 *   type?: number,
 *   name?: string | null,
 *   state?: string | null,
 *   emoji?: string | null,
 * }} PresenceActivity
 */

/** Числовые типы активности Discord (`ActivityType`). */
export const ACTIVITY_TYPES = Object.freeze({
  PLAYING: 0,
  STREAMING: 1,
  LISTENING: 2,
  WATCHING: 3,
  CUSTOM: 4,
  COMPETING: 5,
});

/** Лимит длины кастомного статуса в символах. */
export const MAX_CUSTOM_STATUS_CHARS = 128;

/** @param {unknown} value */
function text(value) {
  return typeof value === "string" ? value : "";
}

/**
 * Собирает строку активности на русском. Неизвестный тип, пустой вход или
 * отсутствие данных дают пустую строку — вызывающий код решает, что показать.
 * @param {PresenceActivity | null | undefined} activity
 * @returns {string}
 */
export function formatActivityLineRu(activity) {
  if (!activity || typeof activity !== "object") return "";
  const name = text(activity.name);
  switch (activity.type) {
    case ACTIVITY_TYPES.CUSTOM: {
      const parts = [text(activity.emoji), text(activity.state)].filter(
        (part) => part.length > 0,
      );
      return parts.join(" ").slice(0, MAX_CUSTOM_STATUS_CHARS);
    }
    case ACTIVITY_TYPES.PLAYING:
      return "Играет в " + name;
    case ACTIVITY_TYPES.STREAMING:
      return "Стримит " + name;
    case ACTIVITY_TYPES.LISTENING:
      return "Слушает " + name;
    case ACTIVITY_TYPES.WATCHING:
      return "Смотрит " + name;
    case ACTIVITY_TYPES.COMPETING:
      return "Соревнуется в " + name;
    default:
      return "";
  }
}

/** @param {PresenceActivity | null | undefined} activity */
function hasCustomContent(activity) {
  return (
    !!activity &&
    typeof activity === "object" &&
    activity.type === ACTIVITY_TYPES.CUSTOM &&
    (text(activity.state).length > 0 || text(activity.emoji).length > 0)
  );
}

/** @param {unknown} value @returns {value is PresenceActivity} */
function isKnownActivity(value) {
  if (!value || typeof value !== "object") return false;
  const type = /** @type {PresenceActivity} */ (value).type;
  return (
    type === ACTIVITY_TYPES.PLAYING ||
    type === ACTIVITY_TYPES.STREAMING ||
    type === ACTIVITY_TYPES.LISTENING ||
    type === ACTIVITY_TYPES.WATCHING ||
    type === ACTIVITY_TYPES.CUSTOM ||
    type === ACTIVITY_TYPES.COMPETING
  );
}

/**
 * Выбирает активность для показа: кастомный статус с содержимым, затем
 * стрим, затем первая известная активность из списка.
 * @param {Array<PresenceActivity | null | undefined> | null | undefined} activities
 * @returns {PresenceActivity | null}
 */
export function selectPrimaryActivity(activities) {
  if (!Array.isArray(activities)) return null;
  // Пустой CUSTOM не считается активностью: без текста и эмодзи показывать нечего.
  const list = /** @type {PresenceActivity[]} */ (
    activities.filter(
      (item) => isKnownActivity(item) && (item.type !== ACTIVITY_TYPES.CUSTOM || hasCustomContent(item)),
    )
  );
  const custom = list.find(hasCustomContent);
  if (custom) return custom;
  const streaming = list.find((item) => item.type === ACTIVITY_TYPES.STREAMING);
  if (streaming) return streaming;
  return list[0] ?? null;
}
