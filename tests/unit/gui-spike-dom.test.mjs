// @ts-check
// RW-38 / W38a: поведенческие тесты GUI-спайка на настоящем `ui/app.js`.
// В отличие от статического контракта (regex по исходникам) здесь app.js реально
// импортируется в минимальный DOM-шим (tests/support/gui-dom-harness.mjs): проверяются
// обработчики, виртуализация, окно данных PageStore и автовызов UI_SNAPSHOT.
//
// Независимый оракул объёмов — DOM и разметка: высота sizer = сумма высот строк,
// индекс строки в тексте связан с её top префиксной суммой высот (D-04:
// 26/48/+22 для сообщений и 42 для участников), а `ui/index.html` задаёт
// обязательные id/классы. Числа берутся из layout.json/контракта, а не из app.js.
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { HTMLInputElement as ShimInputElement, loadApp } from "../support/gui-dom-harness.mjs";

/** @typedef {import("../support/gui-dom-harness.mjs").GuiDom} GuiDom */
/** @typedef {import("../support/gui-dom-harness.mjs").ShimElement} ShimElement */

const uiDir = fileURLToPath(new URL("../../rust/gui-spike/ui/", import.meta.url));
const html = readFileSync(`${uiDir}index.html`, "utf8");

const SCREEN_COUNT = 4;
const CHAT_ROWS = 100_000;
const MEMBER_ROWS = 24_229;
const EMOJI_CATEGORIES = 10;
const SETTINGS_TOGGLES = 38;

// D-04 (REV-2): высота строки сообщения — не константа. Независимый оракул —
// waves/RW-37/RE/layout.json, row_heights_detail:
//   message_cozy_group_start = 48, message_cozy = 26, message_extra_line = 22,
//   row_heights.member = 42.
// Пары «индекс → высота» строятся здесь по правилам демо-данных (не из app.js):
// группа из 4 сообщений (первое — начало), +1 строка у i % 17 === 9,
// ещё +1 у i % 23 === 9 (перенос = +22 каждая).
const ROW_H = Object.freeze({
  CONTINUATION: 26,
  GROUP_START: 48,
  LINE_EXTRA: 22,
  MEMBER: 42,
});
const GROUP_SIZE = 4;

/** @param {number} index @returns {number} строк текста у сообщения */
function oracleLines(index) {
  return 1 + (index % 17 === 9 ? 1 : 0) + (index % 23 === 9 ? 1 : 0);
}

/** @param {number} index @returns {number} высота строки чата, px */
function oracleChatHeight(index) {
  const base = index % GROUP_SIZE === 0 ? ROW_H.GROUP_START : ROW_H.CONTINUATION;
  return base + (oracleLines(index) - 1) * ROW_H.LINE_EXTRA;
}

/** Префиксные суммы высот: оракульный `top(i)` без обращения к коду спайка. */
const CHAT_TOPS = new Uint32Array(CHAT_ROWS + 1);
for (let index = 0; index < CHAT_ROWS; index++) {
  CHAT_TOPS[index + 1] = /** @type {number} */ (CHAT_TOPS[index]) + oracleChatHeight(index);
}
const CHAT_TOTAL_HEIGHT = CHAT_TOPS[CHAT_ROWS];

/** @param {number} index @returns {number} */
function chatTop(index) {
  return CHAT_TOPS[index] ?? 0;
}

/** @param {number} index @returns {string} */
function chatHeightPx(index) {
  return `${oracleChatHeight(index)}px`;
}
// Бюджет окна данных (ADR-1): pageSize*maxPages = 100*5.
const MAX_LOADED_ROWS = 500;
// Верхняя граница DOM-строк: окно экрана (800px) + overscan с запасом.
const MAX_DOM_ROWS = 60;

/** @type {Record<string, string>} */
const TITLES = {
  chat: "Чат · 100 000 сообщений",
  emoji: "Эмодзи-пикер · 10 категорий",
  members: "Участники · 24 229",
  settings: "Настройки · 38 переключателей",
};

/**
 * Каждый тест получает свежий экземпляр модуля `app.js` (свой документ и очереди),
 * поэтому состояние одного кейса не влияет на другой.
 * @param {(app: GuiDom) => void | Promise<void>} body
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
  assert.ok(node, `в шим-DOM нет #${id}`);
  return node;
}

/**
 * @param {GuiDom} app
 * @param {string} selector
 * @returns {ShimElement}
 */
