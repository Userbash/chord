// @ts-check
/**
 * T-190: чистая логика фильтрации и сортировки тредов форума.
 *
 * Модуль без DOM: принимает список тредов и опции панели фильтров,
 * возвращает новый массив. Входные данные не мутируются.
 *
 * Контракт:
 * - закреплённые треды всегда идут первыми (порядок сортировки внутри
 *   группы закреплённых сохраняется);
 * - `matchMode: "any"` (по умолчанию) — хотя бы один выбранный тег,
 *   `matchMode: "all"` — все выбранные теги;
 * - `query` сравнивается регистронезависимой подстрокой с `title`
 *   после `trim()`;
 * - `sortBy`: `latest_activity` (по умолчанию) — убывание
 *   `lastActivityMs ?? createdAtMs ?? 0`; `creation_date` — убывание
 *   `createdAtMs ?? 0`; тай-брейкер — `String(b.id).localeCompare(String(a.id))`.
 */

/**
 * @typedef {object} ForumThread
 * @property {string} id
 * @property {string} title
 * @property {string[]} tagIds
 * @property {boolean} [pinned]
 * @property {number} [lastActivityMs]
 * @property {number} [createdAtMs]
 * @property {number} [messageCount]
 */

/**
 * @typedef {object} ForumFilterOptions
 * @property {string[]} [selectedTagIds]
 * @property {"any" | "all"} [matchMode]
 * @property {string} [query]
 * @property {string} [sortBy]
 */

/** @type {Readonly<{ LATEST_ACTIVITY: "latest_activity", CREATION_DATE: "creation_date" }>} */
export const FORUM_SORT_MODES = Object.freeze({
  LATEST_ACTIVITY: /** @type {const} */ ("latest_activity"),
  CREATION_DATE: /** @type {const} */ ("creation_date"),
});

/**
 * @param {ForumThread} thread
 * @param {string[]} selectedTagIds
 * @param {"any" | "all"} matchMode
 * @returns {boolean}
 */
function matchesTags(thread, selectedTagIds, matchMode) {
  if (selectedTagIds.length === 0) return true;
  const tags = Array.isArray(thread.tagIds) ? thread.tagIds : [];
  if (matchMode === "all") {
    return selectedTagIds.every((tagId) => tags.includes(tagId));
  }
  return selectedTagIds.some((tagId) => tags.includes(tagId));
}

/**
 * @param {ForumThread} thread
 * @param {string} needle
 * @returns {boolean}
 */
function matchesQuery(thread, needle) {
  if (needle === "") return true;
  const title = typeof thread.title === "string" ? thread.title : "";
  return title.toLowerCase().includes(needle);
}

/**
 * @param {ForumThread} thread
 * @param {string} sortBy
 * @returns {number}
 */
function sortValue(thread, sortBy) {
  if (sortBy === FORUM_SORT_MODES.CREATION_DATE) {
    return thread.createdAtMs ?? 0;
  }
  return thread.lastActivityMs ?? thread.createdAtMs ?? 0;
}

/**
 * Фильтрует и сортирует треды форума.
 *
 * @param {ForumThread[]} threads
 * @param {ForumFilterOptions} [options]
 * @returns {ForumThread[]}
 */
export function filterAndSortForumThreads(threads, options = {}) {
  if (!Array.isArray(threads)) return [];
  const selectedTagIds = Array.isArray(options.selectedTagIds)
    ? options.selectedTagIds.filter((tagId) => typeof tagId === "string")
    : [];
  const matchMode = options.matchMode === "all" ? "all" : "any";
  const needle = typeof options.query === "string" ? options.query.trim().toLowerCase() : "";
  const sortBy =
    options.sortBy === FORUM_SORT_MODES.CREATION_DATE
      ? FORUM_SORT_MODES.CREATION_DATE
      : FORUM_SORT_MODES.LATEST_ACTIVITY;

  const filtered = threads.filter(
    (thread) =>
      thread != null &&
      matchesTags(thread, selectedTagIds, matchMode) &&
      matchesQuery(thread, needle),
  );

  return filtered.sort((a, b) => {
    const aPinned = a.pinned === true ? 1 : 0;
    const bPinned = b.pinned === true ? 1 : 0;
    if (aPinned !== bPinned) return bPinned - aPinned;

    const aValue = sortValue(a, sortBy);
    const bValue = sortValue(b, sortBy);
    if (aValue !== bValue) return aValue > bValue ? -1 : 1;

    return String(b.id).localeCompare(String(a.id));
  });
}

/**
 * Считает количество тредов на каждый тег. Тег учитывается не более
 * одного раза на тред.
 *
 * @param {ForumThread[]} threads
 * @returns {Record<string, number>}
 */
export function countThreadsByTag(threads) {
  /** @type {Record<string, number>} */
  const counts = {};
  if (!Array.isArray(threads)) return counts;
  for (const thread of threads) {
    if (thread == null || !Array.isArray(thread.tagIds)) continue;
    const seen = new Set();
    for (const tagId of thread.tagIds) {
      if (typeof tagId !== "string" || seen.has(tagId)) continue;
      seen.add(tagId);
      counts[tagId] = (counts[tagId] ?? 0) + 1;
    }
  }
  return counts;
}
