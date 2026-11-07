// @ts-check
/**
 * T-267 (FE-INT-10): единый реестр контроллеров доменных панелей гильдии
 * и маршрутизатор действий (`shell-controllers-registry.js`).
 *
 * Отвечает за:
 * - замороженный реестр 6 панелей администрирования гильдии `GUILD_ADMIN_PANELS`;
 * - чистый предикат `isValidGuildAdminPanel(panelId)` без сайд-эффектов;
 * - фабрику `createGuildAdminPanelRouter(options)` с реестром контроллеров,
 *   единственной активной панелью (взаимное исключение), маршрутизацией действий
 *   `dispatchPanelAction(panelId, action, payload)` и полной очисткой через `destroy()`.
 *
 * Границы ответственности: модуль не трогает DOM, IPC и сеть. Все побочные эффекты
 * вынесены в колбэки `onPanelChange` и `onDispatchAction`, поэтому роутер
 * детерминированно тестируется без браузерного окружения.
 *
 * @module ui/features/shell-controllers-registry
 */

/**
 * @typedef {"events-modal"
 *   | "role-editor"
 *   | "automod-editor"
 *   | "webhooks-editor"
 *   | "expressions-manager"
 *   | "moderation-panel"} GuildAdminPanelId
 */

/**
 * @typedef {object} GuildAdminPanelController
 * @property {(payload?: unknown) => unknown} [open]
 * @property {() => unknown} [close]
 */

/**
 * @typedef {object} GuildAdminPanelRouterOptions
 * @property {string | null} [initialPanel] Панель, активная сразу после создания роутера.
 * @property {(activePanel: string | null) => void} [onPanelChange] Уведомление о смене активной панели.
 * @property {(panelId: string, action: string, payload?: unknown) => unknown} [onDispatchAction]
 *   Обработчик действий панели; возвращаемое значение пробрасывается наружу.
 */

/**
 * @typedef {object} GuildAdminPanelRouter
 * @property {(panelId: string, controller: GuildAdminPanelController) => GuildAdminPanelController} registerPanel
 * @property {(panelId: string, payload?: unknown) => GuildAdminPanelId} openPanel
 * @property {() => GuildAdminPanelId | null} closeActivePanel
 * @property {(panelId: string, action: string, payload?: unknown) => unknown} dispatchPanelAction
 * @property {() => GuildAdminPanelId | null} getActivePanel
 * @property {() => GuildAdminPanelId[]} getRegisteredPanels
 * @property {() => void} destroy
 */

/**
 * Канонический замороженный реестр доменных панелей администрирования гильдии.
 * Порядок совпадает с порядком точек входа в UI сервера.
 * @type {readonly GuildAdminPanelId[]}
 */
export const GUILD_ADMIN_PANELS = Object.freeze([
  "events-modal",
  "role-editor",
  "automod-editor",
  "webhooks-editor",
  "expressions-manager",
  "moderation-panel",
]);

/**
 * Проверяет, что значение — строка из `GUILD_ADMIN_PANELS`.
 * Чистая функция: не бросает, не мутирует вход и не обращается к внешнему состоянию.
 * @param {unknown} panelId
 * @returns {panelId is GuildAdminPanelId}
 */
export function isValidGuildAdminPanel(panelId) {
  return (
    typeof panelId === "string" &&
    GUILD_ADMIN_PANELS.includes(/** @type {GuildAdminPanelId} */ (panelId))
  );
}

/**
 * Бросает `Error`, если `panelId` не входит в `GUILD_ADMIN_PANELS`.
 * @param {unknown} panelId
 * @returns {asserts panelId is GuildAdminPanelId}
 */
function assertValidPanelId(panelId) {
  if (!isValidGuildAdminPanel(panelId)) {
    throw new Error(`Неизвестная панель управления гильдией: ${String(panelId)}`);
  }
}

/**
 * Создаёт реестр контроллеров панелей гильдии и маршрутизатор действий.
 * @param {GuildAdminPanelRouterOptions} [options]
 * @returns {GuildAdminPanelRouter}
 */
export function createGuildAdminPanelRouter({
  initialPanel = null,
  onPanelChange,
  onDispatchAction,
} = {}) {
  if (initialPanel !== null) {
    assertValidPanelId(initialPanel);
  }

  /** @type {Map<GuildAdminPanelId, GuildAdminPanelController>} */
  const controllers = new Map();

  /** @type {GuildAdminPanelId | null} */
  let activePanel = initialPanel;

  /**
   * Регистрирует контроллер панели; повторная регистрация заменяет предыдущий контроллер.
   * @param {string} panelId
   * @param {GuildAdminPanelController} controller
   * @returns {GuildAdminPanelController}
   */
  function registerPanel(panelId, controller) {
    assertValidPanelId(panelId);
    if (!controller || typeof controller !== "object") {
      throw new TypeError(`Ожидался объект контроллера для панели ${panelId}`);
    }
    controllers.set(panelId, controller);
    return controller;
  }

  /**
   * Делает панель активной. Предыдущая активная панель закрывается своим `close?.()`.
   * @param {string} panelId
   * @param {unknown} [payload]
   * @returns {GuildAdminPanelId}
   */
  function openPanel(panelId, payload) {
    assertValidPanelId(panelId);

    if (activePanel !== null && activePanel !== panelId) {
      controllers.get(activePanel)?.close?.();
    }

    activePanel = panelId;
    controllers.get(panelId)?.open?.(payload);
    onPanelChange?.(panelId);
    return panelId;
  }

  /**
   * Закрывает активную панель, если она есть.
   * @returns {GuildAdminPanelId | null} Идентификатор закрытой панели или `null`.
   */
  function closeActivePanel() {
    if (activePanel === null) {
      return null;
    }
    const closedPanel = activePanel;
    controllers.get(closedPanel)?.close?.();
    activePanel = null;
    onPanelChange?.(null);
    return closedPanel;
  }

  /**
   * Маршрутизирует действие панели в `onDispatchAction`.
   * `action` обрезается по краям; пустое действие отклоняется.
   * @param {string} panelId
   * @param {string} action
   * @param {unknown} [payload]
   * @returns {unknown} Значение, возвращённое `onDispatchAction` (или `undefined`).
   */
  function dispatchPanelAction(panelId, action, payload) {
    assertValidPanelId(panelId);
    if (typeof action !== "string" || action.trim() === "") {
      throw new Error(`Пустое действие панели ${panelId}`);
    }
    return onDispatchAction?.(panelId, action.trim(), payload);
  }

  /**
   * Возвращает идентификатор активной панели.
   * @returns {GuildAdminPanelId | null}
   */
  function getActivePanel() {
    return activePanel;
  }

  /**
   * Возвращает отсортированный список зарегистрированных панелей (копию).
   * @returns {GuildAdminPanelId[]}
   */
  function getRegisteredPanels() {
    return [...controllers.keys()].sort();
  }

  /**
   * Закрывает активную панель и очищает реестр контроллеров.
   * @returns {void}
   */
  function destroy() {
    closeActivePanel();
    controllers.clear();
  }

  return {
    registerPanel,
    openPanel,
    closeActivePanel,
    dispatchPanelAction,
    getActivePanel,
    getRegisteredPanels,
    destroy,
  };
}
