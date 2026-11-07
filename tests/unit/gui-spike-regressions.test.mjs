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
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { loadApp } from "../support/gui-dom-harness.mjs";

/** @typedef {import("../support/gui-dom-harness.mjs").GuiDom} GuiDom */
/** @typedef {import("../support/gui-dom-harness.mjs").ShimElement} ShimElement */

/** READY fixture — тот же артефакт, что у `guild-rail.test.mjs` (7 гильдий). */
const FIXTURE = JSON.parse(
  readFileSync(
    fileURLToPath(new URL("../../fixtures/gateway/by-type/002-READY.json", import.meta.url)),
    "utf8",
  ),
);
/** @type {Array<{ id: string, name: string }>} */
const GUILDS = (FIXTURE?.d?.guilds ?? []).map((/** @type {any} */ guild) => ({
  id: String(guild.id),
  name: String(guild?.properties?.name ?? ""),
}));

/** @param {Array<{ id: string, name: string }>} [guilds] */
function readyPayload(guilds = GUILDS) {
  return {
    user: { id: "42", username: "tester", globalName: "Тестер", avatar: null, email: null, bio: null, premium: false, mfaEnabled: false },
    guilds: guilds.map((guild) => ({ id: guild.id, name: guild.name, icon: null, memberCount: 3, channelCount: 2 })),
    guildCount: guilds.length,
    privateChannelCount: 0,
    relationshipCount: 0,
    readStateCount: 0,
    session: null,
  };
}

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

// C-07 (MERGE-READINESS-05 §1): плитка рейла — `data-guild-id` (G-01b), legacy
// `data-guild` удалён. Свитчер обязан читать новый атрибут: иначе цель пустая и
// выбор уезжает на первую плитку.
describe("C-07: QuickSwitcher находит гильдии по data-guild-id", () => {
  it("выбор гильдии в switcher не зависит от legacy data-guild", async () => {
    const app = await loadApp({
      invokeHandler: (cmd) => {
        if (cmd === "auth_status") return { status: "ready", stored: true, user: { id: "42", username: "tester" } };
        if (cmd === "ready") return readyPayload();
        return undefined;
      },
    });
    try {
      assert.equal(all(app, "[data-guild]").length, 0, "в готовом рейле остался legacy data-guild");
      const target = GUILDS[4];
      assert.ok(target, "фикстура READY без пятой гильдии");

      key(app, "k", { ctrlKey: true });
      const input = firstNode(app, "#quick-switcher-input");
      input.value = target.name;
      input.emit("input");

      const list = firstNode(app, "#quick-switcher-results");
      const guildOptions = list.querySelectorAll('[role="option"]').filter(
        (node) => node.getAttribute("data-kind") === "guild",
      );
      assert.equal(guildOptions.length, 1, "свитчер нашёл не одну гильдию по имени");
      const option = guildOptions[0];
      assert.ok(option, "нет опции гильдии");
      assert.ok(option.textContent.includes(target.name), "опция не просит найденную гильдию");

      option.click();

      const tile = all(app, "button.guild-item").find(
        (node) => node.getAttribute("data-guild-id") === target.id,
      );
      assert.ok(tile, "в рейле нет плитки этой гильдии");
      assert.equal(tile.getAttribute("aria-current"), "true", "выбор ушёл не на найденную гильдию");
      assert.equal(tile.classList.contains("is-selected"), true, "найденная гильдия не выделена");
      assert.equal(
        all(app, "button.guild-item").filter((node) => node.getAttribute("aria-current") === "true").length,
        1,
        "выбранных плиток не одна",
      );
      assert.equal(firstNode(app, ".tree-guild").textContent, target.name, "заголовок дерева не сменился на найденную гильдию");
      assert.equal(firstNode(app, "#quick-switcher").getAttribute("hidden"), "", "switcher не закрылся после выбора");
    } finally {
      app.dispose();
    }
  });
});

