// @ts-check
/**
 * T-215 (RW-40 / G2): unit-тесты контроллера полноэкранного модального окна
 * настроек (`rust/gui-spike/ui/features/settings-modal.js`).
 */
import { describe, it } from "node:test";
import assert from "node:assert/strict";

import {
  SETTINGS_TABS_RU,
  DEFAULT_SETTINGS_CONFIG,
  createSettingsModalController,
} from "../../rust/gui-spike/ui/features/settings-modal.js";

/* ------------------------------------------------------------------ *
 * Минимальный DOM-шим для изолированных unit-тестов
 * ------------------------------------------------------------------ */

class FakeElement {
  /**
   * @param {string} tagName
   * @param {any} [ownerDocument]
   */
  constructor(tagName, ownerDocument = null) {
    this.tagName = String(tagName).toUpperCase();
    this.ownerDocument = ownerDocument;
    /** @type {Map<string, string>} */
    this._attrs = new Map();
    /** @type {FakeElement[]} */
    this.children = [];
    /** @type {FakeElement | null} */
    this.parentNode = null;
    this._text = "";
    this._value = "";
    this._checked = false;
    this._disabled = false;
    this.selected = false;
    /** @type {Map<string, Array<(event: any) => void>>} */
    this._listeners = new Map();
  }

  /** @returns {string} */
  get value() {
    return this._value;
  }

  /** @param {string} val */
  set value(val) {
    this._value = String(val ?? "");
    this._attrs.set("value", this._value);
  }

  /** @returns {boolean} */
  get checked() {
    return this._checked;
  }

  /** @param {boolean} val */
  set checked(val) {
    this._checked = Boolean(val);
    if (this._checked) {
      this._attrs.set("checked", "");
    } else {
      this._attrs.delete("checked");
    }
  }

  /** @returns {boolean} */
  get disabled() {
    return this._disabled || this._attrs.has("disabled");
  }

  /** @param {boolean} val */
  set disabled(val) {
    this._disabled = Boolean(val);
    if (this._disabled) {
      this._attrs.set("disabled", "");
    } else {
      this._attrs.delete("disabled");
    }
  }

  /** @returns {string} */
  get textContent() {
    if (this.children.length > 0) {
      return this.children.map((c) => c.textContent).join("");
    }
    return this._text;
  }

  /** @param {string} val */
  set textContent(val) {
    for (const child of this.children) {
      child.parentNode = null;
    }
    this.children = [];
    this._text = String(val ?? "");
  }

  /**
   * @param {string} name
   * @param {string} value
   */
  setAttribute(name, value) {
    const str = String(value);
    this._attrs.set(name, str);
    if (name === "value") this._value = str;
    if (name === "disabled") this._disabled = true;
    if (name === "checked") this._checked = true;
  }

  /**
   * @param {string} name
   * @returns {string | null}
   */
  getAttribute(name) {
    return this._attrs.has(name) ? (this._attrs.get(name) ?? null) : null;
  }

  /**
   * @param {string} name
   * @returns {boolean}
   */
  hasAttribute(name) {
    return this._attrs.has(name);
  }

  /** @param {string} name */
  removeAttribute(name) {
    this._attrs.delete(name);
    if (name === "disabled") this._disabled = false;
    if (name === "checked") this._checked = false;
  }

  /**
   * @param {FakeElement} child
   * @returns {FakeElement}
   */
  appendChild(child) {
    if (child.parentNode) {
      child.parentNode.removeChild(child);
    }
    child.parentNode = this;
    if (!child.ownerDocument && this.ownerDocument) {
      child.ownerDocument = this.ownerDocument;
    }
    this.children.push(child);
    return child;
  }

  /**
   * @param {FakeElement} child
   * @returns {FakeElement}
   */
  removeChild(child) {
    this.children = this.children.filter((c) => c !== child);
    child.parentNode = null;
    return child;
  }

  /** @param {...FakeElement} nodes */
  replaceChildren(...nodes) {
    for (const child of this.children) {
      child.parentNode = null;
    }
    this.children = [];
    this._text = "";
    for (const node of nodes) {
      this.appendChild(node);
    }
  }

