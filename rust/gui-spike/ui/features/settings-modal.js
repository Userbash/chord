// @ts-check
/**
 * T-215 (RW-40 / G2): контроллер полноэкранного модального окна настроек
 * пользователя (`settings-modal.js`).
 *
 * Оракулы контрактов:
 * - RE-дамп `/home/sanya/re-deep/ui8/23-settings-categories.json`,
 *   `/home/sanya/re-deep/ui8/23-settings-voice.html`,
 *   `/home/sanya/re-deep/frontend-d03.md` (§4.1–§4.2, §5 Modal).
 *
 * Отвечает за:
 * - `SETTINGS_TABS_RU`: замороженный список из 5 разделов настроек
 *   (`appearance`, `voice`, `locale`, `keybinds`, `notifications`) с русскими подписями;
 * - `open(initialTab = 'appearance')`: рендер полноэкранного диалога настроек
 *   (`role="dialog"`, `aria-modal="true"`, `data-settings-open="true"`) с боковой
 *   навигацией из `SETTINGS_TABS_RU`, панелью активной вкладки, кнопкой
 *   «Закрыть (ESC)» и кнопкой «Выйти из аккаунта»;
 * - `setTab(tabId)`: переключение активной вкладки (невалидные `tabId` игнорируются)
 *   с синхронизацией `aria-selected="true" | "false"`;
 * - `updateSetting(key, value)`: обновление внутреннего состояния настроек
 *   (`theme`, `fontScale`, `messageGroupSpacing`, `inputMode`, `vadThresholdDb`,
 *   `locale`, `desktopNotifications` и др.) и вызов `onUpdateConfig({ ...state })`;
 * - `confirmLogout()`: открытие диалога подтверждения выхода
 *   («Вы уверены, что хотите выйти?») с вызовом `onLogout()` после подтверждения;
 * - `close()`: удаление модального окна из контейнера и вызов `onClose()`.
 *
 * @module ui/features/settings-modal
 */

/**
 * @typedef {"appearance" | "voice" | "locale" | "keybinds" | "notifications"} SettingsTabId
 */

/**
 * @typedef {object} SettingsTabEntry
 * @property {SettingsTabId} id
 * @property {string} labelRu
 */

const DEFAULT_TAB_ENTRY = Object.freeze({
  id: /** @type {const} */ ("appearance"),
  labelRu: "Внешний вид",
});

/**
 * Замороженный справочник разделов настроек пользователя с русскими подписями.
 * @type {ReadonlyArray<Readonly<SettingsTabEntry>>}
 */
export const SETTINGS_TABS_RU = Object.freeze([
  DEFAULT_TAB_ENTRY,
  Object.freeze({ id: /** @type {const} */ ("voice"), labelRu: "Голос и видео" }),
  Object.freeze({ id: /** @type {const} */ ("locale"), labelRu: "Язык" }),
  Object.freeze({ id: /** @type {const} */ ("keybinds"), labelRu: "Горячие клавиши" }),
  Object.freeze({ id: /** @type {const} */ ("notifications"), labelRu: "Уведомления" }),
]);

/** Русская подпись кнопки закрытия полноэкранных настроек. */
export const CLOSE_SETTINGS_LABEL_RU = "Закрыть (ESC)";

/** Русская подпись кнопки выхода из аккаунта в боковой навигации. */
export const LOGOUT_BUTTON_LABEL_RU = "Выйти из аккаунта";

/** Текст подтверждения выхода из аккаунта. */
export const LOGOUT_CONFIRM_PROMPT_RU = "Вы уверены, что хотите выйти?";

/**
 * @typedef {object} SettingsConfigState
 * @property {string} theme
 * @property {number} fontScale
 * @property {number} messageGroupSpacing
 * @property {string} inputMode
 * @property {number} vadThresholdDb
 * @property {string} locale
 * @property {boolean} desktopNotifications
 * @property {Record<string, any>} [extra]
 */

/**
 * Значения настроек по умолчанию.
 * @type {Readonly<{
 *   theme: string,
 *   fontScale: number,
 *   messageGroupSpacing: number,
 *   inputMode: string,
 *   vadThresholdDb: number,
 *   locale: string,
 *   desktopNotifications: boolean,
 * }>}
 */
