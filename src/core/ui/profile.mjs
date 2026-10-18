// @ts-check
// Топ функций из CPU-профиля. Контракт: tests/unit/profile.test.mjs
// (паритет ui-walk-settings2 / ui-walk-menus / ui-menu-extra).

/** @typedef {import("../types.js").CpuProfile} CpuProfile */
/** @typedef {import("../types.js").TopFunction} TopFunction */

/**
 * Список шумовых имён, которые не попадают в топ.
 * @type {ReadonlySet<string>}
 */
export const NOISE = new Set([
  "(idle)",
  "(program)",
  "(garbage collector)",
  "(root)",
  "(anonymous)",
]);

/**
 * Топ функций профиля:
 * - узлы без hitCount (0/undefined) отбрасываются;
 * - узлы с именем из NOISE (включая пустое имя → "(anonymous)") отбрасываются;
 * - сортировка по hits по убыванию (стабильная);
 * - берётся не больше `limit` записей (по умолчанию 8);
 * - `url` — последний сегмент пути, обрезанный до 60 символов;
 * - `fn` — имя функции (пустое имя → "(anonymous)", но такие узлы уже отфильтрованы).
 * @param {CpuProfile} profile
 * @param {number} [limit]
 * @returns {TopFunction[]}
 */
export function topFunctions(profile, limit = 8) {
  return profile.nodes
    .map((node, index) => ({ node, index, fn: node.callFrame.functionName || "(anonymous)" }))
    .filter(({ node, fn }) => Boolean(node.hitCount) && !NOISE.has(fn))
    .sort((left, right) => (right.node.hitCount ?? 0) - (left.node.hitCount ?? 0) || left.index - right.index)
    .slice(0, limit)
    .map(({ node, fn }) => ({
      fn,
      url: (node.callFrame.url || "").split("/").pop()?.slice(0, 60) ?? "",
      hits: node.hitCount ?? 0,
    }));
}
