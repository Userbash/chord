// @ts-check
/**
 * task-271 (FE-INT-14): верхнеуровневый фасад проводки оболочки приложения
 * (`app-shell-wiring.js`).
 *
 * Сводит три соседних модуля в одну точку сборки `app.js`:
 * - `shell-bridge.js` — стек модальных слоёв и глобальные сочетания клавиш;
 * - `shell-controllers-registry.js` — реестр панелей администрирования гильдии;
 * - `chat-helpers-bridge.js` — модель бейджей заголовка канала и тултип темы.
 *
 * Ключевое правило фасада — взаимное исключение двух уровней оверлеев:
 * открытие админ-панели закрывает все модальные слои, открытие модального слоя
 * закрывает активную админ-панель. Это гарантирует, что в `app.js` одновременно
 * живёт не больше одного оверлея, даже если UI-контроллеры не знают друг о друге.
 *
 * Модуль не трогает DOM, IPC и сеть: все побочные эффекты уходят в колбэки
 * `onDispatchIpc`, `onNavigate` и `onAdminPanelChange`.
 *
 * @module ui/features/app-shell-wiring
 */

import { SHELL_MODAL_LAYERS, createShellFeatureBridge } from "./shell-bridge.js";

import {
  GUILD_ADMIN_PANELS,
  createGuildAdminPanelRouter,
  isValidGuildAdminPanel
} from "./shell-controllers-registry.js";

import {
  buildChannelHeaderBadgesModel,
  formatChannelTopicTooltip
} from "./chat-helpers-bridge.js";

import { createDiscordNativeBridge } from "./discord-native-bridge.js";

/**
 * @typedef {import("./shell-bridge.js").ShellLayerController} ShellLayerController
 * @typedef {import("./shell-controllers-registry.js").GuildAdminPanelController} GuildAdminPanelController
 */

/**
 * @typedef {object} AppShellWiringOptions
 * @property {(command: string, payload?: unknown) => unknown} [onDispatchIpc]
 *   Проксирование IPC-команд (включая `guild_admin:<panel>:<action>`).
 * @property {(target: unknown) => unknown} [onNavigate] Проксирование навигации.
 * @property {(panelId: string | null) => void} [onAdminPanelChange]
 *   Уведомление о смене активной админ-панели (`null` — панелей нет).
 */

/**
 * Вход модели заголовка канала: поля бейджей из `chat-helpers-bridge.js`
 * плюс тема канала для тултипа.
 * @typedef {object} ChannelHeaderModelOptions
 * @property {number} [unreadCount]
 * @property {{ archived?: boolean, locked?: boolean, autoArchiveDuration?: number } | null} [threadMetadata]
 * @property {number} [boostCount]
 * @property {unknown} [topic] Сырая тема канала (может быть любой тип).
 * @property {number} [topicMaxChars] Лимит символов тултипа (по умолчанию 160).
 */

/**
 * Готовая модель строки заголовка канала.
 * @typedef {Readonly<{
 *   badges: import("./chat-helpers-bridge.js").ChannelHeaderBadgesModel,
 *   topicTooltip: string
 * }>} ChannelHeaderModel
 */

/**
 * Снимок состояния фасада: не больше одного активного оверлея на уровень.
 * `activeModalStack` — замороженная копия стека модальных слоёв в порядке
 * открытия (последний элемент — верхний слой).
 * @typedef {Readonly<{
 *   activeModalLayer: string | null,
 *   activeAdminPanel: string | null,
 *   activeModalStack: readonly string[]
 * }>} AppShellWiringState
 */

/**
 * @typedef {ReturnType<typeof createDiscordNativeBridge>} DiscordNativeBridge
 */

/**
 * @typedef {object} AppShellWiring
 * @property {(layerId: string, payload?: unknown) => string} openModalLayer
 * @property {() => string | null} closeTopLayer
 * @property {(panelId: string, payload?: unknown) => string} openAdminPanel
 * @property {() => string | null} closeAdminPanel
 * @property {(layerId: string, controller: ShellLayerController) => ShellLayerController} registerModalController
 * @property {(panelId: string, controller: GuildAdminPanelController) => GuildAdminPanelController} registerAdminController
 * @property {(panelId: string, action: string, payload?: unknown) => unknown} dispatchAdminAction
 * @property {(options?: ChannelHeaderModelOptions | null) => ChannelHeaderModel} getChannelHeaderModel
 * @property {() => AppShellWiringState} getState
 * @property {() => void} destroy
 * @property {DiscordNativeBridge} nativeBridge
 */

/**
 * Создаёт верхнеуровневый фасад проводки оболочки приложения.
 *
 * @param {AppShellWiringOptions} [options]
 * @returns {AppShellWiring}
 */
