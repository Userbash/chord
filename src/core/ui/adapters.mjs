// @ts-check
// Реестр приложений для UI-прогонов (команда walk). Контракт: tests/unit/walk.test.mjs.
// В этой карточке — только fixture; адаптеры живого Discord (settings/menus) — отдельная работа.

/** @typedef {{ name: string, navSelector: string, screenshotPrefix: string }} WalkAdapter */

/**
 * Реестр адаптеров: имя приложения → селектор навигации и префикс скриншотов.
 * @type {Readonly<Record<string, WalkAdapter>>}
 */
export const ADAPTERS = {
  fixture: { name: "fixture", navSelector: "#nav button", screenshotPrefix: "ui-fixture" },
};

/**
 * Адаптер приложения по имени. Неизвестное имя → ошибка со списком доступных.
 * @param {string} app
 * @returns {WalkAdapter}
 */
export function resolveAdapter(app) {
  const adapter = ADAPTERS[app];
  if (!adapter) {
    throw new Error(`неизвестное приложение «${app}» (доступно: ${Object.keys(ADAPTERS).join(", ")})`);
  }
  return adapter;
}

/**
 * Выражение для `Runtime.evaluate`: клик по пункту навигации с **точным**
 * совпадением нормализованного текста (в отличие от `cdp clicktext` — contains).
 *
 * Поведение (паритет ui-walk-settings2):
 * - кандидаты — все элементы `navSelector`;
 * - текст нормализуется как в `listElementsExpression` (пробелы, trim);
 * - совпадение точное; берётся последний кандидат;
 * - перед кликом `scrollIntoView({block:"center"})`, клик страничный (`el.click()`);
 * - не найдено → `{ clicked: false, reason: "label not found" }`.
 * @param {string} navSelector
 * @param {string} label
 * @returns {string}
 */
export function clickNavItemExpression(navSelector, label) {
  const selector = JSON.stringify(navSelector);
  const wanted = JSON.stringify(String(label).replace(/\s+/g, " ").trim());
  return `(() => {
    const wanted = ${wanted};
    const candidates = [...document.querySelectorAll(${selector})].filter((el) =>
      ((el.innerText || '').replace(/\\s+/g, ' ').trim()) === wanted);
    const el = candidates[candidates.length - 1];
    if (!el) return { clicked: false, reason: 'label not found' };
    el.scrollIntoView({ block: 'center' });
    el.click();
    return { clicked: true, text: ((el.innerText || '').replace(/\\s+/g, ' ').trim()).slice(0, 100) };
  })()`;
}
