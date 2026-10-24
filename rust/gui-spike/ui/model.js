// @ts-check
/**
 * Оконная/постраничная модель данных GUI-спайка (ADR-1, RW-37).
 *
 * Зачем: спайк держал в JS-heap все 124 229 объекта (100k сообщений + 24 229
 * участников) — PSS 394 МБ при бюджете 300 МБ (`waves/RW-36/METRICS.md`).
 * `PageStore` держит только окно страниц (`pageSize * maxPages` строк), остальное
 * подгружается по требованию.
 *
 * Контракт заморожен в `waves/RW-37/TZ.md` §4:
 * ```js
 * new PageStore({ totalRows, pageSize, maxPages, fetchPage })
 * getRow(index) -> row | undefined
 * ensureRange(first, last) -> Promise<void>   // границы включительно
 * stats() -> { pages, loadedPages, loadedRows, maxPages, pageSize, totalRows }
 * ```
 *
 * Семантика диапазона: `first` и `last` — индексы строк включительно. Вызов
 * кламплится к `[0, totalRows - 1]`; диапазон целиком вне данных — no-op.
 * Диапазон, требующий больше `maxPages` страниц, отвергается `RangeError`:
 * бюджет памяти важнее одного запроса, молчаливая частичная выдача скрыла бы
 * ошибку вызывающего.
 */

/** @typedef {any} Row */

/**
 * @param {string} name
 * @param {unknown} value
 * @returns {number}
 */
function requirePositiveInt(name, value) {
  if (typeof value !== "number" || !Number.isInteger(value) || value <= 0) {
    throw new TypeError(`${name} должен быть целым > 0, получено: ${String(value)}`);
  }
  return value;
}

export class PageStore {
  /**
   * @param {{
   *   totalRows: number,
   *   pageSize: number,
   *   maxPages: number,
   *   fetchPage: (pageIndex: number) => Row[] | Promise<Row[]>,
   * }} options
   */
  constructor(options) {
    if (options === null || typeof options !== "object") {
      throw new TypeError("PageStore: нужен объект { totalRows, pageSize, maxPages, fetchPage }");
    }
    this.totalRows = requirePositiveInt("totalRows", options.totalRows);
    this.pageSize = requirePositiveInt("pageSize", options.pageSize);
    this.maxPages = requirePositiveInt("maxPages", options.maxPages);
    if (typeof options.fetchPage !== "function") {
      throw new TypeError("PageStore: fetchPage должен быть функцией");
    }
    this.fetchPage = options.fetchPage;
    /** Всего страниц в сторе (не путать с `loadedPages`). */
    this.pageCount = Math.ceil(this.totalRows / this.pageSize);
    /** @type {Map<number, { rows: Row[], touched: number }>} */
    this.pages = new Map();
    /** Кэш промисов: одна загрузка на страницу даже при гонке вызовов. @type {Map<number, Promise<void>>} */
    this.inFlight = new Map();
    /** Монотонные часы LRU: больше — свежее. */
    this.clock = 0;
  }

  /**
   * Строка окна или `undefined`, если страница ещё не загружена / индекс вне данных.
   * @param {number} index
   * @returns {Row | undefined}
   */
  getRow(index) {
    if (!Number.isInteger(index) || index < 0 || index >= this.totalRows) return undefined;
    const pageIndex = Math.floor(index / this.pageSize);
    const page = this.pages.get(pageIndex);
    if (!page) return undefined;
    page.touched = ++this.clock;
    return page.rows[index - pageIndex * this.pageSize];
  }

