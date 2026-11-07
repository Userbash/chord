// @ts-check
/**
 * T-191: раскладка голосовых плиток (voice grid).
 *
 * Чистые функции без DOM: сколько колонок/рядов занять под N участников и
 * кого из длинного списка показать первыми, а кого свернуть в «+N».
 *
 * Правила сетки:
 * - 0 -> 0x0, 1 -> 1x1, 2 -> 2x1, 3..=4 -> 2 колонки, 5..=9 -> 3,
 *   10..=16 -> 4, 17..=25 -> 5, больше 25 -> 5x5 + overflow.
 */

/** Максимум одновременно видимых плиток в голосовом канале. */
export const MAX_VISIBLE_VOICE_TILES = 25;

/**
 * @typedef {{ columns: number, rows: number, visibleCount: number, overflowCount: number }} VoiceGridDimensions
 */

/**
 * @typedef {{
 *   id: string,
 *   speaking?: boolean,
 *   streaming?: boolean,
 *   videoOn?: boolean,
 *   muted?: boolean,
 *   displayName?: string,
 * }} VoiceParticipant
 */

/**
 * Прижимает количество участников к допустимому диапазону `0..=MAX_VISIBLE_VOICE_TILES`.
 * @param {unknown} count
 * @returns {number}
 */
function normalizeCount(count) {
  const n = typeof count === "number" ? count : Number.NaN;
  if (!Number.isFinite(n) || n <= 0) return 0;
  if (n >= MAX_VISIBLE_VOICE_TILES) return MAX_VISIBLE_VOICE_TILES;
  return Math.floor(n);
}

/**
 * Число колонок для видимой части сетки: ступени 1/2/2/3/4/5.
 * @param {number} visibleCount
 * @returns {number}
 */
function columnsFor(visibleCount) {
  if (visibleCount <= 0) return 0;
  if (visibleCount === 1) return 1;
  if (visibleCount === 2) return 2;
  if (visibleCount <= 4) return 2;
  if (visibleCount <= 9) return 3;
  if (visibleCount <= 16) return 4;
  return 5;
}

/**
 * Размеры сетки для `tileCount` участников.
 * @param {number} tileCount
 * @returns {VoiceGridDimensions}
 */
export function computeVoiceGridDimensions(tileCount) {
  const total = typeof tileCount === "number" && Number.isFinite(tileCount) && tileCount > 0
    ? Math.floor(tileCount)
    : 0;
  const visibleCount = Math.min(total, MAX_VISIBLE_VOICE_TILES);
  const overflowCount = Math.max(0, total - visibleCount);
  const columns = columnsFor(visibleCount);
  const rows = columns === 0 ? 0 : Math.ceil(visibleCount / columns);
  return { columns, rows, visibleCount, overflowCount };
}

/**
 * Вес участника: чем меньше, тем выше в списке.
 * 0 — фокус, 1 — стрим, 2 — видео, 3 — говорит, 4 — без mute, 5 — muted.
 * @param {VoiceParticipant} participant
 * @param {string | null} focusedId
 * @returns {number}
 */
function rankOf(participant, focusedId) {
  if (focusedId !== null && participant.id === focusedId) return 0;
  if (participant.streaming === true) return 1;
  if (participant.videoOn === true) return 2;
  if (participant.speaking === true) return 3;
  if (participant.muted !== true) return 4;
  return 5;
}

/**
 * Сортирует копию списка участников по приоритету показа и режет на
 * видимую часть и переполнение. Входной массив не мутируется.
 * @param {VoiceParticipant[]} participants
 * @param {string | null} [focusedId]
 * @returns {{ visible: VoiceParticipant[], overflow: VoiceParticipant[], grid: VoiceGridDimensions }}
 */
export function prioritizeVoiceParticipants(participants, focusedId = null) {
  const list = Array.isArray(participants) ? participants : [];
  const focus = typeof focusedId === "string" ? focusedId : null;

  /** @type {Array<{ p: VoiceParticipant, rank: number, name: string, id: string, index: number }>} */
  const decorated = list.map((participant, index) => ({
    p: participant,
    rank: rankOf(participant, focus),
    name: typeof participant.displayName === "string" && participant.displayName.length > 0
      ? participant.displayName
      : participant.id,
    id: participant.id,
    index,
  }));

  decorated.sort((a, b) => {
    if (a.rank !== b.rank) return a.rank - b.rank;
    if (a.name !== b.name) return a.name < b.name ? -1 : 1;
    if (a.id !== b.id) return a.id < b.id ? -1 : 1;
    return a.index - b.index;
  });

  const sorted = decorated.map((entry) => entry.p);
  const grid = computeVoiceGridDimensions(sorted.length);
  return {
    visible: sorted.slice(0, grid.visibleCount),
    overflow: sorted.slice(grid.visibleCount),
    grid,
  };
}
