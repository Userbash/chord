// @ts-check
/**
 * FE-P0-03 / FE-P0-04 (task-149) и T-212 (RW-40 / G2):
 * - unit-тесты контроллера `rust/gui-spike/ui/features/reactions-ui.js`
 *   (`REACTION_TYPES`, `createReactionsUiController`: `renderReactionBar`, `openReactorsModal`);
 * - поведенческие тесты `createReactionsController` и его монтирования в `rust/gui-spike/ui/features/chat.js`.
 *
 * Контрактные оракулы:
 * - `waves/RW-40/PM/BACKLOG-RE-FRONTEND.md` (FE-P0-03, FE-P0-04, строки 122–155)
 * - `waves/RW-40/DESIGN/REACTIONS-EXTENDED-SPEC.md`
 * - `rust/discord-client/src/http/reactions.rs` (`type=0` обычные, `type=1` супер-реакции/burst)
 */
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { loadApp } from "../support/gui-dom-harness.mjs";
import {
  REACTION_TYPES,
  createReactionsUiController,
} from "../../rust/gui-spike/ui/features/reactions-ui.js";

/** @typedef {import("../support/gui-dom-harness.mjs").GuiDom} GuiDom */
/** @typedef {import("../support/gui-dom-harness.mjs").ShimElement} ShimElement */

/**
 * Изолированный запуск с настоящим DOM-шимом и сбросом состояния реакций.
 * @param {(app: GuiDom, reactionsMod: any, chatMod: any) => void | Promise<void>} body
 * @param {(cmd: string, args: any) => any} [invokeHandler]
 */
async function withReactionsEnv(body, invokeHandler) {
  const reactionsMod = await import("../../rust/gui-spike/ui/features/reactions.js");
  const chatMod = await import("../../rust/gui-spike/ui/features/chat.js");
  reactionsMod.resetReactions?.();
  const app = await loadApp(invokeHandler ? { invokeHandler } : {});
  try {
    await body(app, reactionsMod, chatMod);
  } finally {
    reactionsMod.resetReactions?.();
    app.dispose();
  }
}

/**
 * Прокрутка `#chat-list` и ожидание отрисовки кадров.
 * @param {GuiDom} app
 * @param {number} scrollTop
 */
async function scrollChatTo(app, scrollTop) {
  const list = app.document.getElementById("chat-list");
  assert.ok(list, "в DOM нет #chat-list");
  list.scrollTop = scrollTop;
  list.emit("scroll");
  await app.settle(6);
}