  /**
   * Догружает страницы, покрывающие `[first, last]` (включительно), и эвиктит
   * лишние по LRU, не трогая запрошенный диапазон. Идемпотентен.
   * @param {number} first
   * @param {number} last
   * @returns {Promise<void>}
   */
  async ensureRange(first, last) {
    if (!Number.isInteger(first) || !Number.isInteger(last)) {
      throw new TypeError(
        `ensureRange: нужны целые first/last, получено ${String(first)}..${String(last)}`,
      );
    }
    if (last < first || first >= this.totalRows) return;
    const from = Math.max(0, first);
    const to = Math.min(this.totalRows - 1, last);
    if (to < from) return;

    const firstPage = Math.floor(from / this.pageSize);
    const lastPage = Math.floor(to / this.pageSize);
    const needed = lastPage - firstPage + 1;
    if (needed > this.maxPages) {
      throw new RangeError(
        `ensureRange(${from}, ${to}): нужно ${needed} страниц, maxPages=${this.maxPages}`,
      );
    }

    /** @type {Set<number>} */
    const rangePages = new Set();
    for (let pageIndex = firstPage; pageIndex <= lastPage; pageIndex++) {
      rangePages.add(pageIndex);
    }

    /** @type {Promise<void>[]} */
    const tasks = [];
    for (const pageIndex of rangePages) {
      const loaded = this.pages.get(pageIndex);
      if (loaded) {
        loaded.touched = ++this.clock;
        continue;
      }
      let task = this.inFlight.get(pageIndex);
      if (!task) {
        task = this.loadPage(pageIndex, rangePages);
      }
      tasks.push(task);
    }
    await Promise.all(tasks);
    this.evict(rangePages);
  }

  /**
   * Грузит одну страницу и кэширует промис до первого await (см. `inFlight`).
   * @param {number} pageIndex
   * @param {Set<number>} pinned
   * @returns {Promise<void>}
   */
  loadPage(pageIndex, pinned) {
    const task = Promise.resolve()
      .then(() => this.fetchPage(pageIndex))
      .then((rows) => {
        if (!Array.isArray(rows)) {
          throw new TypeError(`fetchPage(${pageIndex}) должен вернуть массив строк`);
        }
        const capacity = Math.min(
          this.pageSize,
          this.totalRows - pageIndex * this.pageSize,
        );
        // R1/P2-2: недогрузка — это дыра в окне навсегда: getRow отдаёт undefined,
        // виртуализация считает строку «не загруженной» и повторно не грузит.
        // Требуем точную вместимость для ВСЕХ страниц: полная страница — pageSize,
        // последняя — остаток totalRows. Пустая последняя страница тоже ошибка,
        // как и перегрузка страницы.
        if (rows.length !== capacity) {
          throw new RangeError(
            `fetchPage(${pageIndex}): ожидалось ${capacity} строк (capacity страницы), получено ${rows.length}`,
          );
        }
        this.pages.set(pageIndex, { rows, touched: ++this.clock });
        this.evict(pinned);
      })
      .finally(() => {
        this.inFlight.delete(pageIndex);
      });
    this.inFlight.set(pageIndex, task);
    return task;
  }

  /**
   * LRU-эвикция до `maxPages`; страницы текущего диапазона не вытесняются.
   * @param {Set<number>} pinned
   */
  evict(pinned) {
    while (this.pages.size > this.maxPages) {
      /** @type {number | null} */
      let victim = null;
      let oldest = Number.POSITIVE_INFINITY;
      for (const [pageIndex, page] of this.pages) {
        if (pinned.has(pageIndex) || page.touched >= oldest) continue;
        oldest = page.touched;
        victim = pageIndex;
      }
      if (victim === null) {
        // Недостижимо: ensureRange отвергает диапазоны шире maxPages. Если всё же
        // случилось — не трогаем бюджет молча, а сообщаем о нарушении инварианта.
        console.warn(
          `PageStore: нечего вытеснять (pages=${this.pages.size}, maxPages=${this.maxPages})`,
        );
        return;
      }
      this.pages.delete(victim);
    }
  }

  /**
   * Снимок состояния для диагностики и UI_SNAPSHOT.
   * @returns {{ pages: number, loadedPages: number, loadedRows: number, maxPages: number, pageSize: number, totalRows: number }}
   */
  stats() {
    let loadedRows = 0;
    for (const page of this.pages.values()) loadedRows += page.rows.length;
    return {
      pages: this.pageCount,
      loadedPages: this.pages.size,
      loadedRows,
      maxPages: this.maxPages,
      pageSize: this.pageSize,
      totalRows: this.totalRows,
    };
  }
}

/* ---------- D-04: переменная высота строк (RE-оракул RW-37) ---------- */