/* ========================================================================== */
describe("STUB-16 / MB-03: member profile popover", () => {
  /**
   * @param {GuiDom} app
   * @param {string} screen
   */
  function clickRail(app, screen) {
    const button = all(app, ".rail-btn").find((node) => node.dataset.screen === screen);
    assert.ok(button, `нет .rail-btn с data-screen="${screen}"`);
    button.click();
    return button;
  }

  it("прогрессивное создание DOM-узла #user-profile-popover при активации строки", async () => {
    await withApp(async (app) => {
      clickRail(app, "members");
      await app.settle(8);

      assert.equal(
        app.document.querySelector("#user-profile-popover"),
        null,
        "поповер не должен присутствовать в DOM до первого открытия",
      );

      const rows = all(app, "#member-list .member");
      assert.ok(rows.length >= 10, `список участников #member-list должен содержать >= 10 строк (AT-SPI list item), получено ${rows.length}`);
      for (const r of rows) {
        assert.equal(r.getAttribute("role"), "listitem", "строка #member-list .member обязана сохранять role=listitem для AT-SPI");
      }
      const firstRow = rows[0];
      assert.ok(firstRow, "нет первой строки участника");

      // SPEC §4.1: кнопка-триггер внутри строки участника имеет role="button",
      // при этом сама строка .member остаётся role="listitem" для AT-SPI (#screen-members >= 10 list item)
      const triggerBtn = firstRow.querySelector('[role="button"]');
      assert.ok(triggerBtn, "внутри строки участника .member должна быть кнопка-триггер [role=\"button\"] (SPEC §4.1)");
      assert.equal(triggerBtn.getAttribute("role"), "button");
      assert.equal(triggerBtn.getAttribute("aria-haspopup"), "dialog");
      assert.equal(triggerBtn.getAttribute("aria-controls"), "user-profile-popover");
      assert.equal(triggerBtn.getAttribute("aria-expanded"), "false");
      assert.equal(firstRow.getAttribute("aria-haspopup"), "dialog");
      assert.equal(firstRow.getAttribute("aria-controls"), "user-profile-popover");
      assert.equal(firstRow.getAttribute("aria-expanded"), "false");

      triggerBtn.click();
      await app.settle(2);

      const popover = firstNode(app, "#user-profile-popover");
      assert.equal(popover.getAttribute("role"), "dialog", "роль должна быть dialog");
      assert.equal(popover.getAttribute("aria-label"), "Профиль пользователя");
      assert.equal(popover.getAttribute("aria-modal"), "false");
      assert.equal(popover.getAttribute("data-state"), "populated");
      assert.equal(popover.hasAttribute("hidden"), false, "поповер скрыт после клика");

      assert.equal(triggerBtn.getAttribute("aria-haspopup"), "dialog");
      assert.equal(triggerBtn.getAttribute("aria-expanded"), "true");
      assert.equal(triggerBtn.getAttribute("aria-controls"), "user-profile-popover");
      assert.equal(firstRow.getAttribute("aria-haspopup"), "dialog");
      assert.equal(firstRow.getAttribute("aria-expanded"), "true");
      assert.equal(firstRow.getAttribute("aria-controls"), "user-profile-popover");
      assert.equal(app.document.activeElement, popover, "фокус не переведён на поповер");
    });
  });

  it("отображение отображаемого имени, тега/юзернейма, статуса и ролей участника", async () => {
    await withApp(async (app) => {
      clickRail(app, "members");
      await app.settle(8);

      const rows = all(app, "#member-list .member");
      assert.ok(rows.length > 0);
      const row = rows[0];
      assert.ok(row);

      row.click();
      await app.settle(2);

      const popover = firstNode(app, "#user-profile-popover");
      const displayName = firstNode(app, "#user-profile-popover .profile-display-name");
      const tag = firstNode(app, "#user-profile-popover .profile-tag");
      const status = firstNode(app, "#user-profile-popover .profile-custom-status");
      const badges = all(app, "#user-profile-popover .profile-badges [role=\"listitem\"]");
      const closeBtn = firstNode(app, "#user-profile-popover [data-action=\"close-profile\"]");

      assert.ok(displayName.textContent.length > 0, "имя пользователя не должно быть пустым");
      assert.match(tag.textContent, /#|@/, "тег должен содержать разделитель юзернейма");
      assert.ok(status.textContent.length > 0, "статус/активность должна отображаться");
      assert.ok(badges.length > 0, "роли участника должны быть отображены в виде бейджей");
      assert.equal(closeBtn.getAttribute("aria-label"), "Закрыть");
    });
  });

  it("закрытие по кнопке закрытия возвращает фокус на триггер и сбрасывает aria-expanded", async () => {
    await withApp(async (app) => {
      clickRail(app, "members");
      await app.settle(8);

      const rows = all(app, "#member-list .member");
      const row = rows[0];
      assert.ok(row);

      row.click();
      await app.settle(2);

      const popover = firstNode(app, "#user-profile-popover");
      assert.equal(popover.hasAttribute("hidden"), false);
      assert.equal(row.getAttribute("aria-expanded"), "true");

      const closeBtn = firstNode(app, "#user-profile-popover [data-action=\"close-profile\"]");
      closeBtn.click();
      await app.settle(2);

      assert.equal(popover.hasAttribute("hidden"), true, "кнопка закрытия не скрыла поповер");
      assert.equal(row.getAttribute("aria-expanded"), "false", "aria-expanded не сброшен в false");
      assert.equal(app.document.activeElement, row, "фокус не вернулся на элемент-триггер");
    });
  });

  it("закрытие по клавише Escape возвращает фокус на триггер и сбрасывает aria-expanded", async () => {
    await withApp(async (app) => {
      clickRail(app, "members");
      await app.settle(8);

      const rows = all(app, "#member-list .member");
      const row = rows[0];
      assert.ok(row);

      row.click();
      await app.settle(2);

      const popover = firstNode(app, "#user-profile-popover");
      assert.equal(popover.hasAttribute("hidden"), false);
      assert.equal(row.getAttribute("aria-expanded"), "true");

      key(app, "Escape");
      await app.settle(2);

      assert.equal(popover.hasAttribute("hidden"), true, "Escape не скрыл поповер");
      assert.equal(row.getAttribute("aria-expanded"), "false", "aria-expanded не сброшен после Escape");
      assert.equal(app.document.activeElement, row, "Escape не вернул фокус на элемент-триггер");
    });
  });

  it("закрытие по клику вне поповера возвращает фокус на триггер и сбрасывает aria-expanded", async () => {
    await withApp(async (app) => {
      clickRail(app, "members");
      await app.settle(8);

      const rows = all(app, "#member-list .member");
      const row = rows[0];
      assert.ok(row);

      row.click();
      await app.settle(2);

      const popover = firstNode(app, "#user-profile-popover");
      assert.equal(popover.hasAttribute("hidden"), false);
      assert.equal(row.getAttribute("aria-expanded"), "true");

      app.document.body.click();
      await app.settle(2);

      assert.equal(popover.hasAttribute("hidden"), true, "клик вне поповера не скрыл его");
      assert.equal(row.getAttribute("aria-expanded"), "false", "aria-expanded не сброшен после outside click");
      assert.equal(app.document.activeElement, row, "клик вне поповера не вернул фокус на триггер");
    });
  });

  it("клавиатурная активация по Enter и Space открывает поповер", async () => {
    await withApp(async (app) => {
      clickRail(app, "members");
      await app.settle(8);

      const rows = all(app, "#member-list .member");
      const row = rows[0];
      assert.ok(row);

      row.dispatchEvent("keydown", { key: "Enter" });
      await app.settle(2);

      const popover = firstNode(app, "#user-profile-popover");
      assert.equal(popover.hasAttribute("hidden"), false, "Enter не открыл поповер");
      assert.equal(row.getAttribute("aria-expanded"), "true");

      key(app, "Escape");
      await app.settle(2);
      assert.equal(popover.hasAttribute("hidden"), true);

      row.dispatchEvent("keydown", { key: " " });
      await app.settle(2);
      assert.equal(popover.hasAttribute("hidden"), false, "Space не открыл поповер");
      assert.equal(row.getAttribute("aria-expanded"), "true");
    });
  });

  it("состояния поповера: loading, empty, error с avatar placeholder и name skeletons", async () => {
    const { ensureMemberProfilePopover, renderMemberProfileState } = await import(
      "../../rust/gui-spike/ui/features/members.js"
    );

    await withApp(async (app) => {
      const popover = ensureMemberProfilePopover();
      assert.ok(popover);
      assert.equal(popover.id, "user-profile-popover", "id поповера должен быть user-profile-popover");

      // 1. Loading (PINS-THREADS-PROFILE-SPEC.md:263-267)
      renderMemberProfileState(popover, "loading");
      assert.equal(popover.getAttribute("data-state"), "loading");
      assert.equal(popover.getAttribute("aria-busy"), "true");
      const avatarSkeleton = firstNode(app, "#user-profile-popover .profile-avatar-wrap .profile-avatar-skeleton");
      assert.ok(avatarSkeleton, "в loading state отсутствует avatar placeholder");
      const nameSkeletons = all(app, "#user-profile-popover .profile-body .profile-skeleton-name");
      assert.equal(nameSkeletons.length, 2, "в loading state должно быть 2 name skeleton блока");

      // 2. Empty
      renderMemberProfileState(popover, "empty");
      assert.equal(popover.getAttribute("data-state"), "empty");
      assert.equal(popover.getAttribute("aria-busy"), "false");
      assert.match(popover.textContent, /не найден/i);

      // 3. Error
      renderMemberProfileState(popover, "error", { message: "Сервер участников недоступен" });
      assert.equal(popover.getAttribute("data-state"), "error");
      assert.equal(popover.getAttribute("aria-busy"), "false");
      const alert = firstNode(app, "#user-profile-popover [role=\"alert\"]");
      assert.match(alert.textContent, /Сервер участников недоступен/);
      const retry = firstNode(app, "#user-profile-popover [data-action=\"retry-profile\"]");
      assert.ok(retry, "в состоянии ошибки должна быть кнопка повтора");
    });
  });

  it("закрытие открытого поповера при переключении экрана (body[data-active-screen])", async () => {
    await withApp(async (app) => {
      clickRail(app, "members");
      await app.settle(8);

      const rows = all(app, "#member-list .member");
      const row = rows[0];
      assert.ok(row);

      row.click();
      await app.settle(2);

      const popover = firstNode(app, "#user-profile-popover");
      assert.equal(popover.hasAttribute("hidden"), false);
      assert.equal(row.getAttribute("aria-expanded"), "true");

      // Смена body[data-active-screen]
      app.document.body.setAttribute("data-active-screen", "chat");
      await app.settle(2);

      assert.equal(popover.hasAttribute("hidden"), true, "поповер не закрылся при смене data-active-screen");
      assert.equal(row.getAttribute("aria-expanded"), "false", "aria-expanded не сброшен после смены экрана");
    });
  });

  it("закрытие открытого поповера при смене канала", async () => {
    await withApp(async (app) => {
      clickRail(app, "members");
      await app.settle(8);

      const rows = all(app, "#member-list .member");
      const row = rows[0];
      assert.ok(row);

      row.click();
      await app.settle(2);

      const popover = firstNode(app, "#user-profile-popover");
      assert.equal(popover.hasAttribute("hidden"), false);
      assert.equal(row.getAttribute("aria-expanded"), "true");

      // Клик по каналу в дереве каналов
      const channelItem = firstNode(app, ".channel-item");
      assert.ok(channelItem, "в дереве каналов нет .channel-item");
      channelItem.click();
      await app.settle(2);

      assert.equal(popover.hasAttribute("hidden"), true, "поповер не закрылся при смене канала");
      assert.equal(row.getAttribute("aria-expanded"), "false", "aria-expanded не сброшен после смены канала");
    });
  });

  it("стили members.css оформлены строго на токенах (без hex/rgb и без !important)", () => {
    const membersCss = readFileSync(
      fileURLToPath(new URL("../../rust/gui-spike/ui/components/members.css", import.meta.url)),
      "utf8",
    );

    assert.match(membersCss, /#user-profile-popover|\.user-profile-popover/, "нет селектора #user-profile-popover в members.css");
    assert.match(membersCss, /var\(--z-popover\)/, "поповер должен использовать токен --z-popover");
    assert.match(membersCss, /var\(--shadow-popover\)/, "поповер должен использовать токен --shadow-popover");
    assert.match(membersCss, /\.profile-avatar-skeleton/, "нет селектора .profile-avatar-skeleton в members.css");
    assert.match(membersCss, /\.profile-skeleton-name/, "нет селектора .profile-skeleton-name в members.css");
    assert.doesNotMatch(membersCss, /!important/, "запрещено использовать !important");
    // Проверяем, что в стилях профиля нет hex и rgb
    const profileSection = membersCss.slice(membersCss.indexOf(".user-profile-popover") !== -1 ? membersCss.indexOf(".user-profile-popover") : membersCss.indexOf("#user-profile-popover"));
    assert.doesNotMatch(profileSection, /#[0-9a-fA-F]{3,8}\b/, "в стилях профиля найден hex-литерал");
    assert.doesNotMatch(profileSection, /rgba?\(.*?\)/, "в стилях профиля найден rgb/rgba-литерал");
  });
});

