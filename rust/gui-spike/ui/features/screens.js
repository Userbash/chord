// @ts-check
/**
 * Rail-навигация 4 экранов (RW-38 §2.2, DOM-CONTRACT §10.2).
 *
 * Канонический переключатель видимости — `body[data-active-screen]` (его читает
 * `style.css`); класс `.active`/`aria-hidden` остаются для тестов и AT-SPI.
 * Временный CSS-мост `:has()` в `style.css` становится избыточным, но файл
 * read-only для W39d — удаление за PM.
 */
import { byId, setAriaHidden, toggleClass } from "./dom.js";
import { UIError } from "./errors.js";

/** @type {Record<string, string>} */
export const TITLES = {
  chat: "Чат · 100 000 сообщений",
  emoji: "Эмодзи-пикер · 10 категорий",
  members: "Участники · 24 229",
  settings: "Настройки · 38 переключателей",
};

/** @type {Record<string, string>} */
export const LIVE_TITLES = {
  chat: "Чат",
  emoji: "Эмодзи",
  members: "Участники",
  settings: "Настройки",
};

/** @typedef {"chat" | "emoji" | "members" | "settings"} ScreenName */

/**
 * @param {{
 *   onShow?: (name: ScreenName) => void,
 *   onError?: (error: UIError) => void,
 * }} [options]
 */
export function createScreenRouter(options = {}) {
  const onShow = options.onShow ?? (() => {});
  const onError = options.onError ?? (() => {});
  const buttons = /** @type {HTMLElement[]} */ ([...document.querySelectorAll(".rail-btn")]);
  const screens = /** @type {HTMLElement[]} */ ([...document.querySelectorAll(".screen")]);
  /** @type {ScreenName} */
  let current = "chat";

  /**
   * @param {string} name
   * @returns {name is ScreenName}
   */
  const isScreen = (name) => Object.hasOwn(TITLES, name);

  /**
   * @param {ScreenName} name
   */
  function switchScreen(name) {
    const screen = byId(`screen-${name}`);
    if (!screen) {
      throw new UIError("SCREEN_MISSING", `в DOM нет #screen-${name}`);
    }
    for (const button of buttons) {
      const active = button.dataset.screen === name;
      toggleClass(button, "active", active);
      // Ровер-навигация: у активной — page, у остальных атрибут снят (не "false":
      // aria-current="false" валиден, но лишний шум для AT-SPI).
      if (active) button.setAttribute("aria-current", "page");
      else button.removeAttribute("aria-current");
    }
    for (const node of screens) {
      const active = node.id === `screen-${name}`;
      toggleClass(node, "active", active);
      setAriaHidden(node, !active);
    }
    // Канонический источник видимости для CSS: body[data-active-screen].
    // Пишем и dataset, и атрибут: в браузере это одно и то же, в DOM-шиме
    // dataset и attributes — разные хранилища (см. §9.4).
    document.body.dataset.activeScreen = name;
    document.body.setAttribute("data-active-screen", name);
    const title = document.querySelector(".screen-title");
    if (title) {
      const isLive = Boolean(/** @type {any} */ (globalThis).__LIVE_SESSION_ACTIVE__);
      title.textContent = (isLive ? LIVE_TITLES[name] : TITLES[name]) ?? "";
    }
    current = name;
    onShow(name);
  }

  for (const button of buttons) {
    button.addEventListener("click", () => {
      const name = button.dataset.screen ?? "";
      if (!isScreen(name)) {
        onError(new UIError("SCREEN_UNKNOWN", `rail-кнопка с неизвестным data-screen="${name}"`));
        return;
      }
      switchScreen(name);
    });
  }
  // Стартовый экран выставляется и на body (в html атрибут уже есть, но шим
  // строит body без него — поведение должно совпадать в обоих окружениях).
  switchScreen("chat");

  return {
    /** @param {ScreenName} name */
    switchScreen,
    /** @returns {ScreenName} */
    get current() {
      return current;
    },
  };
}

/** @typedef {ReturnType<typeof createScreenRouter>} ScreenRouter */