/**
 * Высоты строк из `waves/RW-37/RE/layout.json` (`row_heights` /
 * `row_heights_detail`). Прежние 44/40 в DOM-контракте RW-38 — приближение;
 * D-04 заменяет их: сообщение 26 (продолжение группы) / 48 (начало) / +22 за
 * каждую перенесённую строку, участник 42 (и в панели, и в списке).
 */
export const ROW_HEIGHTS = Object.freeze({
  /** `row_heights.message_cozy` — продолжение группы. */
  continuation: 26,
  /** `row_heights_detail.message_cozy_group_start` — начало группы. */
  groupStart: 48,
  /** `row_heights_detail.message_extra_line` — каждая перенесённая строка. */
  lineExtra: 22,
  /** `row_heights.member` — строка участника. */
  member: 42,
});

/** Демо-данные спайка: группа из 4 сообщений, первое — начало группы. */
export const CHAT_GROUP_SIZE = 4;
/** Перенос строки: каждая 17-я строка (i % 17 === 9) даёт +1 строку текста. */
export const CHAT_WRAP_MOD = 17;
/** Второй перенос: каждая 23-я строка (i % 23 === 9) даёт ещё +1 строку. */
export const CHAT_WRAP2_MOD = 23;
/** Смещение внутри модуля, на котором сообщение переносится. */
export const CHAT_WRAP_AT = 9;
/** Период повторения высот: lcm(4, 17, 23) — кратно всем правилам демо-данных. */
export const CHAT_HEIGHT_PERIOD = 4 * 17 * 23;

/**
 * Сколько визуальных строк занимает сообщение с индексом `index`.
 * @param {number} index
 * @returns {number} 1..3
 */
export function messageLines(index) {
  if (!Number.isInteger(index) || index < 0) {
    throw new RangeError(`messageLines: индекс должен быть целым ≥ 0, получено ${String(index)}`);
  }
  return (
    1 +
    (index % CHAT_WRAP_MOD === CHAT_WRAP_AT ? 1 : 0) +
    (index % CHAT_WRAP2_MOD === CHAT_WRAP_AT ? 1 : 0)
  );
}

/**
 * Начало ли это группы (`data-group="start"`) или продолжение.
 * @param {number} index
 * @returns {"start" | "continuation"}
 */
export function messageGroup(index) {
  if (!Number.isInteger(index) || index < 0) {
    throw new RangeError(`messageGroup: индекс должен быть целым ≥ 0, получено ${String(index)}`);
  }
  return index % CHAT_GROUP_SIZE === 0 ? "start" : "continuation";
}

/**
 * Высота строки чата по индексу: 48 (начало) / 26 (продолжение) + 22 за перенос.
 * @param {number} index
 * @returns {number} px
 */
export function messageHeight(index) {
  const base = messageGroup(index) === "start" ? ROW_HEIGHTS.groupStart : ROW_HEIGHTS.continuation;
  return base + (messageLines(index) - 1) * ROW_HEIGHTS.lineExtra;
}

/**
 * Высота строки участника (D-04: 42 и в панели, и в виртуализации).
 * @returns {number} px
 */
export function memberHeight() {
  return ROW_HEIGHTS.member;
}

/**
 * Индекс высот: `height(i)`, `top(i)`, `indexAt(offset)` и `totalHeight`.
 *
 * Высоты обязаны быть периодичными с периодом `period` — тогда префиксные суммы
 * не нужно хранить на все 100 000 строк: достаточно одного периода. Нарушение
 * периодичности — `RangeError` в конструкторе (видимая ошибка, а не тихая
 * неверная геометрия списка).
 */
