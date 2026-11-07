// @ts-check
/**
 * T-188 (UI token map): единая таблица соответствия локальных CSS custom properties
 * каноническим токенам клиента Discord.
 *
 * Зачем: тема `discord-gui-spike` исторически использует короткие локальные имена
 * (`--bg-secondary`, `--text-muted`, ...), которые не совпадают с токенами клиента.
 * Карта фиксирует каноническое соответствие, чтобы CSS-миграция и аудит таблиц
 * стилей опирались на один источник истины, а не на догадки.
 */

/**
 * Разрешённые коллизии: локальное имя не совпадает с каноническим токеном клиента.
 * Каждое значение — точное имя Discord-токена из эталонного набора клиента.
 * @type {Readonly<Record<string, string>>}
 */
export const RESOLVED_COLLISIONS = Object.freeze({
  "--bg-secondary": "--background-secondary",
  "--bg-tertiary": "--background-tertiary",
  "--text-muted": "--text-muted-discord",
  "--border-subtle": "--background-modifier-accent",
  "--brand-hover": "--brand-560",
  "--danger-fg": "--status-danger",
  "--surface-overlay": "--background-floating",
});

/**
 * Каноническая карта локальных токенов в токены клиента Discord.
 * Содержит все 7 разрешённых коллизий плюс базовые шкалы поверхностей и текста.
 * @type {Readonly<Record<string, string>>}
 */
export const CANONICAL_TOKEN_MAP = Object.freeze({
  // Разрешённые коллизии (7).
  ...RESOLVED_COLLISIONS,

  // Фоны: локальная шкала `--bg-0..--bg-4`.
  "--bg-0": "--background-primary",
  "--bg-1": "--background-secondary",
  "--bg-2": "--background-secondary-alt",
  "--bg-3": "--background-tertiary",
  "--bg-4": "--background-accent",

  // Поверхности: `--surface-1..--surface-4`.
  "--surface-1": "--background-primary",
  "--surface-2": "--background-modifier-hover",
  "--surface-3": "--background-modifier-selected",
  "--surface-4": "--background-floating",

  // Текст: `--text-0..--text-2` и акцентный текст.
  "--text-0": "--text-normal",
  "--text-1": "--header-secondary",
  "--text-2": "--interactive-muted",
  "--text-strong": "--header-primary",

  // Акцент и статусы присутствия.
  "--accent": "--brand-500",
  "--online": "--status-online",
  "--idle": "--status-idle",
  "--dnd": "--status-dnd",
  "--offline": "--status-offline",
});

/** @type {RegExp} */
const TOKEN_PATTERN = /--[a-z0-9-]+/g;

/**
 * Разрешает локальный токен в каноническое имя токена клиента Discord.
 *
 * @param {string} localToken локальное имя, например `--bg-secondary`
 * @returns {string | null} имя токена Discord или `null`, если токен неизвестен
 */
export function resolveClientToken(localToken) {
  if (typeof localToken !== "string") return null;
  if (!Object.hasOwn(CANONICAL_TOKEN_MAP, localToken)) return null;
  return CANONICAL_TOKEN_MAP[localToken] ?? null;
}

/**
 * Сканирует текст таблицы стилей и разделяет найденные custom properties на
 * известные, неизвестные и разрешённые коллизии. Результаты дедуплицированы
 * и отсортированы лексикографически, чтобы аудит был детерминированным.
 *
 * @param {string} cssText исходный CSS
 * @returns {{ mapped: string[], unmapped: string[], collisionsResolved: string[] }}
 *   `mapped` — все известные локальные токены, `unmapped` — неизвестные,
 *   `collisionsResolved` — подмножество `mapped`, для которого сработало
 *   разрешение коллизии; все три массива отсортированы.
 */
export function auditStylesheetTokens(cssText) {
  /** @type {Set<string>} */
  const mapped = new Set();
  /** @type {Set<string>} */
  const unmapped = new Set();
  /** @type {Set<string>} */
  const collisionsResolved = new Set();

  if (typeof cssText !== "string" || cssText.length === 0) {
    return { mapped: [], unmapped: [], collisionsResolved: [] };
  }

  const found = cssText.match(TOKEN_PATTERN) ?? [];
  for (const token of found) {
    if (resolveClientToken(token) === null) {
      unmapped.add(token);
      continue;
    }
    mapped.add(token);
    if (Object.hasOwn(RESOLVED_COLLISIONS, token)) collisionsResolved.add(token);
  }

  return {
    mapped: [...mapped].sort(),
    unmapped: [...unmapped].sort(),
    collisionsResolved: [...collisionsResolved].sort(),
  };
}
