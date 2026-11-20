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
import { createVirtualizer, Virtualizer } from "./virtualizer.js";

export const isTanstackVirtualPowered = true;
export { createVirtualizer, Virtualizer };

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
   *   onScroll?: (info: { scrollTop: number, totalHeight: number, clientHeight: number, firstVisibleIndex: number, atBottom: boolean }) => void,
   *   overscan?: number,
   * }} options
   */
  constructor({ container, source, heights, render, onError, onScroll, overscan }) {
    this.container = container;
    this.source = source;
    this.heights = heights;
    this.render = render;
    this.onError = onError ?? (() => {});
    this.onScroll = onScroll ?? (() => {});
    const doc = container.ownerDocument ?? (typeof document !== "undefined" ? document : null);
    if (!doc) throw new UIError("NO_DOCUMENT", "VirtualList requires a document");
    this.doc = doc;
    this.sizer = doc.createElement("div");
    this.sizer.style.position = "relative";
    container.appendChild(this.sizer);
    /** @type {ReturnType<typeof createVirtualizer>} */
    this.virtualizer = createVirtualizer({
      count: source.totalRows,
      estimateSize: (index) => heights.height(index),
      overscan: overscan ?? OVERSCAN,
      scrollOffset: container.scrollTop || 0,
      viewportSize: Math.max(1, container.clientHeight || 0),
    });
    this.sizer.style.height = `${this.virtualizer.getTotalSize()}px`;
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
    const height = Math.max(1, this.container.clientHeight || 0);
    this.virtualizer.setScrollWindow(top, height);
    const totalSize = this.virtualizer.getTotalSize();
    // За концом списка окно пустое: не рисуем строки по клампу — иначе тест
    // «граничный скролл» и реальный overscroll показывали бы фантомный низ.
    if (total <= 0 || top >= totalSize) return { first: total, last: total };
    const items = this.virtualizer.getVirtualItems();
    if (items.length === 0) return { first: 0, last: 0 };
    return {
      first: items[0]?.index ?? 0,
      last: (items[items.length - 1]?.index ?? 0) + 1,
    };
  }

  /**
   * Замерить реальную высоту строки для динамической виртуализации.
   * @param {number} index
   * @param {number} size
   */
  measureElement(index, size) {
    const prevTotal = this.virtualizer.getTotalSize();
    this.virtualizer.measureElement(index, size);
    const nextTotal = this.virtualizer.getTotalSize();
    if (nextTotal !== prevTotal) {
      this.sizer.style.height = `${nextTotal}px`;
    }
  }

  /** @private */
  notifyScroll() {
    const scrollTop = this.container.scrollTop || 0;
    const clientHeight = this.container.clientHeight || 0;
    const totalHeight = this.virtualizer.getTotalSize();
    const atBottom = totalHeight <= clientHeight || scrollTop + clientHeight >= totalHeight - 48;
    const items = this.virtualizer.getVirtualItems();
    const found = items.find((it) => it.end > scrollTop);
    const firstVisibleIndex = found ? found.index : this.heights.indexAt(scrollTop);
    this.onScroll({
      scrollTop,
      totalHeight,
      clientHeight,
      firstVisibleIndex,
      atBottom,
    });
  }

  paint() {
    const top = this.container.scrollTop;
    const height = Math.max(1, this.container.clientHeight || 0);
    this.virtualizer.setScrollWindow(top, height);
    const totalSize = this.virtualizer.getTotalSize();
    this.sizer.style.height = `${totalSize}px`;

    if (this.source.totalRows <= 0 || top >= totalSize) {
      for (const node of this.rows.values()) node.remove();
      this.rows.clear();
      this.notifyScroll();
      return;
    }

    const virtualItems = this.virtualizer.getVirtualItems();
    const activeIndices = new Set(virtualItems.map((it) => it.index));

    for (const [index, node] of this.rows) {
      if (!activeIndices.has(index)) {
        node.remove();
        this.rows.delete(index);
      }
    }
    if (virtualItems.length === 0) {
      this.notifyScroll();
      return;
    }

    const first = virtualItems[0]?.index ?? 0;
    const last = (virtualItems[virtualItems.length - 1]?.index ?? 0) + 1;

    /** @type {number[]} */
    const missing = [];
    for (const vItem of virtualItems) {
      const index = vItem.index;
      const item = this.source.getRow(index);
      let node = this.rows.get(index);
      if (!node) {
        const d = this.doc ?? (typeof document !== "undefined" ? document : null);
        if (!d) continue;
        node = d.createElement("div");
        node.style.position = "absolute";
        node.style.left = "0";
        node.style.right = "0";
        this.sizer.appendChild(node);
        this.rows.set(index, node);
      }
      node.style.top = `${vItem.start}px`;
      node.style.height = `${vItem.size}px`;
      const state = item === undefined ? "placeholder" : "row";
      if (node.dataset.state !== state) {
        node.dataset.state = state;
        this.render(node, item === undefined ? null : item, index);
        if (state === "row" && typeof node.scrollHeight === "number" && node.scrollHeight > vItem.size) {
          this.measureElement(index, node.scrollHeight);
        }
      }
      if (state === "placeholder") missing.push(index);
    }
    if (missing.length > 0) this.requestWindow(first, last, missing);
    this.notifyScroll();
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
   * @param {{ align?: "start" | "center" | "end" | "auto", behavior?: "auto" | "smooth" }} [opts]
   */
  scrollToIndex(index, opts = {}) {
    const total = this.source.totalRows;
    if (total <= 0) return;
    const clamped = Math.max(0, Math.min(total - 1, index));
    this.virtualizer.setScrollWindow(this.container.scrollTop, Math.max(1, this.container.clientHeight || 0));
    const nextOffset = this.virtualizer.scrollToIndex(clamped, opts);
    if (opts.behavior === "smooth" && typeof this.container.scrollTo === "function") {
      this.container.scrollTo({ top: nextOffset, behavior: "smooth" });
    } else {
      this.container.scrollTop = nextOffset;
    }
    this.paint();
  }

  /** @returns {number} индекс первой видимой строки (для клавиатуры) */
  firstVisibleIndex() {
    this.virtualizer.setScrollWindow(this.container.scrollTop, Math.max(1, this.container.clientHeight || 0));
    const items = this.virtualizer.getVirtualItems();
    const top = this.container.scrollTop;
    const found = items.find((it) => it.end > top);
    return found ? found.index : this.heights.indexAt(top);
  }

  /** @returns {number} высота sizer, px (для диагностики/снапшота) */
  get totalHeight() {
    return this.virtualizer.getTotalSize();
  }
}
