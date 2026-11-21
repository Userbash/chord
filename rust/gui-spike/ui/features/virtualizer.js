// @ts-check
/**
 * @fileoverview Промышленный виртуализатор списков на базе проверенной ESM-библиотеки `@tanstack/virtual-core`
 * для ленты сообщений (`<MessageList />`) и панели участников (`#member-list`).
 * Поддерживает динамические замеры высот строк (`measureElement`), `overscan` и `scrollToIndex`.
 */

import {
  Virtualizer,
  elementScroll,
  defaultRangeExtractor,
  defaultKeyExtractor,
} from "@tanstack/virtual-core";

export const isTanstackVirtualPowered = true;
export { Virtualizer, elementScroll, defaultRangeExtractor, defaultKeyExtractor };

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

  const virtualizer = new Virtualizer({
    count,
    estimateSize,
    overscan,
    getItemKey,
    getScrollElement: () => null,
    scrollToFn: (offset) => {
      scrollOffset = offset;
    },
    observeElementRect: (instance, cb) => {
      cb({ width: 800, height: viewportSize });
      return () => {};
    },
    observeElementOffset: (instance, cb) => {
      cb(scrollOffset, false);
      return () => {};
    },
  });

  virtualizer.scrollRect = { width: 800, height: viewportSize };
  virtualizer.scrollOffset = scrollOffset;

  return {
    virtualizer,

    /** @returns {number} */
    getTotalSize() {
      return virtualizer.getTotalSize();
    },

    /** @returns {VirtualItem[]} */
    getVirtualItems() {
      if (count <= 0) return [];
      /** @type {any[]} */
      const items = virtualizer.getVirtualItems();
      return items.map((it) => ({
        index: it.index,
        start: it.start,
        end: it.end,
        size: it.size,
        key: String(it.key),
      }));
    },

    /**
     * @param {number} index
     * @param {number} size
     */
    measureElement(index, size) {
      if (index < 0 || index >= count) return;
      const num = Number(size);
      if (Number.isFinite(num) && num >= 0) {
        virtualizer.resizeItem(index, num);
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
        virtualizer.scrollRect = { width: 800, height: viewportSize };
      }
      virtualizer.scrollOffset = scrollOffset;
    },

    /**
     * @param {number} nextCount
     */
    setCount(nextCount) {
      count = Math.max(0, Number(nextCount) || 0);
      virtualizer.setOptions({
        ...virtualizer.options,
        count,
      });
    },

    /**
     * @param {number} index
     * @param {{ align?: "start" | "center" | "end" | "auto" }} [opts]
     * @returns {number}
     */
    scrollToIndex(index, opts = {}) {
      if (count <= 0) return 0;
      const clamped = Math.max(0, Math.min(count - 1, index));
      const m = /** @type {any} */ (virtualizer).getMeasurements()[clamped];
      if (!m) return 0;
      const align = opts.align || "start";
      let nextOffset = m.start;
      if (align === "auto") {
        if (m.start < scrollOffset) nextOffset = m.start;
        else if (m.end > scrollOffset + viewportSize) nextOffset = m.end - viewportSize;
        else nextOffset = scrollOffset;
      } else if (align === "center") {
        nextOffset = m.start - viewportSize / 2 + m.size / 2;
      } else if (align === "end") {
        nextOffset = m.end - viewportSize;
      }
      const maxOffset = Math.max(0, virtualizer.getTotalSize() - viewportSize);
      scrollOffset = Math.max(0, Math.min(maxOffset, nextOffset));
      virtualizer.scrollOffset = scrollOffset;
      return scrollOffset;
    },
  };
}
