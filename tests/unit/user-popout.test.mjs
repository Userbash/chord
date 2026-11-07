// @ts-check
/**
 * FE-P1-04 (task-159): unit-тесты для контроллера попапа карточки пользователя
 * (#user-popout) и модалки полного профиля (#user-profile-modal).
 */
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { ShimDocument, ShimElement } from "../support/gui-dom-harness.mjs";
import { createUserPopoutController } from "../../rust/gui-spike/ui/features/user-popout.js";

/**
 * Создаёт тестовое DOM-окружение со списком чата и списком участников.
 * @returns {{
 *   doc: ShimDocument,
 *   root: ShimElement,
 *   chatList: ShimElement,
 *   membersList: ShimElement,
 *   createChatRow: (options?: { author?: string, authorId?: string, bot?: boolean }) => ShimElement,
 *   createMemberRow: (options?: { name?: string, userId?: string, status?: string, role?: string, color?: string, bot?: boolean }) => ShimElement,
 * }}
 */
function createTestEnv() {
  const doc = new ShimDocument();
  const root = doc.body;

  const chatList = doc.createElement("div");
  chatList.id = "chat-list";
  chatList.className = "chat-list";
  root.appendChild(chatList);

  const membersList = doc.createElement("div");
  membersList.id = "member-list";
  membersList.className = "member-list";
  root.appendChild(membersList);

  /**
   * @param {{ author?: string, authorId?: string, bot?: boolean }} [options]
   */
  function createChatRow(options = {}) {
    const authorName = options.author ?? "Alice";
    const authorId = options.authorId ?? "1001";
    const isBot = Boolean(options.bot);

    const row = doc.createElement("div");
    row.className = "row";
    row.setAttribute("data-message-id", "msg-1");
    row.setAttribute("data-author-id", authorId);

    const author = doc.createElement("span");
    author.className = "author";

    const avatar = doc.createElement("span");
    avatar.className = "avatar avatar--lg";
    avatar.textContent = authorName.slice(0, 1);
    author.appendChild(avatar);

    const name = doc.createElement("span");
    name.className = "author-name";
    name.textContent = authorName;
    author.appendChild(name);

    if (isBot) {
      const botBadge = doc.createElement("span");
      botBadge.className = "badge badge--bot";
      botBadge.textContent = "БОТ";
      author.appendChild(botBadge);
    }

    const text = doc.createElement("span");
    text.className = "text";
    text.textContent = "Hello world";

    row.appendChild(author);
    row.appendChild(text);
    chatList.appendChild(row);
    return row;
  }

  /**
   * @param {{ name?: string, userId?: string, status?: string, role?: string, color?: string, bot?: boolean }} [options]
   */
  function createMemberRow(options = {}) {
    const name = options.name ?? "Bob";
    const userId = options.userId ?? "2002";
    const status = options.status ?? "online";
    const role = options.role ?? "Moderator";
    const color = options.color ?? "#ff5500";
    const isBot = Boolean(options.bot);

    const row = doc.createElement("div");
    row.className = "member member-row";
    row.setAttribute("data-user-id", userId);
    row.setAttribute("data-status", status);
    row.style.color = color;

    const avatar = doc.createElement("span");
    avatar.className = "avatar avatar--md";
    avatar.textContent = name.slice(0, 1);

    const dot = doc.createElement("span");
    dot.className = "status-dot";
    dot.setAttribute("data-status", status);
    avatar.appendChild(dot);

    const nameWrap = doc.createElement("span");
    nameWrap.className = "member-name";

    const label = doc.createElement("span");
    label.className = "member-label";
    label.textContent = name;
    nameWrap.appendChild(label);

    if (isBot) {
      const botBadge = doc.createElement("span");
      botBadge.className = "badge badge--bot";
      botBadge.textContent = "БОТ";
      nameWrap.appendChild(botBadge);
    }

    const roleEl = doc.createElement("span");
    roleEl.className = "member-role";
    roleEl.textContent = role;

    row.appendChild(avatar);
    row.appendChild(nameWrap);
    row.appendChild(roleEl);
    membersList.appendChild(row);
    return row;
  }

  return { doc, root, chatList, membersList, createChatRow, createMemberRow };
}

