// @ts-check
/**
 * task-155 (FE-P1-03): расчёт окон подписок гильдии OP 37 [0, 99] при прокрутке
 * и группировка по ролям в rust/gui-spike/ui/features/members.js.
 *
 * Требования (Strict TDD):
 * 1. computeSubscriptionRanges(scrollTop, viewportHeight, rowHeight=44):
 *    - всегда включает первый чанк [0, 99];
 *    - вычисляет видимые 100-элементные чанки [start, start+99] (кратные 100);
 *    - максимум 3 чанка на канал;
 *    - дедуплицирует совпадающие окна.
 * 2. createGuildSubscriptionsTracker({ onSubscribe, debounceMs }):
 *    - при смене канала или прокрутке #members-sidebar отправляет структуру OP 37:
 *      { subscriptions: { [guildId]: { typing: true, threads: true, activities: true, channels: { [channelId]: ranges } } } }
 *    - не спамит повторными вызовами при неизменных диапазонах.
 * 3. Группировка участников в renderMembersList / groupMembersByRoleOrStatus:
 *    - поддержка hoisted-групп из GUILD_MEMBER_LIST_UPDATE (groups: [{ id, name, count }]);
 *    - поддержка цветов ролей (roleColor);
 *    - обратная совместимость со стандартными группами статусов (online/idle/dnd/offline).
 */

import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { ShimDocument } from "../support/gui-dom-harness.mjs";
import {
  computeSubscriptionRanges,
  createGuildSubscriptionsTracker,
  groupMembersByRoleOrStatus,
  renderMembersList,
} from "../../rust/gui-spike/ui/features/members.js";

if (typeof globalThis.document === "undefined") {
  // @ts-ignore
  globalThis.document = new ShimDocument();
}

describe("computeSubscriptionRanges (OP 37 windows)", () => {
  it("всегда включает первый чанк [0, 99] при scrollTop=0 и дедуплицирует его", () => {
    // Вьюпорт 600px при высоте строки 44px вмещает ~14 строк (0..13).
    // Все они лежат в чанке 0. Результат должен содержать ровно один чанк [0, 99].
    const ranges = computeSubscriptionRanges(0, 600, 44);
    assert.deepEqual(ranges, [[0, 99]]);
  });

  it("использует rowHeight=44 по умолчанию", () => {
    const rangesDefault = computeSubscriptionRanges(0, 600);
    const rangesExplicit = computeSubscriptionRanges(0, 600, 44);
    assert.deepEqual(rangesDefault, rangesExplicit);
    assert.deepEqual(rangesDefault, [[0, 99]]);
  });

  it("корректно обрабатывает отрицательный scrollTop и нулевую высоту", () => {
    const rangesNegative = computeSubscriptionRanges(-100, 600, 44);
    assert.deepEqual(rangesNegative, [[0, 99]]);

    const rangesZeroHeight = computeSubscriptionRanges(0, 0, 44);
    assert.deepEqual(rangesZeroHeight, [[0, 99]]);
  });

  it("включает первый чанк [0, 99] и вычисляет видимый 100-элементный чанк при прокрутке вниз", () => {
    // scrollTop = 4400, viewportHeight = 440, rowHeight = 44 -> строки 100..109.
    // Они лежат в чанке 1 ([100, 199]).
    // Первый чанк [0, 99] обязательно присутствует.
    const ranges = computeSubscriptionRanges(4400, 440, 44);
    assert.deepEqual(ranges, [
      [0, 99],
      [100, 199],
    ]);
  });

  it("вычисляет видимые чанки на стыке границ чанков", () => {
    // Строки 190..210 (scrollTop = 190 * 44 = 8360, viewportHeight = 21 * 44 = 924).
    // Пересекает чанк 1 ([100, 199]) и чанк 2 ([200, 299]).
    // Вместе с чанком [0, 99] получается 3 чанка.
    const ranges = computeSubscriptionRanges(8360, 924, 44);
    assert.deepEqual(ranges, [
      [0, 99],
      [100, 199],
      [200, 299],
    ]);
  });

  it("ограничивает количество чанков максимум 3 на канал", () => {
    // Большой вьюпорт охватывает строки 500..900 (чанки 5, 6, 7, 8, 9).
    // Первый чанк [0, 99] обязательно должен быть.
    // Всего не должно быть больше 3 чанков!
    const ranges = computeSubscriptionRanges(22000, 20000, 44);
    assert.equal(ranges.length, 3);
    assert.deepEqual(ranges[0], [0, 99]);
    assert.deepEqual(ranges[1], [500, 599]);
    assert.deepEqual(ranges[2], [600, 699]);
  });

  it("гарантирует, что все чанки кратны 100 и имеют вид [start, start + 99]", () => {
    const ranges = computeSubscriptionRanges(13200, 1000, 44);
    for (const [start, end] of ranges) {
      assert.equal(start % 100, 0, `start ${start} должен быть кратен 100`);
      assert.equal(end, start + 99, `end ${end} должен быть равен start + 99`);
    }
  });
});

