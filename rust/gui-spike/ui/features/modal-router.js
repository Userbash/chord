// @ts-check
/**
 * TAURI-BOOT-02: модульный диспетчер открытия и закрытия модальных окон (`modal-router.js`).
 *
 * Обеспечивает:
 * - Единую маршрутизацию и реестр модальных окон UI (`initModalRouter`);
 * - LIFO-стек открытых модалок (`activeStack`);
 * - Взаимное исключение эксклюзивных модальных окон;
 * - Поддержку динамической регистрации кастомных модалок (`registerModal`, `unregisterModal`);
 * - Безопасную интеграцию с объектом приложения (`app.openModal`, `app.closeModal`, `app.closeTopModal`);
 * - Делегирование в существующие контроллеры и мост оболочки `shellBridge`.
 *
 * @module ui/features/modal-router
 */

/**
 * @typedef {object} ModalController
 * @property {(payload?: any) => unknown} [open]
 * @property {() => unknown} [close]
 */

/**
 * @typedef {object} ModalRouterOptions
 * @property {(modalId: string, payload?: unknown) => unknown} [onOpen]
 * @property {(modalId: string) => unknown} [onClose]
 * @property {(activeModalId: string | null, stack: readonly string[]) => void} [onChange]
 * @property {boolean} [listenEscape] Автоматически закрывать верхнюю модалку по Escape
 * @property {EventTarget | { addEventListener: Function, removeEventListener: Function }} [target]
 */

/**
 * @typedef {object} ModalRouter
 * @property {(modalId: string, payload?: unknown) => string} open
 * @property {(modalId?: string) => string | null} close
 * @property {() => string | null} closeTop
 * @property {() => string[]} closeAll
 * @property {(modalId: string) => boolean} isOpen
 * @property {(modalId: string) => boolean} hasModal
 * @property {() => string | null} getActiveModal
 * @property {() => string[]} getActiveStack
 * @property {(modalId: string, controller: ModalController) => ModalController} registerModal
 * @property {(modalId: string) => boolean} unregisterModal
 * @property {() => void} destroy
 */

/**
 * Канонический замороженный список известных слоёв модалок и диалогов.
 * @type {readonly string[]}
 */
export const KNOWN_MODAL_IDS = Object.freeze([
  "quick-switcher",
  "settings-modal",
  "profile-modal",
  "invite-modal",
  "forward-modal",
  "captcha-modal",
  "gift-modal",
  "events-modal",
  "rules-screening-modal",
  "qr-auth-modal",
  "search-panel",
  "expression-picker",
  "context-menu",
  "threads-drawer",
  "inbox-popover",
]);

/**
 * Взаимно исключающие модальные окна (открытие одного закрывает другое).
 * @type {readonly string[]}
 */
export const DEFAULT_EXCLUSIVE_MODALS = Object.freeze([
  "quick-switcher",
  "settings-modal",
  "profile-modal",
  "invite-modal",
  "forward-modal",
  "captcha-modal",
  "gift-modal",
  "events-modal",
  "rules-screening-modal",
  "qr-auth-modal",
]);

/**
 * Инициализирует модульный контроллер маршрутизации модальных окон (TAURI-BOOT-02).
 *
 * @param {any} [app] Объект приложения, в который монтируются хелперы openModal/closeModal.
 * @param {ModalRouterOptions} [options] Опции конфигурации роутера.
 * @returns {ModalRouter}
 */
