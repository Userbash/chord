// @ts-check
// W39d: headless-оценка накладных расходов D-04 (переменная высота) на paint.
// Это НЕ замена живому BENCH (WINDOW-BUDGET: окно запускает только приёмка):
// измеряется только расчёт окна/смещений без layout и рендера.
//   node waves/RW-39/PM/w39d/model-bench.mjs
import assert from "node:assert/strict";
import {
  createChatHeights,
  createMemberHeights,
  CHAT_HEIGHT_PERIOD,
} from "../../../../rust/gui-spike/ui/model.js";
import { EMOJI_CATEGORIES } from "../../../../rust/gui-spike/ui/features/emoji.js";
import { SETTINGS_GROUPS } from "../../../../rust/gui-spike/ui/features/settings.js";

const CHAT_ROWS = 100_000;
const MEMBER_ROWS = 24_229;
const OVERSCAN = 8;
const WINDOW_PX = 800;
const PAINTS = 300;

const builtAt = performance.now();
const chatHeights = createChatHeights(CHAT_ROWS);
const memberHeights = createMemberHeights(MEMBER_ROWS);
const buildMs = performance.now() - builtAt;

/**
 * Один paint виртуального списка: два indexAt + top/height каждой строки окна.
 * @param {import("../../../../rust/gui-spike/ui/model.js").HeightIndex} heights
 * @param {number} scrollTop
 * @returns {{ rows: number, p95Guard: number }}
 */
function paint(heights, scrollTop) {
  const first = Math.max(0, heights.indexAt(scrollTop) - OVERSCAN);
  const last = Math.min(heights.totalRows, heights.indexAt(scrollTop + WINDOW_PX) + 1 + OVERSCAN);
  let checksum = 0;
  for (let index = first; index < last; index++) {
    checksum += heights.top(index) + heights.height(index);
  }
  return { rows: last - first, p95Guard: checksum };
}

/** @param {number[]} values @returns {number} */
function p95(values) {
  const sorted = [...values].sort((a, b) => a - b);
  return sorted[Math.max(0, Math.ceil(0.95 * sorted.length) - 1)] ?? 0;
}

/**
 * @param {string} label
 * @param {import("../../../../rust/gui-spike/ui/model.js").HeightIndex} heights
 */
function measure(label, heights) {
  const total = heights.totalHeight;
  /** @type {number[]} */
  const times = [];
  let rows = 0;
  for (let step = 0; step < PAINTS; step++) {
    const scrollTop = Math.floor((total * step) / PAINTS);
    const started = performance.now();
    const result = paint(heights, scrollTop);
    times.push(performance.now() - started);
    rows = result.rows;
  }
  const avg = times.reduce((sum, value) => sum + value, 0) / times.length;
  const worst = p95(times);
  console.log(
    `${label}: строк в окне ${rows}, sizer ${Math.round(total)}px, paint avg ${avg.toFixed(4)} ms, p95 ${worst.toFixed(4)} ms, max ${Math.max(...times).toFixed(4)} ms (n=${PAINTS})`,
  );
  assert.ok(worst <= 1, `${label}: p95 расчёта окна ${worst.toFixed(3)} ms > 1 ms — накладные D-04 съедают кадр`);
}

const emojiCount = EMOJI_CATEGORIES.length;
const toggleCount = SETTINGS_GROUPS.reduce((sum, group) => sum + group.items.length, 0);
console.log(
  `данные: чат ${CHAT_ROWS}, участники ${MEMBER_ROWS}, эмодзи ${emojiCount}, тумблеры ${toggleCount}; HeightIndex построен за ${buildMs.toFixed(3)} ms (period ${CHAT_HEIGHT_PERIOD})`,
);
assert.equal(emojiCount, 10, "категорий эмодзи не 10");
assert.equal(toggleCount, 38, "тумблеров не 38");

// Round-trip оракула: indexAt(top(i)) === i на границах периода и списка.
for (const index of [0, 1, 9, 26, 32, 1563, 1564, 5000, CHAT_ROWS - 1]) {
  assert.equal(chatHeights.indexAt(chatHeights.top(index)), index, `indexAt(top(${index})) != ${index}`);
  const offset = chatHeights.top(index) + Math.floor(chatHeights.height(index) / 2);
  assert.equal(chatHeights.indexAt(offset), index, `indexAt(середина ${index}) != ${index}`);
}
assert.equal(chatHeights.top(CHAT_ROWS), chatHeights.totalHeight, "top(totalRows) != totalHeight");
assert.equal(memberHeights.top(12), 12 * 42, "участник: top(12) != 12*42");
assert.equal(memberHeights.totalHeight, MEMBER_ROWS * 42, "участник: sizer != total*42");

measure("чат", chatHeights);
measure("участники", memberHeights);
console.log("\nmodel-bench: расчёт окна укладывается в бюджет кадра (headless, без layout)");
