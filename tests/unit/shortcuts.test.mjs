// @ts-check
/**
 * Unit-тесты модуля `rust/gui-spike/ui/features/shortcuts.js` (task-183, FE-P2-05).
 *
 * Контракт (оракулы):
 * - `waves/RW-40/PM/BACKLOG-RE-FRONTEND.md` §3, строка FE-P2-05: 8 шорткатов
 *   (`Ctrl+K`, `Ctrl+Shift+M/D`, `Ctrl+/`) и модалка «Горячие клавиши»;
 * - `re-deep/frontend-d03.md` §5 (Modal: role=dialog, aria-modal, возврат фокуса);
 * - `re-deep/decoded*` — сочетания клавиш клиента Discord (справка «Горячие клавиши»).
 *
 * Тесты проверяют поведение, а не реализацию: таблица сочетаний, приоритет
 * Escape над обработчиками, guard на поля ввода и жизненный цикл модалки.
 */
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { ShimDocument } from "../support/gui-dom-harness.mjs";
import {
  DISCORD_SHORTCUTS,
  matchShortcut,
  createShortcutsController,
} from "../../rust/gui-spike/ui/features/shortcuts.js";

/** @typedef {import("../support/gui-dom-harness.mjs").ShimElement} ShimElement */
/** @typedef {import("../support/gui-dom-harness.mjs").ShimEvent} ShimEvent */

/**
 * Событие клавиатуры с наблюдаемым `preventDefault()`.
 * @param {Partial<ShimEvent>} [init]
 * @returns {ShimEvent}
 */
function keyEvent(init = {}) {
  /** @type {ShimEvent} */
  const event = {
    type: "keydown",
    key: "",
    ctrlKey: false,
    shiftKey: false,
    altKey: false,
    metaKey: false,
    target: null,
    currentTarget: null,
    prevented: false,
    preventDefault() {
      event.prevented = true;
    },
    stopPropagation() {},
    stopImmediatePropagation() {},
    ...init,
  };
  return event;
}

/**
 * Контроллер поверх чистого ShimDocument; `body` — корень прослушивания.
 * @param {Record<string, (event: ShimEvent) => void>} [handlers]
 */
function createEnv(handlers = {}) {
  const doc = new ShimDocument();
  const controller = createShortcutsController(doc, handlers);
  /** @param {Partial<ShimEvent>} [init] */
  const press = (init = {}) => doc.body.dispatchEvent("keydown", init);
  return { doc, controller, press };
}

/**
 * Данные, которые обязана содержать таблица шорткатов (сверка с эталоном Discord).
 * @type {Array<{ id: string, combo: string, key: string, ctrlKey: boolean, shiftKey: boolean, altKey: boolean, labelRu: string }>}
 */
const EXPECTED = [
  {
    id: "quick_switcher",
    combo: "Ctrl+K",
    key: "k",
    ctrlKey: true,
    shiftKey: false,
    altKey: false,
    labelRu: "Быстрый переход",
  },
  {
    id: "toggle_mute",
    combo: "Ctrl+Shift+M",
    key: "m",
    ctrlKey: true,
    shiftKey: true,
    altKey: false,
    labelRu: "Вкл./выкл. микрофон",
  },
  {
    id: "toggle_deafen",
    combo: "Ctrl+Shift+D",
    key: "d",
    ctrlKey: true,
    shiftKey: true,
    altKey: false,
    labelRu: "Вкл./выкл. звук",
  },
  {
    id: "shortcuts_help",
    combo: "Ctrl+/",
    key: "/",
    ctrlKey: true,
    shiftKey: false,
    altKey: false,
    labelRu: "Горячие клавиши",
  },
  {
    id: "search_channel",
    combo: "Ctrl+F",
    key: "f",
    ctrlKey: true,
    shiftKey: false,
    altKey: false,
    labelRu: "Поиск в канале",
  },
  {
    id: "pins_popout",
    combo: "Ctrl+P",
    key: "p",
    ctrlKey: true,
    shiftKey: false,
    altKey: false,
    labelRu: "Закреплённые сообщения",
  },
  {
    id: "mark_channel_read",
    combo: "Escape",
    key: "Escape",
    ctrlKey: false,
    shiftKey: false,
    altKey: false,
    labelRu: "Отметить канал прочитанным / Закрыть модалку",
  },
  {
    id: "navigate_unread_prev",
    combo: "Alt+Shift+ArrowUp",
    key: "ArrowUp",
    ctrlKey: false,
    shiftKey: true,
    altKey: true,
    labelRu: "Предыдущий непрочитанный канал",
  },
];

