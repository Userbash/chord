// @ts-check
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { ShimDocument } from "../support/gui-dom-harness.mjs";
import {
  createInboxPanel,
  INBOX_TEXT,
  INBOX_TABS,
} from "../../rust/gui-spike/ui/features/inbox.js";

/**
 * Создаёт тестовое окружение для тестирования выпадающей панели Inbox.
 * @param {Record<string, any>} [customOpts]
 */
function createTestEnv(customOpts = {}) {
  const doc = new ShimDocument();
  const root = doc.createElement("div");
  root.id = "app-root";
  doc.body.append(root);

  const inboxButton = doc.createElement("button");
  inboxButton.className = "icon-btn";
  inboxButton.setAttribute("data-action", "inbox");
  inboxButton.setAttribute("aria-label", "Входящие");
  root.append(inboxButton);

  /** @type {any[]} */
  const jumped = [];
  /** @type {any[]} */
  const acked = [];

  const defaultMentions = [
    {
      id: "msg-1",
      channelId: "ch-101",
      channelName: "флуд",
      guildName: "Сервер Разработки",
      author: "Alice",
      authorAvatar: "",
      content: "Привет @everyone, посмотрите новый релиз!",
      timestamp: "12:30",
    },
    {
      id: "msg-2",
      channelId: "ch-102",
      channelName: "общее",
      guildName: "Сервер Разработки",
      author: "Bob",
      authorAvatar: "",
      content: "@tester нужно протестировать STUB-11",
      timestamp: "12:45",
    },
  ];

  const defaultUnreads = [
    {
      channelId: "ch-101",
      channelName: "флуд",
      guildName: "Сервер Разработки",
      unreadCount: 5,
    },
  ];

  const controller = createInboxPanel({
    document: doc,
    container: root,
    inboxButton,
    loadMentions: async () => defaultMentions,
    loadUnreads: async () => defaultUnreads,
    onJumpToMessage: (channelId, messageId) => {
      jumped.push({ channelId, messageId });
    },
    onAckChannel: (channelId) => {
      acked.push(channelId);
    },
    ...customOpts,
  });

  return { doc, root, inboxButton, controller, jumped, acked, defaultMentions };
}

describe("STUB-11 (N-03): Inbox Dropdown Panel", () => {
  it("монтирует панель #chat-inbox-panel с правильными a11y атрибутами", () => {
    const { doc, inboxButton } = createTestEnv();

    const panel = /** @type {any} */ (doc.getElementById("chat-inbox-panel"));
    assert.ok(panel, "панель #chat-inbox-panel создана в DOM");
    assert.equal(panel.getAttribute("role"), "dialog");
    assert.equal(panel.getAttribute("aria-label"), INBOX_TEXT.title);
    assert.equal(panel.hidden, true, "по умолчанию панель скрыта");
    assert.equal(inboxButton.getAttribute("aria-expanded"), "false");
    assert.equal(inboxButton.getAttribute("aria-haspopup"), "dialog");
    assert.equal(inboxButton.getAttribute("aria-controls"), "chat-inbox-panel");
  });

  it("управляет открытием, закрытием и состоянием aria-expanded", () => {
    const { doc, inboxButton, controller } = createTestEnv();
    const panel = /** @type {any} */ (doc.getElementById("chat-inbox-panel"));

    controller.open();
    assert.equal(panel.hidden, false, "панель открыта");
    assert.equal(inboxButton.getAttribute("aria-expanded"), "true");
    assert.equal(panel.classList.contains("is-open"), true);

    controller.close();
    assert.equal(panel.hidden, true, "панель закрыта");
    assert.equal(inboxButton.getAttribute("aria-expanded"), "false");
    assert.equal(panel.classList.contains("is-open"), false);
  });

  it("переключает вкладки 'Упоминания' и 'Непрочитанные'", () => {
    const { doc, controller } = createTestEnv();

    controller.open();
    assert.equal(controller.getActiveTab(), INBOX_TABS.mentions);

    // Переключаем на непрочитанные
    controller.setActiveTab(INBOX_TABS.unreads);
    assert.equal(controller.getActiveTab(), INBOX_TABS.unreads);

    const unreadsTabBtn = /** @type {any} */ (
      doc.querySelector('[data-tab="unreads"]')
    );
    assert.ok(unreadsTabBtn);
    assert.equal(unreadsTabBtn.getAttribute("aria-selected"), "true");

    const mentionsTabBtn = /** @type {any} */ (
      doc.querySelector('[data-tab="mentions"]')
    );
    assert.ok(mentionsTabBtn);
    assert.equal(mentionsTabBtn.getAttribute("aria-selected"), "false");
  });

  it("отображает карточки упоминаний и вызывает onJumpToMessage по клику 'Перейти'", async () => {
    const { doc, controller, jumped } = createTestEnv();

    controller.open();
    await controller.refresh();

    const items = doc.querySelectorAll(".inbox-item");
    assert.equal(items.length, 2, "2 карточки упоминаний отрисованы");

    const firstJumpBtn = /** @type {any} */ (
      doc.querySelector('.inbox-item [data-action="jump"]')
    );
    assert.ok(firstJumpBtn, "кнопка jump найдена");

    firstJumpBtn.click();
    assert.equal(jumped.length, 1);
    assert.equal(jumped[0].channelId, "ch-101");
    assert.equal(jumped[0].messageId, "msg-1");

    const panel = /** @type {any} */ (doc.getElementById("chat-inbox-panel"));
    assert.equal(panel.hidden, true, "панель закрылась после перехода");
  });

  it("отображает пустое состояние при отсутствии упоминаний", async () => {
    const { doc, controller } = createTestEnv({
      loadMentions: async () => [],
    });

    controller.open();
    await controller.refresh();

    const empty = doc.querySelector(".inbox-empty");
    assert.ok(empty, "отображается блок .inbox-empty");
    assert.match(empty.textContent || "", /Входящие пусты/);
  });

  it("кнопка 'Очистить всё' / Ack отправляет onAckChannel", async () => {
    const { doc, controller, acked } = createTestEnv();

    controller.open();
    controller.setActiveTab(INBOX_TABS.unreads);
    await controller.refresh();

    const ackAllBtn = /** @type {any} */ (
      doc.querySelector('[data-action="inbox-ack-all"]')
    );
    assert.ok(ackAllBtn, "кнопка ack-all найдена");

    ackAllBtn.click();
    assert.equal(acked.length, 1);
    assert.equal(acked[0], "ch-101");
  });
});
