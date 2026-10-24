// @ts-check
/**
 * Виртуализованный список с переменной высотой строк (D-04).
 *
 * Геометрия — из `model.js` (`HeightIndex`): `top(i)` и `indexAt(offset)` считают
 * префиксные суммы по периоду высот, поэтому 100 000 строк не требуют массива
 * смещений. Строки переиспользуются по индексу окна, sizer не копит отвязанные
 * узлы (`sizer.children.length === rows.size`).
 *
 * Домен: `rowHeight = 26/48/+22` (чат), `42` (участники).
 */
import { UIError, asUIError, reportError, showErrorState } from "./errors.js";

/** Строк сверх видимого окна с каждой стороны — как в RW-38. */
export const OVERSCAN = 8;

/**
 * @typedef {{
 *   totalRows: number,
 *   getRow: (index: number) => any,
 *   ensureRange: (first: number, last: number) => Promise<void>,
 * }} RowSource
 */

/**
 * @typedef {import("../model.js").HeightIndex} HeightIndex
 */

export class VirtualList {
  /**
   * @param {{
   *   container: HTMLElement,
   *   source: RowSource,
   *   heights: HeightIndex,
   *   render: (row: HTMLElement, item: any | null, index: number) => void,
   *   onError?: (error: UIError) => void,
   * }} options
   */
  constructor({ container, source, heights, render, onError }) {
    this.container = container;
    this.source = source;
    this.heights = heights;
    this.render = render;
    this.onError = onError ?? (() => {});
    this.sizer = document.createElement("div");
    this.sizer.style.position = "relative";
    this.sizer.style.height = `${heights.totalHeight}px`;
    container.appendChild(this.sizer);
    /** @type {Map<number, HTMLElement>} */
    this.rows = new Map();
    /** Последний диапазон, о котором сообщили: без спама на каждом кадре. */
    this.lastWarned = "";
    this.pending = false;
    container.addEventListener("scroll", () => this.schedule(), { passive: true });
    // Скрытый экран (display:none) имеет clientHeight=0 — первый paint нарисует
    // только overscan. ResizeObserver перерисовывает, когда контейнер получает
    // реальную высоту: показ экрана или изменение размера окна.
    if (typeof ResizeObserver !== "undefined") {
      this.observer = new ResizeObserver(() => this.paint());
      this.observer.observe(container);
    }
    this.paint();
  }

  schedule() {
    if (this.pending) return;
    this.pending = true;
    requestAnimationFrame(() => {
      this.pending = false;
      this.paint();
    });
  }

  /**
   * Диапазон видимых индексов (включительно) для текущего scrollTop.
   * @returns {{ first: number, last: number }}
   */
  visibleRange() {
    const total = this.source.totalRows;
    const top = this.container.scrollTop;
    const height = this.container.clientHeight;
    // За концом списка окно пустое: не рисуем строки по клампу — иначе тест
    // «граничный скролл» и реальный overscroll показывали бы фантомный низ.
    if (top >= this.heights.totalHeight) return { first: total, last: total };
    const first = Math.max(0, this.heights.indexAt(top) - OVERSCAN);
    const last = Math.min(total, this.heights.indexAt(top + height) + 1 + OVERSCAN);
    return { first, last };
  }

  paint() {
    const { first, last } = this.visibleRange();

    for (const [index, node] of this.rows) {
      if (index < first || index >= last) {
        node.remove();
        this.rows.delete(index);
      }
    }
    if (last <= first) return;

    /** @type {number[]} */
    const missing = [];
    for (let index = first; index < last; index++) {
      const item = this.source.getRow(index);
      let node = this.rows.get(index);
      if (!node) {
        node = document.createElement("div");
        node.style.position = "absolute";
        node.style.left = "0";
        node.style.right = "0";
        node.style.top = `${this.heights.top(index)}px`;
        node.style.height = `${this.heights.height(index)}px`;
        this.sizer.appendChild(node);
        this.rows.set(index, node);
      }
      const state = item === undefined ? "placeholder" : "row";
      if (node.dataset.state !== state) {
        node.dataset.state = state;
        this.render(node, item === undefined ? null : item, index);
      }
      if (state === "placeholder") missing.push(index);
    }
    if (missing.length > 0) this.requestWindow(first, last, missing);
  }

  /**
   * Просит модель догрузить окно, не блокируя скролл: placeholder уже нарисован,
   * по приходу данных список перерисуется.
   * @param {number} first
   * @param {number} last индекс за концом окна (exclusive)
   * @param {number[]} missing
   */
  requestWindow(first, last, missing) {
    this.source.ensureRange(first, last - 1).then(
      () => {
        // Перерисовка только если данные правда появились: иначе вечный rAF-цикл
        // (страница пустая или короче окна).
        if (missing.some((index) => this.source.getRow(index) !== undefined)) {
          this.schedule();
          return;
        }
        const key = `${first}:${last}`;
        if (this.lastWarned === key) return;
        this.lastWarned = key;
        this.fail(new UIError("WINDOW_EMPTY", `окно ${first}..${last}: страницы не дали строк`));
      },
      (/** @type {unknown} */ error) => {
        this.fail(asUIError("RANGE_FAILED", error));
      },
    );
  }

  /**
   * Единая точка видимой ошибки списка: лог + оверлей в родителе контейнера
   * (`#chat-list`/`#member-list` не могут иметь второго ребёнка — §6.1).
   * @param {UIError} error
   */
  fail(error) {
    const reported = reportError(error.code, error);
    const surface = this.container.parentElement;
    if (surface) showErrorState(surface, reported);
    this.onError(reported);
  }

  /**
   * @param {number} index
   */
  scrollToIndex(index) {
    const clamped = Math.max(0, Math.min(this.source.totalRows - 1, index));
    this.container.scrollTop = this.heights.top(clamped);
    this.paint();
  }

  /** @returns {number} индекс первой видимой строки (для клавиатуры) */
  firstVisibleIndex() {
    return this.heights.indexAt(this.container.scrollTop);
  }

  /** @returns {number} высота sizer, px (для диагностики/снапшота) */
  get totalHeight() {
    return this.heights.totalHeight;
  }
}
