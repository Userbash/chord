// @ts-check
/**
 * task-266: чистые помощники очереди заявок на слово в Stage-канале.
 * Без DOM и сети — только классификация голосового состояния и порядок заявок.
 */

/**
 * Статус участника трибуны.
 * @typedef {'speaker' | 'requested' | 'listener'} StageMemberRole
 */

/**
 * Русские подписи статусов трибуны.
 * @type {Readonly<Record<StageMemberRole, string>>}
 */
export const STAGE_MEMBER_ROLES_RU = Object.freeze({
  speaker: "Докладчик",
  requested: "Просит слово",
  listener: "Слушатель"
});

/**
 * Минимальная форма участника Stage-канала, нужная помощникам.
 * @typedef {Object} StageMemberLike
 * @property {string | number} [userId] идентификатор участника
 * @property {string} [displayName] отображаемое имя
 * @property {boolean} [suppress] право говорить (false — микрофон разрешён)
 * @property {string | null} [requestToSpeakTimestamp] время поднятой руки (ISO-строка)
 */

/**
 * Безопасно приводит значение к словарю свойств.
 * @param {unknown} value
 * @returns {Record<string, unknown> | null}
 */
function asRecord(value) {
  if (typeof value !== "object" || value === null) return null;
  return /** @type {Record<string, unknown>} */ (value);
}

/**
 * Классифицирует голосовое состояние участника Stage-канала.
 * `suppress === false` — микрофон уже разрешён (`speaker`); непустая строка
 * `requestToSpeakTimestamp` — рука поднята (`requested`); иначе `listener`.
 * @param {unknown} voiceState
 * @returns {StageMemberRole}
 */
export function classifyStageVoiceState(voiceState) {
  const state = asRecord(voiceState);
  if (!state) return "listener";
  if (state.suppress === false) return "speaker";
  const timestamp = state.requestToSpeakTimestamp;
  if (typeof timestamp === "string" && timestamp.trim().length > 0) return "requested";
  return "listener";
}

/**
 * Время поднятой руки в миллисекундах; невалидная дата считается бесконечной
 * и уходит в конец очереди.
 * @param {unknown} timestamp
 * @returns {number}
 */
function requestTimestampOrInfinity(timestamp) {
  if (typeof timestamp !== "string") return Infinity;
  const parsed = Date.parse(timestamp);
  return Number.isNaN(parsed) ? Infinity : parsed;
}

/**
 * Очередь заявок на слово: только `requested`, раньше поднятая рука — выше,
 * при равенстве — по строковому `userId` по возрастанию. Вход не мутируется.
 * @param {ReadonlyArray<StageMemberLike> | null | undefined} members
 * @returns {ReadonlyArray<StageMemberLike>}
 */
export function sortStageSpeakerRequests(members) {
  const list = Array.isArray(members) ? members : [];
  /** @type {{ member: StageMemberLike, time: number, userId: string }[]} */
  const queue = [];

  for (const member of list) {
    if (classifyStageVoiceState(member) !== "requested") continue;
    const record = asRecord(member);
    queue.push({
      member,
      time: requestTimestampOrInfinity(record?.requestToSpeakTimestamp),
      userId: String(record?.userId ?? "")
    });
  }

  queue.sort((left, right) => {
    if (left.time !== right.time) return left.time < right.time ? -1 : 1;
    if (left.userId === right.userId) return 0;
    return left.userId < right.userId ? -1 : 1;
  });

  return queue.map((entry) => entry.member);
}