describe("FE-P0-04 (task-149): интеграция createReactionsController и chat.js", () => {
  it("экспортирует createReactionsController и 9 категорий пикера реакций", async () => {
    const reactionsMod = await import("../../rust/gui-spike/ui/features/reactions.js");
    assert.equal(
      typeof reactionsMod.createReactionsController,
      "function",
      "reactions.js должен экспортировать createReactionsController",
    );
    assert.ok(
      Array.isArray(reactionsMod.REACTION_CATEGORIES),
      "reactions.js должен экспортировать REACTION_CATEGORIES",
    );
    assert.equal(
      reactionsMod.REACTION_CATEGORIES.length,
      9,
      "пикер реакций должен содержать 9 категорий по контракту FE-P0-04",
    );
    assert.equal(reactionsMod.REACTION_TYPE_NORMAL, 0);
    assert.equal(reactionsMod.REACTION_TYPE_BURST, 1);
  });

  it("пилюли реакций в chat.js вызывают переключение (add_reaction / remove_reaction с burst=false/true) и оптимистично обновляют счётчик и класс active", async () => {
    /** @type {Array<{ action: string, channelId: string, messageId: string, emoji: string, active: boolean, burst: boolean, type: number }>} */
    const toggleEvents = [];

    await withReactionsEnv(
      async (app, reactionsMod, chatMod) => {
        await scrollChatTo(app, 0);
        const list = app.document.getElementById("chat-list");
        assert.ok(list);

        const row = app.document.querySelector('#chat-list .row[data-index="26"]');
        assert.ok(row, "в окне чата должна быть строка 26 с реакциями");

        const controller = reactionsMod.createReactionsController({
          list,
          container: list.parentNode,
          channelId: "news",
          /** @param {any} payload */
          onToggleReaction: (payload) => {
            toggleEvents.push(payload);
          },
        });

        controller.bindMessageRow(row, {
          channelId: "news",
          messageId: "26",
          index: 26,
        });

        const pill = row.querySelector('.reaction[data-emoji="✅"]');
        assert.ok(pill, "у строки 26 должна быть пилюля ✅");
        assert.equal(pill.querySelector(".reaction-count")?.textContent, "1");
        assert.equal(pill.getAttribute("data-me"), "false");
        assert.equal(pill.getAttribute("aria-pressed"), "false");
        assert.equal(pill.classList.contains("active"), false);

        // 1. Обычный клик (burst=false -> add_reaction, count 1 -> 2, класс .active)
        pill.click();
        assert.equal(pill.querySelector(".reaction-count")?.textContent, "2", "счётчик должен оптимистично увеличиться до 2");
        assert.equal(pill.getAttribute("data-me"), "true");
        assert.equal(pill.getAttribute("aria-pressed"), "true");
        assert.equal(pill.classList.contains("active"), true, "активная пилюля должна получить класс .active");

        assert.equal(toggleEvents.length, 1);
        assert.equal(toggleEvents[0]?.action, "add_reaction");
        assert.equal(toggleEvents[0]?.channelId, "news");
        assert.equal(toggleEvents[0]?.messageId, "26");
        assert.equal(toggleEvents[0]?.emoji, "✅");
        assert.equal(toggleEvents[0]?.active, true);
        assert.equal(toggleEvents[0]?.burst, false);
        assert.equal(toggleEvents[0]?.type, 0);

        await app.settle(3);

        // 2. Повторный клик (burst=false -> remove_reaction, count 2 -> 1, снятие класса .active)
        pill.click();
        assert.equal(pill.querySelector(".reaction-count")?.textContent, "1", "счётчик должен оптимистично уменьшиться до 1");
        assert.equal(pill.getAttribute("data-me"), "false");
        assert.equal(pill.getAttribute("aria-pressed"), "false");
        assert.equal(pill.classList.contains("active"), false, "после снятия реакции класс .active должен удаляться");

        assert.equal(toggleEvents.length, 2);
        assert.equal(toggleEvents[1]?.action, "remove_reaction");
        assert.equal(toggleEvents[1]?.active, false);
        assert.equal(toggleEvents[1]?.burst, false);
        assert.equal(toggleEvents[1]?.type, 0);

        await app.settle(3);

        // 3. Shift+клик (burst=true -> супер-реакция type=1)
        pill.dispatchEvent("click", { shiftKey: true });
        assert.equal(pill.querySelector(".reaction-count")?.textContent, "2");
        assert.equal(pill.classList.contains("active"), true);
        assert.equal(pill.getAttribute("data-burst"), "true", "при burst=true должен выставляться data-burst='true'");

        assert.equal(toggleEvents.length, 3);
        assert.equal(toggleEvents[2]?.action, "add_reaction");
        assert.equal(toggleEvents[2]?.burst, true);
        assert.equal(toggleEvents[2]?.type, 1);

        // Проверяем также рендер канала (renderChannelMessage) с реакцией me=true
        const channelRow = app.document.createElement("div");
        list.append(channelRow);
        const channelItem = chatMod.parseChannelPage("c-100", {
          items: [
            {
              id: "m-500",
              author: { id: "u-1", username: "alice" },
              content: "Проверка реакций в канале",
              timestamp: "2026-10-30T12:00:00.000Z",
              reactions: [{ emoji: "🔥", count: 3, me: true }],
            },
          ],
        })[0];
        assert.ok(channelItem);
        chatMod.renderChannelMessage(channelRow, channelItem, 0, 1, { channelId: "c-100" });
        const channelPill = channelRow.querySelector('.reaction[data-emoji="🔥"]');
        assert.ok(channelPill, "в сообщении канала должна отрисоваться пилюля 🔥");
        assert.equal(channelPill.classList.contains("active"), true, "реакция с me=true сразу имеет класс .active");

        controller.destroy();
      },
      (cmd) => (cmd === "toggle_reaction" || cmd === "add_reaction" || cmd === "remove_reaction" ? null : undefined),
    );
  });

  it("всплывающая панель действий (.message-action-toolbar / .msg-hover-toolbar): быстрые реакции, открытие 9-категорийного #reaction-picker-popover и ответ", async () => {
    await withReactionsEnv(
      async (app) => {
        await scrollChatTo(app, 0);
        const list = app.document.getElementById("chat-list");
        assert.ok(list);
        const row = app.document.querySelector('#chat-list .row[data-index="12"]');
        assert.ok(row, "нет строки 12 в окне чата");

        // Наведение на сообщение открывает панель действий (.message-action-toolbar / .msg-hover-toolbar)
        list.dispatchEvent("mouseover", { target: row });
        const toolbar =
          app.document.querySelector(".message-action-toolbar") ??
          app.document.querySelector(".msg-hover-toolbar");
        assert.ok(toolbar, "в DOM должна быть панель .message-action-toolbar / .msg-hover-toolbar");
        assert.equal(toolbar.classList.contains("message-action-toolbar"), true);
        assert.equal(toolbar.classList.contains("msg-hover-toolbar"), true);
        assert.equal(toolbar.getAttribute("data-open"), "true");
        assert.equal(toolbar.getAttribute("data-for-index"), "12");

        // 1. Кнопки быстрого добавления реакции
        const quickBtns = toolbar.querySelectorAll('[data-command="react"]');
        assert.ok(quickBtns.length >= 3, "в панели должны быть кнопки быстрых реакций");
        const firstQuick = quickBtns[0];
        assert.ok(firstQuick);
        const quickEmoji = firstQuick.getAttribute("data-emoji") ?? "👍";
        firstQuick.click();

        const addedPill = app.document
          .querySelector('#chat-list .row[data-index="12"]')
          ?.querySelector(`.reaction[data-emoji="${quickEmoji}"]`);
        assert.ok(addedPill, "клик по быстрой реакции должен добавить пилюлю под сообщением");
        assert.equal(addedPill.classList.contains("active"), true);
        assert.equal(addedPill.getAttribute("aria-pressed"), "true");

        // 2. Кнопка открытия 9-категорийного пикера реакций (#reaction-picker-popover, role="dialog")
        assert.equal(
          app.document.getElementById("reaction-picker-popover"),
          null,
          "до клика поповер #reaction-picker-popover не должен быть в DOM (lazy mount)",
        );
        const addReactionBtn = toolbar.querySelector('[data-command="add-reaction"]');
        assert.ok(addReactionBtn, "в панели должна быть кнопка открытия пикера реакций [data-command='add-reaction']");
        assert.equal(addReactionBtn.getAttribute("aria-haspopup"), "dialog");
        assert.equal(addReactionBtn.getAttribute("aria-expanded"), "false");

        addReactionBtn.click();

        const popover = app.document.getElementById("reaction-picker-popover");
        assert.ok(popover, "клик по [data-command='add-reaction'] должен монтировать #reaction-picker-popover");
        assert.equal(popover.getAttribute("role"), "dialog");
        assert.equal(addReactionBtn.getAttribute("aria-expanded"), "true");
        assert.equal(addReactionBtn.getAttribute("aria-controls"), "reaction-picker-popover");

        // Проверяем 9 категорий эмодзи в пикере
        const tabsContainer = app.document.getElementById("reaction-picker-tabs");
        assert.ok(tabsContainer, "в пикере должен быть контейнер вкладок #reaction-picker-tabs");
        assert.equal(tabsContainer.getAttribute("role"), "tablist");
        const categoryTabs = tabsContainer.querySelectorAll('[role="tab"]');
        assert.equal(categoryTabs.length, 9, "в #reaction-picker-popover должно быть 9 категорий эмодзи");

        // Переключение на вторую категорию меняет сетку
        const grid = app.document.getElementById("reaction-picker-grid");
        assert.ok(grid, "в пикере должна быть сетка #reaction-picker-grid");
        assert.equal(grid.getAttribute("role"), "listbox");
        const firstCellBefore = grid.querySelector('[role="option"]')?.getAttribute("data-emoji");
        categoryTabs[1]?.click();
        assert.equal(categoryTabs[1]?.getAttribute("aria-selected"), "true");
        const firstCellAfter = grid.querySelector('[role="option"]')?.getAttribute("data-emoji");
        assert.notEqual(firstCellAfter, firstCellBefore, "клик по категории должен переключить набор эмодзи в сетке");

        // Поиск в пикере фильтрует эмодзи по всем категориям
        const searchInput = app.document.getElementById("reaction-picker-search");
        assert.ok(searchInput, "в пикере должно быть поле поиска #reaction-picker-search");
        searchInput.value = "🚀";
        searchInput.emit("input");
        const rocketCell = grid.querySelector('[data-emoji="🚀"]');
        assert.ok(rocketCell, "поиск должен находить эмодзи 🚀");

        // Переключатель супер-реакции (burst) в пикере + выбор эмодзи
        const burstToggle = popover.querySelector('[data-action="toggle-burst"]');
        assert.ok(burstToggle, "в пикере должен быть переключатель супер-реакции [data-action='toggle-burst']");
        burstToggle.click();
        assert.equal(burstToggle.getAttribute("aria-pressed"), "true");

        rocketCell.click();
        // После выбора эмодзи пикер закрывается (lazy destroy), а пилюля 🚀 появляется под сообщением
        assert.equal(
          app.document.getElementById("reaction-picker-popover"),
          null,
          "после выбора эмодзи #reaction-picker-popover должен размонтироваться",
        );
        assert.equal(addReactionBtn.getAttribute("aria-expanded"), "false");

        const rocketPill = app.document
          .querySelector('#chat-list .row[data-index="12"]')
          ?.querySelector('.reaction[data-emoji="🚀"]');
        assert.ok(rocketPill, "выбор эмодзи в пикере должен добавить пилюлю 🚀 к сообщению 12");
        assert.equal(rocketPill.classList.contains("active"), true);
        assert.equal(rocketPill.getAttribute("data-burst"), "true", "выбор с включённым burst должен пометить пилюлю data-burst='true'");

        // 3. Кнопка ответа в панели действий
        const replyBtn = toolbar.querySelector('[data-command="reply"]');
        assert.ok(replyBtn, "в панели должна быть кнопка ответа [data-command='reply']");
        replyBtn.click();
        const composer = app.document.getElementById("composer");
        assert.equal(composer?.getAttribute("data-reply"), "true", "клик по reply должен перевести композер в режим ответа");
      },
      (cmd) => (cmd === "toggle_reaction" || cmd === "add_reaction" || cmd === "remove_reaction" ? null : undefined),
    );
  });

  it("открывает модалку «Кто поставил реакцию» (#reaction-users-modal, вкладки Обычные type=0 и Супер-реакции type=1) из контекстного меню пилюли реакции", async () => {
    /** @type {Array<{ channelId: string, messageId: string, emoji: string, type: number }>} */
    const userQueries = [];

    await withReactionsEnv(
      async (app) => {
        await scrollChatTo(app, 0);
        const pill = app.document.querySelector('#chat-list .row[data-index="26"] .reaction[data-emoji="✅"]');
        assert.ok(pill, "нет пилюли ✅ в строке 26");

        // Контекстное меню на пилюле открывает #reaction-users-modal
        const ctxEvent = pill.dispatchEvent("contextmenu");
        assert.equal(ctxEvent.prevented, true, "contextmenu на пилюле должен вызывать preventDefault()");

        const modal = app.document.getElementById("reaction-users-modal");
        assert.ok(modal, "в DOM должна открыться модалка #reaction-users-modal");
        assert.equal(modal.getAttribute("role"), "dialog");
        assert.equal(modal.getAttribute("aria-modal"), "true");
        assert.match(modal.getAttribute("aria-label") ?? "", /Кто поставил реакцию/);

        // Проверяем вкладки Обычные (type=0) и Супер-реакции (type=1)
        const normalTab = modal.querySelector('[role="tab"][data-reaction-type="0"]');
        const burstTab = modal.querySelector('[role="tab"][data-reaction-type="1"]');
        assert.ok(normalTab, "в модалке должна быть вкладка Обычные (data-reaction-type='0')");
        assert.ok(burstTab, "в модалке должна быть вкладка Супер-реакции (data-reaction-type='1')");
        assert.match(normalTab.textContent, /Обычные/);
        assert.match(burstTab.textContent, /Супер-реакции/);
        assert.equal(normalTab.getAttribute("aria-selected"), "true");
        assert.equal(burstTab.getAttribute("aria-selected"), "false");

        await app.settle(3);
        assert.ok(userQueries.some((q) => q.type === 0 && q.emoji === "✅"), "должен запроситься список пользователей для type=0");

        const usersList = modal.querySelector("#reaction-users-list");
        assert.ok(usersList, "в модалке должен быть список #reaction-users-list");
        const normalUsers = usersList.querySelectorAll('[role="listitem"]');
        assert.equal(normalUsers.length, 2, "для type=0 должно отобразиться 2 пользователя");
        assert.match(usersList.textContent, /Alice/);
        assert.match(usersList.textContent, /Bob/);

        // Переключаемся на вкладку «Супер-реакции» (type=1)
        burstTab.click();
        await app.settle(3);
        assert.equal(burstTab.getAttribute("aria-selected"), "true");
        assert.equal(normalTab.getAttribute("aria-selected"), "false");
        assert.ok(userQueries.some((q) => q.type === 1 && q.emoji === "✅"), "должен запроситься список пользователей для type=1");

        const burstUsers = usersList.querySelectorAll('[role="listitem"]');
        assert.equal(burstUsers.length, 1, "для type=1 должен отобразиться 1 пользователь");
        assert.match(usersList.textContent, /SuperStar/);

        // Закрытие модалки по кнопке закрытия удаляет #reaction-users-modal из DOM
        const closeBtn = modal.querySelector('[data-action="reaction-users-close"]');
        assert.ok(closeBtn, "в модалке должна быть кнопка закрытия [data-action='reaction-users-close']");
        closeBtn.click();
        assert.equal(
          app.document.getElementById("reaction-users-modal"),
          null,
          "после закрытия #reaction-users-modal должен удаляться из DOM",
        );
      },
      (cmd, args) => {
        if (cmd === "get_reaction_users") {
          userQueries.push(args);
          if (args?.type === 1) {
            return [{ id: "u-burst", username: "superstar", global_name: "SuperStar" }];
          }
          return [
            { id: "u-1", username: "alice", global_name: "Alice" },
            { id: "u-2", username: "bob", global_name: "Bob" },
          ];
        }
        if (cmd === "toggle_reaction") return null;
        return undefined;
      },
    );
  });

  it("корректно очищает обработчики, поповеры и модалки при перерисовке и destroy()", async () => {
    let toggleCalls = 0;

    await withReactionsEnv(async (app, reactionsMod) => {
      const container = app.document.createElement("div");
      const list = app.document.createElement("div");
      container.append(list);
      app.document.body.append(container);

      const row = app.document.createElement("div");
      row.className = "row";
      row.setAttribute("data-index", "1");
      row.setAttribute("data-message-id", "msg-1");
      const author = app.document.createElement("span");
      author.className = "author";
      author.textContent = "tester";
      const text = app.document.createElement("span");
      text.className = "text";
      const body = app.document.createElement("span");
      body.className = "text-body";
      body.textContent = "#1 тестовое сообщение";
      const group = app.document.createElement("div");
      group.className = "reactions-row";
      const pill = app.document.createElement("button");
      pill.className = "reaction";
      pill.setAttribute("data-emoji", "👍");
      pill.setAttribute("data-me", "false");
      pill.setAttribute("aria-pressed", "false");
      const count = app.document.createElement("span");
      count.className = "reaction-count";
      count.textContent = "1";
      pill.append(count);
      group.append(pill);
      text.append(body, group);
      row.append(author, text);
      list.append(row);

      const controller = reactionsMod.createReactionsController({
        container,
        list,
        channelId: "c-clean",
        onToggleReaction: () => {
          toggleCalls += 1;
        },
      });

      controller.bindMessageRow(row, {
        channelId: "c-clean",
        messageId: "msg-1",
        index: 1,
        base: [{ emoji: "👍", count: 1, me: false }],
      });

      // Повторная привязка при перерисовке не должна дублировать обработчики клика на той же строке
      controller.bindMessageRow(row, {
        channelId: "c-clean",
        messageId: "msg-1",
        index: 1,
        base: [{ emoji: "👍", count: 1, me: false }],
      });

      pill.click();
      assert.equal(toggleCalls, 1, "после перерисовки/повторного bind клик должен срабатывать ровно 1 раз");

      // Открываем пикер и модалку, затем вызываем destroy()
      controller.openPicker({
        channelId: "c-clean",
        messageId: "msg-1",
        index: 1,
        row,
      });
      controller.openReactionUsersModal({
        channelId: "c-clean",
        messageId: "msg-1",
        emoji: "👍",
        type: 0,
      });
      assert.ok(app.document.getElementById("reaction-picker-popover"), "пикер должен быть открыт до destroy()");
      assert.ok(app.document.getElementById("reaction-users-modal"), "модалка должна быть открыта до destroy()");

      controller.destroy();

      assert.equal(
        app.document.getElementById("reaction-picker-popover"),
        null,
        "destroy() должен удалять #reaction-picker-popover",
      );
      assert.equal(
        app.document.getElementById("reaction-users-modal"),
        null,
        "destroy() должен удалять #reaction-users-modal",
      );

      // Клик по пилюле после destroy() не вызывает обработчик
      pill.click();
      assert.equal(toggleCalls, 1, "после destroy() обработчики на пилюлях должны быть сняты");
    });
  });
});

