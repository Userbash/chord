// @ts-check
/**
 * Поиск по загруженному окну данных (chat/member search).
 *
 * Честная граница: в JS-heap живёт ≤500 строк (ADR-1), полный каталог — за
 * серверным поиском (Phase 3). Поэтому ищем только по загруженным страницам и
 * показываем `найдено N из M загруженных`, а не выдаём это за поиск по 100 000.
 */
import { UIError, reportError } from "./errors.js";

/**
 * @typedef {import("../model.js").PageStore} PageStore
 * @typedef {import("./virtual-list.js").VirtualList} VirtualList
 */

/**
 * Подпись счётчика: «найдено N из M загруженных» (W39h: честная граница окна).
 * @param {number} matches
 * @param {number} loadedRows
 * @returns {string}
 */
export function formatFound(matches, loadedRows) {
  return `найдено ${matches} из ${loadedRows} загруженных`;
}

/**
 * @param {PageStore} store
 * @param {number} pageSize
 * @param {(row: any, index: number) => boolean} predicate
 * @returns {{ first: number | null, matches: number, loadedRows: number }}
 */
export function scanLoadedRows(store, pageSize, predicate) {
  /** @type {number | null} */
  let first = null;
  let matches = 0;
  let loadedRows = 0;
  for (const [pageIndex, page] of store.pages) {
    for (let offset = 0; offset < page.rows.length; offset++) {
      const index = pageIndex * pageSize + offset;
      loadedRows += 1;
      if (!predicate(page.rows[offset], index)) continue;
      matches += 1;
      if (first === null) first = index;
    }
  }
  return { first, matches, loadedRows };
}

/**
 * Подписка на `input` списка: прыжок к первому совпадению + счётчик.
 * @param {{
 *   input: HTMLInputElement,
 *   list: VirtualList,
 *   store: PageStore,
 *   pageSize: number,
 *   countLabel: HTMLElement | null,
 *   totalLabel: string,
 *   predicate: (row: any, index: number, query: string) => boolean,
 *   onResult?: (found: { first: number | null, matches: number, loadedRows: number }) => void,
 *   onError?: (error: UIError) => void,
 * }} options
 */
export function bindListSearch(options) {
  const { input, list, store, pageSize, countLabel, totalLabel, predicate } = options;
  const onError = options.onError ?? (() => {});
  input.addEventListener("input", () => {
    try {
      const query = input.value.trim().toLowerCase();
      for (const row of list.rows.values()) row.classList.remove("is-selected");
      if (query === "") {
        if (countLabel) countLabel.textContent = totalLabel;
        options.onResult?.({ first: null, matches: 0, loadedRows: 0 });
        return;
      }
      const found = scanLoadedRows(store, pageSize, (row, index) => predicate(row, index, query));
      // Счётчик виден и озвучивается (aria-live="polite" у счётчиков).
      if (countLabel) countLabel.textContent = formatFound(found.matches, found.loadedRows);
      options.onResult?.(found);
      if (found.first === null) return;
      list.scrollToIndex(found.first);
      const target = list.rows.get(found.first) ?? null;
      if (target) target.classList.add("is-selected");
    } catch (error) {
      onError(reportError("ACTION_FAILED", error));
    }
  });
}