  remove() {
    if (this.parentNode) {
      this.parentNode.removeChild(this);
    }
  }

  /**
   * @param {string} type
   * @param {(event: any) => void} listener
   */
  addEventListener(type, listener) {
    if (!this._listeners.has(type)) {
      this._listeners.set(type, []);
    }
    this._listeners.get(type)?.push(listener);
  }

  /**
   * @param {string} type
   * @param {Record<string, any>} [extra]
   */
  dispatchEvent(type, extra = {}) {
    const event = {
      type,
      target: this,
      currentTarget: this,
      defaultPrevented: false,
      propagationStopped: false,
      preventDefault() {
        this.defaultPrevented = true;
      },
      stopPropagation() {
        this.propagationStopped = true;
      },
      ...extra,
    };
    for (const listener of this._listeners.get(type) ?? []) {
      listener(event);
    }
    return !event.defaultPrevented;
  }

  click() {
    if (this.disabled) return false;
    return this.dispatchEvent("click");
  }

  /**
   * Поддержка базовых селекторов: `[attr]`, `[attr="val"]`, `tag`, `tag[attr="val"]`.
   *
   * @param {string} selector
   * @returns {boolean}
   */
  matches(selector) {
    const trimmed = selector.trim();
    const attrPattern = /^([a-zA-Z0-9_-]+)?\[([a-zA-Z0-9_-]+)(?:=['"]?([^'"\]]*)['"]?)?\]$/;
    const match = trimmed.match(attrPattern);
    if (match) {
      const [, tagPart, attrName, attrVal] = match;
      if (!attrName) return false;
      if (tagPart && this.tagName !== tagPart.toUpperCase()) {
        return false;
      }
      if (!this.hasAttribute(attrName)) {
        return false;
      }
      if (attrVal !== undefined) {
        return this.getAttribute(attrName) === attrVal;
      }
      return true;
    }
    return this.tagName === trimmed.toUpperCase();
  }

  /**
   * @param {string} selector
   * @returns {FakeElement | null}
   */
  querySelector(selector) {
    for (const child of this.children) {
      if (child.matches(selector)) return child;
      const nested = child.querySelector(selector);
      if (nested) return nested;
    }
    return null;
  }

  /**
   * @param {string} selector
   * @returns {FakeElement[]}
   */
  querySelectorAll(selector) {
    /** @type {FakeElement[]} */
    const found = [];
    for (const child of this.children) {
      if (child.matches(selector)) found.push(child);
      found.push(...child.querySelectorAll(selector));
    }
    return found;
  }
}

/**
 * Создаёт изолированный тестовый контейнер с фабрикой `ownerDocument.createElement`.
 *
 * @returns {FakeElement}
 */
function createTestContainer() {
  const doc = {
    /** @param {string} tag */
    createElement(tag) {
      return new FakeElement(tag, doc);
    },
  };
  return new FakeElement("div", doc);
}

/* ------------------------------------------------------------------ *
 * Сьюты тестов
 * ------------------------------------------------------------------ */

describe("SETTINGS_TABS_RU и дефолтная конфигурация", () => {
  it("содержит замороженный список из 5 разделов настроек с русскими подписями", () => {
    assert.ok(Object.isFrozen(SETTINGS_TABS_RU), "SETTINGS_TABS_RU должен быть заморожен");
    assert.equal(SETTINGS_TABS_RU.length, 5);
    assert.deepEqual(SETTINGS_TABS_RU, [
      { id: "appearance", labelRu: "Внешний вид" },
      { id: "voice", labelRu: "Голос и видео" },
      { id: "locale", labelRu: "Язык" },
      { id: "keybinds", labelRu: "Горячие клавиши" },
      { id: "notifications", labelRu: "Уведомления" },
    ]);
    for (const tab of SETTINGS_TABS_RU) {
      assert.ok(Object.isFrozen(tab), `объект вкладки ${tab.id} должен быть заморожен`);
    }
    assert.ok(Object.isFrozen(DEFAULT_SETTINGS_CONFIG));
  });
});

