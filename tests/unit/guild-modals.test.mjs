// @ts-check
/**
 * task-160 (FE-P1-05): Модалка приглашения на сервер/канал #invite-modal и
 * контекстное меню сервера #guild-context-menu: создание инвайта, настройки
 * уведомлений, подтверждение выхода из сервера и присоединение по инвайту.
 *
 * Спецификация и контракты:
 * - Контекстное меню гильдии #guild-context-menu (role='menu', пункты role='menuitem'
 *   и role='menuitemcheckbox', roving tabindex / клавиатурная навигация);
 * - Модалка инвайта #invite-modal (role='dialog', aria-modal='true', вызов create_invite,
 *   #invite-link-input, #invite-copy-btn, #invite-max-age-select, #invite-max-uses-select,
 *   #invite-temporary-checkbox);
 * - Модалка выхода #leave-guild-modal (role='dialog', aria-modal='true', вызов leave_guild);
 * - Модалка присоединения #join-guild-modal (role='dialog', aria-modal='true', вызов resolve_invite + accept_invite / join_guild).
 */
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { ShimDocument } from "../support/gui-dom-harness.mjs";
import {
  createGuildModalsController,
  normalizeInviteCode,
  VALID_INVITE_MAX_AGES,
  VALID_INVITE_MAX_USES,
} from "../../rust/gui-spike/ui/features/guild-modals.js";

/** @typedef {import("../support/gui-dom-harness.mjs").ShimElement} ShimElement */

/**
 * Создаёт тестовое окружение с чистым ShimDocument.
 * @param {Record<string, any>} [customOpts]
 */
function createTestEnv(customOpts = {}) {
  const doc = new ShimDocument();
  const root = doc.createElement("div");
  root.id = "app-root";
  doc.body.append(root);

  /** @type {Array<{ cmd: string, args: any }>} */
  const calls = [];
  /** @type {string[]} */
  const copiedTexts = [];

  /**
   * Локальная функция вызова IPC с именем invoker (контракт INT-07).
   * @param {string} cmd
   * @param {any} [args]
   * @returns {Promise<any>}
   */
  const invoker = async (cmd, args = {}) => {
    calls.push({ cmd, args });
    if (cmd === "create_invite") {
      return {
        code: `test-code-${args?.maxAge ?? 86400}-${args?.maxUses ?? 0}`,
        channelId: args?.channelId ?? "chan-1",
        maxAge: args?.maxAge ?? 86400,
        maxUses: args?.maxUses ?? 0,
        temporary: Boolean(args?.temporary),
      };
    }
    if (cmd === "leave_guild") {
      return null;
    }
    if (cmd === "resolve_invite") {
      return {
        code: args?.code ?? "abc123",
        guildId: "guild-target-1",
        guildName: "Target Guild",
      };
    }
    if (cmd === "accept_invite") {
      return {
        guildId: "guild-target-1",
        targetChannelId: "chan-target-1",
      };
    }
    if (cmd === "join_guild") {
      return null;
    }
    return null;
  };

  const clipboard = {
    /** @param {string} text */
    writeText: async (text) => {
      copiedTexts.push(text);
    },
  };

  const controller = createGuildModalsController({
    document: doc,
    root,
    invoker,
    guildId: "guild-100",
    channelId: "channel-200",
    clipboard,
    ...customOpts,
  });

  return { doc, root, calls, copiedTexts, controller, invoker };
}

