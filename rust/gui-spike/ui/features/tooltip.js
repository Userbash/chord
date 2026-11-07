// @ts-check
/**
 * FE-P2-01 (RW-40): доступный Tooltip вместо сырых `title` на иконках.
 *
 * Оракул — `/home/sanya/re-deep/frontend-d03.md` §5 п.7: компонент Tooltip
 * клиента 1.0.162, CSS-класс `tooltip_c36707` (в дампе рядом
 * `tooltipDisablePointerEvents_c36707`, `tooltipPointer_c36707`,
 * `tooltipContent`). Подписи — русские строки клиента (`ru-catalog.json`);
 * перечень и задержка — `waves/RW-40/PM/BACKLOG-RE-FRONTEND.md` §3 FE-P2-01.
 *
 * Контракт:
 * - один общий элемент `<div id="discord-tooltip" role="tooltip">` в `body`,
 *   а не по подсказке на каждую иконку;
 * - сырой `title` переносится в `data-tooltip` и удаляется из DOM: иначе
 *   браузер рисует нативную подсказку поверх нашей (дубль);
 * - показ — `mouseenter`/`focusin` через `TOOLTIP_SHOW_DELAY_MS`;
 *   скрытие — `mouseleave`/`focusout`/`click`/`Escape` немедленно;
 * - показ связывает цель и подсказку через `aria-describedby`;
 * - таймеры инъецируются (`setTimeoutFn`/`clearTimeoutFn`), поэтому модуль
 *   проверяется в DOM-шиме без ожидания реального времени.
 *
 * Ограничение: координаты (`top`/`left`) не вычисляются — в шиме нет layout,
 * а интеграцию с реальной геометрией иконок делает вызывающий слой;
 * `data-placement` — контракт для этой интеграции.
 */

/** Задержка показа подсказки, мс (FE-P2-01: 400 мс). */
export const TOOLTIP_SHOW_DELAY_MS = 400;
/** `id` общего элемента: на него ссылается `aria-describedby` цели. */
export const TOOLTIP_ELEMENT_ID = "discord-tooltip";
/** Классы клиента: базовый `tooltip` и хеш-класс `tooltip_c36707`. */
export const TOOLTIP_CLASS_NAME = "tooltip tooltip_c36707";
/** Сторона по умолчанию, если у цели нет `data-placement`. */
export const TOOLTIP_DEFAULT_PLACEMENT = "top";
/** Клавиши закрытия: `Escape` и его legacy-имя `Esc`. */
export const TOOLTIP_CLOSE_KEYS = Object.freeze(["Escape", "Esc"]);

/**
 * Подписи иконок клиента (ru). Ключи — наши id иконок, значения — строки из
 * `ru-catalog.json`; `attach` присваивает их через `data-tooltip`.
 * @type {Readonly<Record<string, string>>}
 */
export const ICON_TOOLTIPS = Object.freeze({
  mute: "Заглушить",
  deafen: "Откл. звук",
  settings: "Настройки пользователя",
  voice_settings_input: "Настройки ввода",
  voice_settings_output: "Настройки вывода",
  pins: "Закреплённые сообщения",
  threads: "Ветки",
  members: "Показать список участников",
  inbox: "Почта",
  search: "Поиск",
  add_reaction: "Добавить реакцию",
  // В `ru-catalog.json` строки «Прикрепить файл» нет — там «Прикрепить вложения».
  attach: "Прикрепить файл",
  emoji: "Выбрать эмодзи",
});

/**
 * Событие клавиатуры (в шиме — минимум, который читает контроллер).
 * @typedef {{ key?: string }} TooltipKeyEvent
 */

/**
 * Цель подсказки: DOM-элемент (структурный тип, tsc проверяет модуль без lib DOM).
 * @typedef {{
 *   id: string,
 *   textContent: string,
 *   getAttribute: (name: string) => string | null,
 *   setAttribute: (name: string, value: string) => void,
 *   removeAttribute: (name: string) => void,
 *   addEventListener: (type: string, listener: (event: TooltipKeyEvent) => void) => void,
 *   removeEventListener: (type: string, listener: (event: TooltipKeyEvent) => void) => void,
 *   querySelectorAll: (selector: string) => TooltipTarget[],
 *   appendChild?: (node: any) => unknown,
 * }} TooltipTarget
 */

/**
 * Приёмник клавиатурных событий: документ в браузере или `body` в DOM-шиме.
 * @typedef {{
 *   addEventListener: (type: string, listener: (event: TooltipKeyEvent) => void) => void,
 *   removeEventListener: (type: string, listener: (event: TooltipKeyEvent) => void) => void,
 * }} TooltipKeyHost
 */