export const DEFAULT_SETTINGS_CONFIG = Object.freeze({
  theme: "dark",
  fontScale: 100,
  messageGroupSpacing: 16,
  inputMode: "VOICE_ACTIVITY",
  vadThresholdDb: -60,
  locale: "ru",
  desktopNotifications: true,
});

/**
 * @typedef {object} SettingsModalOptions
 * @property {(nextConfig: Record<string, any>) => void} [onUpdateConfig]
 * @property {() => void} [onLogout]
 * @property {() => void} [onClose]
 */

/**
 * Проверяет, входит ли идентификатор вкладки в `SETTINGS_TABS_RU`.
 *
 * @param {unknown} tabId
 * @returns {tabId is SettingsTabId}
 */
function isValidTabId(tabId) {
  return (
    typeof tabId === "string" &&
    SETTINGS_TABS_RU.some((entry) => entry.id === tabId)
  );
}

/**
 * Возвращает запись вкладки из `SETTINGS_TABS_RU` по её идентификатору.
 *
 * @param {SettingsTabId} tabId
 * @returns {Readonly<SettingsTabEntry>}
 */
function getTabEntry(tabId) {
  const found = SETTINGS_TABS_RU.find((entry) => entry.id === tabId);
  return found ?? DEFAULT_TAB_ENTRY;
}

/**
 * Возвращает фабрику DOM-элементов из контейнера или глобального документа.
 *
 * @param {any} containerEl
 * @returns {{ createElement: (tag: string) => any }}
 */
function resolveDocument(containerEl) {
  if (!containerEl || typeof containerEl !== "object") {
    throw new TypeError("containerEl must be a valid DOM element");
  }
  if (
    containerEl.ownerDocument &&
    typeof containerEl.ownerDocument.createElement === "function"
  ) {
    return containerEl.ownerDocument;
  }
  if (typeof containerEl.createElement === "function") {
    return containerEl;
  }
  const globalDoc = /** @type {any} */ (globalThis).document;
  if (globalDoc && typeof globalDoc.createElement === "function") {
    return globalDoc;
  }
  throw new TypeError("containerEl must provide ownerDocument.createElement");
}

/**
 * Нормализует стартовую конфигурацию настроек поверх `DEFAULT_SETTINGS_CONFIG`.
 *
 * @param {Record<string, any> | null | undefined} initialConfig
 * @returns {Record<string, any>}
 */
function normalizeInitialConfig(initialConfig) {
  /** @type {Record<string, any>} */
  const base = { ...DEFAULT_SETTINGS_CONFIG };
  if (!initialConfig || typeof initialConfig !== "object" || Array.isArray(initialConfig)) {
    return base;
  }
  for (const [key, val] of Object.entries(initialConfig)) {
    if (val !== undefined) {
      base[key] = val;
    }
  }
  return base;
}

/**
 * Создаёт контроллер полноэкранного модального окна настроек.
 *
 * @param {any} containerEl
 * @param {Record<string, any>} [initialConfig]
 * @param {SettingsModalOptions} [options]
 */