describe("createSettingsModalController: open() и рендер диалога", () => {
  it("open() рендерит полноэкранный диалог (role='dialog', aria-modal='true', data-settings-open='true'), боковую навигацию, активную панель, 'Закрыть (ESC)' и 'Выйти из аккаунта'", () => {
    const container = createTestContainer();
    const ctrl = createSettingsModalController(container);

    const dialog = ctrl.open();
    assert.ok(dialog);
    assert.equal(dialog.getAttribute("role"), "dialog");
    assert.equal(dialog.getAttribute("aria-modal"), "true");
    assert.equal(dialog.getAttribute("data-settings-open"), "true");

    const foundDialog = container.querySelector('[data-settings-open="true"]');
    assert.equal(foundDialog, dialog);

    const tabs = dialog.querySelectorAll('[role="tab"]');
    assert.equal(tabs.length, 5);
    assert.deepEqual(
      tabs.map((/** @type {FakeElement} */ t) => ({
        id: t.getAttribute("data-settings-tab"),
        label: t.textContent,
        selected: t.getAttribute("aria-selected"),
      })),
      [
        { id: "appearance", label: "Внешний вид", selected: "true" },
        { id: "voice", label: "Голос и видео", selected: "false" },
        { id: "locale", label: "Язык", selected: "false" },
        { id: "keybinds", label: "Горячие клавиши", selected: "false" },
        { id: "notifications", label: "Уведомления", selected: "false" },
      ],
    );

    const activePane = dialog.querySelector('[role="tabpanel"]');
    assert.ok(activePane, "активная панель настроек должна быть отрендерена");
    assert.equal(activePane.getAttribute("data-settings-pane"), "appearance");
    assert.ok(activePane.textContent.includes("Внешний вид"));

    const closeBtn = dialog.querySelector("[data-settings-close]");
    assert.ok(closeBtn, "кнопка закрытия должна присутствовать");
    assert.equal(closeBtn.textContent, "Закрыть (ESC)");

    const logoutBtn = dialog.querySelector("[data-settings-logout]");
    assert.ok(logoutBtn, "кнопка выхода из аккаунта должна присутствовать");
    assert.equal(logoutBtn.textContent, "Выйти из аккаунта");
  });

  it("open(initialTab) открывает указанную валидную вкладку и откатывается на 'appearance' при невалидной", () => {
    const container = createTestContainer();
    const ctrl = createSettingsModalController(container);

    ctrl.open("voice");
    let voiceTab = container.querySelector('[data-settings-tab="voice"]');
    let appearanceTab = container.querySelector('[data-settings-tab="appearance"]');
    let pane = container.querySelector('[role="tabpanel"]');
    assert.equal(voiceTab?.getAttribute("aria-selected"), "true");
    assert.equal(appearanceTab?.getAttribute("aria-selected"), "false");
    assert.equal(pane?.getAttribute("data-settings-pane"), "voice");
    assert.ok(pane?.textContent.includes("Голос и видео"));

    ctrl.open(/** @type {any} */ ("unknown_tab"));
    voiceTab = container.querySelector('[data-settings-tab="voice"]');
    appearanceTab = container.querySelector('[data-settings-tab="appearance"]');
    pane = container.querySelector('[role="tabpanel"]');
    assert.equal(appearanceTab?.getAttribute("aria-selected"), "true");
    assert.equal(voiceTab?.getAttribute("aria-selected"), "false");
    assert.equal(pane?.getAttribute("data-settings-pane"), "appearance");
  });
});