/**
 * Документ: ровно те методы, которые нужны контроллеру.
 * @typedef {{
 *   body: TooltipTarget | null,
 *   createElement: (tagName: string) => TooltipTarget,
 *   getElementById: (id: string) => TooltipTarget | null,
 *   querySelectorAll: (selector: string) => TooltipTarget[],
 *   addEventListener?: (type: string, listener: (event: TooltipKeyEvent) => void) => void,
 *   removeEventListener?: (type: string, listener: (event: TooltipKeyEvent) => void) => void,
 * }} TooltipDocument
 */

/**
 * @typedef {{
 *   delayMs?: number,
 *   placement?: string,
 *   setTimeoutFn?: (callback: () => void, delayMs: number) => number,
 *   clearTimeoutFn?: (handle: number) => void,
 * }} TooltipOptions
 */

/**
 * Привязка цели: исходный `title` (для восстановления) и те же ссылки на
 * обработчики, что снимаются в `detach`.
 * @typedef {{
 *   title: string | null,
 *   mouseenter: () => void,
 *   mouseleave: () => void,
 *   focusin: () => void,
 *   focusout: () => void,
 *   click: () => void,
 * }} Binding
 */

/**
 * @typedef {{
 *   element: TooltipTarget,
 *   attach: (root: TooltipTarget | TooltipDocument) => number,
 *   detach: (target: TooltipTarget) => boolean,
 *   showFor: (target: TooltipTarget) => TooltipTarget,
 *   hide: () => void,
 *   destroy: () => void,
 *   readonly activeTarget: TooltipTarget | null,
 * }} TooltipController
 */

/**
 * Общий элемент подсказки: переиспользуется, атрибуты сбрасываются в «скрыто».
 * @param {TooltipDocument} doc
 * @returns {TooltipTarget}
 */
function ensureElement(doc) {
  const existing = doc.getElementById(TOOLTIP_ELEMENT_ID);
  const element = existing ?? doc.createElement("div");
  if (!existing) {
    element.setAttribute("id", TOOLTIP_ELEMENT_ID);
    element.setAttribute("class", TOOLTIP_CLASS_NAME);
    element.setAttribute("role", "tooltip");
    doc.body?.appendChild?.(element);
  }
  element.setAttribute("role", "tooltip");
  element.setAttribute("hidden", "");
  element.setAttribute("aria-hidden", "true");
  element.setAttribute("data-open", "false");
  if (element.getAttribute("data-placement") === null) {
    element.setAttribute("data-placement", TOOLTIP_DEFAULT_PLACEMENT);
  }
  return element;
}

/**
 * Подпись цели: `data-tooltip` приоритетнее сырого `title`.
 * @param {TooltipTarget} target
 * @returns {string}
 */
function readLabel(target) {
  const explicit = target.getAttribute("data-tooltip");
  if (explicit !== null && explicit.trim() !== "") return explicit;
  const title = target.getAttribute("title");
  if (title !== null && title.trim() !== "") return title;
  return "";
}

/**
 * Контроллер подсказок на один документ.
 * @param {TooltipDocument} doc
 * @param {TooltipOptions} [options]
 * @returns {TooltipController}
 */