describe("task-160: guild-modals controller & components", () => {
  it("01: константы допустимых max_age и max_uses экспортированы", () => {
    assert.deepEqual(VALID_INVITE_MAX_AGES, [0, 1800, 3600, 21600, 43200, 86400, 604800]);
    assert.deepEqual(VALID_INVITE_MAX_USES, [0, 1, 5, 10, 25, 50, 100]);
    assert.equal(normalizeInviteCode("https://discord.gg/abc-123"), "abc-123");
    assert.equal(normalizeInviteCode("https://discord.gg/invalid@@@$$$"), "");
    assert.equal(normalizeInviteCode("a"), "");
  });

  it("02: контекстное меню гильдии #guild-context-menu создаётся с role='menu' и 4 пунктами", () => {
    const { doc } = createTestEnv();
    const menu = doc.getElementById("guild-context-menu");
    assert.ok(menu, "контекстное меню #guild-context-menu должно присутствовать в DOM");
    assert.equal(menu.getAttribute("role"), "menu");
    assert.equal(menu.hasAttribute("hidden"), true, "меню изначально скрыто");

    const items = menu.querySelectorAll('[role="menuitem"], [role="menuitemcheckbox"]');
    assert.equal(items.length, 4, "меню должно содержать ровно 4 пункта");

    const actions = items.map((i) => i.getAttribute("data-action"));
    assert.deepEqual(actions, ["invite", "notifications", "hide-muted", "leave-guild"]);

    const hideMuted = menu.querySelector('[data-action="hide-muted"]');
    assert.ok(hideMuted);
    assert.equal(hideMuted.getAttribute("role"), "menuitemcheckbox");
    assert.equal(hideMuted.getAttribute("aria-checked"), "false");

    const leaveItem = menu.querySelector('[data-action="leave-guild"]');
    assert.ok(leaveItem);
    assert.ok(
      leaveItem.classList.contains("danger") || leaveItem.getAttribute("data-danger") === "true",
      "пункт выхода должен быть помечен как опасное действие",
    );
  });

  it("03: контекстное меню открывается по openContextMenu() и по contextmenu на триггере", () => {
    const { doc, controller } = createTestEnv();
    const menu = doc.getElementById("guild-context-menu");
    assert.ok(menu);

    controller.openContextMenu({ x: 120, y: 80, guildId: "guild-42" });
    assert.equal(menu.hasAttribute("hidden"), false, "меню открыто (hidden снят)");
    assert.equal(menu.style.left, "120px");
    assert.equal(menu.style.top, "80px");

    // Первый элемент получает фокус
    const firstItem = menu.querySelector('[data-action="invite"]');
    assert.equal(doc.activeElement, firstItem);

    // Закрываем меню
    controller.closeContextMenu();
    assert.equal(menu.hasAttribute("hidden"), true, "меню скрыто после closeContextMenu()");

    // Открытие через attachTrigger
    const trigger = doc.createElement("button");
    trigger.id = "guild-header-btn";
    doc.body.append(trigger);
    controller.attachTrigger(trigger, { guildId: "guild-42" });

    trigger.dispatchEvent("contextmenu", /** @type {any} */ ({
      type: "contextmenu",
      clientX: 200,
      clientY: 150,
      preventDefault: () => {},
    }));
    assert.equal(menu.hasAttribute("hidden"), false, "меню открылось по событию contextmenu");
  });

  it("04: контекстное меню поддерживает циклическую клавиатурную навигацию ArrowDown / ArrowUp", () => {
    const { doc, controller } = createTestEnv();
    const menu = doc.getElementById("guild-context-menu");
    assert.ok(menu);

    controller.openContextMenu();
    const items = menu.querySelectorAll('[role="menuitem"], [role="menuitemcheckbox"]');
    assert.equal(doc.activeElement, items[0]);

    // ArrowDown 0 -> 1 -> 2 -> 3 -> 0
    menu.dispatchEvent("keydown", { key: "ArrowDown" });
    assert.equal(doc.activeElement, items[1]);
    menu.dispatchEvent("keydown", { key: "ArrowDown" });
    assert.equal(doc.activeElement, items[2]);
    menu.dispatchEvent("keydown", { key: "ArrowDown" });
    assert.equal(doc.activeElement, items[3]);
    menu.dispatchEvent("keydown", { key: "ArrowDown" });
    assert.equal(doc.activeElement, items[0], "ArrowDown циклически возвращает на первый пункт");

    // ArrowUp 0 -> 3 -> 2
    menu.dispatchEvent("keydown", { key: "ArrowUp" });
    assert.equal(doc.activeElement, items[3], "ArrowUp с первого пункта переходит на последний");
    menu.dispatchEvent("keydown", { key: "ArrowUp" });
    assert.equal(doc.activeElement, items[2]);
  });

  it("05: клавиши Home и End перемещают фокус на первый и последний пункты", () => {
    const { doc, controller } = createTestEnv();
    const menu = doc.getElementById("guild-context-menu");
    assert.ok(menu);

    controller.openContextMenu();
    const items = menu.querySelectorAll('[role="menuitem"], [role="menuitemcheckbox"]');

    menu.dispatchEvent("keydown", { key: "End" });
    assert.equal(doc.activeElement, items[3], "End перемещает на последний пункт");

    menu.dispatchEvent("keydown", { key: "Home" });
    assert.equal(doc.activeElement, items[0], "Home перемещает на первый пункт");
  });

  it("06: клавиша Escape закрывает контекстное меню и возвращает фокус на триггер", () => {
    const { doc, controller } = createTestEnv();
    const menu = doc.getElementById("guild-context-menu");
    assert.ok(menu);

    const trigger = doc.createElement("button");
    trigger.id = "server-header-btn";
    doc.body.append(trigger);

    controller.openContextMenu({ triggerElement: trigger });
    assert.equal(menu.hasAttribute("hidden"), false);

    menu.dispatchEvent("keydown", { key: "Escape" });
    assert.equal(menu.hasAttribute("hidden"), true, "Escape закрывает меню");
    assert.equal(doc.activeElement, trigger, "фокус вернулся на триггер");
  });

  it("07: клик по пункту 'Пригласить людей' закрывает меню и открывает #invite-modal", () => {
    const { doc, controller } = createTestEnv();
    const menu = doc.getElementById("guild-context-menu");
    const inviteModal = doc.getElementById("invite-modal");
    assert.ok(menu);
    assert.ok(inviteModal);

    controller.openContextMenu();
    const inviteItem = menu.querySelector('[data-action="invite"]');
    assert.ok(inviteItem);

    inviteItem.click();
    assert.equal(menu.hasAttribute("hidden"), true, "меню закрылось");
    assert.equal(inviteModal.hasAttribute("hidden"), false, "модалка инвайта открылась");
  });

  it("08: клик по пункту 'Настройки уведомлений' закрывает меню и вызывает callback", () => {
    /** @type {any[]} */
    const notifCalls = [];
    const { doc, controller } = createTestEnv({
      onOpenNotifications: (/** @type {any} */ params) => notifCalls.push(params),
    });
    const menu = doc.getElementById("guild-context-menu");
    assert.ok(menu);

    controller.openContextMenu({ guildId: "guild-99" });
    const notifItem = menu.querySelector('[data-action="notifications"]');
    assert.ok(notifItem);

    notifItem.click();
    assert.equal(menu.hasAttribute("hidden"), true);
    assert.equal(notifCalls.length, 1);
    assert.equal(notifCalls[0].guildId, "guild-99");
  });

  it("09: клик по пункту 'Скрыть заглушенные каналы' переключает aria-checked и вызывает callback", () => {
    /** @type {boolean[]} */
    const toggles = [];
    const { doc, controller } = createTestEnv({
      onToggleHideMuted: (/** @type {boolean} */ val) => toggles.push(val),
    });
    const menu = doc.getElementById("guild-context-menu");
    assert.ok(menu);

    controller.openContextMenu();
    const hideMuted = menu.querySelector('[data-action="hide-muted"]');
    assert.ok(hideMuted);
    assert.equal(hideMuted.getAttribute("aria-checked"), "false");

    hideMuted.click();
    assert.equal(hideMuted.getAttribute("aria-checked"), "true");
    assert.deepEqual(toggles, [true]);

    hideMuted.click();
    assert.equal(hideMuted.getAttribute("aria-checked"), "false");
    assert.deepEqual(toggles, [true, false]);
  });

  it("10: пункт 'Покинуть сервер' закрывает меню и открывает #leave-guild-modal", () => {
    const { doc, controller } = createTestEnv();
    const menu = doc.getElementById("guild-context-menu");
    const leaveModal = doc.getElementById("leave-guild-modal");
    assert.ok(menu);
    assert.ok(leaveModal);

    controller.openContextMenu({ guildId: "guild-55" });
    const leaveItem = menu.querySelector('[data-action="leave-guild"]');
    assert.ok(leaveItem);

    leaveItem.click();
    assert.equal(menu.hasAttribute("hidden"), true, "меню закрылось");
    assert.equal(leaveModal.hasAttribute("hidden"), false, "модалка подтверждения выхода открыта");
  });

  it("11: #invite-modal автоматически создаёт инвайт при открытии и заполняет #invite-link-input", async () => {
    const { doc, controller, calls } = createTestEnv();
    const modal = doc.getElementById("invite-modal");
    const linkInput = /** @type {any} */ (doc.getElementById("invite-link-input"));
    assert.ok(modal);
    assert.ok(linkInput);

    await controller.openInviteModal({ channelId: "chan-abc" });

    assert.equal(modal.hasAttribute("hidden"), false);
    assert.equal(modal.getAttribute("role"), "dialog");
    assert.equal(modal.getAttribute("aria-modal"), "true");

    // Проверяем вызов create_invite через invoker
    const inviteCall = calls.find((c) => c.cmd === "create_invite");
    assert.ok(inviteCall, "create_invite должен быть вызван при открытии");
    assert.equal(inviteCall.args.channelId, "chan-abc");
    assert.equal(inviteCall.args.maxAge, 86400);
    assert.equal(inviteCall.args.maxUses, 0);
    assert.equal(inviteCall.args.temporary, false);

    assert.match(linkInput.value, /^https:\/\/discord\.gg\/test-code-86400-0/);
  });

  it("12: #invite-copy-btn копирует ссылку в буфер обмена и меняет текст на 'Скопировано!'", async () => {
    const { doc, controller, copiedTexts } = createTestEnv();
    await controller.openInviteModal({ channelId: "chan-copy" });

    const copyBtn = doc.getElementById("invite-copy-btn");
    const linkInput = /** @type {any} */ (doc.getElementById("invite-link-input"));
    assert.ok(copyBtn);
    assert.ok(linkInput);

    assert.equal(copyBtn.textContent, "Копировать");
    copyBtn.click();

    assert.equal(copiedTexts.length, 1);
    assert.equal(copiedTexts[0], linkInput.value);
    assert.equal(copyBtn.textContent, "Скопировано!");
  });

  it("13: смена maxAge, maxUses и temporary пересоздаёт инвайт с новыми параметрами", async () => {
    const { doc, controller, calls } = createTestEnv();
    await controller.openInviteModal({ channelId: "chan-opts" });
    calls.length = 0; // очищаем вызовы после открытия

    const maxAgeSelect = /** @type {any} */ (doc.getElementById("invite-max-age-select"));
    const maxUsesSelect = /** @type {any} */ (doc.getElementById("invite-max-uses-select"));
    const tempCheckbox = /** @type {any} */ (doc.getElementById("invite-temporary-checkbox"));

    assert.ok(maxAgeSelect);
    assert.ok(maxUsesSelect);
    assert.ok(tempCheckbox);

    // Смена maxAge на 3600 (1 час)
    maxAgeSelect.value = "3600";
    await maxAgeSelect.dispatchEvent("change", { type: "change" });

    assert.equal(calls.length, 1);
    const call0 = calls[0];
    assert.ok(call0);
    assert.equal(call0.cmd === "create_invite" ? call0.args.maxAge : -1, 3600);

    // Смена maxUses на 10
    maxUsesSelect.value = "10";
    await maxUsesSelect.dispatchEvent("change", { type: "change" });

    assert.equal(calls.length, 2);
    const call1 = calls[1];
    assert.ok(call1);
    assert.equal(call1.cmd === "create_invite" ? call1.args.maxUses : -1, 10);

    // Включение временного членства
    tempCheckbox.checked = true;
    await tempCheckbox.dispatchEvent("change", { type: "change" });

    assert.equal(calls.length, 3);
    const call2 = calls[2];
    assert.ok(call2);
    assert.equal(call2.cmd === "create_invite" ? call2.args.temporary : false, true);
  });

  it("14: #invite-modal закрывается по кнопке закрытия и по Escape", async () => {
    const { doc, controller } = createTestEnv();
    const modal = doc.getElementById("invite-modal");
    assert.ok(modal);

    await controller.openInviteModal();
    assert.equal(modal.hasAttribute("hidden"), false);

    const closeBtn = doc.getElementById("invite-modal-close");
    assert.ok(closeBtn);
    closeBtn.click();
    assert.equal(modal.hasAttribute("hidden"), true, "кнопка закрытия скрывает модалку");

    await controller.openInviteModal();
    assert.equal(modal.hasAttribute("hidden"), false);
    modal.dispatchEvent("keydown", { key: "Escape" });
    assert.equal(modal.hasAttribute("hidden"), true, "Escape скрывает модалку");
  });

  it("15: #leave-guild-modal подтверждение выхода вызывает leave_guild и закрывает модалку", async () => {
    const { doc, controller, calls } = createTestEnv();
    const modal = doc.getElementById("leave-guild-modal");
    const confirmBtn = doc.getElementById("leave-guild-confirm-btn");
    assert.ok(modal);
    assert.ok(confirmBtn);

    controller.openLeaveModal({ guildId: "guild-leave-1" });
    assert.equal(modal.hasAttribute("hidden"), false);

    confirmBtn.click();
    await new Promise((resolve) => setTimeout(resolve, 10));
    assert.equal(modal.hasAttribute("hidden"), true, "модалка закрылась после успешного выхода");

    const leaveCall = calls.find((c) => c.cmd === "leave_guild");
    assert.ok(leaveCall, "команда leave_guild должна быть вызвана");
    assert.equal(leaveCall.args.guildId, "guild-leave-1");
    assert.equal(leaveCall.args.lurking, false);
  });

  it("16: #leave-guild-modal отмена закрывает модалку без вызова leave_guild", () => {
    const { doc, controller, calls } = createTestEnv();
    const modal = doc.getElementById("leave-guild-modal");
    const cancelBtn = doc.getElementById("leave-guild-cancel-btn");
    assert.ok(modal);
    assert.ok(cancelBtn);

    controller.openLeaveModal({ guildId: "guild-stay" });
    assert.equal(modal.hasAttribute("hidden"), false);

    cancelBtn.click();
    assert.equal(modal.hasAttribute("hidden"), true, "отмена закрывает модалку");

    const leaveCall = calls.find((c) => c.cmd === "leave_guild");
    assert.equal(leaveCall, undefined, "leave_guild не должен вызываться при отмене");
  });

  it("17: #join-guild-modal проверяет код, вызывает resolve_invite и accept_invite", async () => {
    const { doc, controller, calls } = createTestEnv();
    const modal = doc.getElementById("join-guild-modal");
    const input = /** @type {any} */ (doc.getElementById("join-guild-input"));
    const submitBtn = doc.getElementById("join-guild-submit-btn");
    const errorEl = doc.getElementById("join-guild-error");

    assert.ok(modal);
    assert.ok(input);
    assert.ok(submitBtn);
    assert.ok(errorEl);

    controller.openJoinModal();
    assert.equal(modal.hasAttribute("hidden"), false);

    // Пустой ввод -> ошибка валидации
    input.value = "";
    submitBtn.click();
    await new Promise((resolve) => setTimeout(resolve, 10));
    assert.equal(modal.hasAttribute("hidden"), false, "модалка не закрылась при ошибке");
    assert.ok(errorEl.textContent?.length > 0, "отображается сообщение об ошибке");

    // Невалидные символы -> ошибка валидации
    input.value = "https://discord.gg/invalid@@@$$$";
    submitBtn.click();
    await new Promise((resolve) => setTimeout(resolve, 10));
    assert.equal(modal.hasAttribute("hidden"), false, "модалка не закрылась при невалидных символах");
    assert.equal(errorEl.textContent, "Некорректный код приглашения");

    // Ввод ссылки https://discord.gg/cool-code
    input.value = "https://discord.gg/cool-code";
    submitBtn.click();
    await new Promise((resolve) => setTimeout(resolve, 10));

    assert.equal(modal.hasAttribute("hidden"), true, "модалка закрылась после успешного присоединения");

    const resolveCall = calls.find((c) => c.cmd === "resolve_invite");
    assert.ok(resolveCall, "resolve_invite должен быть вызван");
    assert.equal(resolveCall.args.code, "cool-code");

    const acceptCall = calls.find((c) => c.cmd === "accept_invite");
    assert.ok(acceptCall, "accept_invite должен быть вызван");
    assert.equal(acceptCall.args.code, "cool-code");
  });

  it("18: controller.destroy() снимает слушатели и очищает состояние", () => {
    const { doc, controller } = createTestEnv();
    const menu = doc.getElementById("guild-context-menu");
    const inviteModal = doc.getElementById("invite-modal");
    const leaveModal = doc.getElementById("leave-guild-modal");
    const joinModal = doc.getElementById("join-guild-modal");

    controller.openContextMenu();
    assert.equal(menu?.hasAttribute("hidden"), false);

    controller.destroy();

    assert.equal(menu?.hasAttribute("hidden"), true);
    assert.equal(inviteModal?.hasAttribute("hidden"), true);
    assert.equal(leaveModal?.hasAttribute("hidden"), true);
    assert.equal(joinModal?.hasAttribute("hidden"), true);
  });
});