export class HeightIndex {
  /**
   * @param {number} totalRows
   * @param {(index: number) => number} heightAt
   * @param {number} period
   */
  constructor(totalRows, heightAt, period) {
    if (!Number.isInteger(totalRows) || totalRows <= 0) {
      throw new TypeError(`HeightIndex: totalRows должен быть целым > 0, получено ${String(totalRows)}`);
    }
    if (typeof heightAt !== "function") {
      throw new TypeError("HeightIndex: heightAt должен быть функцией");
    }
    if (!Number.isInteger(period) || period <= 0) {
      throw new TypeError(`HeightIndex: period должен быть целым > 0, получено ${String(period)}`);
    }
    this.totalRows = totalRows;
    this.heightAt = heightAt;
    this.period = period;
    /** @type {Float64Array} */
    this.prefix = new Float64Array(period + 1);
    for (let index = 0; index < period; index++) {
      const height = heightAt(index);
      if (typeof height !== "number" || !Number.isFinite(height) || height <= 0) {
        throw new RangeError(`HeightIndex: heightAt(${index}) = ${String(height)} — нужна высота > 0`);
      }
      this.prefix[index + 1] = /** @type {number} */ (this.prefix[index]) + height;
    }
    this.cycleHeight = /** @type {number} */ (this.prefix[period]);
    // Периодичность проверяем на границах периода и в конце списка: этого хватает,
    // чтобы поймать дрейф правила высот (и не сканировать 100k строк на старте).
    for (const index of [period - 1, period, period + 1, 2 * period + 5, totalRows - 1]) {
      if (index < 0 || index >= totalRows) continue;
      if (heightAt(index) !== heightAt(index % period)) {
        throw new RangeError(
          `HeightIndex: высоты не периодичны (period=${period}): heightAt(${index})=${heightAt(index)}, heightAt(${index % period})=${heightAt(index % period)}`,
        );
      }
    }
  }

  /**
   * @param {number} index
   * @returns {number}
   */
  height(index) {
    if (!Number.isInteger(index) || index < 0 || index >= this.totalRows) {
      throw new RangeError(`HeightIndex.height(${String(index)}): индекс вне [0, ${this.totalRows})`);
    }
    return /** @type {number} */ (this.heightAt(index));
  }

  /**
   * Сумма высот строк `[0, index)`; `top(totalRows) === totalHeight`.
   * @param {number} index
   * @returns {number}
   */
  top(index) {
    if (!Number.isFinite(index)) throw new TypeError(`HeightIndex.top(${String(index)}): нужно число`);
    if (index <= 0) return 0;
    if (index >= this.totalRows) return this.totalHeight;
    const cycles = Math.floor(index / this.period);
    const rest = index - cycles * this.period;
    return cycles * this.cycleHeight + /** @type {number} */ (this.prefix[rest]);
  }

  /** @returns {number} высота всего списка, px */
  get totalHeight() {
    const cycles = Math.floor(this.totalRows / this.period);
    const rest = this.totalRows - cycles * this.period;
    return cycles * this.cycleHeight + /** @type {number} */ (this.prefix[rest]);
  }

  /**
   * Индекс строки, в которую попадает вертикальное смещение `offset`
   * (обратная операция к `top`). Смещение кламплится к списку.
   * @param {number} offset
   * @returns {number}
   */
  indexAt(offset) {
    if (!Number.isFinite(offset)) throw new TypeError(`HeightIndex.indexAt(${String(offset)}): нужно число`);
    if (offset <= 0) return 0;
    if (offset >= this.totalHeight) return this.totalRows - 1;
    const cycles = Math.floor(offset / this.cycleHeight);
    const rest = offset - cycles * this.cycleHeight;
    // Наибольшее k, при котором prefix[k] <= rest (rest < cycleHeight).
    let low = 0;
    let high = this.period;
    while (low < high) {
      const middle = (low + high + 1) >> 1;
      if (/** @type {number} */ (this.prefix[middle]) <= rest) low = middle;
      else high = middle - 1;
    }
    // rest внутри последней строки периода: prefix[period-1] <= rest < prefix[period],
    // поэтому k=period означает именно последнюю строку, а не начало следующего цикла.
    const rowInCycle = low === this.period ? this.period - 1 : low;
    const index = cycles * this.period + rowInCycle;
    return Math.min(this.totalRows - 1, index);
  }
}

/**
 * Индекс высот сообщений чата (период CHAT_HEIGHT_PERIOD).
 * @param {number} totalRows
 * @returns {HeightIndex}
 */
export function createChatHeights(totalRows) {
  return new HeightIndex(totalRows, messageHeight, CHAT_HEIGHT_PERIOD);
}

/**
 * Индекс высот участников: 42 на строку.
 * @param {number} totalRows
 * @returns {HeightIndex}
 */
export function createMemberHeights(totalRows) {
  return new HeightIndex(totalRows, () => ROW_HEIGHTS.member, 1);
}
