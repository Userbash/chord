// @ts-check
/**
 * T-224 (FE-P2-23): контракт парсера горячих клавиш и русской подписи для ARIA.
 *
 * Проверяет: замороженный набор DEFAULT_SHORTCUTS, нормализацию события
 * клавиатуры в стабильную строку-комбо, сравнение с комбо, отказ на пустых
 * и модификаторных клавишах, а также формат русской подписи.
 */
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import {
  DEFAULT_SHORTCUTS,
  formatShortcutAriaLabelRu,
  handleKeybind,
  initKeybinds,
  matchesShortcut,
  normalizeKeyCombo,
} from "../../rust/gui-spike/ui/features/keybinds.js";

/** @param {Partial<{ key: string, ctrlKey: boolean, metaKey: boolean, shiftKey: boolean, altKey: boolean }>} [init] */
function makeEvent(init = {}) {
  return {
    key: "k",
    ctrlKey: false,
    metaKey: false,
    shiftKey: false,
    altKey: false,
    ...init,
  };
}

describe("DEFAULT_SHORTCUTS", () => {
  it("содержит точный набор комбинаций", () => {
    assert.deepEqual(DEFAULT_SHORTCUTS, {
      QUICK_SWITCHER: "Ctrl+K",
      SEARCH: "Ctrl+F",
      TOGGLE_MUTE: "Ctrl+Shift+M",
      TOGGLE_DEAFEN: "Ctrl+Shift+D",
      MARK_SERVER_READ: "Shift+Escape",
      CLOSE_MODAL: "Escape",
    });
  });

  it("заморожен", () => {
    assert.equal(Object.isFrozen(DEFAULT_SHORTCUTS), true);
    assert.throws(() => {
      // @ts-expect-error проверка заморозки в рантайме
      DEFAULT_SHORTCUTS.SEARCH = "Ctrl+G";
    }, TypeError);
  });
});

describe("normalizeKeyCombo", () => {
  it("собирает Ctrl + буква и поднимает регистр", () => {
    assert.equal(normalizeKeyCombo(makeEvent({ key: "k", ctrlKey: true })), "Ctrl+K");
  });

  it("трактует metaKey как Ctrl", () => {
    assert.equal(normalizeKeyCombo(makeEvent({ key: "f", metaKey: true })), "Ctrl+F");
  });

  it("не дублирует Ctrl при ctrlKey и metaKey", () => {
    assert.equal(
      normalizeKeyCombo(makeEvent({ key: "m", ctrlKey: true, metaKey: true, shiftKey: true })),
      "Ctrl+Shift+M",
    );
  });

  it("порядок модификаторов: Ctrl, Alt, Shift", () => {
    assert.equal(
      normalizeKeyCombo(makeEvent({ key: "d", ctrlKey: true, shiftKey: true, altKey: true })),
      "Ctrl+Alt+Shift+D",
    );
  });

  it("отдаёт одиночную клавишу без модификаторов", () => {
    assert.equal(normalizeKeyCombo(makeEvent({ key: "Escape" })), "Escape");
  });

  it("кодирует пробел как Space", () => {
    assert.equal(normalizeKeyCombo(makeEvent({ key: " " })), "Space");
    assert.equal(normalizeKeyCombo(makeEvent({ key: " ", shiftKey: true })), "Shift+Space");
  });

  it("возвращает null на пустой клавише и на голом модификаторе", () => {
    assert.equal(normalizeKeyCombo(makeEvent({ key: "" })), null);
    assert.equal(normalizeKeyCombo(makeEvent({ key: "Control", ctrlKey: true })), null);
    assert.equal(normalizeKeyCombo(makeEvent({ key: "Shift", shiftKey: true })), null);
    assert.equal(normalizeKeyCombo(makeEvent({ key: "Alt", altKey: true })), null);
    assert.equal(normalizeKeyCombo(makeEvent({ key: "Meta", metaKey: true })), null);
  });
});

describe("matchesShortcut", () => {
  it("совпадает с дефолтным комбо", () => {
    assert.equal(matchesShortcut(makeEvent({ key: "k", ctrlKey: true }), DEFAULT_SHORTCUTS.QUICK_SWITCHER), true);
  });

  it("не совпадает при другом наборе модификаторов", () => {
    assert.equal(matchesShortcut(makeEvent({ key: "m", ctrlKey: true }), DEFAULT_SHORTCUTS.TOGGLE_MUTE), false);
  });

  it("не совпадает, когда нормализация дала null", () => {
    assert.equal(matchesShortcut(makeEvent({ key: "Shift", shiftKey: true }), "Shift"), false);
  });
});

describe("formatShortcutAriaLabelRu", () => {
  it("добавляет русский префикс к комбо", () => {
    assert.equal(formatShortcutAriaLabelRu("Ctrl+K"), "Горячая клавиша: Ctrl+K");
  });

  it("подставляет «не назначена» на пустом значении", () => {
    assert.equal(formatShortcutAriaLabelRu(""), "Горячая клавиша: не назначена");
    assert.equal(formatShortcutAriaLabelRu(/** @type {any} */ (null)), "Горячая клавиша: не назначена");
  });
});