describe("createGuildSubscriptionsTracker (OP 37 tracking & deduplication)", () => {
  it("отправляет валидный OP 37 payload при смене канала", () => {
    /** @type {any[]} */
    const payloads = [];
    const tracker = createGuildSubscriptionsTracker({
      onSubscribe: (p) => payloads.push(p),
      debounceMs: 0,
    });

    tracker.switchChannel("guild-1", "channel-10");

    assert.equal(payloads.length, 1);
    assert.deepEqual(payloads[0], {
      subscriptions: {
        "guild-1": {
          typing: true,
          threads: true,
          activities: true,
          channels: {
            "channel-10": [[0, 99]],
          },
        },
      },
    });

    tracker.destroy();
  });

  it("не спамит повторными вызовами onSubscribe при неизменных диапазонах", () => {
    /** @type {any[]} */
    const payloads = [];
    const tracker = createGuildSubscriptionsTracker({
      onSubscribe: (p) => payloads.push(p),
      debounceMs: 0,
    });

    tracker.switchChannel("guild-1", "channel-10");
    assert.equal(payloads.length, 1);

    // Повторный вызов switchChannel на тот же канал с тем же скроллом не должен вызывать onSubscribe
    tracker.switchChannel("guild-1", "channel-10");
    assert.equal(payloads.length, 1, "повторный вызов с теми же параметрами вызвал спам");

    // Смена канала вызывает подписку для нового канала
    tracker.switchChannel("guild-1", "channel-20");
    assert.equal(payloads.length, 2);
    assert.deepEqual(payloads[1]?.subscriptions?.["guild-1"]?.channels?.["channel-20"], [[0, 99]]);

    tracker.destroy();
  });

  it("обрабатывает прокрутку сайдбара с дебаунсом и отсекает повторные диапазоны", async () => {
    /** @type {any[]} */
    const payloads = [];
    /** @type {Record<string, Function[]>} */
    const listeners = {};
    const fakeSidebar = {
      scrollTop: 0,
      clientHeight: 440,
      addEventListener(/** @type {string} */ event, /** @type {Function} */ fn) {
        listeners[event] = listeners[event] || [];
        listeners[event].push(fn);
      },
      removeEventListener(/** @type {string} */ event, /** @type {Function} */ fn) {
        listeners[event] = (listeners[event] || []).filter((l) => l !== fn);
      },
    };

    const tracker = createGuildSubscriptionsTracker({
      sidebar: /** @type {any} */ (fakeSidebar),
      onSubscribe: (p) => payloads.push(p),
      debounceMs: 20,
    });

    tracker.switchChannel("guild-99", "channel-1");
    assert.equal(payloads.length, 1);

    // Небольшой скролл внутри того же чанка (строки 0..10)
    fakeSidebar.scrollTop = 200;
    for (const fn of listeners["scroll"] || []) fn();
    for (const fn of listeners["scroll"] || []) fn();

    // Ждем истечения дебаунса
    await new Promise((resolve) => setTimeout(resolve, 50));

    // Диапазоны не изменились ([0, 99]), payload не должен был отправиться
    assert.equal(payloads.length, 1, "скролл без смены диапазонов отправил повторный кадр");

    // Скролл вниз на чанк 1 (строки 100..109)
    fakeSidebar.scrollTop = 4400;
    for (const fn of listeners["scroll"] || []) fn();

    await new Promise((resolve) => setTimeout(resolve, 50));

    // Теперь диапазон изменился -> должен быть 2-й payload
    assert.equal(payloads.length, 2);
    assert.deepEqual(payloads[1]?.subscriptions?.["guild-99"]?.channels?.["channel-1"], [
      [0, 99],
      [100, 199],
    ]);

    tracker.destroy();
  });
});

