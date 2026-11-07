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