function one(app, selector) {
  const node = app.document.querySelector(selector);
  assert.ok(node, `в шим-DOM нет «${selector}»`);
  return node;
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
 * @param {string} screen
 * @returns {ShimElement}
 */
function clickRail(app, screen) {
  const button = all(app, ".rail-btn").find((node) => node.dataset.screen === screen);
  assert.ok(button, `нет .rail-btn с data-screen="${screen}"`);
  button.click();
  return button;
}

/**
 * Скролл как у пользователя: scrollTop + событие `scroll`, затем кадры rAF.
 * @param {GuiDom} app
 * @param {ShimElement} list
 * @param {number} scrollTop
 * @returns {Promise<void>}
 */
async function scrollList(app, list, scrollTop) {
  list.scrollTop = scrollTop;
  list.emit("scroll");
  await app.settle(6);
}

/**
 * Сквозной индекс строки чата: текст начинается с `#<index>`.
 * @param {ShimElement} row
 * @returns {number}
 */
function chatRowIndex(row) {
  const text = row.querySelector(".text");
  assert.ok(text, "у строки чата нет .text");
  const match = text.textContent.match(/^#(\d+)/);
  assert.ok(match, `не разобрать индекс из «${text.textContent.slice(0, 32)}»`);
  return Number(match[1] ?? NaN);
}

/**
 * Сквозной индекс строки участника: имя вида `member_<index>_<mark>`.
 * @param {ShimElement} row
 * @returns {number}
 */
function memberRowIndex(row) {
  const name = row.children[1];
  assert.ok(name, "у строки участника нет элемента с именем");
  const match = name.textContent.match(/^member_(\d+)_/);
  assert.ok(match, `не разобрать индекс из «${name.textContent}»`);
  return Number(match[1] ?? NaN);
}

/** @param {ShimElement} row @returns {number} */
function rowTop(row) {
  return Number.parseFloat(row.style.top.replace("px", ""));
}

/** @param {ShimElement} list @returns {number} */
function sizerHeight(list) {
  const sizer = list.children[0];
  assert.ok(sizer, "у списка нет sizer-контейнера");
  assert.equal(sizer.style.position, "relative");
  return Number.parseFloat(sizer.style.height.replace("px", ""));
}

describe("GUI spike: настоящий app.js в DOM-шиме", () => {
  it("лимит композера 2000, IPC payload не содержит токен/пароль", async () => {
    await withApp(async (app) => {
      const input = one(app, "#composer-input");
      app.setInvokeHandler(async () => ({ id: "m3", channel_id: "news", content: "x", nonce: "n" }));
      input.value = "x".repeat(2001); input.emit("input"); assert.equal(input.value.length, 2000);
      input.dispatchEvent("keydown", { key: "Enter" }); await Promise.resolve();
      const call = app.callsOf("send_message")[0]; assert.equal(call?.args?.content.length, 2000);
      assert.equal(JSON.stringify(call?.args).includes("password"), false);
      assert.equal(JSON.stringify(call?.args).includes("token"), false);
    });
  });
  it("шим-разметка совпадает с ui/index.html по id, классам и rail-кнопкам", async () => {
    await withApp((app) => {
      /** @type {string[]} */
      const ids = [...html.matchAll(/id="([^"]+)"/g)].map((match) => match[1] ?? "");
      assert.ok(ids.length >= 10, `в index.html найдено только ${ids.length} id`);
      for (const id of ids) {
        assert.ok(app.document.getElementById(id), `шим-DOM не повторяет #${id} из index.html`);
      }

      /** @type {Set<string>} */
      const classes = new Set();
      for (const match of html.matchAll(/class="([^"]+)"/g)) {
        for (const name of (match[1] ?? "").split(/\s+/)) if (name) classes.add(name);
      }
      for (const name of classes) {
        assert.ok(all(app, `.${name}`).length > 0, `шим-DOM не повторяет класс .${name}`);
      }

      /** @type {string[]} */
      const screens = [...html.matchAll(/id="screen-([\w-]+)"/g)].map((match) => match[1] ?? "");
      assert.deepEqual(screens, ["chat", "emoji", "members", "settings"]);
      /** @type {string[]} */
      const railScreens = [...html.matchAll(/data-screen="([^"]+)"/g)].map((match) => match[1] ?? "");
      assert.deepEqual(
        all(app, ".rail-btn").map((node) => node.dataset.screen),
        railScreens,
      );
    });
  });

  // RW-39/NET-D: новый id `#auth-gate` — единственный html-носитель оверлея входа;
  // содержимое гейта собирает features/auth.js (шим не разбирает html).
  it("#auth-gate: новый id из index.html есть в шиме и не считается экраном", async () => {
    await withApp((app) => {
      const gate = byId(app, "auth-gate");
      assert.equal(gate.classList.contains("screen"), false, "#auth-gate не должен быть .screen");
      assert.equal(all(app, ".screen").length, SCREEN_COUNT, "оверлей изменил screenCount");
      assert.equal(gate.getAttribute("role"), "dialog", "#auth-gate без роли dialog");
      assert.equal(gate.getAttribute("aria-modal"), "true", "#auth-gate должен быть модальным");
      assert.equal(
        byId(app, "auth-token").getAttribute("type"),
        "password",
        "поле токена должно быть type=password",
      );
      for (const id of ["auth-reveal", "auth-submit", "auth-restore", "auth-error", "auth-logout"]) {
        byId(app, id);
      }
    });
  });

  it("шапка чата: колокол между ветками и закреплёнными (VW-01, D-17)", async () => {
    await withApp((app) => {
      const region = html.match(/<div class="chat-actions">([\s\S]*?)<\/div>/)?.[1] ?? "";
      assert.ok(region, "в index.html нет .chat-actions");
      /** @type {string[]} */
      const fromHtml = [...region.matchAll(/data-action="([^"]+)"/g)].map((match) => match[1] ?? "");
      assert.deepEqual(
        fromHtml,
        ["threads", "inbox", "pins", "members"],
        "VW-01: в index.html не 4 кнопки в порядке ветки/колокол/закреплённые/участники",
      );
      /** @type {string[]} */
      const fromShim = all(app, ".chat-actions .icon-btn").map(
        (node) => node.getAttribute("data-action") ?? "",
      );
      assert.deepEqual(fromShim, fromHtml, "шим разошёлся с index.html по кнопкам шапки");

      const bellTag = html.match(/<button[^>]*data-action="inbox"[^>]*>/)?.[0] ?? "";
      assert.ok(bellTag, "в index.html нет кнопки колокола");
      assert.match(bellTag, /aria-label="Уведомления"/, "колокол без aria-label «Уведомления»");
      assert.match(bellTag, /aria-disabled="true"/, "колокол не disabled-affordance (Phase 3)");
      assert.match(bellTag, /title="[^"]+"/, "у колокола нет подсказки");
      assert.equal(one(app, '[data-action="inbox"]').getAttribute("aria-disabled"), "true");
    });
  });

  it("подарок в композере: disabled-affordance в index.html и в шиме (P1 REVIEW-W39h)", async () => {
    await withApp((app) => {
      const giftTag = html.match(/<button[^>]*data-action="gift"[^>]*>/)?.[0] ?? "";
      assert.ok(giftTag, "в index.html нет кнопки подарка");
      assert.match(giftTag, /aria-label="Подарок"/, "подарок без aria-label");
      assert.match(giftTag, /aria-disabled="true"/, "подарок не disabled-affordance (Phase 3)");
      assert.match(giftTag, /title="[^"]+"/, "у подарка нет подсказки");
      const fromShim = one(app, '[data-action="gift"]');
      assert.equal(fromShim.getAttribute("aria-disabled"), "true", "шим разошёлся с index.html по подарку");
      assert.ok((fromShim.getAttribute("title") ?? "").length > 0, "в шиме у подарка нет подсказки");
    });
  });

  it("четыре экрана: rail-btn делает себя active, заголовок меняется", async () => {
    await withApp((app) => {
      assert.equal(all(app, ".screen").length, SCREEN_COUNT);
      assert.equal(all(app, ".rail-btn").length, SCREEN_COUNT);

      for (const screen of ["emoji", "members", "settings", "chat"]) {
        const button = clickRail(app, screen);
        assert.equal(button.classList.contains("active"), true, `кнопка ${screen} не active`);
        assert.equal(
          all(app, ".rail-btn.active").length,
          1,
          `после клика по ${screen} active не ровно у одной кнопки`,
        );
        const active = all(app, ".screen.active");
        assert.equal(active.length, 1, `после клика по ${screen} активных экранов ${active.length}`);
        assert.equal(active[0]?.id, `screen-${screen}`);
        assert.equal(one(app, ".screen-title").textContent, TITLES[screen]);
      }
    });
  });

  it("чат: 100 000 строк, sizer = сумма оракульных высот, в DOM десятки строк, а не сотни тысяч", async () => {
    await withApp(async (app) => {
      const list = byId(app, "chat-list");
      assert.equal(sizerHeight(list), CHAT_TOTAL_HEIGHT, "sizer чата не равен сумме высот 26/48/+22");
      assert.equal(list.children.length, 1, "в #chat-list должен быть только sizer");

      // Окно по умолчанию — конец списка (как в реальном клиенте): десятки строк на 800px.
      const bottomRows = all(app, "#chat-list .row");
      assert.ok(bottomRows.length > 0 && bottomRows.length < MAX_DOM_ROWS);
      assert.equal(chatRowIndex(/** @type {ShimElement} */ (bottomRows[bottomRows.length - 1])), CHAT_ROWS - 1);

      await scrollList(app, list, chatTop(500));
      const rows = all(app, "#chat-list .row");
      assert.ok(rows.length >= 10, `в окне ${rows.length} строк, ожидалось ≥10`);
      assert.ok(rows.length < CHAT_ROWS / 1000, `в DOM ${rows.length} строк — виртуализация не работает`);
      assert.ok(rows.length <= MAX_DOM_ROWS, `в DOM ${rows.length} строк > ${MAX_DOM_ROWS}`);
      for (const row of rows) {
        assert.equal(row.dataset.state, "row", "строка окна осталась placeholder после загрузки страницы");
        // D-04: высота — функция индекса (26/48/+22), а не константа 44.
        assert.equal(row.style.height, chatHeightPx(chatRowIndex(row)), `высота строки не по оракулу`);
      }
    });
  });

  it("чат: top строки = префикс оракульных высот (26/48/+22), а не index*44", async () => {
    await withApp(async (app) => {
      const list = byId(app, "chat-list");

      // Литеральные пары «индекс → высота» из оракула RW-37: константа 44 их не пройдёт.
      await scrollList(app, list, 0);
      const topRows = all(app, "#chat-list .row");
      /** @type {Array<[number, number]>} */
      const oraclePairs = [[0, 48], [1, 26], [4, 48], [9, 70], [26, 48], [32, 70]];
      /** @type {Map<number, ShimElement>} */
      const byIndex = new Map(topRows.map((row) => [chatRowIndex(row), row]));
      const visible = oraclePairs.filter(([index]) => byIndex.has(index));
      assert.ok(visible.length >= 4, `в окне видно пар: ${visible.length}, ожидалось ≥4`);
      for (const [index, height] of visible) {
        const row = /** @type {ShimElement} */ (byIndex.get(index));
        assert.equal(oracleChatHeight(index), height, `оракул разошёлся для ${index}`);
        assert.equal(row.style.height, `${height}px`, `строка ${index}: высота не ${height}px (D-04)`);
      }

      await scrollList(app, list, chatTop(5000));
      const rows = all(app, "#chat-list .row");
      assert.ok(rows.length >= 10);
      /** @type {number[]} */
      const indices = [];
      for (const row of rows) {
        const index = chatRowIndex(row);
        indices.push(index);
        assert.equal(rowTop(row), chatTop(index), `top строки ${index} не по префиксу высот`);
        assert.equal(row.style.height, chatHeightPx(index), `высота строки ${index} не по оракулу`);
      }
      const first = Math.min(...indices);
      const last = Math.max(...indices);
      // Окно должно накрывать запрошенный индекс и не разъезжаться больше overscan.
      assert.ok(first <= 5000 && last >= 5000, `окно ${first}..${last} не содержит 5000`);
      // Окно = clientHeight / средняя высота (~32.8) + 2*overscan; максимум DOM — MAX_DOM_ROWS.
      assert.ok(5000 - first <= 8 && last - 5000 <= 40, `окно ${first}..${last} шире ожидаемого`);
      assert.equal(sizerHeight(list), CHAT_TOTAL_HEIGHT);
    });
  });

  it("участники: 24 229 строк, sizer = total*42, окно ≥10 и << total", async () => {
    await withApp(async (app) => {
      clickRail(app, "members");
      await app.settle(6);
      const list = byId(app, "member-list");
      assert.equal(sizerHeight(list), MEMBER_ROWS * ROW_H.MEMBER);

      const rows = all(app, "#member-list .member");
      assert.ok(rows.length >= 10, `в окне ${rows.length} строк участников, ожидалось ≥10`);
      assert.ok(rows.length < MEMBER_ROWS / 100, `в DOM ${rows.length} строк — виртуализация не работает`);
      assert.ok(rows.length <= MAX_DOM_ROWS);
      for (const row of rows) {
        assert.equal(row.dataset.state, "row");
        assert.equal(row.style.height, `${ROW_H.MEMBER}px`, "высота строки участника не 42 (D-04)");
        assert.equal(row.children.length, 3, "строка участника: avatar + имя + роль");
      }
      assert.equal(memberRowIndex(/** @type {ShimElement} */ (rows[0])), 0);
    });
  });

  it("скролл участников: окно меняется, старые строки удалены, DOM не растёт", async () => {
    await withApp(async (app) => {
      clickRail(app, "members");
      await app.settle(6);
      const list = byId(app, "member-list");
      const sizer = /** @type {ShimElement} */ (list.children[0]);
      const beforeRows = all(app, "#member-list .member");
      const beforeSizerChildren = sizer.children.length;
      assert.equal(beforeSizerChildren, beforeRows.length);
      const staleRow = /** @type {ShimElement} */ (beforeRows[0]);
      assert.equal(memberRowIndex(staleRow), 0);

      await scrollList(app, list, 300 * ROW_H.MEMBER);
      const afterRows = all(app, "#member-list .member");
      const indices = afterRows.map(memberRowIndex);
      assert.ok(Math.min(...indices) <= 300 && Math.max(...indices) >= 300, "окно не содержит 300");
      assert.ok(!indices.includes(0), "верхние строки остались в окне после скролла");
      assert.equal(staleRow.parentNode, null, "старая строка не удалена из DOM");
      assert.equal(sizer.children.length, afterRows.length, "в sizer остались отвязанные строки");
      // Окно = clientHeight/42 + 2*overscan ≈ 36 строк; рост окна — не утечка.
      assert.ok(afterRows.length <= MAX_DOM_ROWS);

      // Свип по страницам: DOM не растёт, sizer не копит отвязанные строки.
      for (const index of [1_000, 5_000, 300]) {
        await scrollList(app, list, index * ROW_H.MEMBER);
        const rows = all(app, "#member-list .member");
        assert.equal(sizer.children.length, rows.length, `sizer=${sizer.children.length} != rows=${rows.length}`);
        assert.ok(rows.length <= MAX_DOM_ROWS, `после свипа в DOM ${rows.length} строк`);
      }
    });
  });

  it("скролл чата: окно меняется, старые строки удалены, DOM не растёт", async () => {
    await withApp(async (app) => {
      const list = byId(app, "chat-list");
      const sizer = /** @type {ShimElement} */ (list.children[0]);
      const bottomRows = all(app, "#chat-list .row");
      const staleRow = /** @type {ShimElement} */ (bottomRows[bottomRows.length - 1]);
      assert.equal(chatRowIndex(staleRow), CHAT_ROWS - 1);

      await scrollList(app, list, chatTop(60_000));
      const rows = all(app, "#chat-list .row");
      const indices = rows.map(chatRowIndex);
      assert.ok(Math.min(...indices) <= 60_000 && Math.max(...indices) >= 60_000);
      assert.ok(!indices.includes(CHAT_ROWS - 1), "строка конца списка осталась в окне");
      assert.equal(staleRow.parentNode, null);
      assert.equal(sizer.children.length, rows.length);
      assert.ok(rows.length <= MAX_DOM_ROWS, `в DOM ${rows.length} строк > ${MAX_DOM_ROWS}`);
    });
  });

  it("PageStore: loadedRows ≤500 в UI_SNAPSHOT для чата и участников", async () => {
    await withApp((app) => {
      const { payload } = app.snapshot;
      assert.ok(payload, "UI_SNAPSHOT не опубликован — нечего проверять");
      for (const key of ["chatLoadedRows", "memberLoadedRows"]) {
        const loaded = payload[key];
        assert.equal(Number.isInteger(loaded), true, `${key}=${String(loaded)} не целое`);
        assert.ok(loaded > 0, `${key}=${String(loaded)}: окно данных пустое`);
        assert.ok(loaded <= MAX_LOADED_ROWS, `${key}=${String(loaded)} > ${MAX_LOADED_ROWS}`);
        assert.equal(loaded % 100, 0, `${key} не кратен pageSize=100`);
      }
      assert.ok(payload.chatLoadedRows < payload.chatRows);
      assert.ok(payload.memberLoadedRows < payload.memberRows);
    });
  });

  it("эмодзи: 10 вкладок, клик по второй меняет активную вкладку и сетку", async () => {
    await withApp((app) => {
      clickRail(app, "emoji");
      const tabs = all(app, ".tab");
      assert.equal(tabs.length, EMOJI_CATEGORIES);

      // Автоснимок при загрузке оставляет активной вторую вкладку — начинаем с первой.
      const firstTab = /** @type {ShimElement} */ (tabs[0]);
      firstTab.click();
      assert.equal(all(app, ".tab.active").length, 1, "активной должна остаться ровно одна вкладка");
      assert.equal(firstTab.classList.contains("active"), true);

      const cellsBefore = all(app, ".emoji-cell");
      assert.ok(cellsBefore.length > 0 && cellsBefore.length <= 120);
      const firstBefore = cellsBefore[0]?.textContent;

      const second = /** @type {ShimElement} */ (tabs[1]);
      second.click();
      assert.equal(all(app, ".tab.active").length, 1, "активной должна остаться ровно одна вкладка");
      assert.equal(second.classList.contains("active"), true);

      const cellsAfter = all(app, ".emoji-cell");
      assert.ok(cellsAfter.length > 0 && cellsAfter.length <= 120);
      assert.notEqual(cellsAfter[0]?.textContent, firstBefore, "сетка не перерисовалась на другую категорию");
      for (const cell of cellsAfter) assert.equal(cell.title, cell.textContent);
    });
  });

  it("эмодзи-сетка: listbox с option-ячейками и доступными именами (P1-2)", async () => {
    // Роль контейнера задаётся статической разметкой (шим строит DOM из app.js/теста).
    const gridTag = html.match(/<div[^>]*id="emoji-grid"[^>]*>/)?.[0] ?? "";
    assert.match(gridTag, /role="listbox"/, "#emoji-grid должен иметь role=listbox");
    assert.match(gridTag, /aria-label="[^"]+"/, "у #emoji-grid нет непустого aria-label");

    await withApp((app) => {
      clickRail(app, "emoji");
      const cells = all(app, ".emoji-cell");
      assert.ok(cells.length >= 10, `ячеек эмодзи ${cells.length} < 10`);
      for (const cell of cells) {
        assert.equal(
          cell.getAttribute("role"),
          "option",
          `ячейка «${cell.textContent}» должна иметь role=option`,
        );
        assert.ok(
          (cell.getAttribute("aria-label") ?? "").trim().length > 0,
          `у ячейки «${cell.textContent}» пустой aria-label`,
        );
        assert.ok(cell.textContent.trim().length > 0, "ячейка эмодзи без текста");
      }
    });
  });

  it("эмодзи-поиск фильтрует сетку, повторные рендеры не дублируют ячейки", async () => {
    await withApp((app) => {
      clickRail(app, "emoji");
      const search = /** @type {ShimInputElement} */ (byId(app, "emoji-search"));
      /** @type {ShimElement} */ (all(app, ".tab")[0]).click();
      const fullCount = all(app, ".emoji-cell").length;
      assert.ok(fullCount > 10);

      search.value = "заведомо-нет-такого-эмодзи";
      search.emit("input");
      assert.equal(all(app, ".emoji-cell").length, 0, "фильтр не применился");

      search.value = "";
      search.emit("input");
      assert.equal(all(app, ".emoji-cell").length, fullCount, "сброс фильтра потерял ячейки");

      // Цикл вкладок — прямой тест на replaceChildren в renderEmoji().
      const tabs = all(app, ".tab");
      const expected = all(app, ".emoji-cell").length;
      for (const index of [1, 2, 0]) {
        /** @type {ShimElement} */ (tabs[index]).click();
        const cells = all(app, ".emoji-cell");
        assert.ok(cells.length <= 120, `в сетке ${cells.length} ячеек > 120 (накопление)`);
        const unique = new Set(cells.map((cell) => cell.textContent));
        assert.equal(unique.size, cells.length, "в сетке есть дубли ячеек");
        if (index === 0) assert.equal(cells.length, expected, "возврат на первую вкладку изменил число ячеек");
      }
    });
  });

  it("настройки: 38 чекбоксов в 3 группах, состояния из дампа (SET-06/07/08)", async () => {
    await withApp((app) => {
      clickRail(app, "settings");
      const settings = byId(app, "settings");
      assert.equal(settings.querySelectorAll('input[type="checkbox"]').length, SETTINGS_TOGGLES);
      assert.equal(all(app, ".toggle").length, SETTINGS_TOGGLES);
      const groups = settings.querySelectorAll("h3");
      assert.equal(groups.length, 3, "h3-групп не 3 ([F] RW-38)");
      assert.deepEqual(
        groups.map((group) => group.textContent),
        ["Голос и видео", "Внешний вид", "Уведомления"],
      );
      const blocks = settings.querySelectorAll('[data-pane="voice"] .settings-block-title');
      assert.equal(blocks.length, 10, "блоков «Голоса» не 10 (SET-04)");
      const toggles = settings.querySelectorAll('input[type="checkbox"]');
      const off = toggles.filter((toggle) => toggle.checked === false).length;
      assert.equal(off, 4, `выключенных тумблеров ${off}, дамп даёт 4`);
      assert.equal(toggles.length - off, 34, "включённых тумблеров не 34");
    });
  });

  it("клик по тумблеру меняет checked и не создаёт новых тумблеров", async () => {
    await withApp((app) => {
      clickRail(app, "settings");
      const toggle = /** @type {ShimInputElement} */ (
        byId(app, "settings").querySelector('input[type="checkbox"]')
      );
      assert.ok(toggle instanceof ShimInputElement, "тумблер должен быть HTMLInputElement (instanceof в app.js)");
      assert.equal(toggle.checked, true);
      toggle.click();
      assert.equal(toggle.checked, false, "клик не выключил тумблер");
      toggle.click();
      assert.equal(toggle.checked, true, "повторный клик не включил тумблер");
      assert.equal(all(app, ".toggle").length, SETTINGS_TOGGLES, "клик добавил/удалил тумблеры");
    });
  });

  it("UI_SNAPSHOT: все поля контракта и actions=true", async () => {
    await withApp((app) => {
      const { call, payload } = app.snapshot;
      assert.ok(call, "report_ui_snapshot не вызывался");
      assert.equal(typeof call.args?.payload, "string", "payload должен быть JSON-строкой");
      assert.ok(payload, "payload не разобрался как JSON");

      assert.equal(payload.screenCount, SCREEN_COUNT);
      assert.equal(payload.chatRows, CHAT_ROWS);
      assert.equal(payload.memberRows, MEMBER_ROWS);
      assert.equal(payload.emojiCategories, EMOJI_CATEGORIES);
      assert.equal(payload.settingsToggles, SETTINGS_TOGGLES);
      assert.ok(payload.chatRenderedRows >= 10, `chatRenderedRows=${payload.chatRenderedRows}`);
      assert.ok(payload.chatRenderedRows < payload.chatRows);
      assert.ok(payload.memberRenderedRows >= 10, `memberRenderedRows=${payload.memberRenderedRows}`);
      assert.ok(payload.memberRenderedRows < payload.memberRows);
      assert.ok(payload.chatLoadedRows <= MAX_LOADED_ROWS);
      assert.ok(payload.memberLoadedRows <= MAX_LOADED_ROWS);
      assert.deepEqual(payload.actions, {
        screenClicks: SCREEN_COUNT,
        categoryClicks: true,
        toggleClick: true,
        listScroll: true,
      });
    });
  });

  it("autobench=false: report_ui_snapshot один раз, report_bench не вызывается", async () => {
    await withApp(async (app) => {
      assert.equal(app.callsOf("spike_config").length, 1, "spike_config должен вызываться один раз");
      assert.equal(app.callsOf("report_ui_snapshot").length, 1);
      assert.equal(app.callsOf("report_bench").length, 0, "при autobench=false замер запускаться не должен");
      // Автозамер не стартовал: после лишних кадров новых снапшотов нет.
      await app.settle(5);
      assert.equal(app.callsOf("report_ui_snapshot").length, 1);
      assert.equal(app.callsOf("report_bench").length, 0);
    });
  });

  it("process_metrics: RAM/потоки обновляются, интервал записан, но сам не тикает", async () => {
    await withApp((app) => {
      assert.ok(app.callsOf("process_metrics").length >= 1, "первый опрос метрик не выполнен");
      assert.match(one(app, "#m-rss").textContent, /^\d+ МБ$/, "RAM не отформатирована в МБ");
      assert.equal(one(app, "#m-threads").textContent, "7");

      const intervals = [...app.timers.values()].filter((timer) => timer.kind === "interval");
      assert.equal(intervals.length, 1, "опрос метрик должен быть на интервале");
      assert.equal(intervals[0]?.delay, 2000);

      const before = app.callsOf("process_metrics").length;
      app.flushTimers();
      assert.equal(app.callsOf("process_metrics").length, before + 1, "интервал не сработал по flushTimers");
    });
  });

  it("нет дублей: строка чата всегда ровно с одним .author и одним .text", async () => {
    await withApp(async (app) => {
      const list = byId(app, "chat-list");
      /** Проверяет инвариант структуры всех видимых строк (без мутаций). */
      const assertClean = () => {
        for (const row of all(app, "#chat-list .row")) {
          const authors = row.querySelectorAll(".author");
          const texts = row.querySelectorAll(".text");
          if (row.dataset.state === "placeholder") {
            assert.equal(row.children.length, 0, "placeholder унаследовал детей строки");
            continue;
          }
          assert.equal(authors.length, 1, `в строке ${chatRowIndex(row)} авторов: ${authors.length}`);
          assert.equal(texts.length, 1, `в строке ${chatRowIndex(row)} текстов: ${texts.length}`);
          assert.equal(row.children.length, 2, `в строке ${chatRowIndex(row)} детей: ${row.children.length}`);
        }
      };

      for (const index of [0, 49_999, CHAT_ROWS - 1]) {
        await scrollList(app, list, chatTop(index));
        assertClean();
      }

      // Переиспользование элемента: тот же узел после перерисовки не копит детей.
      await scrollList(app, list, chatTop(5000));
      const row = /** @type {ShimElement} */ (all(app, "#chat-list .row")[0]);
      const rowIndex = chatRowIndex(row);
      app.triggerResize();
      await app.settle(3);
      const sameRow = all(app, "#chat-list .row").find((node) => chatRowIndex(node) === rowIndex);
      assert.equal(sameRow, row, "элемент строки пересоздан без скролла");
      assert.equal(row.children.length, 2, "перерисовка накопила детей в строке");
      assertClean();
    });
  });

  it("граничный скролл не бросает: верх, низ, за концом, отрицательный", async () => {
    await withApp(async (app) => {
      const list = byId(app, "chat-list");
      const sizer = /** @type {ShimElement} */ (list.children[0]);

      for (const scrollTop of [0, -50, chatTop(CHAT_ROWS - 1), 10 ** 9]) {
        await scrollList(app, list, scrollTop);
        assert.equal(list.children.length, 1, "sizer потерян при граничном скролле");
        assert.equal(sizer.children.length, all(app, "#chat-list .row").length);
        assert.ok(all(app, "#chat-list .row").length <= MAX_DOM_ROWS);
      }

      // За концом списка окно пустое: getRow вне диапазона не создаёт строк и не бросает.
      assert.equal(all(app, "#chat-list .row").length, 0, "за концом списка остались строки");

      // Верх: первая строка — индекс 0, top=0.
      await scrollList(app, list, 0);
      const firstRow = /** @type {ShimElement} */ (all(app, "#chat-list .row")[0]);
      assert.equal(chatRowIndex(firstRow), 0);
      assert.equal(rowTop(firstRow), 0);

      // ResizeObserver после граничных скроллов не ломает список.
      app.triggerResize();
      await app.settle(3);
      assert.ok(all(app, "#chat-list .row").length > 0);
      assert.deepEqual(app.consoleErrors, [], "граничный скролл дал console.error");
    });
  });

  it("ResizeObserver: изменение высоты контейнера пересчитывает окно строк", async () => {
    await withApp(async (app) => {
      clickRail(app, "chat");
      const list = byId(app, "chat-list");
      await scrollList(app, list, 0);
      const beforeCount = all(app, "#chat-list .row").length;
      assert.ok(beforeCount > 0);

      // Окно стало выше (как при растягивании окна приложения): observer → paint.
      list.boxHeight = 1600;
      app.triggerResize();
      await app.settle(3);
      const grownCount = all(app, "#chat-list .row").length;
      assert.ok(grownCount > beforeCount + 5, `окно не выросло: ${beforeCount} → ${grownCount}`);
      assert.equal(chatRowIndex(/** @type {ShimElement} */ (all(app, "#chat-list .row")[0])), 0);

      // Обратно: лишние строки удаляются, DOM не держит прежнее окно.
      list.boxHeight = 800;
      app.triggerResize();
      await app.settle(3);
      const shrunkCount = all(app, "#chat-list .row").length;
      assert.ok(shrunkCount < grownCount, `окно не ужалось: ${grownCount} → ${shrunkCount}`);
      assert.equal(shrunkCount, beforeCount, "после возврата высоты окно должно совпасть с исходным");
    });
  });

  it("скрытый экран рисует только overscan, показ экрана перерисовывает окно", async () => {
    await withApp(async (app) => {
      // Сначала прячем участников: клик по чату + resize (высота скрытого экрана 0).
      clickRail(app, "chat");
      app.triggerResize();
      await app.settle(3);
      const hiddenRows = all(app, "#member-list .member").length;
      assert.ok(hiddenRows >= 1 && hiddenRows < 10, `скрытый экран нарисовал ${hiddenRows} строк`);

      clickRail(app, "members");
      await app.settle(6);
      const shownRows = all(app, "#member-list .member").length;
      assert.ok(shownRows >= 20, `показанный экран нарисовал ${shownRows} строк`);
      assert.ok(shownRows > hiddenRows * 2, "показ экрана не перерисовал окно (P1-1)");
    });
  });

  it("при обычной работе нет console.error и предупреждений виртуализации", async () => {
    await withApp(async (app) => {
      clickRail(app, "emoji");
      /** @type {ShimElement} */ (all(app, ".tab")[1]).click();
      clickRail(app, "members");
      await scrollList(app, byId(app, "member-list"), 1000 * ROW_H.MEMBER);
      clickRail(app, "chat");
      await scrollList(app, byId(app, "chat-list"), chatTop(700));

      assert.deepEqual(app.consoleErrors, [], "app.js сообщил об ошибке");
      assert.deepEqual(app.consoleWarns, [], "виртуализация предупредила о пустом окне");
    });
  });
});

describe("чат-вью P3-02: новые блоки не ломают инварианты списка", () => {
  it("QuickSwitcher host и ресурсы доступны отдельно от четырёх frozen экранов", async () => {
    assert.match(html, /id="quick-switcher"[^>]*role="dialog"[^>]*aria-modal="true"[^>]*hidden/);
    assert.match(html, /id="quick-switcher-input"[^>]*role="combobox"[^>]*aria-expanded="false"[^>]*aria-controls="quick-switcher-results"/);
    assert.match(html, /id="quick-switcher-results"[^>]*role="listbox"/);
    const quickCss = readFileSync(`${uiDir}components/quick-switcher.css`, "utf8");
    assert.match(quickCss, /\.quick-switcher\[hidden\]/);
    assert.match(quickCss, /\.quick-switcher-option\[aria-selected="true"\]/);
    assert.match(readFileSync(`${uiDir}style.css`, "utf8"), /components\/quick-switcher\.css/);
    await withApp((app) => {
      assert.equal(all(app, ".screen").length, SCREEN_COUNT, "host попал в число frozen экранов");
    });
  });

  it("extras живут внутри .text, у строки по-прежнему ровно 2 ребёнка", async () => {
    await withApp(async (app) => {
      const list = byId(app, "chat-list");
      await scrollList(app, list, 0);
      const rows = all(app, "#chat-list .row");
      assert.ok(rows.length >= 10, "окно пустое");
      let extrasSeen = 0;
      for (const row of rows) {
        assert.equal(row.children.length, 2, `строка ${chatRowIndex(row)}: детей ${row.children.length}, а не 2 (§7.1)`);
        const text = row.querySelector(".text");
        assert.ok(text, "у строки нет .text");
        assert.equal(
          text.querySelectorAll(".author").length,
          0,
          "внутри .text оказался .author",
        );
        if (text.querySelector(".reply-quote, .embed, .day-divider, .reactions-row")) extrasSeen += 1;
      }
      assert.ok(extrasSeen > 0, "в окне строк 0..40 нет ни одной строки с extras");
      // Текст первого ребёнка .text начинается с #index — оракул индекса не сдвинут.
      for (const row of rows) {
        const body = row.querySelector(".text")?.children[0];
        assert.match(body?.className ?? "", /text-body/);
        assert.match(body?.textContent ?? "", /^#\d+/);
      }
      // Панель действий и полоса непрочитанного — не дети списка (только sizer).
      assert.equal(all(app, "#chat-list .msg-toolbar").length, 0);
      assert.equal(all(app, "#chat-list .chat-unread-divider").length, 0);
      assert.equal(list.children.length, 1, "у #chat-list появился второй ребёнок");
      const sizer = /** @type {ShimElement} */ (list.children[0]);
      assert.equal(sizer.children.length, all(app, "#chat-list .row").length, "sizer разошёлся с окном");
    });
  });

  it("D-13: unread-полоса 34 и блок группы участников 42 — из токенов, не литералов", async () => {
    const tokens = readFileSync(`${uiDir}tokens.css`, "utf8");
    assert.match(tokens, /--unread-banner-h:\s*34px;/, "нет --unread-banner-h: 34px (D-13)");
    assert.match(tokens, /--row-member-group:\s*42px;/, "нет --row-member-group: 42px (D-13)");
    const chatCss = readFileSync(`${uiDir}components/chat.css`, "utf8");
    assert.match(chatCss, /var\(--unread-banner-h\)/, "chat.css не использует --unread-banner-h");
    const membersCss = readFileSync(`${uiDir}components/members.css`, "utf8");
    assert.match(membersCss, /var\(--row-member-group\)/, "members.css не использует --row-member-group");

    await withApp(async (app) => {
      const band = all(app, ".chat-unread-divider")[0];
      assert.ok(band, "нет .chat-unread-divider");
      assert.equal(band.getAttribute("data-height"), "34");
      await app.settle(8);
      const titles = all(app, ".member-group-title");
      assert.equal(titles.length, 6, `заголовков групп ${titles.length}`);
      for (const title of titles) assert.equal(title.getAttribute("data-height"), "42");
    });
  });

  it("окно данных не растёт от новых панелей: loadedRows ≤ 500, inFlight пуст", async () => {
    await withApp(async (app) => {
      const instance = app.appModule?.currentApp?.();
      assert.ok(instance, "нет currentApp()");
      // Открываем поиск, гоняем фильтры и переходы — окно не должно утечь.
      const header = byId(app, "chat-search");
      for (const query of ["привет", "тест", "волна"]) {
        header.value = query;
        header.emit("input");
      }
      const panel = all(app, ".search-overlay")[0];
      panel?.querySelector(".search-result")?.click();
      await app.settle(4);
      const stats = instance.chat.store.stats();
      assert.ok(stats.loadedRows <= MAX_LOADED_ROWS, `loadedRows=${stats.loadedRows} > ${MAX_LOADED_ROWS}`);
      assert.equal(instance.chat.store.inFlight.size, 0, "остались незавершённые загрузки страниц");
      assert.equal(instance.members.store.inFlight.size, 0, "участники: незавершённые загрузки страниц");
      const { payload } = app.snapshot;
      assert.ok(payload.chatLoadedRows <= MAX_LOADED_ROWS);
    });
  });
});