describe("createSettingsModalController: setTab(tabId)", () => {
  it("переключает активную вкладку, обновляет aria-selected='true'|'false' и панель контента", () => {
    const container = createTestContainer();
    const ctrl = createSettingsModalController(container);
    ctrl.open();

    for (const tabSpec of SETTINGS_TABS_RU) {
      const changed = ctrl.setTab(tabSpec.id);
      assert.equal(changed, true, `setTab('${tabSpec.id}') должен возвращать true`);

      const allTabs = container.querySelectorAll('[role="tab"]');
      for (const tabBtn of allTabs) {
        const id = tabBtn.getAttribute("data-settings-tab");
        assert.equal(
          tabBtn.getAttribute("aria-selected"),
          id === tabSpec.id ? "true" : "false",
          `некорректный aria-selected для вкладки ${id} при активной ${tabSpec.id}`,
        );
      }

      const pane = container.querySelector('[role="tabpanel"]');
      assert.ok(pane);
      assert.equal(pane.getAttribute("data-settings-pane"), tabSpec.id);
      assert.ok(pane.textContent.includes(tabSpec.labelRu));
    }
  });

  it("игнорирует невалидные tabId без изменения активной вкладки и поддерживает клик по вкладке в сайдбаре", () => {
    const container = createTestContainer();
    const ctrl = createSettingsModalController(container);
    ctrl.open("locale");

    for (const invalid of ["invalid", "", "APPEARANCE", null, undefined, 123]) {
      assert.equal(ctrl.setTab(/** @type {any} */ (invalid)), false);
    }

    const localeTab = container.querySelector('[data-settings-tab="locale"]');
    assert.equal(localeTab?.getAttribute("aria-selected"), "true");
    assert.equal(
      container.querySelector('[role="tabpanel"]')?.getAttribute("data-settings-pane"),
      "locale",
    );

    const keybindsTab = container.querySelector('[data-settings-tab="keybinds"]');
    assert.ok(keybindsTab);
    keybindsTab.click();
    assert.equal(keybindsTab.getAttribute("aria-selected"), "true");
    assert.equal(localeTab?.getAttribute("aria-selected"), "false");
    assert.equal(
      container.querySelector('[role="tabpanel"]')?.getAttribute("data-settings-pane"),
      "keybinds",
    );
  });
});

describe("createSettingsModalController: updateSetting(key, value)", () => {
  it("обновляет внутреннее состояние по всем ключам настроек и вызывает onUpdateConfig({ ...state }) с копией", () => {
    const container = createTestContainer();
    /** @type {Array<Record<string, any>>} */
    const updates = [];

    const ctrl = createSettingsModalController(
      container,
      {
        theme: "dark",
        fontScale: 100,
        messageGroupSpacing: 16,
        inputMode: "VOICE_ACTIVITY",
        vadThresholdDb: -60,
        locale: "ru",
        desktopNotifications: true,
      },
      {
        onUpdateConfig(nextState) {
          updates.push(nextState);
        },
      },
    );

    ctrl.open("appearance");

    assert.equal(ctrl.updateSetting("theme", "onyx"), true);
    assert.equal(ctrl.updateSetting("fontScale", 125), true);
    assert.equal(ctrl.updateSetting("messageGroupSpacing", 24), true);
    assert.equal(ctrl.updateSetting("inputMode", "PUSH_TO_TALK"), true);
    assert.equal(ctrl.updateSetting("vadThresholdDb", -45), true);
    assert.equal(ctrl.updateSetting("locale", "en-US"), true);
    assert.equal(ctrl.updateSetting("desktopNotifications", false), true);

    assert.equal(updates.length, 7);
    assert.deepEqual(updates[6], {
      theme: "onyx",
      fontScale: 125,
      messageGroupSpacing: 24,
      inputMode: "PUSH_TO_TALK",
      vadThresholdDb: -45,
      locale: "en-US",
      desktopNotifications: false,
    });

    // Мутация аргумента колбэка не должна портить внутренний стейт контроллера
    updates[6].theme = "corrupted";
    assert.equal(ctrl.getState().theme, "onyx");

    // Пустой/невалидный ключ игнорируется и не вызывает onUpdateConfig
    assert.equal(ctrl.updateSetting("", "value"), false);
    assert.equal(ctrl.updateSetting(/** @type {any} */ (null), "value"), false);
    assert.equal(updates.length, 7);
  });

  it("синхронизирует DOM-контролы вкладок с updateSetting", () => {
    const container = createTestContainer();
    /** @type {Array<Record<string, any>>} */
    const updates = [];
    const ctrl = createSettingsModalController(
      container,
      { theme: "dark", desktopNotifications: true },
      { onUpdateConfig: (cfg) => updates.push(cfg) },
    );

    ctrl.open("appearance");
    const themeControl = container.querySelector('[data-setting-key="theme"]');
    assert.ok(themeControl);
    assert.equal(themeControl.value, "dark");

    themeControl.value = "light";
    themeControl.dispatchEvent("change");
    assert.equal(updates.length, 1);
    assert.equal(updates[0]?.theme, "light");

    ctrl.setTab("notifications");
    const notifControl = container.querySelector('[data-setting-key="desktopNotifications"]');
    assert.ok(notifControl);
    assert.equal(notifControl.getAttribute("aria-checked"), "true");

    notifControl.click();
    assert.equal(updates.length, 2);
    assert.equal(updates[1]?.desktopNotifications, false);
    assert.equal(notifControl.getAttribute("aria-checked"), "false");
  });
});