describe("handleKeybind (TAURI-BOOT-01)", () => {
  it("обрабатывает Ctrl+K и вызывает открытие quick-switcher", () => {
    let qsCalled = false;
    const app = {
      openQuickSwitcher() {
        qsCalled = true;
      },
    };
    assert.equal(handleKeybind("Ctrl+K", app), true);
    assert.equal(qsCalled, true);

    // Поддержка KeyEventLike
    let qsLayerOpened = false;
    const appBridge = {
      shellBridge: {
        /** @param {string} layerId */
        openLayer(layerId) {
          if (layerId === "quick-switcher") qsLayerOpened = true;
        },
      },
    };
    assert.equal(handleKeybind(makeEvent({ key: "k", ctrlKey: true }), appBridge), true);
    assert.equal(qsLayerOpened, true);
  });

  it("обрабатывает Ctrl+F и вызывает открытие search-panel", () => {
    let searchCalled = false;
    const app = {
      openSearch() {
        searchCalled = true;
      },
    };
    assert.equal(handleKeybind("Ctrl+F", app), true);
    assert.equal(searchCalled, true);

    let searchLayerOpened = false;
    const appBridge = {
      shellBridge: {
        /** @param {string} layerId */
        openLayer(layerId) {
          if (layerId === "search-panel") searchLayerOpened = true;
        },
      },
    };
    assert.equal(handleKeybind(makeEvent({ key: "f", ctrlKey: true }), appBridge), true);
    assert.equal(searchLayerOpened, true);
  });

  it("обрабатывает Escape и закрывает верхний модальный слой", () => {
    let closeCalled = false;
    const app = {
      closeTopModal() {
        closeCalled = true;
      },
    };
    assert.equal(handleKeybind("Escape", app), true);
    assert.equal(closeCalled, true);

    let bridgeCloseCalled = false;
    const appBridge = {
      shellBridge: {
        closeTopLayer() {
          bridgeCloseCalled = true;
        },
      },
    };
    assert.equal(handleKeybind(makeEvent({ key: "Escape" }), appBridge), true);
    assert.equal(bridgeCloseCalled, true);
  });

  it("обрабатывает кастомные горячие клавиши, зарегистрированные в app", () => {
    let customExecuted = false;
    const customKeybinds = new Map();
    customKeybinds.set("Ctrl+P", (/** @type {any} */ _appContext, /** @type {string} */ combo) => {
      assert.equal(combo, "Ctrl+P");
      customExecuted = true;
    });

    const app = { customKeybinds };
    assert.equal(handleKeybind("Ctrl+P", app), true);
    assert.equal(customExecuted, true);

    // Кастомный обработчик через объект
    let slashExecuted = false;
    const appObj = {
      handlers: {
        "Ctrl+/": () => {
          slashExecuted = true;
        },
      },
    };
    assert.equal(handleKeybind("Ctrl+/", appObj), true);
    assert.equal(slashExecuted, true);
  });

  it("возвращает false для неизвестных или пустых комбинаций", () => {
    const app = {};
    assert.equal(handleKeybind("Alt+X", app), false);
    assert.equal(handleKeybind("", app), false);
    assert.equal(handleKeybind(null, app), false);
  });
});

describe("initKeybinds (TAURI-BOOT-01)", () => {
  function makeMockTarget() {
    /** @type {Record<string, ((event: any) => void)[]>} */
    const listeners = {};
    return {
      listeners,
      /** @param {string} type @param {(event: any) => void} fn */
      addEventListener(type, fn) {
        listeners[type] = listeners[type] || [];
        listeners[type].push(fn);
      },
      /** @param {string} type @param {(event: any) => void} fn */
      removeEventListener(type, fn) {
        if (!listeners[type]) return;
        listeners[type] = listeners[type].filter((cb) => cb !== fn);
      },
      /** @param {string} type @param {any} event */
      dispatch(type, event) {
        for (const fn of listeners[type] || []) {
          fn(event);
        }
      },
    };
  }

  it("инициализирует keydown-слушатель и вызывает preventDefault на Ctrl+K, Ctrl+F, Escape", () => {
    const target = makeMockTarget();
    let qsCalled = false;
    let searchCalled = false;
    let closeCalled = false;

    const app = {
      openQuickSwitcher: () => { qsCalled = true; },
      openSearch: () => { searchCalled = true; },
      closeTopModal: () => { closeCalled = true; },
    };

    const controller = initKeybinds(app, { target });
    assert.equal(target.listeners.keydown?.length, 1);

    // Ctrl+K
    let preventedK = false;
    target.dispatch("keydown", {
      key: "k",
      ctrlKey: true,
      preventDefault: () => { preventedK = true; },
      stopPropagation: () => {},
    });
    assert.equal(qsCalled, true);
    assert.equal(preventedK, true);

    // Ctrl+F
    let preventedF = false;
    target.dispatch("keydown", {
      key: "f",
      ctrlKey: true,
      preventDefault: () => { preventedF = true; },
      stopPropagation: () => {},
    });
    assert.equal(searchCalled, true);
    assert.equal(preventedF, true);

    // Escape
    let preventedEsc = false;
    target.dispatch("keydown", {
      key: "Escape",
      preventDefault: () => { preventedEsc = true; },
      stopPropagation: () => {},
    });
    assert.equal(closeCalled, true);
    assert.equal(preventedEsc, true);

    controller.destroy();
    assert.equal(target.listeners.keydown?.length, 0);
  });

  it("поддерживает динамическую регистрацию и удаление кастомных шорткатов через controller.register / unregister", () => {
    const target = makeMockTarget();
    const app = {};
    const controller = initKeybinds(app, { target });

    let customInvoked = false;
    controller.register("Ctrl+Shift+U", () => {
      customInvoked = true;
    });

    // Вызов через событие keydown
    target.dispatch("keydown", {
      key: "u",
      ctrlKey: true,
      shiftKey: true,
      preventDefault: () => {},
      stopPropagation: () => {},
    });
    assert.equal(customInvoked, true);

    // Удаление шортката
    assert.equal(controller.unregister("Ctrl+Shift+U"), true);
    customInvoked = false;
    target.dispatch("keydown", {
      key: "u",
      ctrlKey: true,
      shiftKey: true,
      preventDefault: () => {},
      stopPropagation: () => {},
    });
    assert.equal(customInvoked, false);

    controller.destroy();
  });
});
