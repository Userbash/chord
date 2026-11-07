// @ts-check
/**
 * T-224 (FE-P2-23): парсер горячих клавиш и русская подпись для ARIA.
 *
 * Основной контракт: набор комбинаций по умолчанию, нормализация события
 * клавиатуры в стабильную строку вида "Ctrl+Shift+M", сравнение события с
 * сохранённым комбо и текст подписи `aria-keyshortcuts` на русском.
 */

/**
 * @typedef {object} KeyEventLike
 * @property {string} [key] нажатая клавиша
 * @property {boolean} [ctrlKey] зажат Ctrl
 * @property {boolean} [metaKey] зажат Meta (Cmd)
 * @property {boolean} [shiftKey] зажат Shift
 * @property {boolean} [altKey] зажат Alt
 */

/**
 * Комбинации по умолчанию. Порядок модификаторов — Ctrl, Alt, Shift.
 * @type {Readonly<Record<string, string>>}
 */
export const DEFAULT_SHORTCUTS = Object.freeze({
  QUICK_SWITCHER: "Ctrl+K",
  SEARCH: "Ctrl+F",
  TOGGLE_MUTE: "Ctrl+Shift+M",
  TOGGLE_DEAFEN: "Ctrl+Shift+D",
  MARK_SERVER_READ: "Shift+Escape",
  CLOSE_MODAL: "Escape",
});

/** @type {readonly string[]} */
const MODIFIER_KEYS = Object.freeze(["Control", "Shift", "Alt", "Meta"]);

/**
 * Приводит клавишу к виду, используемому в комбо: одиночный символ — в верхний
 * регистр, пробел — "Space", остальное без изменений.
 * @param {string} key
 * @returns {string}
 */
function normalizeKey(key) {
  if (key === " ") return "Space";
  return key.length === 1 ? key.toUpperCase() : key;
}

/**
 * Собирает строку-комбо из события клавиатуры.
 * Возвращает null, если клавиша пустая или сам вызов — это модификатор.
 * @param {KeyEventLike | null | undefined} event
 * @returns {string | null}
 */
export function normalizeKeyCombo(event) {
  if (!event) return null;
  const key = typeof event.key === "string" ? event.key : "";
  if (!key) return null;
  if (MODIFIER_KEYS.includes(key)) return null;

  /** @type {string[]} */
  const parts = [];
  if (event.ctrlKey || event.metaKey) parts.push("Ctrl");
  if (event.altKey) parts.push("Alt");
  if (event.shiftKey) parts.push("Shift");
  parts.push(normalizeKey(key));
  return parts.join("+");
}

/**
 * Проверяет, что событие соответствует сохранённому комбо.
 * @param {KeyEventLike | null | undefined} event
 * @param {string | null | undefined} comboString
 * @returns {boolean}
 */
export function matchesShortcut(event, comboString) {
  const normalized = normalizeKeyCombo(event);
  if (normalized === null) return false;
  return normalized === comboString;
}

/**
 * Строит русскую подпись для `aria-keyshortcuts`.
 * @param {string | null | undefined} comboString
 * @returns {string}
 */
export function formatShortcutAriaLabelRu(comboString) {
  return `Горячая клавиша: ${comboString || "не назначена"}`;
}
