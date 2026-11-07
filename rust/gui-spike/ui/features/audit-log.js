// @ts-check
/**
 * task-225 (FE-P2-24): журнал аудита сервера — русские метки действий и
 * форматирование записи/диффа изменений.
 *
 * Модуль чистый (без DOM и сети): принимает уже нормализованную запись и
 * отдаёт строку для списка аудита. Коды действий совпадают с
 * `AuditLogActionTypes` Discord.
 */

/**
 * Коды действий журнала аудита (подмножество Discord audit log events).
 */
export const AUDIT_ACTION_TYPES = Object.freeze({
  GUILD_UPDATE: 1,
  CHANNEL_CREATE: 10,
  CHANNEL_UPDATE: 11,
  CHANNEL_DELETE: 12,
  MEMBER_KICK: 20,
  MEMBER_BAN_ADD: 22,
  MEMBER_BAN_REMOVE: 23,
  ROLE_CREATE: 30,
  ROLE_UPDATE: 31,
  ROLE_DELETE: 32,
  MESSAGE_DELETE: 72,
});

/**
 * Русские метки действий по коду.
 * @type {Readonly<Record<number, string>>}
 */
export const AUDIT_ACTION_LABELS_RU = Object.freeze({
  1: "Обновление сервера",
  10: "Создание канала",
  11: "Изменение канала",
  12: "Удаление канала",
  20: "Исключение участника",
  22: "Бан участника",
  23: "Разбан участника",
  30: "Создание роли",
  31: "Изменение роли",
  32: "Удаление роли",
  72: "Удаление сообщения",
});

/** Метка для автора записи, если имя не пришло. */
const UNKNOWN_ACTOR_RU = "Неизвестный";

/** Подстановка для отсутствующей стороны диффа. */
const EMPTY_VALUE_RU = "нет";

/**
 * Метка действия на русском; для неизвестного кода — числовой фолбэк,
 * чтобы запись не теряла смысл и не роняла рендер.
 * @param {number} actionType
 * @returns {string}
 */
function labelFor(actionType) {
  return AUDIT_ACTION_LABELS_RU[actionType] ?? `Действие ${String(actionType)}`;
}

/**
 * Форматирует строку записи журнала аудита:
 * `актор: метка (цель)` плюс ` — Причина: …`, если причина непустая.
 * @param {{
 *   actionType: number,
 *   actorName?: string | null,
 *   targetName?: string | null,
 *   reason?: string | null,
 * }} entry
 * @returns {string}
 */
export function formatAuditLogEntryRu(entry) {
  const actorName = typeof entry.actorName === "string" ? entry.actorName.trim() : "";
  const actor = actorName === "" ? UNKNOWN_ACTOR_RU : actorName;
  const label = labelFor(entry.actionType);
  const targetName = typeof entry.targetName === "string" ? entry.targetName.trim() : "";
  const target = targetName === "" ? "" : ` (${targetName})`;
  const reason = typeof entry.reason === "string" ? entry.reason.trim() : "";
  const reasonPart = reason === "" ? "" : ` — Причина: ${reason}`;
  return `${actor}: ${label}${target}${reasonPart}`;
}

/**
 * Форматирует одну пару изменений: `ключ: было → стало`.
 * Пустые значения (`null`/`undefined`) показываются как «нет»;
 * остальные falsy-значения (`0`, `false`, `""`) остаются как есть.
 * @param {{ key: string, oldValue?: unknown, newValue?: unknown }} change
 * @returns {string}
 */
export function formatAuditChangeDiffRu(change) {
  const side = (/** @type {unknown} */ value) =>
    value === null || value === undefined ? EMPTY_VALUE_RU : String(value);
  return `${change.key}: ${side(change.oldValue)} → ${side(change.newValue)}`;
}