describe("DISCORD_SHORTCUTS: каноническая таблица из 8 сочетаний", () => {
  it("содержит ровно 8 шорткатов в порядке Discord", () => {
    assert.equal(DISCORD_SHORTCUTS.length, 8);
    assert.deepEqual(
      DISCORD_SHORTCUTS.map((shortcut) => shortcut.id),
      EXPECTED.map((shortcut) => shortcut.id),
    );
  });

  it("фиксирует combo/key/модификаторы/русскую подпись каждого шортката", () => {
    for (const [index, expected] of EXPECTED.entries()) {
      const shortcut = DISCORD_SHORTCUTS[index];
      assert.ok(shortcut, `шорткат #${index} отсутствует`);
      assert.equal(shortcut.id, expected.id);
      assert.equal(shortcut.combo, expected.combo);
      assert.equal(shortcut.key, expected.key);
      assert.equal(shortcut.ctrlKey, expected.ctrlKey);
      assert.equal(shortcut.shiftKey, expected.shiftKey);
      assert.equal(shortcut.altKey, expected.altKey);
      assert.equal(shortcut.labelRu, expected.labelRu);
    }
  });

  it("замораживает массив и записи, у каждой есть непустая категория", () => {
    assert.ok(Object.isFrozen(DISCORD_SHORTCUTS));
    for (const shortcut of DISCORD_SHORTCUTS) {
      assert.ok(Object.isFrozen(shortcut), `${shortcut.id} не заморожен`);
      assert.equal(typeof shortcut.category, "string");
      assert.ok(shortcut.category.length > 0, `${shortcut.id}: пустая категория`);
    }
  });
});

describe("matchShortcut: сопоставление события с таблицей", () => {
  it("находит каждый шорткат по его сочетанию", () => {
    for (const expected of EXPECTED) {
      const event = keyEvent({
        key: expected.key,
        ctrlKey: expected.ctrlKey,
        shiftKey: expected.shiftKey,
        altKey: expected.altKey,
      });
      const matched = matchShortcut(event);
      assert.ok(matched, `${expected.combo} не сопоставлен`);
      assert.equal(matched.id, expected.id);
    }
  });

  it("сравнивает буквы без учёта регистра (K и k)", () => {
    assert.equal(matchShortcut(keyEvent({ key: "K", ctrlKey: true }))?.id, "quick_switcher");
    assert.equal(matchShortcut(keyEvent({ key: "k", ctrlKey: true }))?.id, "quick_switcher");
  });

  it("требует точного совпадения модификаторов", () => {
    assert.equal(matchShortcut(keyEvent({ key: "k" })), null);
    assert.equal(matchShortcut(keyEvent({ key: "k", ctrlKey: true, shiftKey: true })), null);
    assert.equal(matchShortcut(keyEvent({ key: "k", ctrlKey: true, altKey: true })), null);
    assert.equal(matchShortcut(keyEvent({ key: "m", ctrlKey: true })), null);
    assert.equal(matchShortcut(keyEvent({ key: "Escape", ctrlKey: true })), null);
    assert.equal(matchShortcut(keyEvent({ key: "ArrowUp", altKey: true })), null);
  });

  it("не реагирует на незарегистрированные клавиши", () => {
    assert.equal(matchShortcut(keyEvent({ key: "q" })), null);
    assert.equal(matchShortcut(keyEvent({ key: "q", ctrlKey: true })), null);
    assert.equal(matchShortcut(keyEvent({ key: "Enter" })), null);
  });

  it("в полях ввода пропускает только Ctrl-шорткаты из списка и Escape", () => {
    const doc = new ShimDocument();
    const textarea = doc.createElement("textarea");
    const input = doc.createElement("input");
    const editable = doc.createElement("div");
    editable.setAttribute("contenteditable", "true");

    for (const target of [textarea, input, editable]) {
      /** @param {Partial<ShimEvent>} init */
      const press = (init) => matchShortcut(keyEvent({ target, ...init }));

      assert.equal(press({ key: "k", ctrlKey: true })?.id, "quick_switcher");
      assert.equal(press({ key: "m", ctrlKey: true, shiftKey: true })?.id, "toggle_mute");
      assert.equal(press({ key: "d", ctrlKey: true, shiftKey: true })?.id, "toggle_deafen");
      assert.equal(press({ key: "/", ctrlKey: true })?.id, "shortcuts_help");
      assert.equal(press({ key: "Escape" })?.id, "mark_channel_read");

      assert.equal(press({ key: "f", ctrlKey: true }), null, "Ctrl+F не должен перехватываться в поле ввода");
      assert.equal(press({ key: "p", ctrlKey: true }), null, "Ctrl+P не должен перехватываться в поле ввода");
      assert.equal(
        press({ key: "ArrowUp", altKey: true, shiftKey: true }),
        null,
        "Alt+Shift+ArrowUp не должен перехватываться в поле ввода",
      );
    }
  });

  it("вне полей ввода Ctrl+F и Ctrl+P доступны", () => {
    const doc = new ShimDocument();
    const button = doc.createElement("button");
    assert.equal(matchShortcut(keyEvent({ key: "f", ctrlKey: true, target: button }))?.id, "search_channel");
    assert.equal(matchShortcut(keyEvent({ key: "p", ctrlKey: true, target: button }))?.id, "pins_popout");
  });

  it("распознаёт contentEditable по свойству isContentEditable", () => {
    const doc = new ShimDocument();
    const editable = doc.createElement("div");
    /** @type {any} */ (editable).isContentEditable = true;
    assert.equal(matchShortcut(keyEvent({ key: "f", ctrlKey: true, target: editable })), null);
  });
});

