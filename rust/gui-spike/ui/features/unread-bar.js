// @ts-check
/**
 * T-207 (UI unread bar): чистые вычисления состояния баннера непрочитанных
 * сообщений и кнопки «Перейти к последним сообщениям».
 *
 * Модуль не трогает DOM: рендер получает готовые строки и флаги, поэтому
 * контракт проверяется юнит-тестами без стенда.
 */

/** Порог прокрутки (px) от нижней границы, после которого показывается кнопка. */
export const JUMP_TO_PRESENT_THRESHOLD_PX = 300;

/** @typedef {{
 *   visible: boolean,
 *   count: number,
 *   textRu: string,
 *   actionLabelRu: string
 * }} UnreadBannerState */

/** @typedef {{
 *   visible: boolean,
 *   badgeText: string,
 *   labelRu: string,
 *   actionLabelRu: string
 * }} JumpToPresentState */

const BANNER_ACTION_RU = "Отметить как прочитанное";
const JUMP_LABEL_RU = "Перейти к последним сообщениям";
const JUMP_ACTION_RU = "К настоящему моменту";

/**
 * Состояние баннера «Новых сообщений: N [с <отметка>]».
 * @param {number} unreadCount число непрочитанных сообщений
 * @param {string} [sinceLabelRu] русская подпись начала отсчёта (например, дата)
 * @returns {UnreadBannerState}
 */
export function computeUnreadBannerState(unreadCount, sinceLabelRu = "") {
  if (!Number.isFinite(unreadCount) || unreadCount <= 0) {
    return { visible: false, count: 0, textRu: "", actionLabelRu: BANNER_ACTION_RU };
  }
  const n = Math.trunc(unreadCount);
  if (n <= 0) {
    return { visible: false, count: 0, textRu: "", actionLabelRu: BANNER_ACTION_RU };
  }
  const label = sinceLabelRu
    ? "Новых сообщений: " + n + " с " + sinceLabelRu
    : "Новых сообщений: " + n;
  return { visible: true, count: n, textRu: label, actionLabelRu: BANNER_ACTION_RU };
}

/**
 * Состояние плавающей кнопки перехода к последним сообщениям.
 * @param {number} distanceFromBottomPx расстояние прокрутки от нижней границы
 * @param {number} [newMessagesCount] число новых сообщений ниже области чтения
 * @returns {JumpToPresentState}
 */
export function computeJumpToPresentState(distanceFromBottomPx, newMessagesCount = 0) {
  const count = Number.isFinite(newMessagesCount) && newMessagesCount > 0
    ? Math.trunc(newMessagesCount)
    : 0;
  const farEnough =
    Number.isFinite(distanceFromBottomPx) && distanceFromBottomPx >= JUMP_TO_PRESENT_THRESHOLD_PX;
  const badgeText = count > 99 ? "99+" : count > 0 ? String(count) : "";
  return {
    visible: farEnough || count > 0,
    badgeText,
    labelRu: JUMP_LABEL_RU,
    actionLabelRu: JUMP_ACTION_RU,
  };
}
