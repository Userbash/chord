// @ts-check
/**
 * Мелкие DOM-хелперы фронтенда спайка.
 *
 * Ограничения DOM-шима тестов (`tests/support/gui-dom-harness.mjs` §9.4):
 * только `#id`, `.class`, `tag`, `tag.class`, `[attr="value"]`, потомок и группы.
 * `innerHTML`/`insertAdjacentHTML` в шиме нет — узлы создаём `createElement`.
 */

/**
 * @param {string} id
 * @returns {HTMLElement | null}
 */
export function byId(id) {
  return document.getElementById(id);
}

/**
 * @param {string} tag
 * @param {string} [className]
 * @param {string} [value]
 * @returns {HTMLElement}
 */
export function make(tag, className = "", value = "") {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (value) node.textContent = value;
  return node;
}

/**
 * @param {number} value
 * @returns {string} число с тонкими пробелами, как в заголовках экранов
 */
export function formatNumber(value) {
  return String(Math.round(value)).replace(/\B(?=(\d{3})+(?!\d))/g, " ");
}

/**
 * @param {number | null | undefined} value
 * @param {string} [suffix]
 * @returns {string}
 */
export function fmt(value, suffix = "") {
  return value == null ? "—" : `${Math.round(value)}${suffix}`;
}

/**
 * Снимает `aria-hidden=false`-мусор: атрибут либо "true", либо отсутствует.
 * @param {HTMLElement} node
 * @param {boolean} hidden
 */
export function setAriaHidden(node, hidden) {
  if (hidden) node.setAttribute("aria-hidden", "true");
  else node.removeAttribute("aria-hidden");
}

/**
 * `node.classList.toggle` без лишних записей: шим и браузер ведут себя одинаково.
 * @param {HTMLElement} node
 * @param {string} name
 * @param {boolean} force
 */
export function toggleClass(node, name, force) {
  if (force) node.classList.add(name);
  else node.classList.remove(name);
}