export function createSettingsModalController(
  containerEl,
  initialConfig = {},
  options = {},
) {
  const doc = resolveDocument(containerEl);
  const safeOptions = options && typeof options === "object" ? options : {};

  /** @type {Record<string, any>} */
  const state = normalizeInitialConfig(initialConfig);

  /** @type {SettingsTabId} */
  let activeTab = "appearance";

  /** @type {any | null} */
  let modalEl = null;

  /** @type {any | null} */
  let contentPaneEl = null;

  /** @type {any | null} */
  let logoutPromptEl = null;

  /** @type {Map<SettingsTabId, any>} */
  const tabButtonMap = new Map();

  /** @type {Map<string, { kind: "value" | "switch", el: any }>} */
  const activeControlMap = new Map();

  /**
   * Удаляет открытый диалог подтверждения выхода, если он есть.
   */
  function removeLogoutPrompt() {
    if (!logoutPromptEl) return;
    const current = logoutPromptEl;
    logoutPromptEl = null;
    if (typeof current.remove === "function") {
      current.remove();
    } else if (current.parentNode && typeof current.parentNode.removeChild === "function") {
      current.parentNode.removeChild(current);
    }
  }

  /**
   * Удаляет DOM-узел модального окна без вызова `onClose`.
   */
  function unmountModalNode() {
    removeLogoutPrompt();
    tabButtonMap.clear();
    activeControlMap.clear();
    contentPaneEl = null;
    if (!modalEl) return;
    const current = modalEl;
    modalEl = null;
    if (typeof current.remove === "function") {
      current.remove();
    } else if (current.parentNode && typeof current.parentNode.removeChild === "function") {
      current.parentNode.removeChild(current);
    }
  }

  /**
   * Строит элемент управления текстовым/числовым значением настройки.
   *
   * @param {string} key
   * @param {string} labelText
   * @param {"string" | "number"} valueKind
   * @returns {any}
   */
  function buildValueControl(key, labelText, valueKind) {
    const row = doc.createElement("div");
    row.setAttribute("data-setting-row", key);

    const label = doc.createElement("label");
    label.textContent = labelText;
    row.appendChild(label);

    const input = doc.createElement("input");
    input.setAttribute("data-setting-key", key);
    input.setAttribute("aria-label", labelText);
    input.value = String(state[key] ?? "");
    input.addEventListener("change", (/** @type {any} */ event) => {
      const raw = event?.target?.value ?? input.value;
      const nextVal = valueKind === "number" ? Number(raw) : String(raw ?? "");
      updateSetting(key, nextVal);
    });

    activeControlMap.set(key, { kind: "value", el: input });
    row.appendChild(input);
    return row;
  }

  /**
   * Строит переключатель булевой настройки (`role="switch"`).
   *
   * @param {string} key
   * @param {string} labelText
   * @returns {any}
   */
  function buildSwitchControl(key, labelText) {
    const row = doc.createElement("div");
    row.setAttribute("data-setting-row", key);

    const btn = doc.createElement("button");
    btn.setAttribute("type", "button");
    btn.setAttribute("role", "switch");
    btn.setAttribute("data-setting-key", key);
    btn.setAttribute("aria-label", labelText);
    const currentChecked = Boolean(state[key]);
    btn.setAttribute("aria-checked", currentChecked ? "true" : "false");
    btn.textContent = labelText;

    btn.addEventListener("click", () => {
      const nextChecked = !Boolean(state[key]);
      updateSetting(key, nextChecked);
    });

    activeControlMap.set(key, { kind: "switch", el: btn });
    row.appendChild(btn);
    return row;
  }

  /**
   * Синхронизирует смонтированный DOM-контрол настройки без пересоздания узла.
   *
   * @param {string} key
   */
  function syncRenderedControl(key) {
    const entry = activeControlMap.get(key);
    if (!entry) return;
    if (entry.kind === "switch") {
      entry.el.setAttribute("aria-checked", Boolean(state[key]) ? "true" : "false");
    } else {
      entry.el.value = String(state[key] ?? "");
    }
  }

  /**
   * Перерисовывает панель активной вкладки внутри открытого модального окна.
   */
  function renderActiveTabPane() {
    if (!contentPaneEl) return;
    activeControlMap.clear();

    const tabEntry = getTabEntry(activeTab);
    contentPaneEl.setAttribute("data-settings-pane", activeTab);
    contentPaneEl.setAttribute("aria-label", tabEntry.labelRu);

    /** @type {any[]} */
    const nodes = [];

    const titleEl = doc.createElement("h2");
    titleEl.setAttribute("data-settings-pane-title", activeTab);
    titleEl.textContent = tabEntry.labelRu;
    nodes.push(titleEl);

    if (activeTab === "appearance") {
      nodes.push(buildValueControl("theme", "Тема", "string"));
      nodes.push(buildValueControl("fontScale", "Масштаб шрифта", "number"));
      nodes.push(
        buildValueControl("messageGroupSpacing", "Расстояние между группами сообщений", "number"),
      );
    } else if (activeTab === "voice") {
      nodes.push(buildValueControl("inputMode", "Режим ввода", "string"));
      nodes.push(buildValueControl("vadThresholdDb", "Порог чувствительности (дБ)", "number"));
    } else if (activeTab === "locale") {
      nodes.push(buildValueControl("locale", "Язык интерфейса", "string"));
    } else if (activeTab === "keybinds") {
      const list = doc.createElement("ul");
      list.setAttribute("data-settings-keybinds-list", "");
      const items = [
        "Escape — Закрыть (ESC)",
        "Ctrl+K — Быстрый переход",
        "Ctrl+Shift+M — Вкл./выкл. микрофон",
        "Ctrl+Shift+D — Вкл./выкл. звук",
      ];
      for (const text of items) {
        const li = doc.createElement("li");
        li.textContent = text;
        list.appendChild(li);
      }
      nodes.push(list);
    } else if (activeTab === "notifications") {
      nodes.push(
        buildSwitchControl("desktopNotifications", "Уведомления на рабочем столе"),
      );
    }

    if (typeof contentPaneEl.replaceChildren === "function") {
      contentPaneEl.replaceChildren(...nodes);
    } else {
      if (Array.isArray(contentPaneEl.children)) {
        contentPaneEl.children.length = 0;
      }
      for (const node of nodes) {
        contentPaneEl.appendChild(node);
      }
    }
  }

  /**
   * Переключает активную вкладку настроек. Невалидные `tabId` игнорируются.
   *
   * @param {string} tabId
   * @returns {boolean}
   */
  function setTab(tabId) {
    if (!isValidTabId(tabId)) {
      return false;
    }

    activeTab = tabId;

    if (modalEl) {
      modalEl.setAttribute("data-active-tab", activeTab);
      for (const [id, btn] of tabButtonMap.entries()) {
        const isSelected = id === activeTab;
        btn.setAttribute("aria-selected", isSelected ? "true" : "false");
        btn.setAttribute("tabindex", isSelected ? "0" : "-1");
      }
      renderActiveTabPane();
    }

    return true;
  }

  /**
   * Обновляет значение настройки в состоянии контроллера и вызывает `onUpdateConfig({ ...state })`.
   *
   * @param {string} key
   * @param {any} value
   * @returns {boolean}
   */
  function updateSetting(key, value) {
    if (typeof key !== "string" || key.trim().length === 0) {
      return false;
    }

    state[key] = value;

    if (modalEl) {
      syncRenderedControl(key);
    }

    if (typeof safeOptions.onUpdateConfig === "function") {
      safeOptions.onUpdateConfig({ ...state });
    }

    return true;
  }

  /**
   * Открывает диалог подтверждения выхода из аккаунта («Вы уверены, что хотите выйти?»)
   * и вызывает `onLogout()` при подтверждении.
   *
   * @returns {any}
   */
  function confirmLogout() {
    removeLogoutPrompt();

    const prompt = doc.createElement("div");
    prompt.setAttribute("role", "alertdialog");
    prompt.setAttribute("aria-modal", "true");
    prompt.setAttribute("data-logout-confirm", "true");

    const messageEl = doc.createElement("p");
    messageEl.setAttribute("data-logout-prompt-text", "");
    messageEl.textContent = LOGOUT_CONFIRM_PROMPT_RU;
    prompt.appendChild(messageEl);

    const actionsEl = doc.createElement("div");
    actionsEl.setAttribute("data-logout-confirm-actions", "");

    const cancelBtn = doc.createElement("button");
    cancelBtn.setAttribute("type", "button");
    cancelBtn.setAttribute("data-logout-confirm-cancel", "");
    cancelBtn.textContent = "Отмена";
    cancelBtn.addEventListener("click", () => {
      removeLogoutPrompt();
    });

    const acceptBtn = doc.createElement("button");
    acceptBtn.setAttribute("type", "button");
    acceptBtn.setAttribute("data-logout-confirm-accept", "");
    acceptBtn.textContent = "Выйти";
    acceptBtn.addEventListener("click", () => {
      removeLogoutPrompt();
      if (typeof safeOptions.onLogout === "function") {
        safeOptions.onLogout();
      }
    });

    actionsEl.appendChild(cancelBtn);
    actionsEl.appendChild(acceptBtn);
    prompt.appendChild(actionsEl);

    const host = modalEl ?? containerEl;
    host.appendChild(prompt);
    logoutPromptEl = prompt;
    return prompt;
  }

  /**
   * Открывает полноэкранное модальное окно настроек.
   *
   * @param {SettingsTabId | string} [initialTab="appearance"]
   * @returns {any}
   */
  function open(initialTab = "appearance") {
    unmountModalNode();

    activeTab = isValidTabId(initialTab) ? initialTab : "appearance";

    const dialog = doc.createElement("div");
    dialog.setAttribute("role", "dialog");
    dialog.setAttribute("aria-modal", "true");
    dialog.setAttribute("data-settings-open", "true");
    dialog.setAttribute("data-active-tab", activeTab);
    dialog.setAttribute("aria-label", "Настройки пользователя");

    dialog.addEventListener("keydown", (/** @type {any} */ event) => {
      if (event?.key === "Escape") {
        if (typeof event.preventDefault === "function") {
          event.preventDefault();
        }
        close();
      }
    });

    const sidebar = doc.createElement("aside");
    sidebar.setAttribute("data-settings-sidebar", "");

    const navList = doc.createElement("div");
    navList.setAttribute("role", "tablist");
    navList.setAttribute("aria-label", "Разделы настроек");

    for (const tabEntry of SETTINGS_TABS_RU) {
      const tabBtn = doc.createElement("button");
      tabBtn.setAttribute("type", "button");
      tabBtn.setAttribute("role", "tab");
      tabBtn.setAttribute("data-settings-tab", tabEntry.id);
      const isSelected = tabEntry.id === activeTab;
      tabBtn.setAttribute("aria-selected", isSelected ? "true" : "false");
      tabBtn.setAttribute("tabindex", isSelected ? "0" : "-1");
      tabBtn.textContent = tabEntry.labelRu;
      tabBtn.addEventListener("click", () => {
        setTab(tabEntry.id);
      });
      tabButtonMap.set(tabEntry.id, tabBtn);
      navList.appendChild(tabBtn);
    }

    sidebar.appendChild(navList);

    const logoutBtn = doc.createElement("button");
    logoutBtn.setAttribute("type", "button");
    logoutBtn.setAttribute("data-settings-logout", "");
    logoutBtn.textContent = LOGOUT_BUTTON_LABEL_RU;
    logoutBtn.addEventListener("click", () => {
      confirmLogout();
    });
    sidebar.appendChild(logoutBtn);

    const mainArea = doc.createElement("div");
    mainArea.setAttribute("data-settings-content", "");

    const pane = doc.createElement("div");
    pane.setAttribute("role", "tabpanel");
    contentPaneEl = pane;
    mainArea.appendChild(pane);

    const closeBtn = doc.createElement("button");
    closeBtn.setAttribute("type", "button");
    closeBtn.setAttribute("data-settings-close", "");
    closeBtn.setAttribute("aria-label", CLOSE_SETTINGS_LABEL_RU);
    closeBtn.textContent = CLOSE_SETTINGS_LABEL_RU;
    closeBtn.addEventListener("click", () => {
      close();
    });
    mainArea.appendChild(closeBtn);

    dialog.appendChild(sidebar);
    dialog.appendChild(mainArea);

    containerEl.appendChild(dialog);
    modalEl = dialog;
    renderActiveTabPane();

    return dialog;
  }

  /**
   * Закрывает и удаляет модальное окно настроек, вызывая `onClose()`.
   *
   * @returns {boolean}
   */
  function close() {
    if (!modalEl) {
      return false;
    }
    unmountModalNode();
    if (typeof safeOptions.onClose === "function") {
      safeOptions.onClose();
    }
    return true;
  }

  return {
    open,
    setTab,
    updateSetting,
    confirmLogout,
    close,
    /** @returns {Record<string, any>} */
    getState() {
      return { ...state };
    },
    /** @returns {SettingsTabId} */
    getActiveTab() {
      return activeTab;
    },
    /** @returns {boolean} */
    isOpen() {
      return modalEl !== null;
    },
  };
}
