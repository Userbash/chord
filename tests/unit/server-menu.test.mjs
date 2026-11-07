// @ts-check
// RW-40 / G-05-REST: меню сервера — доводка 8 пунктов и дефектов D1–D9.
// Контракт: waves/RW-40/DESIGN/G05-REST-SPEC.md (rev. 4) §2.3, §3.9, §3.10, §4,
// §7 (T-05-03, 05, 06, 07, 08, 09, 10, 12, 13, 14, 16).
//
// Гоняем настоящий `ui/app.js` в DOM-шиме (`tests/support/gui-dom-harness.mjs`).
// Сети нет: `leave_guild` — фейк через `setInvokeHandler`; неизвестная IPC-команда
// реджектится харнессом, поэтому «тихий» поход в IPC виден как падение.
// Ограничения шима (§7.4): layout не считается, события не всплывают, CSS не
// применяется — геометрия проверяется чтением CSS как текста (T-05-09).
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { loadApp } from "../support/gui-dom-harness.mjs";

/** @typedef {import("../support/gui-dom-harness.mjs").GuiDom} GuiDom */
/** @typedef {import("../support/gui-dom-harness.mjs").ShimElement} ShimElement */

// D1-guild-menu-items.json: порядок и тексты 10 пунктов (дословно).
const MENU_LABELS = [
  "Пометить как прочитанное",
  "Пригласить на сервер",
  "Заглушить сервер",
  "Параметры уведомлений",
  "Скрыть заглушённые каналы",
  "Настройки конфиденциальности",
  "Редактировать личный профиль сервера",
  "Переместить...",
  "Покинуть сервер",
  "Копировать ID сервера",
];

// §2.3 п.5: машиночитаемые причины disabled и их приоритет.
const REASONS = ["not-implemented", "capability-unknown", "owner"];
const OWNER_TEXT = "Владелец сервера";

const uiDir = fileURLToPath(new URL("../../rust/gui-spike/ui/", import.meta.url));
const cssText = readFileSync(`${uiDir}components/server-menu.css`, "utf8");
const tokensText = readFileSync(`${uiDir}tokens.css`, "utf8");

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
 * @param {string} id
 * @returns {ShimElement}
 */
function byId(app, id) {
  const node = app.document.getElementById(id);
  assert.ok(node, `нет #${id}`);
  return node;
}

/** @param {GuiDom} app @param {string} selector @returns {ShimElement[]} */
function all(app, selector) {
  return app.document.querySelectorAll(selector);
}

/** @param {GuiDom} app @param {string} selector @returns {ShimElement} */
function firstNode(app, selector) {
  const node = app.document.querySelector(selector);
  assert.ok(node, `нет ${selector}`);
  return node;
}