export function createAppShellWiring({ onDispatchIpc, onNavigate, onAdminPanelChange } = {}) {
  const shellBridge = createShellFeatureBridge({ onDispatchIpc, onNavigate });

  const adminRouter = createGuildAdminPanelRouter({
    onPanelChange: onAdminPanelChange,
    onDispatchAction: (panelId, action, payload) =>
      onDispatchIpc?.(`guild_admin:${panelId}:${action}`, payload)
  });

  const nativeBridge = createDiscordNativeBridge({
    invoke: async (channel, ...args) => onDispatchIpc?.(channel, args.length <= 1 ? args[0] : args),
    send: (channel, ...args) => {
      onDispatchIpc?.(channel, args.length <= 1 ? args[0] : args);
    }
  });

  /** @type {boolean} */
  let destroyed = false;

  /**
   * Бросает, если фасад уже уничтожен: после `destroy()` мутаторы непригодны.
   * @param {string} method Имя метода — попадает в сообщение об ошибке.
   * @returns {void}
   */
  function assertAlive(method) {
    if (destroyed) {
      throw new Error(`Фасад app-shell-wiring уничтожен: ${method} недоступен после destroy()`);
    }
  }

  /**
   * Проверяет id модального слоя до любых побочных эффектов, чтобы опечатка
   * не сносила уже открытый оверлей.
   * @param {string} layerId
   * @returns {void}
   */
  function assertValidModalLayerId(layerId) {
    if (
      typeof layerId !== "string" ||
      !SHELL_MODAL_LAYERS.includes(
        /** @type {import("./shell-bridge.js").ShellLayerId} */ (layerId)
      )
    ) {
      throw new Error(`Неизвестный слой оболочки UI: ${String(layerId)}`);
    }
  }

  /**
   * Верхний открытый модальный слой (LIFO) или `null`.
   * `shell-bridge.js` не отдаёт верхний слой отдельным методом, поэтому берём
   * последний элемент снимка стека.
   * @returns {string | null}
   */
  function getTopModalLayer() {
    const stack = shellBridge.getActiveOverlayStack();
    return stack.length > 0 ? /** @type {string} */ (stack[stack.length - 1]) : null;
  }

  /**
   * Открывает модальный слой оболочки, закрывая активную админ-панель.
   * Id валидируется до побочных эффектов: при неизвестном id состояние
   * остаётся прежним.
   * @param {string} layerId
   * @param {unknown} [payload]
   * @returns {string}
   */
  function openModalLayer(layerId, payload) {
    assertAlive("openModalLayer");
    assertValidModalLayerId(layerId);
    adminRouter.closeActivePanel();
    return shellBridge.openLayer(layerId, payload);
  }

  /**
   * Закрывает верхний оверлей: сначала активную админ-панель, затем — верхний
   * модальный слой из стека `shell-bridge.js`.
   * @returns {string | null}
   */
  function closeTopLayer() {
    if (adminRouter.getActivePanel() !== null) {
      return adminRouter.closeActivePanel();
    }
    return shellBridge.closeTopLayer();
  }

  /**
   * Открывает админ-панель гильдии, закрывая все модальные слои.
   * Id валидируется до побочных эффектов: при неизвестном id открытые
   * модальные слои не трогаются.
   * @param {string} panelId
   * @param {unknown} [payload]
   * @returns {string}
   */
  function openAdminPanel(panelId, payload) {
    assertAlive("openAdminPanel");
    if (!isValidGuildAdminPanel(panelId)) {
      throw new Error(`Неизвестная панель управления гильдией: ${String(panelId)}`);
    }
    shellBridge.closeAllLayers();
    return adminRouter.openPanel(panelId, payload);
  }

  /**
   * Закрывает активную админ-панель, если она есть.
   * @returns {string | null}
   */
  function closeAdminPanel() {
    return adminRouter.closeActivePanel();
  }

  /**
   * Регистрирует контроллер модального слоя.
   * @param {string} layerId
   * @param {ShellLayerController} controller
   * @returns {ShellLayerController}
   */
  function registerModalController(layerId, controller) {
    return shellBridge.registerController(layerId, controller);
  }

  /**
   * Регистрирует контроллер админ-панели.
   * @param {string} panelId
   * @param {GuildAdminPanelController} controller
   * @returns {GuildAdminPanelController}
   */
  function registerAdminController(panelId, controller) {
    return adminRouter.registerPanel(panelId, controller);
  }

  /**
   * Маршрутизирует действие панели в IPC как `guild_admin:<panel>:<action>`.
   * @param {string} panelId
   * @param {string} action
   * @param {unknown} [payload]
   * @returns {unknown}
   */
  function dispatchAdminAction(panelId, action, payload) {
    assertAlive("dispatchAdminAction");
    return adminRouter.dispatchPanelAction(panelId, action, payload);
  }

  /**
   * Собирает модель строки заголовка канала: бейджи и тултип темы.
   * @param {ChannelHeaderModelOptions | null} [options]
   * @returns {ChannelHeaderModel}
   */
  function getChannelHeaderModel(options) {
    const badges = buildChannelHeaderBadgesModel(options ?? undefined);
    const topicTooltip = formatChannelTopicTooltip(
      options?.topic,
      options?.topicMaxChars ?? undefined
    );
    return Object.freeze({ badges, topicTooltip });
  }

  /**
   * Возвращает замороженный снимок активных оверлеев: сам объект и вложенный
   * массив `activeModalStack` защищены от внешней мутации.
   * @returns {AppShellWiringState}
   */
  function getState() {
    return Object.freeze({
      activeModalLayer: getTopModalLayer(),
      activeAdminPanel: adminRouter.getActivePanel(),
      activeModalStack: Object.freeze([...shellBridge.getActiveOverlayStack()])
    });
  }

  /**
   * Закрывает активную панель и все модальные слои, очищает реестры.
   * Повторный вызов безопасен. После `destroy()` мутаторы
   * `openModalLayer`, `openAdminPanel` и `dispatchAdminAction` бросают `Error`,
   * то есть фасад дальше не открывает оверлеи.
   * @returns {void}
   */
  function destroy() {
    if (destroyed) {
      return;
    }
    destroyed = true;
    adminRouter.destroy();
    shellBridge.closeAllLayers();
  }

  return {
    openModalLayer,
    closeTopLayer,
    openAdminPanel,
    closeAdminPanel,
    registerModalController,
    registerAdminController,
    dispatchAdminAction,
    getChannelHeaderModel,
    getState,
    destroy,
    nativeBridge
  };
}

export { GUILD_ADMIN_PANELS, SHELL_MODAL_LAYERS };