describe("FE-P1-04 (task-159): createUserPopoutController", () => {
  it("01. инициализируется и возвращает публичный API контроллера", () => {
    const { doc, root } = createTestEnv();
    const controller = createUserPopoutController({ document: doc, root });

    assert.equal(typeof controller.openPopout, "function");
    assert.equal(typeof controller.closePopout, "function");
    assert.equal(typeof controller.openProfileModal, "function");
    assert.equal(typeof controller.closeProfileModal, "function");
    assert.equal(typeof controller.attachToChatAndMembers, "function");
    assert.equal(typeof controller.destroy, "function");

    controller.destroy();
  });

  it("02. открывает компактный поповер #user-popout с правильными ARIA-атрибутами", () => {
    const { doc, root } = createTestEnv();
    const controller = createUserPopoutController({ document: doc, root });

    controller.openPopout({
      userId: "123",
      displayName: "ShadowByte",
      username: "shadowbyte",
    });

    const popout = doc.getElementById("user-popout");
    assert.ok(popout, "в DOM должен присутствовать #user-popout");
    assert.equal(popout.getAttribute("role"), "dialog");
    assert.equal(popout.getAttribute("aria-modal"), "false");
    assert.equal(popout.getAttribute("aria-label"), "Профиль пользователя ShadowByte");

    controller.destroy();
  });

  it("03. отображает аватар, имя, username/тег и индикатор статуса", () => {
    const { doc, root } = createTestEnv();
    const controller = createUserPopoutController({ document: doc, root });

    controller.openPopout({
      userId: "456",
      displayName: "Alice In Borderland",
      username: "alice",
      discriminator: "1337",
      status: "dnd",
    });

    const popout = doc.getElementById("user-popout");
    assert.ok(popout);

    const nameEl = popout.querySelector(".user-popout-display-name");
    assert.ok(nameEl, "должен быть элемент .user-popout-display-name");
    assert.equal(nameEl.textContent, "Alice In Borderland");

    const tagEl = popout.querySelector(".user-popout-tag");
    assert.ok(tagEl, "должен быть элемент .user-popout-tag");
    assert.equal(tagEl.textContent, "alice#1337");

    const statusDot = popout.querySelector(".status-dot");
    assert.ok(statusDot, "должен быть индикатор .status-dot");
    assert.equal(statusDot.getAttribute("data-status"), "dnd");

    controller.destroy();
  });

  it("04. отображает бейдж БОТ только когда bot=true", () => {
    const { doc, root } = createTestEnv();
    const controller = createUserPopoutController({ document: doc, root });

    // 1. Бот
    controller.openPopout({
      userId: "bot-1",
      displayName: "ProBot",
      username: "probot",
      bot: true,
    });
    const popoutBot = doc.getElementById("user-popout");
    assert.ok(popoutBot);
    const badge = popoutBot.querySelector(".badge--bot, .user-popout-bot-badge");
    assert.ok(badge, "у бота должен быть бейдж БОТ");
    assert.match(badge.textContent ?? "", /БОТ|BOT/);

    // 2. Обычный пользователь
    controller.openPopout({
      userId: "human-1",
      displayName: "Charlie",
      username: "charlie",
      bot: false,
    });
    const badgeHuman = doc.getElementById("user-popout")?.querySelector(".badge--bot, .user-popout-bot-badge");
    assert.equal(badgeHuman, null, "у обычного пользователя не должно быть бейджа БОТ");

    controller.destroy();
  });

  it("05. отображает кастомный статус и текущую активность", () => {
    const { doc, root } = createTestEnv();
    const controller = createUserPopoutController({ document: doc, root });

    controller.openPopout({
      userId: "789",
      displayName: "Gamer123",
      username: "gamer",
      customStatus: "Играет в Cyberpunk 2077",
    });

    const popout = doc.getElementById("user-popout");
    assert.ok(popout);

    const customStatusEl = popout.querySelector(".user-popout-custom-status");
    assert.ok(customStatusEl, "должен быть блок кастомного статуса");
    assert.equal(customStatusEl.textContent, "Играет в Cyberpunk 2077");

    controller.destroy();
  });

  it("06. отображает список ролей .user-popout-roles с цветными точками", () => {
    const { doc, root } = createTestEnv();
    const controller = createUserPopoutController({ document: doc, root });

    controller.openPopout({
      userId: "999",
      displayName: "Commander",
      username: "commander",
      roles: [
        { name: "Администратор", color: "#e91e63" },
        { name: "VIP", color: "#f1c40f" },
      ],
    });

    const popout = doc.getElementById("user-popout");
    assert.ok(popout);

    const rolesContainer = popout.querySelector(".user-popout-roles");
    assert.ok(rolesContainer, "должен присутствовать контейнер ролей .user-popout-roles");

    const roleItems = rolesContainer.querySelectorAll(".user-popout-role");
    assert.equal(roleItems.length, 2, "должно быть 2 роли");

    const firstRole = roleItems[0];
    assert.ok(firstRole, "первая роль должна существовать");
    const dot = firstRole.querySelector(".role-dot, .user-popout-role-dot");
    assert.ok(dot, "у роли должна быть цветная точка");
    assert.equal(dot.style.getPropertyValue("background-color"), "#e91e63");
    assert.match(firstRole.textContent ?? "", /Администратор/);

    controller.destroy();
  });

  it("07. сохраняет заметку по userId в памяти и восстанавливает при повторном открытии", () => {
    const { doc, root } = createTestEnv();
    const controller = createUserPopoutController({ document: doc, root });

    // Открываем пользователя A и пишем заметку
    controller.openPopout({
      userId: "user-a",
      displayName: "User A",
      username: "usera",
    });
    let noteInput = /** @type {ShimElement} */ (doc.getElementById("user-popout-note"));
    assert.ok(noteInput, "должно быть поле #user-popout-note");
    assert.equal(noteInput.value, "");

    noteInput.value = "Заметка о пользователе A: тестировщик";
    noteInput.dispatchEvent("input");

    // Открываем пользователя B — поле заметки должно быть пустым
    controller.openPopout({
      userId: "user-b",
      displayName: "User B",
      username: "userb",
    });
    noteInput = /** @type {ShimElement} */ (doc.getElementById("user-popout-note"));
    assert.equal(noteInput.value, "");

    // Снова открываем пользователя A — заметка восстановилась!
    controller.openPopout({
      userId: "user-a",
      displayName: "User A",
      username: "usera",
    });
    noteInput = /** @type {ShimElement} */ (doc.getElementById("user-popout-note"));
    assert.equal(noteInput.value, "Заметка о пользователе A: тестировщик");

    controller.destroy();
  });

  it("08. быстрое поле отправки ЛС #user-popout-dm-input вызывает onSendDm по Enter", () => {
    const { doc, root } = createTestEnv();
    /** @type {Array<{ userId: string, content: string }>} */
    const sent = [];

    const controller = createUserPopoutController({
      document: doc,
      root,
      onSendDm: (userId, content) => {
        sent.push({ userId, content });
      },
    });

    controller.openPopout({
      userId: "target-42",
      displayName: "Recipient",
      username: "recipient",
    });

    const dmInput = /** @type {ShimElement} */ (doc.getElementById("user-popout-dm-input"));
    assert.ok(dmInput, "должен быть инпут #user-popout-dm-input");

    // 1. Пустой ввод по Enter не отправляет
    dmInput.value = "   ";
    dmInput.dispatchEvent("keydown", { key: "Enter" });
    assert.equal(sent.length, 0, "пустое сообщение не должно отправляться");

    // 2. Заполненный ввод отправляет и очищает поле
    dmInput.value = "Привет! Как дела?";
    dmInput.dispatchEvent("keydown", { key: "Enter" });
    assert.equal(sent.length, 1);
    const firstSent = sent[0];
    assert.ok(firstSent, "сообщение должно быть отправлено");
    assert.equal(firstSent.userId, "target-42");
    assert.equal(firstSent.content, "Привет! Как дела?");
    assert.equal(dmInput.value, "", "поле должно быть очищено после отправки");

    controller.destroy();
  });

  it("09. кнопка «Полный профиль» #user-popout-full-profile-btn открывает модалку #user-profile-modal", () => {
    const { doc, root } = createTestEnv();
    const controller = createUserPopoutController({ document: doc, root });

    controller.openPopout({
      userId: "modal-user",
      displayName: "Full Profile Guy",
      username: "fullprof",
    });

    const fullBtn = doc.getElementById("user-popout-full-profile-btn");
    assert.ok(fullBtn, "должна быть кнопка #user-popout-full-profile-btn");

    fullBtn.click();

    // Поповер закрылся, модалка открылась
    const popout = doc.getElementById("user-popout");
    assert.equal(popout?.getAttribute("hidden"), "");

    const modal = doc.getElementById("user-profile-modal");
    assert.ok(modal, "должна появиться модалка #user-profile-modal");
    assert.equal(modal.getAttribute("role"), "dialog");
    assert.equal(modal.getAttribute("aria-modal"), "true");
    assert.equal(modal.getAttribute("aria-label"), "Профиль пользователя Full Profile Guy");

    controller.destroy();
  });

  it("10. модалка полного профиля: 3 вкладки и переключение панелей", () => {
    const { doc, root } = createTestEnv();
    const controller = createUserPopoutController({ document: doc, root });

    controller.openProfileModal({
      userId: "tab-user",
      displayName: "Tab Explorer",
      username: "tabby",
      bio: "Разработчик Discord-клиентов на Rust",
      mutualGuilds: [{ id: "g1", name: "Rust Developers" }],
      mutualFriends: [{ id: "u2", username: "Ferris" }],
    });

    const modal = doc.getElementById("user-profile-modal");
    assert.ok(modal);

    const tablist = modal.querySelector('[role="tablist"]');
    assert.ok(tablist, "должен быть tablist");

    const tabAbout = modal.querySelector('[role="tab"][data-tab="about"]');
    const tabGuilds = modal.querySelector('[role="tab"][data-tab="mutual-guilds"]');
    const tabFriends = modal.querySelector('[role="tab"][data-tab="mutual-friends"]');

    assert.ok(tabAbout, "вкладка «Обо мне»");
    assert.ok(tabGuilds, "вкладка «Общие серверы»");
    assert.ok(tabFriends, "вкладка «Общие друзья»");

    const panelAbout = modal.querySelector('[role="tabpanel"][data-panel="about"]');
    const panelGuilds = modal.querySelector('[role="tabpanel"][data-panel="mutual-guilds"]');
    const panelFriends = modal.querySelector('[role="tabpanel"][data-panel="mutual-friends"]');

    assert.ok(panelAbout);
    assert.ok(panelGuilds);
    assert.ok(panelFriends);

    // По умолчанию активна первая вкладка "Обо мне"
    assert.equal(tabAbout.getAttribute("aria-selected"), "true");
    assert.equal(panelAbout.getAttribute("hidden"), null);
    assert.equal(panelGuilds.getAttribute("hidden"), "");

    // Переключаем на "Общие серверы"
    tabGuilds.click();
    assert.equal(tabGuilds.getAttribute("aria-selected"), "true");
    assert.equal(tabAbout.getAttribute("aria-selected"), "false");
    assert.equal(panelGuilds.getAttribute("hidden"), null);
    assert.equal(panelAbout.getAttribute("hidden"), "");

    // Переключаем на "Общие друзья"
    tabFriends.click();
    assert.equal(tabFriends.getAttribute("aria-selected"), "true");
    assert.equal(panelFriends.getAttribute("hidden"), null);
    assert.equal(panelGuilds.getAttribute("hidden"), "");

    controller.destroy();
  });

  it("11. кнопка действия «Добавить в друзья» / «Отправить сообщение» в модалке", () => {
    const { doc, root } = createTestEnv();
    let friendAdded = "";

    const controller = createUserPopoutController({
      document: doc,
      root,
      onAddFriend: (userId) => {
        friendAdded = userId;
      },
    });

    controller.openProfileModal({
      userId: "friend-candidate",
      displayName: "New Friend",
      username: "newfriend",
    });

    const actionBtn = doc.getElementById("user-profile-action-btn");
    assert.ok(actionBtn, "должна быть кнопка действия в профиле");
    actionBtn.click();

    assert.equal(friendAdded, "friend-candidate");

    controller.destroy();
  });

  it("12. закрытие модалки по кнопке #user-profile-close-btn с возвратом фокуса на триггер", () => {
    const { doc, root } = createTestEnv();
    const trigger = doc.createElement("button");
    trigger.id = "my-trigger";
    root.appendChild(trigger);
    trigger.focus();

    const controller = createUserPopoutController({ document: doc, root });

    controller.openProfileModal(
      {
        userId: "close-test-user",
        displayName: "Close Test",
        username: "closetest",
      },
      trigger,
    );

    const modal = doc.getElementById("user-profile-modal");
    assert.ok(modal);
    assert.equal(modal.getAttribute("hidden"), null);

    const closeBtn = doc.getElementById("user-profile-close-btn");
    assert.ok(closeBtn, "должна быть кнопка закрытия");
    closeBtn.focus();
    assert.equal(doc.activeElement, closeBtn, "фокус перемещён на кнопку закрытия");

    closeBtn.click();

    assert.equal(modal.getAttribute("hidden"), "");
    assert.equal(doc.activeElement, trigger, "фокус должен вернуться на триггер");

    controller.destroy();
  });

  it("13. закрытие модалки по клику на backdrop с возвратом фокуса на триггер", () => {
    const { doc, root } = createTestEnv();
    const trigger = doc.createElement("button");
    root.appendChild(trigger);
    trigger.focus();

    const controller = createUserPopoutController({ document: doc, root });

    controller.openProfileModal(
      {
        userId: "backdrop-user",
        displayName: "Backdrop User",
        username: "backdropper",
      },
      trigger,
    );

    const backdrop = doc.getElementById("user-profile-modal-backdrop");
    assert.ok(backdrop, "должен быть бэкдроп модалки");
    const otherEl = doc.createElement("button");
    root.appendChild(otherEl);
    otherEl.focus();
    assert.equal(doc.activeElement, otherEl);

    backdrop.click();

    const modal = doc.getElementById("user-profile-modal");
    assert.equal(modal?.getAttribute("hidden"), "");
    assert.equal(doc.activeElement, trigger, "фокус вернулся на триггер");

    controller.destroy();
  });

  it("14. закрытие по Escape закрывает поповер и возвращает фокус на триггер", () => {
    const { doc, root } = createTestEnv();
    const trigger = doc.createElement("button");
    root.appendChild(trigger);
    trigger.focus();

    const controller = createUserPopoutController({ document: doc, root });

    controller.openPopout(
      {
        userId: "escape-user",
        displayName: "Escape Me",
        username: "escapeme",
      },
      trigger,
    );

    const popout = doc.getElementById("user-popout");
    assert.ok(popout);
    assert.equal(popout.getAttribute("hidden"), null);

    const noteInput = doc.getElementById("user-popout-note");
    assert.ok(noteInput);
    noteInput.focus();
    assert.equal(doc.activeElement, noteInput);

    // Нажатие Escape
    doc.body.dispatchEvent("keydown", { key: "Escape" });

    assert.equal(popout.getAttribute("hidden"), "");
    assert.equal(doc.activeElement, trigger, "фокус вернулся на триггер после Escape");

    controller.destroy();
  });

  it("15. attachToChatAndMembers: клик по .author в чате открывает поповер", () => {
    const { doc, root, chatList, membersList, createChatRow } = createTestEnv();
    const row = createChatRow({ author: "Elena", authorId: "elena-1", bot: true });
    const authorEl = row.querySelector(".author");
    assert.ok(authorEl);

    const controller = createUserPopoutController({ document: doc, root });
    controller.attachToChatAndMembers({ chatList, membersList });

    // Клик по имени автора в чате
    chatList.dispatchEvent("click", { target: authorEl });

    const popout = doc.getElementById("user-popout");
    assert.ok(popout);
    assert.equal(popout.getAttribute("hidden"), null);

    const nameEl = popout.querySelector(".user-popout-display-name");
    assert.equal(nameEl?.textContent, "Elena");

    const badge = popout.querySelector(".badge--bot, .user-popout-bot-badge");
    assert.ok(badge, "у автора-бота отображается бейдж");

    controller.destroy();
  });

  it("16. attachToChatAndMembers: клик по .member в списке участников открывает поповер с цветом и ролью", () => {
    const { doc, root, chatList, membersList, createMemberRow } = createTestEnv();
    const row = createMemberRow({
      name: "Dmitry",
      userId: "dmitry-7",
      status: "idle",
      role: "Lead Developer",
      color: "#2ecc71",
    });
    const label = row.querySelector(".member-label");
    assert.ok(label);

    const controller = createUserPopoutController({ document: doc, root });
    controller.attachToChatAndMembers({ chatList, membersList });

    // Клик по строке участника
    membersList.dispatchEvent("click", { target: label });

    const popout = doc.getElementById("user-popout");
    assert.ok(popout);
    assert.equal(popout.getAttribute("hidden"), null);

    const nameEl = popout.querySelector(".user-popout-display-name");
    assert.equal(nameEl?.textContent, "Dmitry");

    const statusDot = popout.querySelector(".status-dot");
    assert.equal(statusDot?.getAttribute("data-status"), "idle");

    const rolesContainer = popout.querySelector(".user-popout-roles");
    assert.ok(rolesContainer);
    assert.match(rolesContainer.textContent ?? "", /Lead Developer/);

    controller.destroy();
  });

  it("17. destroy() очищает DOM и слушатели, повторные клики не открывают поповер", () => {
    const { doc, root, chatList, membersList, createChatRow } = createTestEnv();
    const row = createChatRow({ author: "Ghost", authorId: "ghost-0" });
    const authorEl = row.querySelector(".author");

    const controller = createUserPopoutController({ document: doc, root });
    controller.attachToChatAndMembers({ chatList, membersList });

    controller.destroy();

    assert.equal(doc.getElementById("user-popout"), null, "#user-popout должен быть удалён из DOM");
    assert.equal(doc.getElementById("user-profile-modal"), null, "#user-profile-modal должен быть удалён из DOM");

    // Клик после destroy не создаёт поповер
    chatList.dispatchEvent("click", { target: authorEl });
    assert.equal(doc.getElementById("user-popout"), null);
  });
});