/** @param {GuiDom} app @param {string} name @returns {ShimElement} */
function action(app, name) {
  const node = app.document.querySelector(`[data-action="${name}"]`);
  assert.ok(node, `нет [data-action=${name}]`);
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

/** @param {GuiDom} app @returns {ShimElement[]} */
function menuItems(app) {
  return all(app, '#server-menu [role="menuitem"], #server-menu [role="menuitemcheckbox"]');
}

/** @param {GuiDom} app @returns {ShimElement} */
function openMenu(app) {
  action(app, "guild-menu").click();
  return byId(app, "server-menu");
}

/**
 * Переоткрыть закрытое меню: клик по открытому opener — это toggle, поэтому
 * сначала закрываем (клик по body), затем открываем заново.
 * @param {GuiDom} app
 * @returns {ShimElement}
 */
function reopenMenu(app) {
  const menu = byId(app, "server-menu");
  if (menu.getAttribute("hidden") === null) app.document.body.click();
  return openMenu(app);
}

/**
 * Пункт по дословному label (порядок и текст — оракул D1).
 * @param {GuiDom} app
 * @param {string} label
 * @returns {ShimElement}
 */
function menuItem(app, label) {
  const item = menuItems(app).find(
    (node) => node.querySelector(".menu-item-label")?.textContent === label,
  );
  assert.ok(item, `нет пункта «${label}»`);
  return item;
}

/**
 * Плитка гильдии: в шиме `dataset.guild` не виден селектору `[data-guild="…"]`.
 * @param {GuiDom} app
 * @param {string} id
 * @returns {ShimElement | null}
 */
function guildTile(app, id) {
  return all(app, ".guild-item").find((node) => node.dataset.guild === id) ?? null;
}

/**
 * Подмена `navigator` на время теста: clipboard — только инъекцией (§3.10).
 * @param {unknown} value
 * @param {() => Promise<void>} body
 */
async function withNavigator(value, body) {
  const descriptor = Object.getOwnPropertyDescriptor(globalThis, "navigator");
  Object.defineProperty(globalThis, "navigator", { value, configurable: true, writable: true });
  try {
    await body();
  } finally {
    if (descriptor) Object.defineProperty(globalThis, "navigator", descriptor);
    else delete /** @type {any} */ (globalThis).navigator;
  }
}

/**
 * Открыть confirm выхода и подтвердить с заданной ошибкой/успехом.
 * @param {GuiDom} app
 * @param {{ kind?: string, status?: number, retry_after_ms?: number, retryAfterMs?: number }} failure
 */
async function confirmLeaveWith(app, failure) {
  app.setInvokeHandler((cmd) => {
    if (cmd !== "leave_guild") return undefined;
    if (failure) throw failure;
    return null;
  });
  openMenu(app);
  firstNode(app, '#server-menu [data-action="guild-leave"]').click();
  byId(app, "leave-confirm").click();
  await app.settle(3);
}

/* ========================================================================== */
describe("G-05 D1: disabled-пункт — no-op (ни статуса, ни закрытия, ни IPC)", () => {
  it("клик по каждому disabled-пункту ничего не меняет", async () => {
    await withApp((app) => {
      const menu = openMenu(app);
      const status = byId(app, "server-status");
      const before = app.invokeCalls.length;
      const disabled = menuItems(app).filter(
        (node) => node.getAttribute("aria-disabled") === "true",
      );
      assert.equal(disabled.length, 8, `disabled-пунктов ${disabled.length}, а не 8`);

      for (const item of disabled) {
        const statusBefore = status.textContent;
        item.click();
        assert.equal(status.textContent, statusBefore, "disabled-пункт изменил #server-status");
        assert.equal(menu.getAttribute("hidden"), null, "disabled-пункт закрыл меню");
        assert.notEqual(
          byId(app, "leave-server-dialog").getAttribute("hidden"),
          null,
          "disabled-пункт открыл диалог выхода",
        );
      }
      assert.equal(app.invokeCalls.length, before, "disabled-пункт сходил в IPC");
      assert.deepEqual(app.consoleErrors, [], "disabled-пункт дал console.error");
      assert.equal(all(app, ".error-state").length, 0, "disabled-пункт показал оверлей ошибки");
    });
  });

  it("Enter/Space на disabled-пункте — no-op (guard в едином предикате)", async () => {
    await withApp((app) => {
      const menu = openMenu(app);
      const invite = menuItem(app, "Пригласить на сервер");
      const status = byId(app, "server-status");
      app.document.activeElement = invite;
      for (const name of ["Enter", " "]) {
        const event = menu.dispatchEvent("keydown", { key: name, target: menu });
        // disabled-пункт не должен присваивать клавишу активации: иначе он
        // «съедает» Enter/Space и полагается только на guard клика.
        assert.equal(event.prevented, false, `disabled invite перехватил ${name}`);
      }
      assert.equal(status.textContent, "", "disabled invite ответил статусом");
      assert.equal(menu.getAttribute("hidden"), null, "disabled invite закрыл меню");
      assert.deepEqual(app.consoleErrors, [], "disabled invite дал console.error");
    });
  });

  it("guard диспетчера гасит disabled data-action вне меню", async () => {
    await withApp((app) => {
      // Целевой путь P1: диспетчер кликов — единственная точка проверки
      // aria-disabled. Берём data-action без собственного guard'а, чтобы
      // снятие диспетчерской проверки было видно тесту напрямую.
      const toggle = action(app, "toggle-tree");
      const tree = byId(app, "channel-tree");
      const expandedBefore = toggle.getAttribute("aria-expanded");
      toggle.setAttribute("aria-disabled", "true");
      toggle.click();
      assert.equal(
        toggle.getAttribute("aria-expanded"),
        expandedBefore,
        "disabled data-action выполнил обработчик — guard диспетчера не сработал",
      );
      assert.equal(tree.style.display, "", "disabled data-action изменил дерево каналов");
      assert.deepEqual(app.consoleErrors, [], "disabled data-action дал console.error");
    });
  });

  it("capability=true не включает invite до G-06: disabled + not-implemented", async () => {
    await withApp((app) => {
      const menu = byId(app, "server-menu");
      menu.setAttribute("data-capability-invite", "true");
      openMenu(app);
      const invite = menuItem(app, "Пригласить на сервер");
      assert.equal(invite.getAttribute("aria-disabled"), "true", "invite включён без диалога G-06");
      assert.equal(
        invite.getAttribute("data-disabled-reason"),
        "not-implemented",
        "у invite с capability=true неверная причина",
      );
      invite.click();
      assert.equal(byId(app, "server-status").textContent, "", "invite выдал статус-заглушку");
      assert.equal(menu.getAttribute("hidden"), null, "invite закрыл меню без диалога");
    });
  });
});

/* ========================================================================== */
describe("G-05 D2: roving tabindex переезжает вместе с фокусом", () => {
  it("↑/↓/Home/End: tabindex=0 ровно у сфокусированного enabled", async () => {
    await withApp((app) => {
      const menu = openMenu(app);
      /** @param {string} label @returns {void} */
      const check = (label) => {
        const focused = menuItem(app, label);
        assert.equal(app.document.activeElement, focused, `фокус не на «${label}»`);
        for (const node of menuItems(app)) {
          const expected = node === focused ? "0" : "-1";
          assert.equal(
            node.getAttribute("tabindex"),
            expected,
            `tabindex «${node.querySelector(".menu-item-label")?.textContent}» = ${node.getAttribute("tabindex")}`,
          );
        }
      };

      const leave = menuItem(app, "Покинуть сервер");
      const copy = menuItem(app, "Копировать ID сервера");
      assert.equal(leave.getAttribute("tabindex"), "0", "при открытии tabindex не у первого enabled");
      assert.equal(copy.getAttribute("tabindex"), "-1", "у второго enabled tabindex не -1");

      menu.dispatchEvent("keydown", { key: "ArrowDown", target: menu });
      check("Копировать ID сервера");
      menu.dispatchEvent("keydown", { key: "ArrowUp", target: menu });
      check("Покинуть сервер");
      menu.dispatchEvent("keydown", { key: "End", target: menu });
      check("Копировать ID сервера");
      menu.dispatchEvent("keydown", { key: "Home", target: menu });
      check("Покинуть сервер");
    });
  });
});

/* ========================================================================== */
describe("G-05 D3: Esc и Tab не переключают экран", () => {
  it("Esc из открытого меню на экране «Настройки» оставляет settings", async () => {
    await withApp((app) => {
      const settingsButton = all(app, ".rail-btn").find((node) => node.dataset.screen === "settings");
      assert.ok(settingsButton, "нет кнопки настроек");
      settingsButton.click();
      assert.equal(app.document.body.getAttribute("data-active-screen"), "settings");

      const opener = action(app, "guild-menu");
      const menu = openMenu(app);
      const event = key(app, "Escape");
      assert.equal(event.prevented, true, "Esc не предотвращён");
      assert.notEqual(menu.getAttribute("hidden"), null, "Esc не закрыл меню");
      assert.equal(app.document.activeElement, opener, "фокус не вернулся на opener");
      assert.equal(
        app.document.body.getAttribute("data-active-screen"),
        "settings",
        "Esc из меню переключил экран (D3)",
      );
    });
  });

  it("Tab закрывает меню, фокус на opener, default не отменяется", async () => {
    await withApp((app) => {
      const opener = action(app, "guild-menu");
      const menu = openMenu(app);
      const event = key(app, "Tab");
      assert.equal(event.prevented, false, "Tab из меню отменил default (меню не modal)");
      assert.notEqual(menu.getAttribute("hidden"), null, "Tab не закрыл меню");
      assert.equal(app.document.activeElement, opener, "Tab не вернул фокус на opener");
      assert.equal(opener.getAttribute("aria-expanded"), "false", "aria-expanded не сброшен");
    });
  });
});

/* ========================================================================== */
describe("G-05 D3: клик вне не крадёт фокус у фокусируемого контрола", () => {
  it("клик по плитке гильдии закрывает меню, но фокус не на opener", async () => {
    await withApp((app) => {
      const opener = action(app, "guild-menu");
      const menu = openMenu(app);
      const tile = guildTile(app, "2");
      assert.ok(tile, "нет плитки гильдии 2");
      // Шим не всплывает: эмулируем клик по контролу событием на body с target.
      app.document.body.dispatchEvent("click", { target: tile });
      assert.notEqual(menu.getAttribute("hidden"), null, "клик по контролу не закрыл меню");
      assert.notEqual(
        app.document.activeElement,
        opener,
        "клик по фокусируемому контролу перехвачен opener (D3)",
      );
    });
  });

  it("клик по body закрывает меню и возвращает фокус на opener", async () => {
    await withApp((app) => {
      const opener = action(app, "guild-menu");
      const menu = openMenu(app);
      app.document.body.click();
      assert.notEqual(menu.getAttribute("hidden"), null, "клик по body не закрыл меню");
      assert.equal(app.document.activeElement, opener, "клик по body не вернул фокус на opener");
    });
  });
});

/* ========================================================================== */
describe("G-05 D4: копирование ID — успех только из resolved writeText", () => {
  it("resolved writeText → «ID скопирован» тем же id, без утечки id в консоль", async () => {
    await withApp(async (app) => {
      /** @type {string[]} */
      const copied = [];
      const clipboard = {
        /** @param {string} value */
        writeText: async (value) => {
          copied.push(value);
        },
      };
      await withNavigator({ clipboard }, async () => {
        openMenu(app);
        menuItem(app, "Копировать ID сервера").click();
        await app.settle(2);
      });
      assert.deepEqual(copied, ["1"], "writeText получил не id выбранного сервера");
      assert.equal(
        byId(app, "server-status").textContent,
        "ID сервера скопирован",
        "нет успеха после resolved writeText",
      );
      assert.deepEqual(app.consoleErrors, [], "копирование писало в console.error");
    });
  });

  it("reject writeText → ошибка, ложного успеха нет", async () => {
    await withApp(async (app) => {
      const clipboard = { writeText: async () => Promise.reject(new Error("denied")) };
      await withNavigator({ clipboard }, async () => {
        openMenu(app);
        menuItem(app, "Копировать ID сервера").click();
        await app.settle(2);
      });
      const text = byId(app, "server-status").textContent;
      assert.equal(text, "Не удалось скопировать ID", `статус при reject: «${text}»`);
      assert.equal(text.includes("скопирован"), false, "ложный успех при reject");
    });
  });

  it("нет clipboard API → видимая ошибка, ложного успеха нет", async () => {
    await withApp(async (app) => {
      await withNavigator({}, async () => {
        openMenu(app);
        menuItem(app, "Копировать ID сервера").click();
        await app.settle(2);
      });
      const text = byId(app, "server-status").textContent;
      assert.equal(text, "Не удалось скопировать ID", `статус без clipboard: «${text}»`);
      assert.equal(text.includes("скопирован"), false, "ложный успех без clipboard");
    });
  });

  it("пустой/битый id → «ID сервера неизвестен», writeText не вызывается", async () => {
    await withApp(async (app) => {
      const tile = guildTile(app, "1");
      assert.ok(tile, "нет плитки гильдии 1");
      tile.dataset.guild = "";
      let calls = 0;
      const clipboard = { writeText: async () => { calls++; } };
      await withNavigator({ clipboard }, async () => {
        openMenu(app);
        menuItem(app, "Копировать ID сервера").click();
        await app.settle(2);
      });
      const text = byId(app, "server-status").textContent;
      assert.equal(text, "ID сервера неизвестен", `статус при пустом id: «${text}»`);
      assert.equal(calls, 0, "writeText вызван с пустым id");
    });
  });
});

/* ========================================================================== */
describe("G-05 D6: причины disabled машиночитаемы и озвучиваемы", () => {
  it("у каждого disabled-пункта data-disabled-reason + aria-describedby + title", async () => {
    await withApp((app) => {
      openMenu(app);
      const disabled = menuItems(app).filter(
        (node) => node.getAttribute("aria-disabled") === "true",
      );
      assert.equal(disabled.length, 8, "disabled-пунктов не 8");
      for (const item of disabled) {
        const reason = item.getAttribute("data-disabled-reason");
        assert.ok(reason && REASONS.includes(reason), `причина «${reason}» вне §2.3 п.5`);
        const describedBy = item.getAttribute("aria-describedby");
        assert.ok(describedBy, "нет aria-describedby у disabled-пункта");
        const textNode = app.document.getElementById(String(describedBy));
        assert.ok(textNode, `aria-describedby указывает на несуществующий #${describedBy}`);
        assert.ok((textNode.textContent ?? "").length > 0, "узел причины пуст");
        assert.equal(
          item.getAttribute("title"),
          textNode.textContent,
          "title и aria-describedby расходятся",
        );
        assert.ok(
          textNode.classes.has("visually-hidden"),
          "узел причины не скрыт от визуального потока",
        );
      }
    });
  });

  it("приоритет причин: owner > capability-unknown > not-implemented", async () => {
    await withApp((app) => {
      openMenu(app);
      assert.equal(
        menuItem(app, "Пригласить на сервер").getAttribute("data-disabled-reason"),
        "capability-unknown",
        "invite без capability не capability-unknown",
      );
      assert.equal(
        menuItem(app, "Заглушить сервер").getAttribute("data-disabled-reason"),
        "not-implemented",
        "статичная заглушка не not-implemented",
      );

      const tile = guildTile(app, "1");
      assert.ok(tile, "нет плитки гильдии 1");
      tile.setAttribute("data-owner", "true");
      reopenMenu(app);
      const leave = menuItem(app, "Покинуть сервер");
      assert.equal(leave.getAttribute("aria-disabled"), "true", "leave владельца не disabled");
      assert.equal(leave.getAttribute("data-disabled-reason"), "owner", "причина владельца не owner");
      assert.equal(leave.getAttribute("title"), OWNER_TEXT, "текст владельца не «Владелец сервера»");
    });
  });

  it("один polite-статус #server-status и один hidden-узел причины на причину", async () => {
    await withApp((app) => {
      openMenu(app);
      const own = all(app, '#server-status');
      assert.equal(own.length, 1, `узлов #server-status ${own.length}, а не 1`);
      assert.equal(byId(app, "server-status").getAttribute("role"), "status", "нет role=status");
      assert.equal(
        byId(app, "server-status").getAttribute("aria-live"),
        "polite",
        "статус меню не polite",
      );
      for (const reason of REASONS) {
        assert.equal(
          all(app, `#menu-reason-${reason}`).length <= 1,
          true,
          `узел причины ${reason} продублирован`,
        );
      }
    });
  });
});

/* ========================================================================== */
describe("G-05: #4 subtitle и #5 aria-checked — только из проекции", () => {
  it("без данных subtitle не рендерится (hidden), с данными — берётся из проекции", async () => {
    await withApp((app) => {
      const menu = byId(app, "server-menu");
      openMenu(app);
      const item = menuItem(app, "Параметры уведомлений");
      const subtitle = item.querySelector(".menu-item-subtitle");
      assert.ok(subtitle, "нет узла subtitle у #4");
      assert.notEqual(subtitle.getAttribute("hidden"), null, "subtitle отрендерен без данных");

      menu.setAttribute("data-notifications-label", "Все сообщения");
      reopenMenu(app);
      const withData = menuItem(app, "Параметры уведомлений").querySelector(".menu-item-subtitle");
      assert.ok(withData, "нет узла subtitle при наличии проекции");
      assert.equal(withData.getAttribute("hidden"), null, "subtitle из проекции не показан");
      assert.equal(withData.textContent, "Все сообщения", "subtitle не из проекции");
    });
  });

  it("aria-checked #5 из явного data-hide-muted-channels (строка, не truthy)", async () => {
    await withApp((app) => {
      const menu = byId(app, "server-menu");
      openMenu(app);
      const item = menuItem(app, "Скрыть заглушённые каналы");
      assert.equal(item.getAttribute("aria-checked"), "false", "по умолчанию не false");

      menu.setAttribute("data-hide-muted-channels", "false");
      reopenMenu(app);
      assert.equal(
        menuItem(app, "Скрыть заглушённые каналы").getAttribute("aria-checked"),
        "false",
        "строка \"false\" принята за истину",
      );

      menu.setAttribute("data-hide-muted-channels", "true");
      reopenMenu(app);
      assert.equal(
        menuItem(app, "Скрыть заглушённые каналы").getAttribute("aria-checked"),
        "true",
        "aria-checked не следует за проекцией",
      );
    });
  });

  it("визуальные узлы D9: чекбокс #5 и иконка #10 с aria-hidden", async () => {
    await withApp((app) => {
      openMenu(app);
      const checkbox = menuItem(app, "Скрыть заглушённые каналы").querySelector(".menu-item-check");
      assert.ok(checkbox, "у #5 нет визуального чекбокса");
      assert.equal(checkbox.getAttribute("aria-hidden"), "true", "чекбокс не aria-hidden");
      const icon = menuItem(app, "Копировать ID сервера").querySelector(".menu-item-icon");
      assert.ok(icon, "у #10 нет иконки");
      assert.equal(icon.getAttribute("aria-hidden"), "true", "иконка не aria-hidden");

      const separators = all(app, '#server-menu [role="separator"]');
      assert.equal(separators.length, 6, `разделителей ${separators.length}, а не 6`);
    });
  });
});

/* ========================================================================== */
describe("G-05 D7: 429 — обратный отсчёт, disabled, ровно один ручной повтор", () => {
  it("3 с → тик каждую секунду до 0, затем кнопка активна, автоповтора нет", async () => {
    await withApp(async (app) => {
      let attempts = 0;
      app.setInvokeHandler((cmd) => {
        if (cmd !== "leave_guild") return undefined;
        attempts++;
        if (attempts === 1) throw { kind: "rateLimited", retry_after_ms: 3000 };
        return null;
      });
      openMenu(app);
      menuItem(app, "Покинуть сервер").click();
      byId(app, "leave-confirm").click();
      await app.settle(3);

      const error = byId(app, "leave-error");
      const confirm = byId(app, "leave-confirm");
      assert.match(error.textContent, /Повторите через 3 с\./, "нет отсчёта 3 с");
      assert.notEqual(confirm.getAttribute("disabled"), null, "кнопка активна сразу после 429");
      assert.equal(attempts, 1, "429 вызвал автоповтор");

      app.flushTimers();
      assert.match(error.textContent, /Повторите через 2 с\./, "отсчёт не уменьшился до 2 с");
      app.flushTimers();
      assert.match(error.textContent, /Повторите через 1 с\./, "отсчёт не уменьшился до 1 с");
      app.flushTimers();
      assert.match(error.textContent, /Повторите через 0 с\./, "отсчёт не дошёл до 0");
      assert.equal(confirm.getAttribute("disabled"), null, "кнопка не разблокирована после 0");
      assert.equal(attempts, 1, "автоповтор ушёл на нуле отсчёта");

      confirm.click();
      await app.settle(3);
      assert.equal(attempts, 2, "ручной повтор после 0 не ушёл");
      assert.equal(guildTile(app, "1"), null, "успешный повтор не удалил плитку");
    });
  });
});

/* ========================================================================== */
describe("G-05 T-05-14: типизированные ошибки leave без ложного «неизвестно»", () => {
  const KNOWN = [
    { kind: "client", status: 400 },
    { kind: "storage" },
    { kind: "invalidArg" },
    { kind: "missingToken" },
    { kind: "invalidResponse" },
  ];

  it("client/storage/invalidArg/missingToken/invalidResponse — видимый отказ", async () => {
    await withApp(async (app) => {
      for (const failure of KNOWN) {
        await confirmLeaveWith(app, failure);
        const text = byId(app, "leave-error").textContent ?? "";
        assert.ok(text.length > 0, `нет текста для kind=${failure.kind}`);
        assert.equal(
          text.includes("Результат неизвестен"),
          false,
          `kind=${failure.kind} ушёл в «результат неизвестен»`,
        );
        assert.ok(guildTile(app, "1"), `kind=${failure.kind} удалил плитку`);
        assert.equal(
          byId(app, "leave-server-dialog").getAttribute("hidden"),
          null,
          `kind=${failure.kind} закрыл диалог`,
        );
      }
    });
  });

  it("timeout/неизвестный kind — «результат неизвестен», не успех", async () => {
    await withApp(async (app) => {
      await confirmLeaveWith(app, { kind: "timeout" });
      assert.match(
        byId(app, "leave-error").textContent,
        /Результат неизвестен/,
        "timeout не дал «результат неизвестен»",
      );
      assert.ok(guildTile(app, "1"), "timeout удалил плитку");
    });
  });
});

/* ========================================================================== */
describe("G-05 T-05-08: capability invite не выводится из ролей", () => {
  it("false/true/absent → invite всегда disabled, без диалога и IPC", async () => {
    await withApp((app) => {
      const menu = byId(app, "server-menu");
      for (const value of [null, "false", "true"]) {
        if (value === null) menu.removeAttribute("data-capability-invite");
        else menu.setAttribute("data-capability-invite", value);
        reopenMenu(app);
        const invite = menuItem(app, "Пригласить на сервер");
        assert.equal(
          invite.getAttribute("aria-disabled"),
          "true",
          `capability=${value}: invite включён`,
        );
        invite.click();
        assert.equal(
          byId(app, "server-status").textContent,
          "",
          `capability=${value}: invite выдал статус`,
        );
        assert.equal(byId(app, "server-menu").getAttribute("hidden"), null, "invite закрыл меню");
        assert.deepEqual(app.consoleErrors, [], `capability=${value}: ошибка в консоль`);
      }
    });
  });
});

/* ========================================================================== */
describe("G-05 T-05-16: статус меню единственный", () => {
  it("disabled-активация не пишет в #server-status; leave — ровно один статус", async () => {
    await withApp(async (app) => {
      const status = byId(app, "server-status");
      const statusesBefore = all(app, '[role="status"]').length;
      openMenu(app);
      menuItem(app, "Скрыть заглушённые каналы").click();
      menuItem(app, "Пригласить на сервер").click();
      assert.equal(status.textContent, "", "disabled-пункт написал статус");
      assert.equal(
        all(app, '[role="status"]').length,
        statusesBefore,
        "disabled-пункт создал новый status-регион",
      );

      app.setInvokeHandler((cmd) => (cmd === "leave_guild" ? null : undefined));
      menuItem(app, "Покинуть сервер").click();
      byId(app, "leave-confirm").click();
      await app.settle(3);
      assert.equal(status.textContent, "Вы покинули сервер", "статус успеха не единственный");
      assert.equal(
        all(app, '[role="status"]').length,
        statusesBefore,
        "после leave появился второй status-регион",
      );
    });
  });
});

/* ========================================================================== */
describe("G-05 D9/D5: геометрия CSS — статическая проверка контракта", () => {
  /**
   * Тело правила по точному селектору (первое вхождение), без вложенных блоков.
   * @param {string} selector
   * @returns {string}
   */
  function block(selector) {
    const escaped = selector.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
    const match = new RegExp(`${escaped}\\s*\\{([^}]*)\\}`).exec(cssText);
    assert.ok(match, `в CSS нет правила ${selector}`);
    return match[1] ?? "";
  }

  it("меню: ширина/высота с клампом по viewport и прокрутка", () => {
    const menu = block(".server-menu");
    assert.match(menu, /width:\s*min\(288px,\s*calc\(100vw - 16px\)\)/, "нет клампа ширины");
    assert.match(menu, /max-height:\s*min\(480px,\s*calc\(100vh - 16px\)\)/, "нет клампа высоты");
    assert.match(menu, /overflow-y:\s*auto/, "нет прокрутки меню");
    assert.match(menu, /z-index:\s*var\(--z-popover\)/, "меню не на слое --z-popover");
    assert.match(tokensText, /--z-popover:\s*100/, "--z-popover не 100");
  });

  it("разделитель: 8 px по вертикали, 0 по горизонтали (17 между группами)", () => {
    const sep = block(".menu-sep");
    assert.match(sep, /margin-block:\s*var\(--space-4\)/, "вертикальный отступ разделителя не --space-4");
    assert.match(sep, /margin-inline:\s*0/, "разделитель выходит в padding меню");
    assert.match(tokensText, /--space-4:\s*8px/, "--space-4 не 8px");
  });

  it("пункт: 32, двухстрочный 44–46, чекбокс 20×20, иконка 20", () => {
    const item = block(".menu-item");
    assert.match(item, /min-height:\s*var\(--row-menu-item\)/, "пункт не 32 (--row-menu-item)");
    const subtitle = block(".menu-item.has-subtitle");
    assert.match(subtitle, /min-height:\s*44px/, "двухстрочный пункт не 44px");
    const check = block(".menu-item-check");
    assert.match(check, /width:\s*20px/, "чекбокс не 20 по ширине");
    assert.match(check, /height:\s*20px/, "чекбокс не 20 по высоте");
    const icon = block(".menu-item-icon");
    assert.match(icon, /width:\s*20px/, "иконка не 20 по ширине");
    assert.match(icon, /height:\s*20px/, "иконка не 20 по высоте");
  });

  it("D5: диалог и backdrop прокручиваются, ширина карточки не изменилась", () => {
    const dialog = block(".dialog");
    assert.match(dialog, /max-height:\s*calc\(100vh - 48px\)/, "у .dialog нет max-height (D5)");
    assert.match(dialog, /overflow-y:\s*auto/, "у .dialog нет прокрутки (D5)");
    assert.match(
      dialog,
      /width:\s*min\(var\(--modal-w-sm\),\s*calc\(100vw - 48px\)\)/,
      "ширина карточки изменилась",
    );
    const backdrop = block(".dialog-backdrop");
    assert.match(backdrop, /overflow-y:\s*auto/, "backdrop не прокручивается (D5)");
  });
});