/* ------------------------------------------------------------------ *
 * T-212 (RW-40 / G2): изолированный DOM-шим и unit-тесты
 * `rust/gui-spike/ui/features/reactions-ui.js`
 * ------------------------------------------------------------------ */

class FakeStyle {
  constructor() {
    this.color = "";
    /** @type {Map<string, string>} */
    this._props = new Map();
  }

  /**
   * @param {string} name
   * @param {string} value
   */
  setProperty(name, value) {
    const str = String(value);
    this._props.set(name, str);
    if (name === "color") this.color = str;
  }

  /**
   * @param {string} name
   * @returns {string}
   */
  getPropertyValue(name) {
    if (name === "color" && this.color) return this.color;
    return this._props.get(name) ?? "";
  }
}

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
    this.style = new FakeStyle();
    /** @type {Map<string, Array<(event: any) => void>>} */
    this._listeners = new Map();
  }

  /** @returns {string} */
  get id() {
    return this.getAttribute("id") || "";
  }

  /** @param {string} val */
  set id(val) {
    this.setAttribute("id", val);
  }

  /** @returns {boolean} */
  get hidden() {
    return this.hasAttribute("hidden");
  }

  /** @param {boolean} val */
  set hidden(val) {
    if (val) this.setAttribute("hidden", "");
    else this.removeAttribute("hidden");
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
    for (const child of this.children) child.parentNode = null;
    this.children = [];
    this._text = String(val ?? "");
  }

  /**
   * @param {string} name
   * @param {string} value
   */
  setAttribute(name, value) {
    this._attrs.set(name, String(value));
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
  }

  /**
   * @param {FakeElement} child
   * @returns {FakeElement}
   */
  appendChild(child) {
    child.remove();
    child.parentNode = this;
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
    for (const child of this.children) child.parentNode = null;
    this.children = [];
    this._text = "";
    for (const node of nodes) this.appendChild(node);
  }

  remove() {
    if (this.parentNode) this.parentNode.removeChild(this);
  }

  /**
   * @param {string} type
   * @param {(event: any) => void} listener
   */
  addEventListener(type, listener) {
    if (!this._listeners.has(type)) this._listeners.set(type, []);
    this._listeners.get(type)?.push(listener);
  }

  /**
   * @param {string | { type: string, [key: string]: any }} typeOrEvent
   * @param {Record<string, any>} [init]
   */
  dispatchEvent(typeOrEvent, init = {}) {
    /** @type {{ type: string, [key: string]: any }} */
    const base =
      typeof typeOrEvent === "string"
        ? { type: typeOrEvent, ...init }
        : { ...typeOrEvent };
    const event = {
      ...base,
      type: String(base.type),
      target: base.target ?? this,
      currentTarget: this,
      defaultPrevented: false,
      preventDefault() {
        this.defaultPrevented = true;
      },
    };
    for (const listener of this._listeners.get(event.type) || []) {
      listener(event);
    }
    return !event.defaultPrevented;
  }

  click() {
    return this.dispatchEvent("click");
  }

  /**
   * @param {string} selector
   * @returns {FakeElement | null}
   */
  querySelector(selector) {
    return this.querySelectorAll(selector)[0] ?? null;
  }

  /**
   * @param {string} selector
   * @returns {FakeElement[]}
   */
  querySelectorAll(selector) {
    /** @type {FakeElement[]} */
    const out = [];
    const walk = (/** @type {FakeElement} */ el) => {
      for (const child of el.children) {
        if (matchesSelector(child, selector)) out.push(child);
        walk(child);
      }
    };
    walk(this);
    return out;
  }
}