describe("createShortcutsController: доступная модалка и глобальные клавиши", () => {
  it("рендерит #shortcuts-help-modal в doc.body с ARIA-контрактом", () => {
    const { doc } = createEnv();
    const modal = doc.getElementById("shortcuts-help-modal");
    assert.ok(modal, "модалка не создана");
    assert.equal(modal.getAttribute("role"), "dialog");
    assert.equal(modal.getAttribute("aria-modal"), "true");
    assert.equal(modal.getAttribute("aria-label"), "Горячие клавиши");
    assert.ok(modal.hasAttribute("hidden"), "модалка должна стартовать скрытой");
    assert.equal(modal.getAttribute("aria-hidden"), "true");
  });

  it("перечисляет все 8 шорткатов с kbd-бейджами и подписями", () => {
    const { doc } = createEnv();
    const modal = doc.getElementById("shortcuts-help-modal");
    assert.ok(modal);
    const keys = modal.querySelectorAll("kbd");
    assert.equal(keys.length, DISCORD_SHORTCUTS.length);
    assert.deepEqual(
      keys.map((node) => node.textContent),
      EXPECTED.map((shortcut) => shortcut.combo),
    );
    const text = modal.textContent ?? "";
    for (const shortcut of DISCORD_SHORTCUTS) {
      assert.ok(text.includes(shortcut.labelRu), `нет подписи «${shortcut.labelRu}»`);
      assert.ok(text.includes(shortcut.combo), `нет combo «${shortcut.combo}»`);
    }
  });

  it("содержит кнопку закрытия [data-action=close-shortcuts-help]", () => {
    const { doc } = createEnv();
    const modal = doc.getElementById("shortcuts-help-modal");
    assert.ok(modal);
    const close = modal.querySelector('[data-action="close-shortcuts-help"]');
    assert.ok(close, "нет кнопки закрытия");
    assert.equal(close.tagName, "BUTTON");
    assert.ok((close.getAttribute("aria-label") ?? "").length > 0);
  });

  it("Ctrl+/ открывает и повторным нажатием закрывает справку", () => {
    const { doc, controller, press } = createEnv();
    const modal = /** @type {ShimElement} */ (doc.getElementById("shortcuts-help-modal"));
    assert.equal(controller.isHelpModalOpen(), false);

    const opened = press({ key: "/", ctrlKey: true });
    assert.equal(opened.prevented, true, "Ctrl+/ должен отменять дефолт браузера");
    assert.equal(controller.isHelpModalOpen(), true);
    assert.equal(modal.hasAttribute("hidden"), false);
    assert.equal(modal.getAttribute("aria-hidden"), "false");

    const closed = press({ key: "/", ctrlKey: true });
    assert.equal(closed.prevented, true);
    assert.equal(controller.isHelpModalOpen(), false);
    assert.equal(modal.hasAttribute("hidden"), true);
    assert.equal(modal.getAttribute("aria-hidden"), "true");
  });

  it("Escape закрывает открытую справку и возвращает фокус прежнему элементу", () => {
    const { doc, controller, press } = createEnv();
    const composer = doc.createElement("textarea");
    composer.id = "composer-input";
    doc.body.append(composer);
    composer.focus();
    assert.equal(doc.activeElement, composer);

    controller.openHelpModal();
    assert.equal(controller.isHelpModalOpen(), true);
    assert.notEqual(doc.activeElement, composer, "фокус должен уйти в модалку");

    const escape = press({ key: "Escape" });
    assert.equal(escape.prevented, true);
    assert.equal(controller.isHelpModalOpen(), false);
    assert.equal(doc.activeElement, composer, "фокус не восстановлен");
  });

  it("кнопка закрытия закрывает модалку и возвращает фокус", () => {
    const { doc, controller } = createEnv();
    const trigger = doc.createElement("button");
    doc.body.append(trigger);
    trigger.focus();

    controller.openHelpModal();
    const close = /** @type {ShimElement} */ (
      doc.getElementById("shortcuts-help-modal")?.querySelector('[data-action="close-shortcuts-help"]')
    );
    close.click();

    assert.equal(controller.isHelpModalOpen(), false);
    assert.equal(doc.activeElement, trigger);
  });

  it("вызывает handlers[id] и отменяет дефолт для зарегистрированного шортката", () => {
    /** @type {ShimEvent[]} */
    const calls = [];
    const { controller, press } = createEnv({
      quick_switcher: (event) => calls.push(event),
    });

    const event = press({ key: "k", ctrlKey: true });
    assert.equal(calls.length, 1);
    assert.equal(calls[0], event);
    assert.equal(event.prevented, true);
    assert.equal(controller.isHelpModalOpen(), false);
  });

  it("Escape при закрытой справке уходит в handlers.mark_channel_read", () => {
    let called = 0;
    const { press } = createEnv({ mark_channel_read: () => { called += 1; } });
    const event = press({ key: "Escape" });
    assert.equal(called, 1);
    assert.equal(event.prevented, true);
  });

  it("Escape при открытой справке закрывает её и не дергает обработчик", () => {
    let called = 0;
    const { controller, press } = createEnv({ mark_channel_read: () => { called += 1; } });
    controller.openHelpModal();
    press({ key: "Escape" });
    assert.equal(called, 0, "Escape должен закрывать модалку, а не помечать канал прочитанным");
    assert.equal(controller.isHelpModalOpen(), false);
  });

  it("без обработчика шорткат всё равно отменяет дефолт браузера", () => {
    const { press } = createEnv();
    const event = press({ key: "f", ctrlKey: true });
    assert.equal(event.prevented, true);
  });

  it("игнорирует Ctrl+F в поле ввода, но Ctrl+K обрабатывает", () => {
    const { doc, press } = createEnv();
    const textarea = doc.createElement("textarea");
    doc.body.append(textarea);

    const search = press({ key: "f", ctrlKey: true, target: textarea });
    assert.equal(search.prevented, false);

    const switcher = press({ key: "k", ctrlKey: true, target: textarea });
    assert.equal(switcher.prevented, true);
  });

  it("openHelpModal/closeHelpModal идемпотентны", () => {
    const { controller } = createEnv();
    controller.closeHelpModal();
    assert.equal(controller.isHelpModalOpen(), false);
    controller.openHelpModal();
    controller.openHelpModal();
    assert.equal(controller.isHelpModalOpen(), true);
    controller.closeHelpModal();
    controller.closeHelpModal();
    assert.equal(controller.isHelpModalOpen(), false);
  });

  it("destroy снимает слушатель и удаляет модалку из документа", () => {
    const { doc, controller, press } = createEnv();
    controller.openHelpModal();
    controller.destroy();

    assert.equal(doc.getElementById("shortcuts-help-modal"), null);
    assert.equal(controller.isHelpModalOpen(), false);

    const event = press({ key: "/", ctrlKey: true });
    assert.equal(event.prevented, false, "после destroy хоткей не должен перехватываться");
  });

  it("глобальный слушатель вешается на body, а не на документ", () => {
    const { doc } = createEnv();
    const bucket = doc.body.listeners.get("keydown") ?? [];
    assert.equal(bucket.length, 1);
  });
});