describe("groupMembersByRoleOrStatus & renderMembersList", () => {
  it("поддерживает hoisted-группы из GUILD_MEMBER_LIST_UPDATE с точным сохранением count", () => {
    const members = [
      { userId: "u1", name: "AdminUser", roles: ["role-admin"], status: "online" },
      { userId: "u2", name: "ModUser", roles: ["role-mod"], status: "idle" },
      { userId: "u3", name: "RegularUser", roles: [], status: "online" },
      { userId: "u4", name: "OfflineUser", roles: [], status: "offline" },
    ];

    const groupsSpec = [
      { id: "role-admin", name: "Администраторы", count: 1 },
      { id: "role-mod", name: "Модераторы", count: 1 },
      { id: "online", name: "В сети", count: 100 },
      { id: "offline", name: "Не в сети", count: 50 },
    ];

    const grouped = groupMembersByRoleOrStatus(members, { groups: groupsSpec });
    assert.equal(grouped.length, 4);

    assert.equal(grouped[0]?.id, "role-admin");
    assert.equal(grouped[0]?.name, "Администраторы");
    assert.equal(grouped[0]?.count, 1);
    assert.equal(grouped[0]?.members.length, 1);
    assert.equal(grouped[0]?.members[0]?.userId, "u1");

    assert.equal(grouped[1]?.id, "role-mod");
    assert.equal(grouped[1]?.name, "Модераторы");
    assert.equal(grouped[1]?.count, 1);
    assert.equal(grouped[1]?.members[0]?.userId, "u2");

    assert.equal(grouped[2]?.id, "online");
    assert.equal(grouped[2]?.count, 100, "count из шлюза должен сохраняться независимо от загруженных членов");
    assert.equal(grouped[2]?.members[0]?.userId, "u3");

    assert.equal(grouped[3]?.id, "offline");
    assert.equal(grouped[3]?.count, 50);
    assert.equal(grouped[3]?.members[0]?.userId, "u4");
  });

  it("поддерживает числовые и hex roleColor для групп и участников", () => {
    const members = [
      {
        userId: "u1",
        name: "ColoredAdmin",
        roles: ["role-red"],
        roleColor: 16711680, // 0xFF0000
        status: "online",
      },
      {
        userId: "u2",
        name: "HexMod",
        roles: ["role-blue"],
        roleColor: "#0000ff",
        status: "online",
      },
    ];

    const groupsSpec = [
      { id: "role-red", name: "Красные", count: 1, roleColor: 16711680 },
      { id: "role-blue", name: "Синие", count: 1, roleColor: "#0000ff" },
    ];

    const grouped = groupMembersByRoleOrStatus(members, { groups: groupsSpec });
    assert.equal(grouped[0]?.roleColor, "#ff0000");
    assert.equal(grouped[1]?.roleColor, "#0000ff");
    assert.equal(grouped[0]?.members[0]?.roleColor, "#ff0000");
    assert.equal(grouped[1]?.members[0]?.roleColor, "#0000ff");
  });

  it("сохраняет обратную совместимость со стандартными группами статусов, если groups не переданы", () => {
    const members = [
      { userId: "u1", name: "UserA", status: "online" },
      { userId: "u2", name: "UserB", status: "idle" },
      { userId: "u3", name: "UserC", status: "dnd" },
      { userId: "u4", name: "UserD", status: "offline" },
    ];

    const grouped = groupMembersByRoleOrStatus(members);
    assert.ok(grouped.length >= 2, "должны быть сформированы группы статусов");

    const ids = grouped.map((g) => g.id);
    assert.ok(ids.includes("online") || ids.includes("status-online"));
    assert.ok(ids.includes("offline") || ids.includes("status-offline"));
  });

  it("renderMembersList рендерит группы с заголовками data-height=42, счетчиками и раскраской roleColor", () => {
    const container = document.createElement("div");

    const members = [
      {
        userId: "u1",
        name: "Admin Alice",
        roles: ["role-lead"],
        roleColor: 16711680, // red
        status: "online",
      },
      {
        userId: "u2",
        name: "Member Bob",
        roles: [],
        status: "online",
      },
    ];

    const groupsSpec = [
      { id: "role-lead", name: "Лидеры", count: 1, roleColor: 16711680 },
      { id: "online", name: "В сети", count: 1 },
    ];

    renderMembersList(container, members, { groups: groupsSpec });

    assert.equal(container.children.length, 2, "должно быть создано 2 DOM-узла групп");

    const group0 = /** @type {import("../support/gui-dom-harness.mjs").ShimElement} */ (/** @type {unknown} */ (container.children[0]));
    assert.ok(group0.classList.contains("member-group"), "узел группы должен иметь класс member-group");

    const title0 = /** @type {import("../support/gui-dom-harness.mjs").ShimElement | null} */ (/** @type {unknown} */ (group0.querySelector(".member-group-title")));
    assert.ok(title0, "заголовок группы должен иметь класс member-group-title");
    assert.equal(title0.getAttribute("data-height"), "42", "заголовок группы должен иметь data-height=42");
    assert.ok(title0.textContent.includes("Лидеры — 1"), "заголовок должен содержать название и счетчик");

    const member0 = /** @type {import("../support/gui-dom-harness.mjs").ShimElement | null} */ (/** @type {unknown} */ (group0.querySelector(".member")));
    assert.ok(member0, "строка участника должна иметь класс member");
    assert.ok(
      member0.style.color === "rgb(255, 0, 0)" ||
      member0.style.color === "#ff0000" ||
      member0.getAttribute("data-role-color") === "#ff0000",
      "цвет роли должен быть применен к участнику",
    );
  });
});
