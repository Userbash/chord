// @ts-check
/**
 * T-197: чистая логика папок серверов (guild folders).
 *
 * Модуль без DOM: принимает список id серверов папки и карту их состояний,
 * возвращает агрегат для рендера папки. Входные данные не мутируются.
 *
 * Контракт:
 * - `hasUnread` — истина, если хотя бы у одного сервера папки
 *   `unread === true` или сумма упоминаний больше нуля;
 * - `mentionCount` — сумма только положительных целых `mentionCount`;
 *   отрицательные, дробные и отсутствующие значения игнорируются;
 * - `badgeText` — `''` при нуле, `'99+'` при значении больше 99,
 *   иначе `String(mentionCount)` (число не обрезается);
 * - `hasActiveVoice` — истина, если хотя бы у одного сервера папки
 *   `hasActiveVoice === true`.
 */

/**
 * @typedef {object} GuildState
 * @property {boolean} [unread]
 * @property {number} [mentionCount]
 * @property {boolean} [hasActiveVoice]
 */

/**
 * @typedef {object} GuildFolderState
 * @property {number} guildCount
 * @property {boolean} hasUnread
 * @property {number} mentionCount
 * @property {string} badgeText
 * @property {boolean} hasActiveVoice
 */

/** @type {Readonly<{ MENTION_BADGE_CAP: number }>} */
export const GUILD_FOLDER_LIMITS = Object.freeze({ MENTION_BADGE_CAP: 99 });

/**
 * @param {unknown} value
 * @returns {value is number}
 */
function isPositiveInteger(value) {
  return typeof value === "number" && Number.isInteger(value) && value > 0;
}

/**
 * @param {string[]} folderGuildIds
 * @param {Record<string, GuildState | undefined>} guildStatesById
 * @returns {GuildFolderState}
 */
export function computeGuildFolderState(folderGuildIds, guildStatesById) {
  const ids = Array.isArray(folderGuildIds) ? folderGuildIds : [];
  const states = guildStatesById ?? {};

  let hasUnread = false;
  let mentionCount = 0;
  let hasActiveVoice = false;

  for (const guildId of ids) {
    const state = states[guildId];
    if (!state) continue;

    const guildMentions = isPositiveInteger(state.mentionCount) ? state.mentionCount : 0;

    if (state.unread === true || guildMentions > 0) hasUnread = true;
    if (state.hasActiveVoice === true) hasActiveVoice = true;
    mentionCount += guildMentions;
  }

  return {
    guildCount: ids.length,
    hasUnread,
    mentionCount,
    badgeText: formatMentionBadge(mentionCount),
    hasActiveVoice,
  };
}

/**
 * @param {number} mentionCount
 * @returns {string}
 */
function formatMentionBadge(mentionCount) {
  if (mentionCount <= 0) return "";
  if (mentionCount > GUILD_FOLDER_LIMITS.MENTION_BADGE_CAP) return "99+";
  return String(mentionCount);
}

/**
 * Переключает свёрнутость папки: снимает id, если папка уже свёрнута,
 * иначе добавляет в конец. Возвращает новый массив.
 *
 * @param {string[]} collapsedFolderIds
 * @param {string} folderId
 * @returns {string[]}
 */
export function toggleFolderCollapsed(collapsedFolderIds, folderId) {
  const collapsed = Array.isArray(collapsedFolderIds) ? collapsedFolderIds : [];
  if (collapsed.includes(folderId)) {
    return collapsed.filter((id) => id !== folderId);
  }
  return [...collapsed, folderId];
}
