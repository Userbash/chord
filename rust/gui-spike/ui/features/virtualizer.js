// @ts-check
/**
 * @fileoverview Промышленный виртуализатор списков (`@tanstack/virtual-core`-совместимый API)
 * для ленты сообщений (`<MessageList />`) и панели участников (`#member-list`).
 * Поддерживает динамические замеры высот строк (`measureElement`), `overscan` и `scrollToIndex`.
 */

/**
 * @typedef {Object} VirtualItem
 * @property {number} index
 * @property {number} start
 * @property {number} end
 * @property {number} size
 * @property {string} key
 */

/**
 * @param {{
 *   count: number,
 *   estimateSize: (index: number) => number,
 *   overscan?: number,
 *   scrollOffset?: number,
 *   viewportSize?: number,
 *   getItemKey?: (index: number) => string,
 * }} options
 */
export function createVirtualizer(options) {
  let count = Math.max(0, Number(options?.count ?? 0));
  const estimateSize =
    typeof options?.estimateSize === "function" ? options.estimateSize : () => 44;
  let overscan = Math.max(0, Number(options?.overscan ?? 3));
  let scrollOffset = Math.max(0, Number(options?.scrollOffset ?? 0));
  let viewportSize = Math.max(1, Number(options?.viewportSize ?? 600));
  const getItemKey =
    typeof options?.getItemKey === "function"
      ? options.getItemKey
      : (/** @type {number} */ idx) => String(idx);

  /** @type {Map<number, number>} */
  const measuredSizes = new Map();

  /**
   * @param {number} index
   * @returns {number}
   */
  function getSizeForIndex(index) {
    const measured = measuredSizes.get(index);
    if (measured !== undefined && Number.isFinite(measured) && measured > 0) {
      return measured;
    }
    const est = Number(estimateSize(index));
    return Number.isFinite(est) && est > 0 ? est : 44;
  }

  /**
   * @returns {VirtualItem[]}
   */
  function buildAllMeasurements() {
    /** @type {VirtualItem[]} */
    const all = new Array(count);
    let offset = 0;
    for (let i = 0; i < count; i += 1) {
      const size = getSizeForIndex(i);
      const start = offset;
      const end = start + size;
      all[i] = {
        index: i,
        start,
        end,
        size,
        key: getItemKey(i),
      };
      offset = end;
    }
    return all;
  }

  return {
    /** @returns {number} */
    getTotalSize() {
      const all = buildAllMeasurements();
      const last = all[all.length - 1];
      return last ? last.end : 0;
    },

    /** @returns {VirtualItem[]} */
    getVirtualItems() {
      if (count <= 0) return [];
      const all = buildAllMeasurements();
      const viewStart = scrollOffset;
      const viewEnd = scrollOffset + viewportSize;

      let startIdx = 0;
      while (startIdx < all.length - 1 && (all[startIdx]?.end ?? 0) <= viewStart) {
        startIdx += 1;
      }
      let endIdx = startIdx;
      while (endIdx < all.length - 1 && (all[endIdx]?.end ?? 0) < viewEnd) {
        endIdx += 1;
      }

      const from = Math.max(0, startIdx - overscan);
      const to = Math.min(all.length - 1, endIdx + overscan);
      return all.slice(from, to + 1);
    },

    /**
     * @param {number} index
     * @param {number} size
     */
    measureElement(index, size) {
      if (index < 0 || index >= count) return;
      const num = Number(size);
      if (Number.isFinite(num) && num > 0) {
        measuredSizes.set(index, num);
      }
    },

    /**
     * @param {number} nextOffset
     * @param {number} [nextViewportSize]
     */
    setScrollWindow(nextOffset, nextViewportSize) {
      scrollOffset = Math.max(0, Number(nextOffset) || 0);
      if (nextViewportSize !== undefined && Number(nextViewportSize) > 0) {
        viewportSize = Number(nextViewportSize);
      }
    },

    /**
     * @param {number} nextCount
     */
    setCount(nextCount) {
      count = Math.max(0, Number(nextCount) || 0);
    },

    /**
     * @param {number} index
     * @param {{ align?: "start" | "center" | "end" }} [opts]
     * @returns {number}
     */
    scrollToIndex(index, opts = {}) {
      if (count <= 0) return 0;
      const clamped = Math.max(0, Math.min(count - 1, index));
      const all = buildAllMeasurements();
      const target = all[clamped];
      if (!target) return 0;
      const align = opts.align || "start";
      let nextOffset = target.start;
      if (align === "center") {
        nextOffset = target.start - viewportSize / 2 + target.size / 2;
      } else if (align === "end") {
        nextOffset = target.end - viewportSize;
      }
      const maxOffset = Math.max(0, (all[all.length - 1]?.end ?? 0) - viewportSize);
      scrollOffset = Math.max(0, Math.min(maxOffset, nextOffset));
      return scrollOffset;
    },
  };
}
