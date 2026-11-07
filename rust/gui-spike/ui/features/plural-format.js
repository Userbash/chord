// @ts-check
/**
 * Русские плюральные формы для строк UI: категория CLDR (one/few/many/other),
 * подстановка '{count}' и готовые пресеты счётчиков Discord.
 * Слой G2 (WebView UI) — чистые функции без DOM.
 */

/**
 * @typedef {"one" | "few" | "many" | "other"} PluralCategory
 * @typedef {{ one?: string, few?: string, many?: string, other?: string }} PluralForms
 */

/** Плейсхолдер счётчика в шаблонах пресетов. */
const COUNT_PLACEHOLDER = "{count}";

/**
 * Категория русского склонения для целого неотрицательного числа.
 * Нецелые, отрицательные и нечисловые входы дают 'other'.
 * @param {unknown} count
 * @returns {PluralCategory}
 */
export function selectRussianPluralCategory(count) {
  const kind = typeof count;
  if (kind !== "number" && kind !== "string") return "other";
  const n = Number(count);
  if (!Number.isInteger(n) || n < 0) return "other";
  const mod10 = n % 10;
  const mod100 = n % 100;
  if (mod10 === 1 && mod100 !== 11) return "one";
  if (mod10 >= 2 && mod10 <= 4 && (mod100 < 12 || mod100 > 14)) return "few";
  return "many";
}

/**
 * Целое неотрицательное значение счётчика для подстановки в шаблон.
 * @param {unknown} count
 * @returns {number}
 */
function normalizeCount(count) {
  const n = Math.trunc(Number(count) || 0);
  return Math.max(0, n);
}

/**
 * Форматирует строку по формам склонения, заменяя все '{count}'.
 * Без подходящей формы (или при нестроковом шаблоне) возвращает ''.
 * @param {unknown} count
 * @param {PluralForms | undefined} forms
 * @returns {string}
 */
export function formatPluralRu(count, forms) {
  if (!forms || typeof forms !== "object") return "";
  const value = normalizeCount(count);
  const category = selectRussianPluralCategory(value);
  const template = /** @type {Record<string, unknown>} */ (forms)[category];
  if (typeof template !== "string") return "";
  return template.split(COUNT_PLACEHOLDER).join(String(value));
}

/** Готовые пресеты счётчиков для частых строк интерфейса. */
export const RU_PLURAL_PRESETS = Object.freeze({
  unreadMessages: Object.freeze({
    one: "{count} непрочитанное сообщение",
    few: "{count} непрочитанных сообщения",
    many: "{count} непрочитанных сообщений",
  }),
  membersOnline: Object.freeze({
    one: "{count} участник в сети",
    few: "{count} участника в сети",
    many: "{count} участников в сети",
  }),
  typingUsers: Object.freeze({
    one: "{count} пользователь печатает...",
    few: "{count} пользователя печатают...",
    many: "{count} пользователей печатают...",
  }),
  attachmentsCount: Object.freeze({
    one: "{count} вложение",
    few: "{count} вложения",
    many: "{count} вложений",
  }),
});

/**
 * Форматирует строку по имени пресета; неизвестный ключ даёт ''.
 * @param {string} presetKey
 * @param {unknown} count
 * @returns {string}
 */
export function formatPresetRu(presetKey, count) {
  const presets = /** @type {Record<string, PluralForms>} */ (RU_PLURAL_PRESETS);
  const forms = Object.prototype.hasOwnProperty.call(presets, presetKey) ? presets[presetKey] : undefined;
  return formatPluralRu(count, forms);
}