export function initModalRouter(app = {}, options = {}) {
  /** @type {Map<string, ModalController>} */
  const controllers = new Map();

  /** @type {string[]} */
  const activeStack = [];

  const exclusiveModals = new Set(DEFAULT_EXCLUSIVE_MODALS);

  /**
   * Регистрирует контроллер для модального окна.
   * @param {string} modalId Идентификатор модального окна.
   * @param {ModalController} controller Объект контроллера с методами open и close.
   * @returns {ModalController}
   */
  function registerModal(modalId, controller) {
    if (typeof modalId !== "string" || modalId.trim() === "") {
      throw new TypeError("modalId must be a non-empty string");
    }
    if (!controller || typeof controller !== "object") {
      throw new TypeError(`Expected controller object for modal: ${modalId}`);
    }
    controllers.set(modalId, controller);
    return controller;
  }

  /**
   * Удаляет зарегистрированный контроллер модального окна.
   * Если модалка сейчас открыта, закрывает её перед удалением.
   * @param {string} modalId
   * @returns {boolean}
   */
  function unregisterModal(modalId) {
    if (activeStack.includes(modalId)) {
      close(modalId);
    }
    return controllers.delete(modalId);
  }

  /**
   * Проверяет, зарегистрирован ли контроллер для указанного modalId.
   * @param {string} modalId
   * @returns {boolean}
   */
  function hasModal(modalId) {
    return controllers.has(modalId) ||
      Boolean(app?.shellBridge?.getController?.(modalId));
  }

  /**
   * Проверяет, открыто ли модальное окно в активном стеке.
   * @param {string} modalId
   * @returns {boolean}
   */
  function isOpen(modalId) {
    return activeStack.includes(modalId);
  }

  /**
   * Возвращает идентификатор текущего верхнего открытого модального окна (или null).
   * @returns {string | null}
   */
  function getActiveModal() {
    return activeStack.length > 0 ? (activeStack[activeStack.length - 1] ?? null) : null;
  }

  /**
   * Возвращает копию текущего LIFO-стека открытых модалок.
   * @returns {string[]}
   */
  function getActiveStack() {
    return [...activeStack];
  }

  /**
   * Открывает модальное окно с переданным payload.
   * Если открывается эксклюзивная модалка, закрывает любые другие открытые эксклюзивные окна.
   * @param {string} modalId
   * @param {unknown} [payload]
   * @returns {string}
   */
  function open(modalId, payload) {
    if (typeof modalId !== "string" || modalId.trim() === "") {
      throw new TypeError("modalId must be a non-empty string");
    }

    // Если модалка эксклюзивная — закрываем все другие открытые эксклюзивные модалки
    if (exclusiveModals.has(modalId)) {
      for (let i = activeStack.length - 1; i >= 0; i -= 1) {
        const id = activeStack[i];
        if (id && exclusiveModals.has(id) && id !== modalId) {
          close(id);
        }
      }
    }

    // Если модалка уже есть в стеке — перемещаем наверх
    const existingIndex = activeStack.indexOf(modalId);
    if (existingIndex !== -1) {
      activeStack.splice(existingIndex, 1);
    }

    const controller = controllers.get(modalId);
    if (controller?.open) {
      controller.open(payload);
    } else if (typeof app?.shellBridge?.openLayer === "function") {
      app.shellBridge.openLayer(modalId, payload);
    }

    activeStack.push(modalId);
    options.onOpen?.(modalId, payload);
    options.onChange?.(getActiveModal(), getActiveStack());
    return modalId;
  }

  /**
   * Закрывает указанное модальное окно (или верхнее, если modalId не передан).
   * @param {string} [modalId]
   * @returns {string | null}
   */
  function close(modalId) {
    const targetId = modalId ?? getActiveModal();
    if (!targetId) return null;

    const idx = activeStack.lastIndexOf(targetId);
    if (idx === -1) return null;

    activeStack.splice(idx, 1);

    const controller = controllers.get(targetId);
    if (controller?.close) {
      controller.close();
    } else if (typeof app?.shellBridge?.closeTopLayer === "function" && targetId === getActiveModal()) {
      app.shellBridge.closeTopLayer();
    }

    options.onClose?.(targetId);
    options.onChange?.(getActiveModal(), getActiveStack());
    return targetId;
  }

  /**
   * Закрывает верхнюю модалку в порядке LIFO.
   * @returns {string | null}
   */
  function closeTop() {
    return close();
  }

  /**
   * Закрывает все открытые модальные окна в порядке LIFO.
   * @returns {string[]}
   */
  function closeAll() {
    /** @type {string[]} */
    const closed = [];
    while (activeStack.length > 0) {
      const id = closeTop();
      if (id !== null) {
        closed.push(id);
      }
    }
    return closed;
  }

  const target = options.target ??
    app?.target ??
    app?.window ??
    app?.document ??
    (typeof window !== "undefined" ? window : (typeof document !== "undefined" ? document : null));

  /** @param {any} event */
  function onKeyDown(event) {
    if (event?.key === "Escape" && activeStack.length > 0) {
      event?.preventDefault?.();
      closeTop();
    }
  }

  if (options.listenEscape && target && typeof target.addEventListener === "function") {
    target.addEventListener("keydown", onKeyDown);
  }

  function destroy() {
    if (options.listenEscape && target && typeof target.removeEventListener === "function") {
      target.removeEventListener("keydown", onKeyDown);
    }
    closeAll();
    controllers.clear();
  }

  /** @type {ModalRouter} */
  const router = {
    open,
    close,
    closeTop,
    closeAll,
    isOpen,
    hasModal,
    getActiveModal,
    getActiveStack,
    registerModal,
    unregisterModal,
    destroy,
  };

  if (app && typeof app === "object") {
    app.modalRouter = router;
    app.openModal = open;
    app.closeModal = close;
    app.closeTopModal = closeTop;
  }

  return router;
}
