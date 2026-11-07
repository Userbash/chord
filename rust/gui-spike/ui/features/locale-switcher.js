// @ts-check
/**
 * Переключатель языка интерфейса: реестр локалей, хранение выбора и select для настроек.
 * Слой G2 (WebView UI) — не тянет DOM-хелперы, работает с переданным document.
 */

/** @typedef {{ code: string, bcp47: string, name: string }} Locale */
/** @typedef {{ getItem(key: string): string | null, setItem(key: string, value: string): void }} LocaleStorage */

/** Реестр поддерживаемых локалей. */
export const SUPPORTED_LOCALES = Object.freeze([
  Object.freeze(/** @type {Locale} */ ({ code: "ru", bcp47: "ru-RU", name: "Русский" })),
  Object.freeze(/** @type {Locale} */ ({ code: "en-US", bcp47: "en-US", name: "English, US" })),
]);

/** Ключ localStorage для выбранной локали. */
export const LOCALE_STORAGE_KEY = "discord.locale";

/** @param {string} code */
function isSupported(code) {
  return SUPPORTED_LOCALES.some((locale) => locale.code === code);
}

/** @param {string} code @returns {Locale} */
function localeByCode(code) {
  const found = SUPPORTED_LOCALES.find((locale) => locale.code === code);
  if (!found) throw new RangeError(`Неподдерживаемая локаль: ${code}`);
  return found;
}

/**
 * @param {any} doc
 * @param {{ storage?: LocaleStorage | null, defaultLocale?: string }} [options]
 */
export function createLocaleSwitcher(doc, options = {}) {
  const storage = options.storage ?? null;
  const fallback = typeof options.defaultLocale === "string" && isSupported(options.defaultLocale)
    ? options.defaultLocale
    : "ru";

  /** @type {string} */
  let current = fallback;
  if (storage && typeof storage.getItem === "function") {
    const stored = storage.getItem(LOCALE_STORAGE_KEY);
    if (typeof stored === "string" && isSupported(stored)) current = stored;
  }

  /** @type {Array<(code: string) => void>} */
  const listeners = [];
  /** @type {any} */
  let selectEl = null;

  /** @param {string} code */
  function applyAttributes(code) {
    doc.documentElement.setAttribute("lang", code);
    doc.documentElement.setAttribute("data-locale", code);
  }

  applyAttributes(current);

  /** @param {string} code */
  function notify(code) {
    for (const listener of listeners.slice()) {
      try {
        listener(code);
      } catch {
        // Ошибка одного подписчика не должна ломать смену локали.
      }
    }
  }

  return {
    /** @returns {string} */
    getLocale() {
      return current;
    },

    /**
     * @param {string} code
     * @returns {boolean}
     */
    setLocale(code) {
      localeByCode(code);
      current = code;
      applyAttributes(code);
      if (storage && typeof storage.setItem === "function") storage.setItem(LOCALE_STORAGE_KEY, code);
      if (selectEl) selectEl.value = code;
      notify(code);
      return true;
    },

    /**
     * @param {(code: string) => void} listener
     * @returns {() => void} отписка
     */
    onChange(listener) {
      listeners.push(listener);
      return () => {
        const index = listeners.indexOf(listener);
        if (index !== -1) listeners.splice(index, 1);
      };
    },

    /**
     * Рендерит <select> в контейнер и связывает change с setLocale.
     * @param {any} container
     * @returns {any}
     */
    renderSelect(container) {
      const select = doc.createElement("select");
      select.setAttribute("data-role", "locale-select");
      select.setAttribute("aria-label", "Язык интерфейса");
      for (const locale of SUPPORTED_LOCALES) {
        const option = doc.createElement("option");
        option.setAttribute("value", locale.code);
        option.textContent = locale.name;
        option.selected = locale.code === current;
        select.appendChild(option);
      }
      select.value = current;
      select.addEventListener("change", () => {
        const next = String(select.value);
        try {
          this.setLocale(next);
        } catch {
          // Некорректное значение из DOM: возвращаем select к текущей локали.
          select.value = current;
        }
      });
      if (container && typeof container.appendChild === "function") container.appendChild(select);
      selectEl = select;
      return select;
    },
  };
}