/**
 * @param {FakeElement} el
 * @param {string} selector
 * @returns {boolean}
 */
function matchesSelector(el, selector) {
  const tokens = selector.match(/\[[^\]]+\]|[#.][\w-]+|[a-zA-Z][\w-]*/g) || [];
  if (tokens.length === 0) return false;
  return tokens.every((token) => {
    if (token.startsWith("#")) return el.id === token.slice(1);
    if (token.startsWith(".")) {
      const classes = (el.getAttribute("class") || "").split(/\s+/).filter(Boolean);
      return classes.includes(token.slice(1));
    }
    if (token.startsWith("[")) {
      const m = token
        .slice(1, -1)
        .match(/^([\w-]+)(?:=(?:"([^"]*)"|'([^']*)'|([^\]]*)))?$/);
      if (!m) return false;
      const attrName = m[1];
      if (!attrName) return false;
      const value = m[2] ?? m[3] ?? m[4];
      if (value === undefined) return el.hasAttribute(attrName);
      return el.getAttribute(attrName) === value;
    }
    return el.tagName === token.toUpperCase();
  });
}

function createTestContainer() {
  const doc = {
    /** @param {string} tag */
    createElement(tag) {
      return new FakeElement(tag, doc);
    },
  };
  return new FakeElement("div", doc);
}

describe("T-212: REACTION_TYPES", () => {
  it("заморожен (Object.freeze) и содержит NORMAL: 0, BURST: 1", () => {
    assert.equal(Object.isFrozen(REACTION_TYPES), true);
    assert.deepEqual(REACTION_TYPES, {
      NORMAL: 0,
      BURST: 1,
    });
    assert.throws(() => {
      // @ts-expect-error проверка неизменяемости замороженного объекта
      REACTION_TYPES.NORMAL = 99;
    }, TypeError);
  });
});

describe("T-212: createReactionsUiController — renderReactionBar", () => {
  it("рендерит пилюли с data-emoji, data-reaction-burst ('true' при burstCount > 0 иначе 'false'), aria-pressed ('true' при me || meBurst иначе 'false') и общим счётчиком count + (burstCount || 0)", () => {
    const container = createTestContainer();
    const ctrl = createReactionsUiController(container);

    ctrl.renderReactionBar([
      { emoji: "👍", count: 3, me: false, meBurst: false },
      { emoji: "🔥", count: 2, burstCount: 0, me: true, meBurst: false },
      {
        emoji: "🚀",
        count: 4,
        burstCount: 2,
        me: false,
        meBurst: true,
        burstColors: ["#ff4500", "#ffd700"],
      },
      { emoji: "✨", count: 0, burstCount: 5, me: false, meBurst: false },
    ]);

    const pills = container.querySelectorAll("button[data-emoji]");
    assert.equal(pills.length, 4);

    // 1. 👍: count=3, burstCount не задан -> data-reaction-burst='false', aria-pressed='false', total=3
    assert.equal(pills[0]?.getAttribute("data-emoji"), "👍");
    assert.equal(pills[0]?.getAttribute("data-reaction-burst"), "false");
    assert.equal(pills[0]?.getAttribute("aria-pressed"), "false");
    assert.equal(pills[0]?.querySelector("[data-reaction-count]")?.textContent, "3");
    assert.ok(pills[0]?.textContent.includes("👍"));
    assert.ok(pills[0]?.textContent.includes("3"));

    // 2. 🔥: count=2, burstCount=0, me=true -> data-reaction-burst='false', aria-pressed='true', total=2
    assert.equal(pills[1]?.getAttribute("data-emoji"), "🔥");
    assert.equal(pills[1]?.getAttribute("data-reaction-burst"), "false");
    assert.equal(pills[1]?.getAttribute("aria-pressed"), "true");
    assert.equal(pills[1]?.querySelector("[data-reaction-count]")?.textContent, "2");

    // 3. 🚀: count=4, burstCount=2, meBurst=true, burstColors -> data-reaction-burst='true', aria-pressed='true', total=6
    assert.equal(pills[2]?.getAttribute("data-emoji"), "🚀");
    assert.equal(pills[2]?.getAttribute("data-reaction-burst"), "true");
    assert.equal(pills[2]?.getAttribute("aria-pressed"), "true");
    assert.equal(pills[2]?.querySelector("[data-reaction-count]")?.textContent, "6");
    assert.equal(pills[2]?.getAttribute("data-burst-colors"), "#ff4500,#ffd700");

    // 4. ✨: count=0, burstCount=5, me=false, meBurst=false -> data-reaction-burst='true', aria-pressed='false', total=5
    assert.equal(pills[3]?.getAttribute("data-emoji"), "✨");
    assert.equal(pills[3]?.getAttribute("data-reaction-burst"), "true");
    assert.equal(pills[3]?.getAttribute("aria-pressed"), "false");
    assert.equal(pills[3]?.querySelector("[data-reaction-count]")?.textContent, "5");
  });

  it("клик по пилюле вызывает onToggleReaction(emoji, meBurst ? REACTION_TYPES.BURST : REACTION_TYPES.NORMAL, !(me || meBurst))", () => {
    const container = createTestContainer();
    /** @type {Array<{ emoji: string, reactionType: number, nextActive: boolean }>} */
    const calls = [];
    const ctrl = createReactionsUiController(container, {
      onToggleReaction(emoji, reactionType, nextActive) {
        calls.push({ emoji, reactionType, nextActive });
      },
    });

    ctrl.renderReactionBar([
      { emoji: "👍", count: 1, me: false, meBurst: false },
      { emoji: "❤️", count: 2, me: true, meBurst: false },
      { emoji: "🚀", count: 1, burstCount: 1, me: false, meBurst: true },
      { emoji: "🎉", count: 2, burstCount: 3, me: true, meBurst: true },
    ]);

    const thumbPill = container.querySelector('button[data-emoji="👍"]');
    const heartPill = container.querySelector('button[data-emoji="❤️"]');
    const rocketPill = container.querySelector('button[data-emoji="🚀"]');
    const partyPill = container.querySelector('button[data-emoji="🎉"]');

    assert.ok(thumbPill && heartPill && rocketPill && partyPill);

    // 1. Неактивная обычная реакция (me=false, meBurst=false) -> NORMAL (0), nextActive=true
    thumbPill.click();
    // 2. Активная обычная реакция (me=true, meBurst=false) -> NORMAL (0), nextActive=false
    heartPill.click();
    // 3. Активная суперреакция (me=false, meBurst=true) -> BURST (1), nextActive=false
    rocketPill.click();
    // 4. Активные обе (me=true, meBurst=true) -> BURST (1), nextActive=false
    partyPill.click();

    assert.deepEqual(calls, [
      { emoji: "👍", reactionType: REACTION_TYPES.NORMAL, nextActive: true },
      { emoji: "❤️", reactionType: REACTION_TYPES.NORMAL, nextActive: false },
      { emoji: "🚀", reactionType: REACTION_TYPES.BURST, nextActive: false },
      { emoji: "🎉", reactionType: REACTION_TYPES.BURST, nextActive: false },
    ]);
  });

  it("повторный вызов renderReactionBar перерисовывает панель без дублирования и корректно обрабатывает пустой массив", () => {
    const container = createTestContainer();
    const ctrl = createReactionsUiController(container);

    ctrl.renderReactionBar([{ emoji: "👍", count: 1 }]);
    assert.equal(container.querySelectorAll("button[data-emoji]").length, 1);

    ctrl.renderReactionBar([
      { emoji: "🔥", count: 2 },
      { emoji: "✅", count: 4, burstCount: 1, meBurst: true },
    ]);
    const pills = container.querySelectorAll("button[data-emoji]");
    assert.equal(pills.length, 2);
    assert.equal(pills[0]?.getAttribute("data-emoji"), "🔥");
    assert.equal(pills[1]?.getAttribute("data-emoji"), "✅");
    assert.equal(pills[1]?.querySelector("[data-reaction-count]")?.textContent, "5");

    ctrl.renderReactionBar([]);
    assert.equal(container.querySelectorAll("button[data-emoji]").length, 0);
  });
});

describe("T-212: createReactionsUiController — openReactorsModal", () => {
  const SAMPLE_REACTIONS_WITH_USERS = Object.freeze([
    {
      emoji: "🔥",
      count: 3,
      users: [
        {
          id: "u-1",
          displayName: "Алиса",
          username: "alice",
          reactionType: REACTION_TYPES.BURST,
        },
        {
          id: "u-2",
          displayName: "Борис",
          username: "boris",
          reactionType: REACTION_TYPES.NORMAL,
        },
        {
          id: "u-3",
          displayName: "Виктор",
          username: "victor",
          reactionType: REACTION_TYPES.BURST,
        },
      ],
    },
    {
      emoji: "👍",
      count: 1,
      users: [
        {
          id: "u-4",
          displayName: "Дарья",
          username: "daria",
          reactionType: REACTION_TYPES.NORMAL,
        },
      ],
    },
  ]);

  it("рендерит русский диалог 'Реакции' с вкладками фильтра эмодзи (эмодзи + счётчик) и списком пользователей с бейджем 'Суперреакция' при reactionType === 1", () => {
    const container = createTestContainer();
    /** @type {Array<{ messageId: string, count: number }>} */
    const openedCalls = [];
    const ctrl = createReactionsUiController(container, {
      onOpenReactors(messageId, list) {
        openedCalls.push({
          messageId,
          count: Array.isArray(list) ? list.length : 0,
        });
      },
    });

    ctrl.openReactorsModal("msg-777", SAMPLE_REACTIONS_WITH_USERS);

    assert.deepEqual(openedCalls, [{ messageId: "msg-777", count: 2 }]);

    const dialog = container.querySelector('[role="dialog"]');
    assert.ok(dialog, "диалог реакций должен быть смонтирован");
    assert.equal(dialog.getAttribute("aria-modal"), "true");
    assert.equal(dialog.getAttribute("aria-label"), "Реакции");
    assert.equal(dialog.getAttribute("data-message-id"), "msg-777");

    const titleEl = dialog.querySelector("[data-reactors-title]");
    assert.ok(titleEl);
    assert.equal(titleEl.textContent, "Реакции");

    // Вкладки эмодзи (показывают emoji + count)
    const tabs = dialog.querySelectorAll('[role="tab"]');
    assert.equal(tabs.length, 2);
    assert.equal(tabs[0]?.getAttribute("data-emoji"), "🔥");
    assert.equal(tabs[0]?.getAttribute("aria-selected"), "true");
    assert.ok(tabs[0]?.textContent.includes("🔥"));
    assert.ok(tabs[0]?.textContent.includes("3"));

    assert.equal(tabs[1]?.getAttribute("data-emoji"), "👍");
    assert.equal(tabs[1]?.getAttribute("aria-selected"), "false");
    assert.ok(tabs[1]?.textContent.includes("👍"));
    assert.ok(tabs[1]?.textContent.includes("1"));

    // Список пользователей для первой вкладки (🔥)
    const userRows = dialog.querySelectorAll("[data-reactor-user-id]");
    assert.equal(userRows.length, 3);

    // u-1: Алиса (@alice), reactionType === 1 -> есть бейдж 'Суперреакция'
    assert.equal(userRows[0]?.getAttribute("data-reactor-user-id"), "u-1");
    assert.equal(userRows[0]?.getAttribute("data-reaction-type"), "1");
    assert.equal(
      userRows[0]?.querySelector("[data-reactor-display-name]")?.textContent,
      "Алиса",
    );
    assert.equal(
      userRows[0]?.querySelector("[data-reactor-username]")?.textContent,
      "alice",
    );
    const badge0 = userRows[0]?.querySelector("[data-reactor-burst-badge]");
    assert.ok(badge0, "у пользователя с reactionType === 1 должен быть бейдж Суперреакция");
    assert.equal(badge0.textContent, "Суперреакция");

    // u-2: Борис (@boris), reactionType === 0 -> нет бейджа 'Суперреакция'
    assert.equal(userRows[1]?.getAttribute("data-reactor-user-id"), "u-2");
    assert.equal(userRows[1]?.getAttribute("data-reaction-type"), "0");
    assert.equal(
      userRows[1]?.querySelector("[data-reactor-display-name]")?.textContent,
      "Борис",
    );
    assert.equal(
      userRows[1]?.querySelector("[data-reactor-username]")?.textContent,
      "boris",
    );
    assert.equal(
      userRows[1]?.querySelector("[data-reactor-burst-badge]"),
      null,
      "у пользователя с reactionType === 0 не должно быть бейджа Суперреакция",
    );

    // u-3: Виктор (@victor), reactionType === 1 -> есть бейдж 'Суперреакция'
    assert.equal(userRows[2]?.getAttribute("data-reactor-user-id"), "u-3");
    assert.equal(
      userRows[2]?.querySelector("[data-reactor-burst-badge]")?.textContent,
      "Суперреакция",
    );
  });

  it("переключает активную вкладку эмодзи по клику и фильтрует список пользователей", () => {
    const container = createTestContainer();
    const ctrl = createReactionsUiController(container);

    ctrl.openReactorsModal("msg-777", SAMPLE_REACTIONS_WITH_USERS);

    const tabs = container.querySelectorAll('[role="tab"]');
    assert.equal(tabs.length, 2);

    // Кликаем по вкладке 👍
    tabs[1]?.click();

    assert.equal(tabs[0]?.getAttribute("aria-selected"), "false");
    assert.equal(tabs[1]?.getAttribute("aria-selected"), "true");

    const userRows = container.querySelectorAll("[data-reactor-user-id]");
    assert.equal(userRows.length, 1);
    assert.equal(userRows[0]?.getAttribute("data-reactor-user-id"), "u-4");
    assert.equal(
      userRows[0]?.querySelector("[data-reactor-display-name]")?.textContent,
      "Дарья",
    );
    assert.equal(
      userRows[0]?.querySelector("[data-reactor-username]")?.textContent,
      "daria",
    );
    assert.equal(userRows[0]?.querySelector("[data-reactor-burst-badge]"), null);
  });

  it("рендерит русское пустое состояние 'Нет реакций' при пустом списке реакций или пустом списке пользователей", () => {
    const container = createTestContainer();
    const ctrl = createReactionsUiController(container);

    ctrl.openReactorsModal("msg-empty-1", []);

    const dialog = container.querySelector('[role="dialog"]');
    assert.ok(dialog);
    assert.equal(dialog.querySelector("[data-reactors-title]")?.textContent, "Реакции");

    const emptyEl = dialog.querySelector("[data-reactors-empty]");
    assert.ok(emptyEl, "должно отображаться пустое состояние");
    assert.equal(emptyEl.textContent, "Нет реакций");
    assert.equal(dialog.querySelectorAll("[data-reactor-user-id]").length, 0);

    // Также проверяем случай, когда передана группа с пустым массивом users
    ctrl.openReactorsModal("msg-empty-2", [{ emoji: "👍", count: 0, users: [] }]);
    const emptyEl2 = container.querySelector("[data-reactors-empty]");
    assert.ok(emptyEl2);
    assert.equal(emptyEl2.textContent, "Нет реакций");
  });

  it("выбрасывает TypeError при невалидном containerEl и поддерживает закрытие модалки closeReactorsModal()", () => {
    assert.throws(() => createReactionsUiController(null), TypeError);

    const container = createTestContainer();
    const ctrl = createReactionsUiController(container);
    ctrl.openReactorsModal("msg-10", SAMPLE_REACTIONS_WITH_USERS);
    assert.ok(container.querySelector('[role="dialog"]'));

    ctrl.closeReactorsModal();
    assert.equal(container.querySelector('[role="dialog"]'), null);
  });
});

