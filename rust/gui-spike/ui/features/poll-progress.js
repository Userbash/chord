// @ts-check
/**
 * Poll (опрос) presentation helpers: aggregated results, Russian expiry text and
 * answer-selection toggling for single- and multi-select polls.
 * @module ui/features/poll-progress
 */

/**
 * @typedef {{ answerId: string, text: string }} PollAnswer
 * @typedef {{ id: string, count?: number, meVoted?: boolean }} PollCount
 * @typedef {{
 *   answerId: string,
 *   text: string,
 *   count: number,
 *   percentage: number,
 *   meVoted: boolean,
 *   isLeading: boolean,
 * }} PollResultItem
 * @typedef {{ totalVotes: number, items: PollResultItem[] }} PollResults
 */

/**
 * Aggregate poll answers with their vote counts.
 *
 * `percentage` is rounded to the nearest integer over the total vote count and is
 * `0` while nobody has voted. `isLeading` marks every answer tied at the maximum
 * count; with `totalVotes === 0` no answer leads.
 *
 * @param {PollAnswer[]} answers
 * @param {PollCount[]} counts
 * @returns {PollResults}
 */
export function computePollResults(answers, counts) {
  const safeAnswers = Array.isArray(answers) ? answers : [];
  const safeCounts = Array.isArray(counts) ? counts : [];

  /** @type {Map<string, PollCount>} */
  const byId = new Map();
  for (const entry of safeCounts) {
    if (entry && typeof entry.id === 'string') byId.set(entry.id, entry);
  }

  /** @type {Array<{ answerId: string, text: string, count: number, meVoted: boolean }>} */
  const bare = safeAnswers.map((answer) => {
    const match = byId.get(answer.answerId);
    const raw = match && Number.isFinite(match.count) ? Number(match.count) : 0;
    return {
      answerId: answer.answerId,
      text: typeof answer.text === 'string' ? answer.text : '',
      count: raw,
      meVoted: Boolean(match && match.meVoted),
    };
  });

  const totalVotes = bare.reduce((sum, item) => sum + item.count, 0);
  const maxCount = bare.reduce((max, item) => Math.max(max, item.count), 0);

  const items = bare.map((item) => ({
    answerId: item.answerId,
    text: item.text,
    count: item.count,
    percentage: totalVotes > 0 ? Math.round((item.count * 100) / totalVotes) : 0,
    meVoted: item.meVoted,
    isLeading: totalVotes > 0 && item.count === maxCount,
  }));

  return { totalVotes, items };
}

const MINUTE_MS = 60_000;
const HOUR_MS = 3_600_000;
const DAY_MS = 86_400_000;

/**
 * Human-readable Russian countdown for a poll expiry timestamp.
 *
 * @param {number | null | undefined} expiryMs
 * @param {number} [nowMs]
 * @returns {string}
 */
export function formatPollExpiryRu(expiryMs, nowMs = Date.now()) {
  if (expiryMs == null || !Number.isFinite(expiryMs) || expiryMs <= nowMs) {
    return 'Опрос завершён';
  }
  const diffMs = expiryMs - nowMs;
  if (diffMs < MINUTE_MS) return 'Осталось меньше минуты';
  if (diffMs < HOUR_MS) return 'Осталось ' + Math.ceil(diffMs / MINUTE_MS) + ' мин.';
  if (diffMs < DAY_MS) return 'Осталось ' + Math.ceil(diffMs / HOUR_MS) + ' ч.';
  return 'Осталось ' + Math.ceil(diffMs / DAY_MS) + ' дн.';
}

/**
 * Toggle one answer in a poll selection.
 *
 * Without multiselect the selection behaves like a radio group (pick or clear).
 * With multiselect the answer is removed if already selected, otherwise appended
 * while the selection is below `maxChoices`; at the limit an unchanged copy is
 * returned. The input array is never mutated.
 *
 * @param {string[]} selectedIds
 * @param {string} answerId
 * @param {boolean} [allowMultiselect]
 * @param {number} [maxChoices]
 * @returns {string[]}
 */
export function togglePollAnswerSelection(selectedIds, answerId, allowMultiselect = false, maxChoices = 10) {
  const current = Array.isArray(selectedIds) ? selectedIds.slice() : [];

  if (!allowMultiselect) {
    return current.includes(answerId) ? [] : [answerId];
  }

  if (current.includes(answerId)) {
    return current.filter((id) => id !== answerId);
  }
  if (current.length < maxChoices) {
    current.push(answerId);
  }
  return current;
}
