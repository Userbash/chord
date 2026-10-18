// @ts-check
// Пути проекта и имена артефактов. Контракт: tests/unit/paths.test.mjs.

import { dirname, isAbsolute, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "../..");

/**
 * Абсолютный путь к корню проекта `discord-function-lab`
 * (вычисляется от расположения этого файла: src/core → корень).
 * @returns {string}
 */
export function projectRoot() {
  return root;
}

/** Каталог рантайм-инвентаря: `<root>/functions`. @returns {string} */
export function functionsDir() {
  return join(root, "functions");
}

/** Каталог логов: `<root>/logs`. @returns {string} */
export function logsDir() {
  return join(root, "logs");
}

/** Каталог скриншотов UI-прогонов: `<root>/evidence/screens`. @returns {string} */
export function screensDir() {
  return join(root, "evidence", "screens");
}

/**
 * Имя файла скриншота шага UI-прогона.
 *
 * Формат: `<prefix>-<step с ведущим нулём до 2 знаков>-<slug>.png`, где
 * slug = label, у которого все последовательности не-букв/не-цифр заменены
 * на `-`, обрезанный до 40 символов; пустой label → `item`.
 * (Slug включает `\p{L}`/`\p{N}` — кириллица сохраняется.)
 *
 * @param {string} prefix например "ui-setting" | "ui-setting2" | "ui-menu"
 * @param {number} step номер шага (7 → "07", 100 → "100")
 * @param {string} label подпись пункта, например "Мой аккаунт"
 * @returns {string} например "ui-setting-07-Мой-аккаунт.png"
 */
export function screenshotName(prefix, step, label) {
  const slug = label.replace(/[^\p{L}\p{N}]+/gu, "-").slice(0, 40) || "item";
  return `${prefix}-${String(step).padStart(2, "0")}-${slug}.png`;
}

/**
 * Путь для записи в журнал: если `absolutePath` лежит внутри корня проекта,
 * возвращает путь относительно корня, иначе — как есть.
 * @param {string} absolutePath
 * @returns {string}
 */
export function relativeToRoot(absolutePath) {
  const pathFromRoot = relative(root, resolve(absolutePath));
  if (pathFromRoot === ".." || pathFromRoot.startsWith(`..${process.platform === "win32" ? "\\" : "/"}`) || isAbsolute(pathFromRoot)) {
    return absolutePath;
  }
  return pathFromRoot;
}
