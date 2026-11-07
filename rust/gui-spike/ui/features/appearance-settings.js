// @ts-check
/**
 * Настройки внешнего вида: тема, масштаб шрифта, отступы групп сообщений и
 * уменьшенная анимация.
 *
 * Контроллер держит 4 значения, немедленно переносит их в `documentElement`
 * (`data-theme`, `data-reduced-motion`, CSS-переменные `--chat-font-scale`,
 * `--chat-group-spacing`) и сохраняет JSON в storage. Недопустимые значения
 * отвергаются без мутации состояния.
 */

/** Допустимые масштабы шрифта в процентах. */
export const FONT_SCALE_STEPS = Object.freeze([87.5, 93.75, 100, 106.25, 112.5, 125, 150]);

/** Допустимые отступы между группами сообщений в пикселях. */
export const GROUP_SPACING_STEPS = Object.freeze([0, 4, 8, 16, 24]);

/** Допустимые темы. */
export const THEMES = Object.freeze(["dark", "light", "ash", "onyx"]);

/** Ключ хранения настроек внешнего вида. */
export const APPEARANCE_STORAGE_KEY = "discord.appearance";

/**
 * @typedef {{ theme: string, fontScale: number, groupSpacing: number, reducedMotion: boolean }} AppearanceState
 * @typedef {{ getItem(key: string): string | null, setItem(key: string, value: string): void }} AppearanceStorage
 * @typedef {{ storage?: AppearanceStorage | null }} AppearanceOptions
 */

/** @type {Readonly<AppearanceState>} */
export const DEFAULT_APPEARANCE = Object.freeze({
  theme: "dark",
  fontScale: 100,
  groupSpacing: 16,
  reducedMotion: false,
});

/**
 * @param {unknown} value
 * @returns {value is string}
 */
function isTheme(value) {
  return typeof value === "string" && THEMES.includes(value);
}

/**
 * @param {unknown} value
 * @returns {value is number}
 */
function isFontScale(value) {
  return typeof value === "number" && FONT_SCALE_STEPS.includes(value);
}

/**
 * @param {unknown} value
 * @returns {value is number}
 */
function isGroupSpacing(value) {
  return typeof value === "number" && GROUP_SPACING_STEPS.includes(value);
}

/**
 * Разбирает сохранённый JSON, оставляя только валидные поля.
 * @param {string | null} raw
 * @returns {AppearanceState}
 */
function hydrate(raw) {
  /** @type {AppearanceState} */
  const state = { ...DEFAULT_APPEARANCE };
  if (typeof raw !== "string" || raw.length === 0) return state;
  /** @type {unknown} */
  let parsed;
  try {
    parsed = JSON.parse(raw);
  } catch {
    return state;
  }
  if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed)) return state;
  const source = /** @type {Record<string, unknown>} */ (parsed);
  if (isTheme(source.theme)) state.theme = source.theme;
  if (isFontScale(source.fontScale)) state.fontScale = source.fontScale;
  if (isGroupSpacing(source.groupSpacing)) state.groupSpacing = source.groupSpacing;
  if (typeof source.reducedMotion === "boolean") state.reducedMotion = source.reducedMotion;
  return state;
}

/**
 * Создаёт контроллер настроек внешнего вида и сразу применяет состояние.
 * @param {any} doc
 * @param {AppearanceOptions} [options]
 * @returns {{
 *   setTheme(theme: string): boolean,
 *   setFontScale(scale: number): boolean,
 *   setGroupSpacing(px: number): boolean,
 *   setReducedMotion(enabled: boolean): boolean,
 *   getState(): AppearanceState,
 *   resetDefaults(): AppearanceState,
 * }}
 */
export function createAppearanceController(doc, options = {}) {
  const storage = options.storage ?? null;
  const root = doc.documentElement;

  /** @type {string | null} */
  let raw = null;
  if (storage) {
    try {
      raw = storage.getItem(APPEARANCE_STORAGE_KEY);
    } catch {
      raw = null; // хранилище недоступно — стартуем с дефолтов, это осознанный fallback
    }
  }

  /** @type {AppearanceState} */
  const state = hydrate(raw);

  function apply() {
    if (!root) return;
    if (typeof root.setAttribute === "function") {
      root.setAttribute("data-theme", state.theme);
      root.setAttribute("data-reduced-motion", String(state.reducedMotion));
    }
    if (root.style && typeof root.style.setProperty === "function") {
      root.style.setProperty("--chat-font-scale", `${state.fontScale}%`);
      root.style.setProperty("--chat-group-spacing", `${state.groupSpacing}px`);
    }
  }

  function persist() {
    if (!storage) return;
    try {
      storage.setItem(APPEARANCE_STORAGE_KEY, JSON.stringify(state));
    } catch {
      // квота/недоступность storage не должны отменять уже применённый к DOM вид
    }
  }

  apply();

  return {
    /** @param {string} theme */
    setTheme(theme) {
      if (!isTheme(theme)) return false;
      state.theme = theme;
      apply();
      persist();
      return true;
    },
    /** @param {number} scale */
    setFontScale(scale) {
      if (!isFontScale(scale)) return false;
      state.fontScale = scale;
      apply();
      persist();
      return true;
    },
    /** @param {number} px */
    setGroupSpacing(px) {
      if (!isGroupSpacing(px)) return false;
      state.groupSpacing = px;
      apply();
      persist();
      return true;
    },
    /** @param {boolean} enabled */
    setReducedMotion(enabled) {
      if (typeof enabled !== "boolean") return false;
      state.reducedMotion = enabled;
      apply();
      persist();
      return true;
    },
    getState() {
      return { ...state };
    },
    resetDefaults() {
      state.theme = DEFAULT_APPEARANCE.theme;
      state.fontScale = DEFAULT_APPEARANCE.fontScale;
      state.groupSpacing = DEFAULT_APPEARANCE.groupSpacing;
      state.reducedMotion = DEFAULT_APPEARANCE.reducedMotion;
      apply();
      persist();
      return { ...state };
    },
  };
}
