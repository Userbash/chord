// @ts-check
// RW-40 / Q-G4-REG: регрессии mutation-дыр G2-03 и G2-11 (TEST-AUDIT QA4-01/QA4-02, M1/M6).
//
// Оракул — состояние DOM-шима, а не текст реализации:
//   G2-03: пустое состояние QuickSwitcher скрыто, пока есть опции, и видно только
//          при нулевой выдаче (`hidden` — как это видит браузер/AT).
//   G2-11: подтверждение удаления — состояние текущей строки: уход на другую строку
//          и завершение IPC (успех/ошибка) снимают его, один клик не удаляет.
//
// Файл отдельный от `gui-spike-features.test.mjs`, чтобы правки не пересекались.
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { loadApp } from "../support/gui-dom-harness.mjs";

/** @typedef {import("../support/gui-dom-harness.mjs").GuiDom} GuiDom */
/** @typedef {import("../support/gui-dom-harness.mjs").ShimElement} ShimElement */

/**
 * @param {(app: GuiDom) => void | Promise<void>} body
 * @returns {Promise<void>}
 */
async function withApp(body) {
  const app = await loadApp();
  try {
    await body(app);
  } finally {
    app.dispose();
  }
}

/**
 * @param {GuiDom} app
 * @param {string} selector
 * @returns {ShimElement[]}
 */
function all(app, selector) {
  return app.document.querySelectorAll(selector);
}

/**
 * @param {GuiDom} app
 * @param {string} selector
 * @returns {ShimElement}
 */
function firstNode(app, selector) {
  const node = app.document.querySelector(selector);
  assert.ok(node, `нет ${selector}`);
  return node;
}

/**
 * @param {GuiDom} app
 * @param {string} value
 * @param {Record<string, unknown>} [init]
 */
function key(app, value, init = {}) {
  return app.document.body.dispatchEvent("keydown", {
    key: value,
    altKey: false,
    ctrlKey: false,
    metaKey: false,
    shiftKey: false,
    ...init,
  });
}

/** @param {GuiDom} app */
function api(app) {
  const instance = app.appModule?.currentApp?.();
  assert.ok(instance, "app.js не экспортирует currentApp()");
  return instance;
}

/**
 * @param {GuiDom} app
 * @param {number} index
 * @returns {Promise<void>}
 */
async function scrollChat(app, index) {
  api(app).chat.list.scrollToIndex(index);
  await app.settle(6);
}

/**
 * @param {GuiDom} app
 * @param {number} index
 * @returns {ShimElement | null}
 */
function chatRow(app, index) {
  return all(app, "#chat-list .row").find(
    (row) => Number(row.getAttribute("data-index")) === index,
  ) ?? null;
}

/** Видимость по правилам DOM: атрибут `hidden` скрывает узел. @param {ShimElement} node */
function isHidden(node) {
  return node.hasAttribute("hidden");
}

describe("G2-03: пустое состояние QuickSwitcher соответствует выдаче", () => {
  it("empty скрыт при опциях и виден только при нуле совпадений", async () => {
    await withApp((app) => {
      key(app, "k", { ctrlKey: true });
      const list = firstNode(app, "#quick-switcher-results");
      const empty = firstNode(app, ".quick-switcher-empty");
      const hint = firstNode(app, ".quick-switcher-hint");
      const input = firstNode(app, "#quick-switcher-input");

      const options = list.querySelectorAll('[role="option"]');
      assert.ok(options.length > 0, "свежий switcher без опций — нечего проверять");
      assert.equal(options.length, 8, "выдача должна быть ограничена восемью строками");
      assert.equal(isHidden(empty), true, "«Ничего не найдено» видно при 8 опциях");
      assert.equal(isHidden(hint), false, "подсказка навигации спрятана вместе с empty");

      input.value = "никакого совпадения 8472";
      input.emit("input");
      assert.equal(list.querySelectorAll('[role="option"]').length, 0);
      assert.equal(isHidden(empty), false, "empty не показан при нулевой выдаче");
      assert.match(empty.textContent, /не найдено/i);
      assert.equal(isHidden(hint), false, "подсказка навигации не должна скрываться");

      input.value = "#объяв";
      input.emit("input");
      assert.ok(list.querySelectorAll('[role="option"]').length > 0);
      assert.equal(isHidden(empty), true, "empty остался видимым после появления опций");
    });
  });
});

describe("G2-11: подтверждение удаления не липнет к следующей строке", () => {
  it("смена строки и завершение onDelete снимают confirm", async () => {
    await withApp(async (app) => {
      let failing = true;
      app.setInvokeHandler(async (cmd) => {
        if (cmd === "delete_message") {
          if (failing) throw { status: 401, message: "unauthorized" };
          return null;
        }
        return null;
      });
      await scrollChat(app, 0);
      const list = firstNode(app, "#chat-list");
      const rowA = chatRow(app, 12);
      const rowB = chatRow(app, 13);
      assert.ok(rowA && rowB, "окно чата не содержит строк 12/13");
      const remove = firstNode(app, '.msg-toolbar [data-command="delete"]');
      const label = () => remove.textContent;

      // A: первый клик только взводит подтверждение.
      list.dispatchEvent("mouseover", { target: rowA });
      remove.click();
      assert.equal(app.callsOf("delete_message").length, 0, "первый клик удалил без подтверждения");
      assert.match(label(), /подтверд/i);

      // Переход на B сбрасывает состояние A: следующая строка не «уже подтверждена».
      list.dispatchEvent("mouseover", { target: rowB });
      assert.equal(label(), "Удалить сообщение", "confirm строки A уехал на строку B");

      // Возврат на A — тоже заново: подтверждение не хранится за строкой между заходами.
      list.dispatchEvent("mouseover", { target: rowA });
      assert.equal(label(), "Удалить сообщение", "возврат на A показал старое подтверждение");
      remove.click();
      assert.equal(app.callsOf("delete_message").length, 0, "один клик по A после возврата удалил");

      // B: один клик взводит, второй — ровно один delete_message.
      list.dispatchEvent("mouseover", { target: rowB });
      remove.click();
      assert.equal(app.callsOf("delete_message").length, 0, "один клик по B удалил без подтверждения");
      assert.match(label(), /подтверд/i);
      remove.click();
      await app.settle(2);
      assert.equal(app.callsOf("delete_message").length, 1);
      assert.deepEqual(app.callsOf("delete_message")[0]?.args, { channelId: "news", messageId: "13" });
      assert.equal(label(), "Удалить сообщение", "после ошибки delete confirm не сброшен");

      // Повторный confirm A — ровно один вызов, а не «залипший» второй.
      list.dispatchEvent("mouseover", { target: rowA });
      remove.click();
      assert.equal(label(), "Подтвердить удаление", "новый confirm A не взведён");
      remove.click();
      await app.settle(2);
      assert.equal(app.callsOf("delete_message").length, 2, "confirm A не отправил ровно один delete_message");
      assert.deepEqual(app.callsOf("delete_message")[1]?.args, { channelId: "news", messageId: "12" });

      // Успешное удаление тоже не оставляет confirm на панели.
      failing = false;
      list.dispatchEvent("mouseover", { target: rowB });
      remove.click();
      assert.match(label(), /подтверд/i);
      remove.click();
      await app.settle(4);
      assert.equal(app.callsOf("delete_message").length, 3);
      assert.equal(label(), "Удалить сообщение", "после успешного delete confirm остался на панели");
      assert.equal(chatRow(app, 13), null, "успешное удаление не убрало строку 13");
    });
  });
});
