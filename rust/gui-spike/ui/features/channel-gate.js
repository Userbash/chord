// @ts-check
/**
 * Гейты UI канала: диалог возрастного ограничения (18+) и блокировка композера.
 *
 * Обе функции — чистые предикаты без DOM: результат описывает, что показать,
 * а разметку и обработчики строит вызывающий код.
 */

/**
 * @typedef {{
 *   id: string,
 *   nsfw?: boolean | null,
 *   name?: string | null,
 * }} ChannelAgeGateInput
 */

/**
 * @typedef {{
 *   gated: boolean,
 *   titleRu: string,
 *   confirmLabelRu: string,
 *   cancelLabelRu: string,
 * }} ChannelAgeGateResult
 */

/**
 * @typedef {{
 *   name?: string | null,
 *   canSendMessages?: boolean | null,
 *   threadArchived?: boolean | null,
 *   threadLocked?: boolean | null,
 *   canManageThreads?: boolean | null,
 * }} ComposerChannelInput
 */

/**
 * @typedef {{
 *   disabled: boolean,
 *   placeholderRu: string,
 * }} ComposerLockState
 */

const AGE_GATED = Object.freeze({
  gated: true,
  titleRu: "Канал с возрастным ограничением (18+)",
  confirmLabelRu: "Продолжить",
  cancelLabelRu: "Назад",
});

const AGE_OPEN = Object.freeze({
  gated: false,
  titleRu: "",
  confirmLabelRu: "",
  cancelLabelRu: "",
});

/**
 * Решает, нужен ли диалог подтверждения возраста для канала.
 *
 * Гейт срабатывает только для `nsfw === true` и только если канал ещё не
 * подтверждён в текущей сессии (`acknowledgedChannelIds`).
 *
 * @param {ChannelAgeGateInput | null | undefined} channel
 * @param {ReadonlyArray<string> | null | undefined} [acknowledgedChannelIds]
 * @returns {ChannelAgeGateResult}
 */
export function evaluateChannelAgeGate(channel, acknowledgedChannelIds = []) {
  const acknowledged = Array.isArray(acknowledgedChannelIds)
    ? acknowledgedChannelIds
    : [];
  const nsfw = channel?.nsfw === true;
  const id = typeof channel?.id === "string" ? channel.id : "";
  const accepted = id !== "" && acknowledged.includes(id);

  return nsfw && !accepted ? { ...AGE_GATED } : { ...AGE_OPEN };
}

/**
 * Состояние композера для канала: заблокирован ли ввод и какая подсказка видна.
 *
 * Приоритет: блокировка ветки модератором, затем отсутствие права отправки,
 * иначе обычный ввод с именем канала.
 *
 * @param {ComposerChannelInput | null | undefined} channel
 * @returns {ComposerLockState}
 */
export function evaluateComposerLockState(channel) {
  if (channel?.threadLocked === true && channel?.canManageThreads !== true) {
    return {
      disabled: true,
      placeholderRu: "Эта ветка заблокирована модератором",
    };
  }

  if (channel?.canSendMessages === false) {
    return {
      disabled: true,
      placeholderRu: "У вас нет прав на отправку сообщений в этом канале",
    };
  }

  const name =
    typeof channel?.name === "string" && channel.name !== ""
      ? channel.name
      : "канал";

  return { disabled: false, placeholderRu: `Написать в #${name}` };
}