describe("createSettingsModalController: confirmLogout() и close()", () => {
  it("confirmLogout() открывает диалог подтверждения 'Вы уверены, что хотите выйти?' и вызывает onLogout() только после подтверждения", () => {
    const container = createTestContainer();
    let logoutCalls = 0;

    const ctrl = createSettingsModalController(
      container,
      {},
      {
        onLogout() {
          logoutCalls += 1;
        },
      },
    );

    ctrl.open();

    // Открытие через метод confirmLogout()
    const promptEl = ctrl.confirmLogout();
    assert.ok(promptEl);
    assert.equal(promptEl.getAttribute("data-logout-confirm"), "true");
    assert.ok(promptEl.textContent.includes("Вы уверены, что хотите выйти?"));
    assert.equal(logoutCalls, 0, "до подтверждения onLogout не должен вызываться");

    // Отмена закрывает промпт без вызова onLogout
    const cancelBtn = promptEl.querySelector("[data-logout-confirm-cancel]");
    assert.ok(cancelBtn);
    cancelBtn.click();
    assert.equal(container.querySelector('[data-logout-confirm="true"]'), null);
    assert.equal(logoutCalls, 0);

    // Открытие через кнопку 'Выйти из аккаунта' в сайдбаре и подтверждение
    const sidebarLogoutBtn = container.querySelector("[data-settings-logout]");
    assert.ok(sidebarLogoutBtn);
    sidebarLogoutBtn.click();

    const promptAgain = container.querySelector('[data-logout-confirm="true"]');
    assert.ok(promptAgain);
    assert.ok(promptAgain.textContent.includes("Вы уверены, что хотите выйти?"));

    const confirmBtn = promptAgain.querySelector("[data-logout-confirm-accept]");
    assert.ok(confirmBtn);
    confirmBtn.click();

    assert.equal(logoutCalls, 1);
    assert.equal(container.querySelector('[data-logout-confirm="true"]'), null);
  });

  it("close() и кнопка 'Закрыть (ESC)' удаляют модальное окно и вызывают onClose()", () => {
    const container = createTestContainer();
    let closeCalls = 0;

    const ctrl = createSettingsModalController(
      container,
      {},
      {
        onClose() {
          closeCalls += 1;
        },
      },
    );

    ctrl.open();
    assert.ok(container.querySelector('[data-settings-open="true"]'));

    const closeBtn = container.querySelector("[data-settings-close]");
    assert.ok(closeBtn);
    closeBtn.click();

    assert.equal(container.querySelector('[data-settings-open="true"]'), null);
    assert.equal(closeCalls, 1);

    // Повторный вызов close() на уже закрытом окне безопасен и не дублирует onClose
    assert.equal(ctrl.close(), false);
    assert.equal(closeCalls, 1);

    // Открываем снова и закрываем через метод close()
    ctrl.open("keybinds");
    assert.ok(container.querySelector('[data-settings-open="true"]'));
    assert.equal(ctrl.close(), true);
    assert.equal(container.querySelector('[data-settings-open="true"]'), null);
    assert.equal(closeCalls, 2);
  });

  it("нажатие Escape на диалоге закрывает модальное окно, а невалидный containerEl бросает TypeError", () => {
    assert.throws(() => createSettingsModalController(null), TypeError);

    const container = createTestContainer();
    let closed = 0;
    const ctrl = createSettingsModalController(
      container,
      {},
      { onClose: () => (closed += 1) },
    );

    const dialog = ctrl.open();
    dialog.dispatchEvent("keydown", { key: "Escape" });
    assert.equal(container.querySelector('[data-settings-open="true"]'), null);
    assert.equal(closed, 1);
  });
});
