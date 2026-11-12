// @ts-check
/**
 * Извлечение и форматирование приглашений Discord для UI.
 * Чистые функции без DOM — тестируются в tests/unit/invite-parser.test.mjs.
 */

/** Максимум приглашений, которые разворачиваются в embeds на одно сообщение. */
export const MAX_INVITE_EMBEDS = 3;

/**
 * Ссылка-приглашение: опциональная схема, известный хост, код из [A-Za-z0-9-]{2,32}.
 * Хосты: discord.gg, discord.com/invite, discordapp.com/invite.
 */
const INVITE_PATTERN =
  /(?:https?:\/\/)?(?:discord\.gg|discord(?:app)?\.com\/invite)\/([A-Za-z0-9-]{2,32})/g;

/**
 * Достаёт коды приглашений из текста сообщения.
 * Дубликаты отбрасываются, порядок — первого вхождения, результат не длиннее MAX_INVITE_EMBEDS.
 * @param {string} text Текст сообщения или вставленный пользователем ввод.
 * @returns {string[]} Коды приглашений без дубликатов.
 */
export function extractInviteCodes(text) {
  if (typeof text !== 'string' || text.length === 0) return [];

  /** @type {string[]} */
  const codes = [];
  const re = new RegExp(INVITE_PATTERN.source, 'g');
  /** @type {RegExpExecArray | null} */
  let match = re.exec(text);
  while (match !== null) {
    const code = match[1];
    if (code && !codes.includes(code)) {
      codes.push(code);
      if (codes.length >= MAX_INVITE_EMBEDS) break;
    }
    match = re.exec(text);
  }
  return codes;
}

/**
 * @typedef {object} InviteLike
 * @property {string} [guildName] Название сервера.
 * @property {number|string|null} [onlineCount] Участников в сети.
 * @property {number|string|null} [memberCount] Всего участников.
 * @property {boolean} [joined] Пользователь уже на сервере.
 * @property {boolean} [expired] Приглашение истекло или удалено.
 */

/**
 * @typedef {object} InviteSummaryRu
 * @property {boolean} valid Приглашение можно использовать.
 * @property {string} title Заголовок embed.
 * @property {string} subtitle Строка со счётчиками.
 * @property {'Вступить'|'Перейти'|'Недоступно'} buttonText Текст кнопки.
 */

/**
 * Приводит счётчик к неотрицательному целому; мусор даёт 0.
 * @param {unknown} value Исходное значение из payload приглашения.
 * @returns {number} Целое >= 0.
 */
function toCount(value) {
  return Math.max(0, Math.trunc(Number(value) || 0));
}

/**
 * Готовит русские подписи карточки приглашения.
 * @param {InviteLike|null|undefined} invite Данные приглашения из API.
 * @returns {InviteSummaryRu} Тексты для embed и кнопки.
 */
export function formatInviteSummaryRu(invite) {
  if (!invite || invite.expired === true) {
    return {
      valid: false,
      title: 'Приглашение недействительно',
      subtitle: 'Срок действия приглашения истёк или оно было удалено',
      buttonText: 'Недоступно',
    };
  }

  const online = toCount(invite.onlineCount);
  const members = toCount(invite.memberCount);

  return {
    valid: true,
    title: String(invite.guildName || 'Chord Server'),
    subtitle: `В сети: ${online} • Участников: ${members}`,
    buttonText: invite.joined ? 'Перейти' : 'Вступить',
  };
}