export function createTooltipController(doc, options = {}) {
  const delayMs =
    typeof options.delayMs === "number" && Number.isFinite(options.delayMs) && options.delayMs >= 0
      ? options.delayMs
      : TOOLTIP_SHOW_DELAY_MS;
  const placement = options.placement ?? TOOLTIP_DEFAULT_PLACEMENT;
  const setTimeoutFn =
    options.setTimeoutFn ??
    ((/** @type {() => void} */ callback, /** @type {number} */ delay) =>
      Number(globalThis.setTimeout(callback, delay)));
  const clearTimeoutFn =
    options.clearTimeoutFn ??
    ((/** @type {number} */ handle) => {
      globalThis.clearTimeout(handle);
    });

  const element = ensureElement(doc);
  /** @type {Map<TooltipTarget, Binding>} */
  const bindings = new Map();
  /** @type {TooltipTarget | null} */
  let active = null;
  /** @type {TooltipTarget | null} */
  let pending = null;
  /** @type {number | null} */
  let timer = null;
  let destroyed = false;

  /** @param {TooltipKeyEvent | undefined} event */
  const onKeyDown = (event) => {
    if (destroyed) return;
    const key = event?.key;
    if (typeof key === "string" && TOOLTIP_CLOSE_KEYS.includes(key)) hide();
  };
  /**
   * Документ в браузере слушает клавиши сам; в DOM-шиме их принимает `body`.
   * @type {TooltipKeyHost | null}
   */
  const keyHost =
    typeof doc.addEventListener === "function"
      ? /** @type {TooltipKeyHost} */ (doc)
      : (doc.body ?? null);
  keyHost?.addEventListener("keydown", onKeyDown);

  function clearTimer() {
    if (timer === null) return;
    clearTimeoutFn(timer);
    timer = null;
  }

  /** @param {TooltipTarget} target */
  function release(target) {
    if (target.getAttribute("aria-describedby") === TOOLTIP_ELEMENT_ID) {
      target.removeAttribute("aria-describedby");
    }
  }

  /** @param {TooltipTarget} target */
  function reveal(target) {
    if (destroyed) return;
    clearTimer();
    pending = null;
    const label = readLabel(target);
    if (label === "") return;
    if (active !== null && active !== target) release(active);
    active = target;
    element.textContent = label;
    element.setAttribute(
      "data-placement",
      target.getAttribute("data-placement") ?? placement,
    );
    element.removeAttribute("hidden");
    element.setAttribute("aria-hidden", "false");
    element.setAttribute("data-open", "true");
    target.setAttribute("aria-describedby", TOOLTIP_ELEMENT_ID);
  }

  function hide() {
    clearTimer();
    pending = null;
    if (active !== null) {
      release(active);
      active = null;
    }
    element.setAttribute("hidden", "");
    element.setAttribute("aria-hidden", "true");
    element.setAttribute("data-open", "false");
  }

  /** @param {TooltipTarget} target */
  function schedule(target) {
    if (destroyed || active === target) return;
    clearTimer();
    pending = target;
    timer = setTimeoutFn(() => {
      timer = null;
      const next = pending;
      pending = null;
      if (next !== null) reveal(next);
    }, delayMs);
  }

  /**
   * Привязать цель: `title` → `data-tooltip`, слушатели мыши/фокуса.
   * @param {TooltipTarget} target
   * @returns {boolean} цель привязана впервые
   */
  function bind(target) {
    if (destroyed || bindings.has(target)) return false;
    const label = readLabel(target);
    if (label === "") return false;
    const title = target.getAttribute("title");
    if (title !== null) {
      if (target.getAttribute("data-tooltip") === null) target.setAttribute("data-tooltip", label);
      // Сырой title удаляем: браузер не должен рисовать вторую подсказку.
      target.removeAttribute("title");
    }
    /** @type {Binding} */
    const binding = {
      title,
      mouseenter: () => schedule(target),
      mouseleave: () => hide(),
      focusin: () => schedule(target),
      focusout: () => hide(),
      click: () => hide(),
    };
    target.addEventListener("mouseenter", binding.mouseenter);
    target.addEventListener("mouseleave", binding.mouseleave);
    target.addEventListener("focusin", binding.focusin);
    target.addEventListener("focusout", binding.focusout);
    target.addEventListener("click", binding.click);
    bindings.set(target, binding);
    return true;
  }

  /**
   * Снять привязку и вернуть исходный `title` (если он был).
   * @param {TooltipTarget} target
   * @returns {boolean}
   */
  function detach(target) {
    const binding = bindings.get(target);
    if (!binding) return false;
    target.removeEventListener("mouseenter", binding.mouseenter);
    target.removeEventListener("mouseleave", binding.mouseleave);
    target.removeEventListener("focusin", binding.focusin);
    target.removeEventListener("focusout", binding.focusout);
    target.removeEventListener("click", binding.click);
    if (binding.title !== null && target.getAttribute("title") === null) {
      target.setAttribute("title", binding.title);
    }
    bindings.delete(target);
    if (active === target) {
      release(target);
      active = null;
    }
    return true;
  }

  /**
   * Привязать всё, у чего есть `data-tooltip` или `title`, внутри `root`.
   * @param {TooltipTarget | TooltipDocument} root
   * @returns {number} сколько целей привязано этим вызовом
   */
  function attach(root) {
    if (destroyed || !root || typeof root.querySelectorAll !== "function") return 0;
    let count = 0;
    for (const node of root.querySelectorAll("[data-tooltip], [title]")) {
      if (bind(node)) count += 1;
    }
    return count;
  }

  /**
   * Привязать цель и показать подсказку сразу (без задержки) — для вызовов
   * из кода, где задержка не нужна (например, программный фокус).
   * @param {TooltipTarget} target
   * @returns {TooltipTarget}
   */
  function showFor(target) {
    bind(target);
    reveal(target);
    return element;
  }

  function destroy() {
    if (destroyed) return;
    hide();
    for (const target of [...bindings.keys()]) detach(target);
    bindings.clear();
    keyHost?.removeEventListener("keydown", onKeyDown);
    destroyed = true;
  }

  return {
    element,
    attach,
    detach,
    showFor,
    hide,
    destroy,
    get activeTarget() {
      return active;
    },
  };
}
