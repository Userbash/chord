// @ts-check
/**
 * task-263: звуковые сигналы голосовой сессии.
 * Чистые помощники без DOM, Audio и сети: справочник событий, решение об
 * воспроизведении с учётом настроек пользователя и нормализация громкости.
 */

/**
 * Русские подписи звуковых событий голосовой сессии.
 * @type {Readonly<{
 *   mute: string,
 *   unmute: string,
 *   deafen: string,
 *   undeafen: string,
 *   user_join: string,
 *   user_leave: string,
 *   stream_start: string,
 *   stream_end: string
 * }>}
 */
export const VOICE_SOUND_EVENTS_RU = Object.freeze({
  mute: "Микрофон выключен",
  unmute: "Микрофон включён",
  deafen: "Звук выключен",
  undeafen: "Звук включён",
  user_join: "Участник подключился",
  user_leave: "Участник отключился",
  stream_start: "Трансляция началась",
  stream_end: "Трансляция завершена"
});

/**
 * Состояние голосовой сессии и настроек звука, влияющих на сигналы.
 * @typedef {Object} VoiceCueState
 * @property {boolean} [disableAllSounds] Выключить все звуковые сигналы.
 * @property {ReadonlyArray<string>} [mutedEvents] Точечно заглушённые события.
 * @property {boolean} [selfDeaf] Пользователь выключил себе звук.
 * @property {boolean} [streamerMode] Режим стримера (скрытие активности участников).
 */

/**
 * Проверяет, что ключ есть в справочнике событий.
 * @param {unknown} eventKey
 * @returns {eventKey is string}
 */
function isKnownCue(eventKey) {
  return (
    typeof eventKey === "string" &&
    Object.prototype.hasOwnProperty.call(VOICE_SOUND_EVENTS_RU, eventKey)
  );
}

/**
 * Решает, играть ли звуковой сигнал для события с учётом настроек.
 *
 * Порядок фильтров: неизвестное событие -> `disableAllSounds` -> `mutedEvents`
 * -> `selfDeaf` (оставляет только deafen/undeafen) -> `streamerMode`
 * (глушит только user_join/user_leave).
 * @param {string} eventKey Ключ из {@link VOICE_SOUND_EVENTS_RU}.
 * @param {VoiceCueState | null} [state] Текущие настройки; отсутствие = всё включено.
 * @returns {boolean}
 */
export function shouldPlayVoiceCue(eventKey, state) {
  if (!isKnownCue(eventKey)) return false;
  if (state?.disableAllSounds === true) return false;
  if (state?.mutedEvents?.includes(eventKey) === true) return false;
  if (state?.selfDeaf === true && eventKey !== "undeafen" && eventKey !== "deafen") return false;
  if (state?.streamerMode === true && (eventKey === "user_join" || eventKey === "user_leave")) {
    return false;
  }
  return true;
}

/**
 * Переводит процент громкости 0..200 в множитель 0..2 с шагом 0.01.
 * Битый вход (не число, NaN, бесконечность) даёт значение по умолчанию 1.
 * @param {number} volumePercent Громкость в процентах.
 * @returns {number} Множитель громкости 0..2, округлённый до двух знаков.
 */
export function normalizeCueVolume(volumePercent) {
  if (typeof volumePercent !== "number" || !Number.isFinite(volumePercent)) return 1;
  const clamped = Math.min(200, Math.max(0, volumePercent));
  return Math.round(clamped) / 100;
}
