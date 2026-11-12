// @ts-check
// RW-39 / QA-1: постоянные тесты интеракций GUI-спайка W39d.
// Порт оракула `waves/RW-39/PM/w39d/features-check.mjs` (9 проверок) на node:test.
// Гоняет настоящий `ui/app.js` в DOM-шиме (tests/support/gui-dom-harness.mjs):
//   body[data-active-screen]/aria-current, плоский listbox эмодзи (D-07), roving
//   tabindex + стрелки, 38 канонических тумблеров и #settings-nav, панель
//   участников 42px из model.js, клавиши §8.2, data-action §8.1, aria-live только
//   у счётчиков, отсутствие console.error/warn в штатном сценарии.
//
// Отличие от оракула только в изоляции: каждый кейс получает свежий экземпляр
// модуля (свой документ и очереди), поэтому падение одного не заражает остальные.
// Последняя проверка (консоль) проигрывает весь штатный сценарий целиком — так
// смысл «штатная работа без console.error/warn» сохранён, а не ослаблен.
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { loadApp } from "../support/gui-dom-harness.mjs";
// W39h3 / P0: проверяем сам рендер embed, а не только данные демо-строки.
import { createEmbed } from "../../rust/gui-spike/ui/features/embeds.js";
// DP-02 / D-04: высота строки — функция текста/переносов, а не фиксированный трек.
import { CHAT_ROWS, makeMessage, renderChatRow } from "../../rust/gui-spike/ui/features/chat.js";
import { createChatHeights, messageHeight } from "../../rust/gui-spike/ui/model.js";

/** @typedef {import("../support/gui-dom-harness.mjs").GuiDom} GuiDom */
/** @typedef {import("../support/gui-dom-harness.mjs").ShimElement} ShimElement */

const SCREENS = ["chat", "emoji", "members", "settings"];
const EMOJI_CATEGORIES = 10;
const SETTINGS_TOGGLES = 38;
// SET-01 (RE S-01): 18 разделов в порядке дампа + «Выйти» отдельной кнопкой.
const SETTINGS_NAV = [
  "Учётная запись",
  "Данные и конфиденциальность",
  "Разрешения для сообщений",
  "Уведомления",
  "Nitro",
  "Буст сервера",
  "Подписки",
  "Склад подарков",
  "Выставление счетов",
  "Голос и видео",
  "Внешний вид",
  "Специальные возможности",
  "Система",
  "Язык и время",
  "Зарегистрированные игры",
  "Конфиденциальность активности",
  "Подключённые приложения",
  "Разработчик",
];
// D-17: контент есть только у трёх разделов; остальные — disabled.
const SETTINGS_ENABLED = ["Уведомления", "Голос и видео", "Внешний вид"];
// SET-04: 10 блоков «Голоса» в порядке дампа.
const SETTINGS_BLOCKS = [
  "Голос",
  "Профиль ввода",
  "Чувствительность ввода",
  "Шумоподавление",
  "Эхо/рация/АРУ",
  "Камера",
  "Стриминг",
  "Звуки",
  "Связанные настройки",
  "Расширенные",
];
// SET-06/07/08: из 38 тумблеров выключены ровно эти четыре (остальные вкл).
const SETTINGS_OFF = ["Приоритет QoS", "Режим рации", "Оверлей стрима", "Запись звука для диагностики"];
// ST-01 (RW-40): независимый оракул `/home/sanya/re-deep/ui8/23-settings-categories.json`
// (63 записи: профиль 2 + 19 навигационных × 3 обёртки + 4 развёрнутых подпункта).
// Здесь — только тексты, извлечённые из дампа, а не из текущего `settings.js`.
const ORACLE_PROFILE = /** @type {const} */ ({
  name: "User",
  action: "Редактировать профили",
});
const ORACLE_ACCOUNT_ITEMS = [
  "Информация об учётной записи",
  "Пароль и безопасность",
  "Статус учётной записи",
  "Семейный центр",
];

/**
 * Приложение с управляемым IPC: нужен для «Выйти» → `auth_logout`
 * (успех/отказ). Неизвестные команды отдаются штатной заглушке `loadApp`.
 * @param {{ onLogout?: (args: any) => any }} script
 * @param {(app: GuiDom) => void | Promise<void>} body
 * @returns {Promise<void>}
 */
async function withAppLogout(script, body) {
  const app = await loadApp({
    invokeHandler: async (cmd, args) => {
      if (cmd !== "auth_logout") return undefined;
      return script.onLogout ? script.onLogout(args) : null;
    },
  });
  try {
    await body(app);
  } finally {
    app.dispose();
  }
}

/**
 * Свежий экземпляр `app.js` на кейс: состояние одного теста не утекает в другой.
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

/**
 * @param {GuiDom} app
 * @param {string} selector
 * @returns {ShimElement[]}
 */
function all(app, selector) {
  return app.document.querySelectorAll(selector);
}

/**
 * Единственный узел по селектору: пропавшая разметка — падение теста, а не
 * `possibly undefined` при обращении к `array[0]` (strict/noUncheckedIndexedAccess).
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
 * @param {string} screen
 * @returns {ShimElement}
 */
function clickRail(app, screen) {
  const button = all(app, ".rail-btn").find((node) => node.dataset.screen === screen);
  assert.ok(button, `нет rail-кнопки ${screen}`);
  button.click();
  return button;
}

/**
 * @param {GuiDom} app
 * @param {string} name
 * @returns {ShimElement}
 */
function action(app, name) {
  const node = app.document.querySelector(`[data-action="${name}"]`);
  assert.ok(node, `нет [data-action=${name}]`);
  return node;
}

/**
 * Клавиша на body: как в оракуле — модификаторы по умолчанию выключены.
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

describe("gui-spike features: интеракции W39d", () => {
  it("QuickSwitcher открывается по Ctrl/Cmd+K, фильтрует, перемещает выбор и переключает канал/экран", async () => {
    await withApp(async (app) => {
      key(app, "k", { ctrlKey: true });
      const dialog = firstNode(app, ".quick-switcher");
      assert.equal(dialog.getAttribute("role"), "dialog");
      assert.equal(dialog.getAttribute("aria-modal"), "true");
      assert.equal(dialog.getAttribute("hidden"), null);
      const input = firstNode(app, "#quick-switcher-input");
      assert.equal(app.document.activeElement, input, "фокус не перемещён в поле");
      assert.equal(input.getAttribute("role"), "combobox");
      assert.equal(input.getAttribute("aria-expanded"), "true");
      const list = firstNode(app, "#quick-switcher-results");
      assert.equal(list.getAttribute("role"), "listbox");
      assert.ok(list.querySelectorAll('[role="option"]').length >= 8, "нет результатов каналов, гильдий и экранов");
      assert.ok(list.querySelectorAll('[role="option"]').length <= 8, "выдача превышает восемь строк");
      assert.equal(input.getAttribute("aria-activedescendant"), list.querySelector('[aria-selected="true"]')?.id);

      input.value = "объяв";
      input.emit("input");
      assert.equal(list.querySelectorAll('[role="option"]').length, 1, "fuzzy-фильтр не сузил результаты");
      assert.match(list.textContent, /объявления/);

      input.value = "настр";
      input.emit("input");
      const options = list.querySelectorAll('[role="option"]');
      assert.ok(options.length >= 1);
      key(app, "ArrowDown");
      key(app, "Enter");
      assert.equal(app.document.body.getAttribute("data-active-screen"), "settings", "экран не переключён роутером");
      assert.equal(app.document.querySelector('.rail-btn[data-screen="settings"]')?.getAttribute("aria-current"), "page");
      assert.notEqual(dialog.getAttribute("hidden"), null, "switcher не закрылся после выбора экрана");

      key(app, "k", { metaKey: true });
      const reopened = firstNode(app, ".quick-switcher");
      const query = firstNode(app, "#quick-switcher-input");
      query.value = "#объяв";
      query.emit("input");
      firstNode(app, '#quick-switcher-results [role="option"]').click();
      assert.equal(firstNode(app, '.channel-item[aria-current="page"] .channel-name').textContent, "объявления");
      assert.notEqual(reopened.getAttribute("hidden"), null, "switcher не закрылся после выбора канала");
      assert.equal(app.document.activeElement?.dataset.screen, "chat", "фокус не вернулся к активному экрану");
      key(app, "k", { ctrlKey: true });
      key(app, "Escape");
      assert.notEqual(reopened.getAttribute("hidden"), null, "Escape не закрыл switcher");
    });
  });

  it("QuickSwitcher показывает пустое состояние и ловит Tab внутри диалога", async () => {
    await withApp((app) => {
      key(app, "k", { ctrlKey: true });
      const input = firstNode(app, "#quick-switcher-input");
      input.value = "никакого совпадения 8472";
      input.emit("input");
      assert.equal(all(app, '#quick-switcher-results [role="option"]').length, 0);
      assert.equal(input.hasAttribute("aria-activedescendant"), false, "пустая выдача ссылается на отсутствующий option");
      assert.match(firstNode(app, ".quick-switcher-empty").textContent, /не найдено/i);
      const tab = key(app, "Tab");
      assert.equal(tab.prevented, true, "Tab вышел из модального диалога");
      assert.equal(app.document.activeElement?.className, "quick-switcher-close", "Tab не перевёл фокус на закрытие");
      const reverseTab = key(app, "Tab", { shiftKey: true });
      assert.equal(reverseTab.prevented, true, "Shift+Tab вышел из модального диалога");
      assert.equal(app.document.activeElement, input, "Shift+Tab не вернул фокус в поле");
      input.value = "сервер 2";
      input.emit("input");
      key(app, "Enter");
      assert.equal(all(app, ".guild-item").find((node) => node.dataset.guild === "2")?.classList.contains("is-selected"), true);
    });
  });

  it("композер отправляет Enter, сохраняет Shift+Enter и не отправляет пустое", async () => {
    await withApp(async (app) => {
      const input = byId(app, "composer-input");
      /** @type {any[]} */
      const calls = [];
      app.setInvokeHandler(async (cmd, args) => { if (cmd === "send_message") calls.push(args); return { id: "m1", channel_id: args?.channelId, content: args?.content, nonce: args?.nonce }; });
      input.value = "   "; input.emit("input"); input.dispatchEvent("keydown", { key: "Enter" });
      assert.equal(calls.length, 0);
      input.value = "hello"; input.emit("input"); input.dispatchEvent("keydown", { key: "Enter", shiftKey: true });
      assert.equal(calls.length, 0);
      input.dispatchEvent("keydown", { key: "Enter" }); await Promise.resolve();
      assert.equal(calls.length, 1); assert.equal(calls[0]?.content, "hello");
      assert.equal(typeof calls[0]?.channelId, "string"); assert.equal(typeof calls[0]?.nonce, "string");
      assert.equal(input.value, "");
    });
  });

  it("optimistic строка получает error и ручной retry переводит её в sent", async () => {
    await withApp(async (app) => {
      const input = byId(app, "composer-input"); let reject = true;
      app.setInvokeHandler(async (cmd, args) => { if (cmd !== "send_message") return null; if (reject) throw { kind: "Network", message: "сеть недоступна" }; return { id: "m2", channel_id: args.channelId, content: args.content, nonce: args.nonce }; });
      input.value = "retry me"; input.emit("input"); input.dispatchEvent("keydown", { key: "Enter" });
      assert.equal(app.document.querySelectorAll('[data-message-status="pending"]').length, 1);
      await new Promise((resolve) => setImmediate(resolve));
      const failed = app.document.querySelector('[data-message-status="error"]'); assert.ok(failed);
      assert.match(failed.textContent, /сеть недоступна/); assert.equal(input.value, "retry me");
      reject = false; const retry = failed.querySelector('[data-action="retry"]'); assert.ok(retry); retry.click(); await new Promise((resolve) => setImmediate(resolve));
      assert.ok(app.document.querySelector('[data-message-status="sent"]'));
    });
  });

  it("ошибка отправки не затирает набранный поверх черновик, пустое поле — возвращает текст", async () => {
    /** @type {((reason?: any) => void) | null} */
    let rejectSend = null;
    const app = await loadApp({ invokeHandler: (cmd) => cmd === "send_message"
      ? new Promise((_resolve, reject) => { rejectSend = reject; })
      : undefined });
    try {
      const input = byId(app, "composer-input");
      input.value = "первое"; input.emit("input");
      input.dispatchEvent("keydown", { key: "Enter" });
      assert.equal(input.value, "", "композер не очистился после отправки");
      // Пользователь успел набрать новый черновик до ответа IPC.
      input.value = "новый черновик"; input.emit("input");
      /** @type {any} */ (rejectSend)({ kind: "Network", message: "offline" });
      await app.settle(4);
      assert.equal(input.value, "новый черновик", "ошибка отправки затёрла новый черновик");

      // Если поле пустое — недописанное сообщение по-прежнему возвращается.
      app.setInvokeHandler(async (cmd) => { if (cmd === "send_message") throw { kind: "Network", message: "offline" }; return null; });
      input.value = ""; input.emit("input");
      input.value = "третье"; input.emit("input");
      input.dispatchEvent("keydown", { key: "Enter" });
      await app.settle(4);
      assert.equal(input.value, "третье", "текст не вернулся в пустой композер");
    } finally {
      app.dispose();
    }
  });

  it("401/429 показывают классовое сообщение и не делают автоповтор", async () => {
    await withApp(async (app) => {
      const input = byId(app, "composer-input"); let mode = "Unauthorized"; let attempts = 0;
      app.setInvokeHandler(async () => { attempts++; throw mode === "Unauthorized" ? { kind: mode, message: "secret-token" } : { kind: mode, retry_after_ms: 4200 }; });
      input.value = "one"; input.emit("input"); input.dispatchEvent("keydown", { key: "Enter" }); await new Promise((resolve) => setImmediate(resolve));
      assert.match(app.document.querySelector('[data-message-status="error"]')?.textContent ?? "", /сессия истекла/);
      assert.doesNotMatch(app.document.body.textContent, /secret-token/);
      mode = "RateLimited"; input.value = "two"; input.emit("input"); input.dispatchEvent("keydown", { key: "Enter" }); await new Promise((resolve) => setImmediate(resolve));
      assert.match(app.document.body.textContent, /4200|4 с/); assert.equal(attempts, 2);
      mode = "forbidden"; input.value = "three"; input.emit("input"); input.dispatchEvent("keydown", { key: "Enter" }); await new Promise((resolve) => setImmediate(resolve));
      assert.match(app.document.body.textContent, /нет прав на отправку сообщений в этом канале \(403\)/); assert.equal(attempts, 3);
    });
  });
  it("body[data-active-screen] + aria-current на 4 экранах", async () => {
    await withApp((app) => {
      const doc = app.document;
      assert.equal(doc.body.getAttribute("data-active-screen"), "chat", "стартовый экран не chat");
      for (const screen of ["emoji", "members", "settings", "chat"]) {
        const button = clickRail(app, screen);
        assert.equal(
          doc.body.getAttribute("data-active-screen"),
          screen,
          `body не переключился на ${screen}`,
        );
        assert.equal(button.getAttribute("aria-current"), "page", `aria-current у ${screen}`);
        const current = all(app, ".rail-btn").filter(
          (node) => node.getAttribute("aria-current") === "page",
        );
        assert.equal(current.length, 1, `aria-current=page у ${current.length} кнопок`);
        assert.equal(all(app, ".screen.active").length, 1, `активных экранов ${all(app, ".screen.active").length}`);
      }
      assert.equal(all(app, ".screen").length, SCREENS.length);
    });
  });

  it("шапка чата: 4 icon-btn — ветки, колокол, закреплённые, участники (VW-01, D-17)", async () => {
    await withApp((app) => {
      const actions = all(app, ".chat-actions .icon-btn").map(
        (node) => node.getAttribute("data-action"),
      );
      assert.deepEqual(actions, ["threads", "inbox", "pins", "members"], "порядок кнопок шапки не VW-01");
      const bell = action(app, "inbox");
      assert.equal(bell.getAttribute("aria-label"), "Уведомления", "колокол без aria-label");
      assert.equal(bell.getAttribute("aria-disabled"), "true", "колокол не disabled-affordance (Phase 3)");
      assert.ok((bell.getAttribute("title") ?? "").length > 0, "у колокола нет подсказки");
      // Disabled affordance не должен ронять обработчик и менять экран.
      bell.click();
      assert.equal(app.document.body.getAttribute("data-active-screen"), "chat");
      assert.deepEqual(app.consoleErrors, [], "клик по колоколу дал console.error");
      assert.equal(app.document.querySelectorAll(".error-state").length, 0, "клик по колоколу показал оверлей ошибки");
    });
  });

  it("эмодзи: плоский listbox (role=option, без .emoji-row) и 10 вкладок-глифов", async () => {
    await withApp((app) => {
      clickRail(app, "emoji");
      assert.equal(byId(app, "emoji-grid").getAttribute("role"), "listbox", "#emoji-grid не listbox");
      assert.equal(all(app, ".emoji-row").length, 0, "в плоском listbox остались .emoji-row");
      assert.equal(all(app, ".tab").length, EMOJI_CATEGORIES, "вкладок не 10");

      const activeTabs = all(app, ".tab").filter((tab) => tab.getAttribute("aria-selected") === "true");
      assert.equal(activeTabs.length, 1, `aria-selected=true у ${activeTabs.length} вкладок`);
      for (const tab of all(app, ".tab")) {
        assert.equal(tab.getAttribute("role"), "tab", "вкладка без role=tab");
        assert.ok((tab.title ?? "").length > 2, "у вкладки нет title с названием категории");
        assert.equal(tab.textContent.trim(), tab.textContent, "глиф вкладки с пробелами");
        assert.ok(tab.textContent.length <= 4, `во вкладке текст «${tab.textContent}», а не глиф`);
        assert.equal(tab.getAttribute("aria-controls"), "emoji-grid", "aria-controls вкладки");
      }

      const cells = all(app, ".emoji-cell");
      assert.ok(cells.length >= 10 && cells.length <= 120, `ячеек ${cells.length}`);
      const names = new Set();
      let roving = 0;
      for (const cell of cells) {
        assert.equal(cell.getAttribute("role"), "option", "ячейка без role=option");
        assert.ok((cell.getAttribute("aria-label") ?? "").trim().length > 0, "пустой aria-label ячейки");
        assert.equal(cell.title, cell.textContent, "title != глиф");
        names.add(cell.getAttribute("aria-label"));
        if (cell.getAttribute("tabindex") === "0") roving++;
      }
      assert.equal(names.size, cells.length, "aria-label ячеек не уникальны в категории");
      assert.equal(roving, 1, `roving tabindex=0 у ${roving} ячеек`);
    });
  });

  it("эмодзи: стрелки двигают aria-selected и roving tabindex", async () => {
    await withApp((app) => {
      clickRail(app, "emoji");
      const grid = byId(app, "emoji-grid");
      const cells = all(app, ".emoji-cell");
      assert.ok(cells.length > 2, "мало ячеек для roving");
      grid.dispatchEvent("keydown", { key: "ArrowRight" });
      const selected = all(app, ".emoji-cell").filter(
        (cell) => cell.getAttribute("aria-selected") === "true",
      );
      assert.equal(selected.length, 1, `aria-selected=true у ${selected.length} ячеек`);
      assert.equal(selected[0], cells[1], "ArrowRight не перевёл выбор на вторую ячейку");
      assert.equal(cells[1]?.getAttribute("tabindex"), "0", "tabindex не переехал");
      assert.equal(cells[0]?.getAttribute("tabindex"), "-1", "старая ячейка осталась в табе");
      grid.dispatchEvent("keydown", { key: "End" });
      const lastCells = all(app, ".emoji-cell");
      assert.equal(
        lastCells[lastCells.length - 1]?.getAttribute("aria-selected"),
        "true",
        "End не выбрал последнюю",
      );
    });
  });

  it("эмодзи-поиск: глобальный по всем категориям, вкладка только листает (EMO-16)", async () => {
    // EMO-16 (RE E-16): поиск идёт по всему набору, а не по активной категории;
    // нормализация — EMO-11 (`:`/пробелы/`_`, регистр). Сброс возвращает категорию.
    await withApp((app) => {
      clickRail(app, "emoji");
      const search = byId(app, "emoji-search");
      const consoleApi = api(app).emoji;
      /** @type {ShimElement} */ (all(app, ".tab")[0]).click();
      const firstCategory = consoleApi.categories[0]?.items.length ?? 0;
      assert.ok(firstCategory > 10, `в категории ${firstCategory} ячеек — мало для проверки`);
      assert.equal(all(app, ".emoji-cell").length, firstCategory);

      // «кофе» есть только в «Еде и напитках» (4-я категория), активна первая.
      search.value = "кофе";
      search.emit("input");
      const coffee = all(app, ".emoji-cell");
      assert.equal(coffee.length, 1, `глобальный поиск вернул ${coffee.length} ячеек вместо 1`);
      assert.equal(coffee[0]?.textContent, "☕", "глобальный поиск не нашёл кофе");
      assert.equal(consoleApi.getActiveCategory(), 0, "поиск переключил активную вкладку");

      // Вкладка только листает: на глобальную выдачу она не влияет.
      /** @type {ShimElement} */ (all(app, ".tab")[3]).click();
      assert.equal(all(app, ".emoji-cell").length, 1, "вкладка сузила глобальную выдачу");
      assert.equal(consoleApi.getActiveCategory(), 3, "вкладка не переключилась");

      // EMO-11: ведущее/хвостовое `:`, пробелы, `_` и регистр не мешают.
      for (const query of [":кофе", "КОФЕ", ":КОФЕ:", " кофе "]) {
        search.value = query;
        search.emit("input");
        assert.equal(all(app, ".emoji-cell").length, 1, `нормализация «${query}» не сработала`);
        assert.equal(all(app, ".emoji-cell")[0]?.textContent, "☕", `«${query}» нашёл не кофе`);
      }

      // Дедупликация: «огонь» есть и в «Часто используемых», и в «Символах».
      search.value = "огонь";
      search.emit("input");
      assert.equal(all(app, ".emoji-cell").length, 1, "глобальный поиск не дедуплицировал огонь");

      // Сброс — снова активная (4-я) категория, без накопления ячеек.
      search.value = "";
      search.emit("input");
      const expected = consoleApi.categories[3]?.items.length ?? -1;
      assert.equal(all(app, ".emoji-cell").length, expected, "сброс вернул не активную категорию");
      const unique = new Set(all(app, ".emoji-cell").map((cell) => cell.textContent));
      assert.equal(unique.size, expected, "повторный рендер задвоил ячейки");
    });
  });

  it("настройки: 18 разделов навигации, 3 доступных, остальные disabled (SET-01, D-17)", async () => {
    await withApp((app) => {
      clickRail(app, "settings");
      const settings = byId(app, "settings");
      const nav = byId(app, "settings-nav");
      assert.equal(nav.querySelectorAll("h3").length, 0, "в #settings-nav попали h3 (счётчик групп)");

      const items = nav.querySelectorAll(".settings-nav-item");
      assert.equal(items.length, 18, `разделов ${items.length}, SET-01 требует 18`);
      assert.deepEqual(items.map((button) => button.textContent), SETTINGS_NAV);

      const enabled = items.filter((button) => button.getAttribute("disabled") === null);
      assert.deepEqual(
        enabled.map((button) => button.textContent),
        SETTINGS_ENABLED,
        "доступны не ровно три раздела с контентом",
      );
      for (const button of items) {
        assert.equal(button.getAttribute("type"), "button", "пункт навигации без type=button");
        const isEnabled = SETTINGS_ENABLED.includes(button.textContent);
        assert.equal(
          button.getAttribute("aria-disabled"),
          isEnabled ? null : "true",
          `aria-disabled рассинхронизирован у «${button.textContent}»`,
        );
      }

      // «Выйти» — отдельная кнопка, не входит в 18 разделов (SET-01: 18 + «Выйти»).
      const logout = nav.querySelector(".settings-nav-logout");
      assert.ok(logout, "в навигации нет кнопки «Выйти»");
      assert.equal(logout.textContent, "Выйти");

      // Стартовый контент — «Голос и видео» (D-17), тумблеры уже в DOM.
      assert.equal(settings.getAttribute("data-active-pane"), "voice", "стартовая панель не voice");
      assert.equal(byId(app, "settings-modal").getAttribute("hidden"), null, "voice-панель настроек скрыта");
      assert.equal(settings.querySelectorAll(".toggle").length, SETTINGS_TOGGLES);
      assert.equal(
        settings.querySelectorAll('input[type="checkbox"]').length,
        SETTINGS_TOGGLES,
        "чекбоксов не 38",
      );

      // «Внешний вид» — честная заглушка с подписью (S-14, SET-15).
      /** @type {ShimElement} */ (items[10]).click();
      assert.equal(settings.getAttribute("data-active-pane"), "appearance", "клик не открыл «Внешний вид»");
      assert.notEqual(byId(app, "settings-modal").getAttribute("hidden"), null, "voice-панель видна вне voice");
      const appearance = settings.querySelector('[data-pane="appearance"]');
      assert.ok(appearance, "нет панели «Внешний вид»");
      assert.equal(appearance.getAttribute("hidden"), null, "панель «Внешний вид» скрыта");
      assert.match(appearance.textContent, /Компактное|Уютное/, "нет вариантов плотности (S-14)");
      assert.match(appearance.textContent, /не подтвержд|заглушк/i, "нет честной подписи заглушки");

      // S-14: плотность переключает html[data-density], а не только aria-pressed.
      const density = appearance.querySelectorAll(".settings-density-option");
      assert.equal(density.length, 2, "вариантов плотности не 2");
      /** @type {ShimElement} */ (density[0]).click();
      assert.equal(
        app.document.documentElement.getAttribute("data-density"),
        "compact",
        "клик не выставил html[data-density=compact]",
      );
      assert.equal(density[0]?.getAttribute("aria-pressed"), "true", "aria-pressed не переключился");
      assert.equal(density[1]?.getAttribute("aria-pressed"), "false", "второй вариант остался нажатым");

      /** @type {ShimElement} */ (items[3]).click();
      assert.equal(settings.getAttribute("data-active-pane"), "notifications", "клик не открыл «Уведомления»");
      assert.notEqual(byId(app, "settings-modal").getAttribute("hidden"), null, "voice-панель видна на вкладке уведомлений");
      const notifications = settings.querySelector('[data-pane="notifications"]');
      assert.ok(notifications, "нет панели «Уведомления»");
      assert.match(notifications.textContent, /не открывал|неизвест/i, "нет честной подписи SET-15");

      // Disabled-раздел не меняет контент и не становится активным.
      /** @type {ShimElement} */ (items[4]).click();
      assert.equal(settings.getAttribute("data-active-pane"), "notifications", "disabled-раздел открыл панель");
      assert.equal(items[4]?.getAttribute("aria-current"), null, "disabled-раздел стал активным");
    });
  });

  // ST-01 (RW-40): профиль + 4 подпункта «Учётной записи» строго из оракула
  // `ui8/23-settings-categories.json`; свёрнутые подпункты остальных 17 разделов
  // не выдумываются (`[A]` в G6-REPORT §3.11).
  it("настройки ST-01: профиль и 4 развёрнутых подпункта «Учётной записи» из оракула", async () => {
    await withApp((app) => {
      clickRail(app, "settings");
      const nav = byId(app, "settings-nav");

      const profile = firstNode(app, "#settings-nav .settings-profile");
      assert.equal(
        profile.querySelector(".settings-profile-name")?.textContent,
        ORACLE_PROFILE.name,
        "имя профиля не из оракула",
      );
      const monogram = profile.querySelector(".settings-profile-avatar");
      assert.ok(monogram, "нет аватара-монограммы профиля");
      assert.equal(monogram.textContent, "U", "монограмма не совпадает с инициалами");
      const edit = profile.querySelector(".settings-profile-edit");
      assert.ok(edit, "нет действия «Редактировать профили»");
      assert.equal(edit.textContent, ORACLE_PROFILE.action, "подпись действия не из оракула");
      // Редактирования профиля в спайке нет (нет панели) — честно disabled с причиной.
      assert.equal(edit.getAttribute("disabled"), "");
      const editReason = app.document.getElementById(edit.getAttribute("aria-describedby") ?? "");
      assert.ok(editReason, "у «Редактировать профили» нет aria-describedby-причины");
      assert.ok(editReason.textContent.length > 0);

      const account = nav
        .querySelectorAll(".settings-nav-item")
        .find((button) => button.textContent === "Учётная запись");
      assert.ok(account, "нет раздела «Учётная запись»");
      assert.equal(account.getAttribute("aria-expanded"), "true", "родитель не помечен раскрытым");
      const subnavId = account.getAttribute("aria-controls") ?? "";
      assert.ok(subnavId, "у «Учётной записи» нет aria-controls на список подпунктов");
      assert.ok(app.document.getElementById(subnavId), "aria-controls ведёт не на список подпунктов");

      const subnav = firstNode(app, `#${subnavId}`);
      const subs = subnav.querySelectorAll(".settings-subnav-item");
      assert.deepEqual(
        subs.map((node) => node.textContent),
        ORACLE_ACCOUNT_ITEMS,
        "подпункты «Учётной записи» не совпадают с развёрнутым дампом",
      );
      // Ровно 4: 17 остальных разделов в захвате свёрнуты — их состав не придумываем.
      assert.equal(
        nav.querySelectorAll(".settings-subnav-item").length,
        ORACLE_ACCOUNT_ITEMS.length,
        "в навигацию попали подпункты свёрнутых разделов",
      );
      // Подпункты идут сразу за «Учётной записью» (оракул: вложенный subnav).
      assert.equal(account.nextElementSibling, subnav, "список подпунктов не под своим разделом");
      for (const sub of subs) {
        assert.equal(sub.getAttribute("type"), "button", "подпункт без type=button");
        assert.equal(sub.getAttribute("disabled"), "", "подпункт без контента должен быть disabled");
        assert.equal(sub.getAttribute("aria-disabled"), "true");
        assert.ok((sub.getAttribute("title") ?? "").length > 0, "у подпункта нет причины в title");
        const reason = app.document.getElementById(sub.getAttribute("aria-describedby") ?? "");
        assert.ok(reason, "у подпункта нет aria-describedby-причины");
        assert.ok(reason.textContent.length > 0, "причина подпункта пустая");
      }
    });
  });

  it("настройки ST-01: «Выйти» → auth_logout без токена, результат видим", async () => {
    await withAppLogout({}, async (app) => {
      clickRail(app, "settings");
      const logout = firstNode(app, "#settings-nav .settings-nav-logout");
      assert.equal(logout.textContent, "Выйти");
      assert.equal(logout.getAttribute("disabled"), null, "«Выйти» заблокирован при живом IPC");
      assert.equal(logout.getAttribute("aria-disabled"), null, "aria-disabled остался у «Выйти»");
      assert.equal(logout.getAttribute("data-logout-state"), "idle");

      logout.click();
      await app.settle();

      const calls = app.callsOf("auth_logout");
      assert.equal(calls.length, 1, `auth_logout вызван ${calls.length} раз`);
      // Команда не принимает секретов: ни токена, ни аргументов.
      assert.equal(calls[0]?.args, null, "auth_logout получил аргументы (токен?)");
      assert.equal(logout.getAttribute("data-logout-state"), "done", "состояние выхода не обновилось");
      const status = firstNode(app, "#settings .settings-save-status");
      assert.equal(status.getAttribute("hidden"), null, "успех выхода не показан");
      assert.match(status.textContent, /вышл[ии]/i, "нет сообщения о выходе");
    });
  });

  it("настройки ST-01: ошибка logout видима, кнопка снова доступна", async () => {
    await withAppLogout(
      {
        onLogout: () => {
          throw new Error("keyring: locked");
        },
      },
      async (app) => {
        clickRail(app, "settings");
        const logout = firstNode(app, "#settings-nav .settings-nav-logout");
        logout.click();
        await app.settle();

        assert.equal(app.callsOf("auth_logout").length, 1, "auth_logout не вызван");
        assert.equal(logout.getAttribute("data-logout-state"), "error", "ошибка выхода не отмечена");
        const status = firstNode(app, "#settings .settings-save-status");
        assert.equal(status.getAttribute("hidden"), null, "ошибка выхода скрыта (молчаливый отказ)");
        assert.match(status.textContent, /не удалось выйти/i, "нет сообщения об ошибке");
        assert.match(status.textContent, /keyring: locked/, "текст ошибки IPC потерян");
        assert.equal(logout.getAttribute("disabled"), null, "«Выйти» остался заблокирован после ошибки");
        assert.equal(logout.getAttribute("aria-disabled"), null);
      },
    );
  });

  it("настройки ST-01: клавиатура, aria-current и причина disabled-раздела", async () => {
    await withApp((app) => {
      clickRail(app, "settings");
      const nav = byId(app, "settings-nav");
      const items = nav.querySelectorAll(".settings-nav-item");
      /** @param {string} label */
      const byLabel = (label) => items.find((button) => button.textContent === label);

      // Стартовый текущий раздел — ровно один aria-current="page".
      assert.equal(byLabel("Голос и видео")?.getAttribute("aria-current"), "page");
      assert.equal(
        items.filter((button) => button.getAttribute("aria-current") === "page").length,
        1,
        "aria-current=page не единственный",
      );

      // Недоступный раздел: причина видна и в title, и через aria-describedby.
      for (const button of items) {
        if (SETTINGS_ENABLED.includes(button.textContent)) continue;
        assert.ok((button.getAttribute("title") ?? "").length > 0, `«${button.textContent}» без title-причины`);
        const describedBy = button.getAttribute("aria-describedby") ?? "";
        assert.ok(describedBy.length > 0, `«${button.textContent}» без aria-describedby`);
        const reason = app.document.getElementById(describedBy);
        assert.ok(reason, `aria-describedby «${button.textContent}» ведёт в пустоту`);
        assert.ok(reason.textContent.length > 0, `пустая причина у «${button.textContent}»`);
      }

      // Стрелки идут по доступным пунктам (включая «Выйти»), Home/End и зацикливание.
      app.document.body.focus();
      nav.dispatchEvent("keydown", { key: "ArrowDown" });
      assert.equal(app.document.activeElement?.textContent, "Уведомления", "ArrowDown не дал первый пункт");
      nav.dispatchEvent("keydown", { key: "ArrowDown" });
      assert.equal(app.document.activeElement?.textContent, "Голос и видео", "ArrowDown пропустил пункт");
      nav.dispatchEvent("keydown", { key: "End" });
      assert.equal(app.document.activeElement?.textContent, "Выйти", "End не довёл до «Выйти»");
      nav.dispatchEvent("keydown", { key: "ArrowDown" });
      assert.equal(app.document.activeElement?.textContent, "Уведомления", "нет зацикливания вниз");
      nav.dispatchEvent("keydown", { key: "ArrowUp" });
      assert.equal(app.document.activeElement?.textContent, "Выйти", "ArrowUp не завернул вверх");
      nav.dispatchEvent("keydown", { key: "Home" });
      assert.equal(app.document.activeElement?.textContent, "Уведомления", "Home не в начало");

      // Активация сфокусированного пункта переключает панель и aria-current.
      const notifications = /** @type {ShimElement} */ (app.document.activeElement);
      notifications.click();
      assert.equal(byId(app, "settings").getAttribute("data-active-pane"), "notifications");
      assert.equal(notifications.getAttribute("aria-current"), "page", "активация не дала aria-current");
      assert.equal(byLabel("Голос и видео")?.getAttribute("aria-current"), null, "старый aria-current не снят");

      // Клавиатура не трогает disabled-разделы.
      nav.dispatchEvent("keydown", { key: "ArrowDown" });
      nav.dispatchEvent("keydown", { key: "ArrowDown" });
      assert.equal(
        app.document.activeElement?.textContent,
        "Внешний вид",
        "фокус попал на disabled-раздел",
      );
    });
  });

  it("настройки: «Голос и видео» — 38 тумблеров (12+22+4), 10 блоков, состояния из дампа", async () => {
    await withApp((app) => {
      clickRail(app, "settings");
      const settings = byId(app, "settings");
      const voice = settings.querySelector('[data-pane="voice"]');
      assert.ok(voice, "нет панели «Голос и видео»");
      const toggles = voice.querySelectorAll(".toggle");
      assert.equal(toggles.length, SETTINGS_TOGGLES, "тумблеров не 38");

      // SET-04: 10 блоков в порядке дампа (h4 внутри панели, h3 остаются 3).
      const blocks = voice.querySelectorAll(".settings-block-title");
      assert.deepEqual(blocks.map((node) => node.textContent), SETTINGS_BLOCKS);
      assert.equal(blocks.every((node) => node.tagName === "H4"), true, "блоки — не h4");

      for (const label of toggles) {
        assert.ok(label.querySelector(".toggle-label"), "нет .toggle-label");
        const input = label.querySelector('input[type="checkbox"]');
        assert.ok(input, "нет input[type=checkbox]");
        assert.equal(input.getAttribute("class"), "toggle-input", "input без .toggle-input");
        assert.equal(input.getAttribute("role"), "switch", "input без role=switch");
        assert.equal(
          input.getAttribute("aria-checked"),
          input.checked ? "true" : "false",
          "aria-checked рассинхронизирован",
        );
        assert.ok(label.querySelector(".toggle-track"), "нет .toggle-track");
        assert.ok(label.querySelector(".toggle-track .toggle-thumb"), "нет .toggle-thumb");
      }

      // SET-06/08: стартовые состояния — как в дампе, а не «все вкл».
      const off = toggles
        .filter((label) => {
          const input = label.querySelector('input[type="checkbox"]');
          return input?.checked === false;
        })
        .map((label) => label.querySelector(".toggle-label")?.textContent ?? "");
      assert.deepEqual(off, SETTINGS_OFF, "выключены не те четыре тумблера");

      // SET-07: 22 звуковых = 4 видимых + 18 за «Показать ещё 18 звуков».
      const soundBlock = blocks.find((node) => node.textContent === "Звуки");
      assert.ok(soundBlock, "нет блока «Звуки»");
      const soundToggles = () => {
        let node = soundBlock.nextElementSibling;
        /** @type {ShimElement[]} */
        const found = [];
        while (node && node.className !== "settings-block-title") {
          if (node.classList.contains("toggle")) found.push(node);
          else found.push(...node.querySelectorAll(".toggle"));
          node = node.nextElementSibling;
        }
        return found;
      };
      const sounds = soundToggles();
      assert.equal(sounds.length, 22, `звуковых тумблеров ${sounds.length}, SET-07 требует 22`);
      const more = settings.querySelector(".settings-sounds-more");
      assert.ok(more, "нет кнопки «Показать ещё 18 звуков»");
      assert.equal(more.getAttribute("aria-expanded"), "false");
      const hiddenBox = settings.querySelector(".settings-sounds-extra");
      assert.ok(hiddenBox, "нет контейнера для 18 скрытых звуков");
      assert.equal(hiddenBox.getAttribute("hidden"), "", "18 звуков не скрыты");
      assert.equal(hiddenBox.querySelectorAll(".toggle").length, 18, "скрыто не 18 звуков");
      /** @type {ShimElement} */ (more).click();
      assert.equal(hiddenBox.getAttribute("hidden"), null, "клик не показал 18 звуков");
      assert.equal(more.getAttribute("aria-expanded"), "true", "aria-expanded не переключился");

      // SET-09…12: селекты, ползунки, радиогруппа и кнопки вкладки.
      assert.equal(settings.querySelectorAll("select").length, 4, "селектов не 4 (SET-09)");
      const sliders = settings.querySelectorAll('input[type="range"]');
      assert.equal(sliders.length, 4, "ползунков не 4 (SET-10)");
      assert.deepEqual(
        settings.querySelectorAll(".settings-slider-value").map((node) => node.textContent),
        ["100 %", "100 %", "49 %", "26 %"],
        "подписи ползунков не из дампа (SET-10)",
      );
      const radios = settings.querySelectorAll('[role="radio"]');
      assert.equal(radios.length, 3, "радио «Профиль ввода» не 3 (SET-11)");
      assert.equal(
        radios.filter((node) => node.getAttribute("aria-checked") === "true").length,
        1,
        "отмечено не одно радио",
      );
      assert.ok(settings.querySelectorAll(".settings-action").length >= 5, "кнопок вкладки меньше 5");

      // Синхронизация aria-checked на click и change (контракт тумблера).
      const first = /** @type {ShimElement} */ (settings.querySelector('input[type="checkbox"]'));
      assert.equal(first.getAttribute("aria-checked"), "true");
      first.click();
      assert.equal(first.getAttribute("aria-checked"), "false", "click не синхронизировал aria-checked");
      first.checked = false;
      first.emit("change");
      assert.equal(first.getAttribute("aria-checked"), "false", "change не синхронизировал aria-checked");
    });
  });

  it("панель участников: ≤50 строк из model.js, высота 42, data-window=preview", async () => {
    await withApp(async (app) => {
      clickRail(app, "chat");
      await app.settle(6);
      const list = byId(app, "member-panel-list");
      const rows = list.querySelectorAll(".member");
      assert.ok(rows.length >= 10 && rows.length <= 50, `в панели ${rows.length} строк`);
      for (const row of rows) {
        assert.equal(row.getAttribute("data-window"), "preview", "нет data-window=preview");
        assert.equal(row.getAttribute("role"), "listitem", "строка панели без role=listitem");
        assert.equal(row.style.height, "42px", "высота строки панели не 42");
        assert.equal(row.children.length, 3, "в строке панели не 3 ребёнка");
        assert.match(row.children[1]?.textContent ?? "", /^member_\d+_/, "имя строки панели не member_<i>_");
      }
      const count = byId(app, "member-online").textContent.trim();
      assert.ok(/^\d[\d\s]*$/.test(count), `#member-online = «${count}», а не число`);
    });
  });

  it("клавиши: Alt+1…4, Escape, Tab-trap верхнего слоя", async () => {
    await withApp((app) => {
      const doc = app.document;
      key(app, "2", { altKey: true });
      assert.equal(doc.body.getAttribute("data-active-screen"), "emoji", "Alt+2 не открыл эмодзи");
      const trapped = key(app, "Tab");
      assert.equal(trapped.prevented, true, "Tab не удерживается в панели эмодзи");
      const escape = key(app, "Escape");
      assert.equal(escape.prevented, true, "Escape не обработан");
      assert.equal(doc.body.getAttribute("data-active-screen"), "chat", "Escape не вернул в чат");
      key(app, "4", { altKey: true });
      assert.equal(doc.body.getAttribute("data-active-screen"), "settings", "Alt+4 не открыл настройки");
      key(app, "Escape");
      key(app, "1", { altKey: true });
      assert.equal(doc.body.getAttribute("data-active-screen"), "chat", "Alt+1 не вернул чат");
    });
  });

  it("data-action: mute/deafen, emoji/emoji-close, settings/settings-close", async () => {
    await withApp((app) => {
      const doc = app.document;
      const mute = action(app, "mute");
      mute.click();
      assert.equal(mute.getAttribute("aria-pressed"), "true", "mute не переключил aria-pressed");
      mute.click();
      assert.equal(mute.getAttribute("aria-pressed"), "false", "mute не вернулся в false");
      action(app, "emoji").click();
      assert.equal(doc.body.getAttribute("data-active-screen"), "emoji", "data-action=emoji не открыл пикер");
      action(app, "emoji-close").click();
      assert.equal(doc.body.getAttribute("data-active-screen"), "chat", "emoji-close не вернул чат");
      action(app, "settings").click();
      assert.equal(doc.body.getAttribute("data-active-screen"), "settings", "data-action=settings");
      action(app, "settings-close").click();
      assert.equal(doc.body.getAttribute("data-active-screen"), "chat", "settings-close не вернул чат");
      action(app, "all-members").click();
      assert.equal(doc.body.getAttribute("data-active-screen"), "members", "all-members не открыл экран");
      action(app, "settings").click();
      action(app, "settings-close").click();
    });
  });

  it("клавиши §11: Ctrl+F/Ctrl+Shift+F, Ctrl+U, PageUp/PageDown, Shift+Esc", async () => {
    await withApp((app) => {
      const doc = app.document;
      const chatApi = api(app);
      // KEY-03: Ctrl+F — поиск с префиллом канала, Ctrl+Shift+F — без.
      key(app, "f", { ctrlKey: true });
      const overlay = api(app).search;
      assert.equal(overlay.open, true, "Ctrl+F не открыл оверлей поиска");
      const inField = /** @type {ShimElement | null} */ (
        doc.querySelector('.search-section[data-key="in"] .search-control')
      );
      assert.ok(inField, "нет поля «в» в оверлее");
      assert.equal(inField.value, "новости", "Ctrl+F не подставил канал в поле «в»");

      key(app, "F", { ctrlKey: true, shiftKey: true });
      assert.equal(overlay.open, true, "Ctrl+Shift+F не открыл оверлей");
      assert.equal(inField.value, "", "Ctrl+Shift+F оставил префилл канала");
      key(app, "Escape");

      // KEY-03: Ctrl+U — видимость панели участников (toggle).
      const memberPanel = byId(app, "member-panel");
      assert.notEqual(memberPanel.style.display, "none", "панель участников изначально скрыта");
      key(app, "u", { ctrlKey: true });
      assert.equal(memberPanel.style.display, "none", "Ctrl+U не скрыл панель");
      key(app, "u", { ctrlKey: true });
      assert.notEqual(memberPanel.style.display, "none", "Ctrl+U не вернул панель");

      // KEY-09: PageDown/PageUp прокручивают #chat-list, Home/End — края.
      chatApi.chat.list.scrollToIndex(0);
      const before = chatApi.chat.list.firstVisibleIndex();
      const pageDown = chatApi.chat.list.container.dispatchEvent("keydown", { key: "PageDown" });
      assert.equal(pageDown.prevented, true, "PageDown не обработан списком");
      const afterDown = chatApi.chat.list.firstVisibleIndex();
      assert.ok(afterDown > before, `PageDown не прокрутил (${before} → ${afterDown})`);
      chatApi.chat.list.container.dispatchEvent("keydown", { key: "PageUp" });
      assert.ok(
        chatApi.chat.list.firstVisibleIndex() < afterDown,
        "PageUp не вернул список назад",
      );

      // KEY-09: Shift+Esc сбрасывает плашку непрочитанного.
      const band = /** @type {ShimElement | null} */ (doc.querySelector(".chat-unread-divider"));
      assert.ok(band, "нет плашки непрочитанного");
      assert.notEqual(band.style.display, "none", "плашка изначально скрыта");
      const shiftEsc = key(app, "Escape", { shiftKey: true });
      assert.equal(shiftEsc.prevented, true, "Shift+Esc не обработан");
      assert.equal(band.getAttribute("data-cleared"), "true", "Shift+Esc не пометил плашку сброшенной");
      assert.equal(band.style.display, "none", "Shift+Esc не скрыл плашку");
    });
  });

  it("счётчики live (polite), FPS/p95 — не live", async () => {
    await withApp((app) => {
      assert.equal(byId(app, "member-count").getAttribute("aria-live"), "polite", "member-count не counter-live");
      const metrics = app.document.querySelector(".metrics");
      assert.ok(metrics, "нет .metrics");
      assert.equal(metrics.getAttribute("aria-live"), null, ".metrics (FPS) остался aria-live");
    });
  });

  it("штатная работа без console.error/warn (весь сценарий)", async () => {
    await withApp(async (app) => {
      const doc = app.document;
      // Навигация по всем экранам.
      for (const screen of ["emoji", "members", "settings", "chat"]) {
        clickRail(app, screen);
        await app.settle(2);
      }
      // Эмодзи: вкладка, стрелки, поиск и сброс.
      clickRail(app, "emoji");
      /** @type {ShimElement} */ (all(app, ".tab")[1]).click();
      byId(app, "emoji-grid").dispatchEvent("keydown", { key: "ArrowRight" });
      const search = byId(app, "emoji-search");
      search.value = "сердце";
      search.emit("input");
      search.value = "";
      search.emit("input");
      // Настройки: переключение тумблера.
      clickRail(app, "settings");
      const toggle = byId(app, "settings").querySelector('input[type="checkbox"]');
      assert.ok(toggle, "нет первого чекбокса");
      toggle.click();
      toggle.emit("change");
      clickRail(app, "chat");
      await app.settle(2);
      // Клавиши и действия.
      key(app, "2", { altKey: true });
      key(app, "Tab");
      key(app, "Escape");
      action(app, "mute").click();
      action(app, "mute").click();
      action(app, "emoji").click();
      action(app, "emoji-close").click();
      key(app, "1", { altKey: true });
      await app.settle(2);

      assert.equal(doc.body.getAttribute("data-active-screen"), "chat");
      assert.deepEqual(app.consoleErrors, [], "app.js дал console.error");
      assert.deepEqual(app.consoleWarns, [], "app.js дал console.warn");
    });
  });
});

/* =============================================================================
 * W39h / P3-02 «чат-вью» (SPEC-P3-02 §0 D-13, DOM-CONTRACT §8/§9.5).
 * Проверяем структуру и поведение, которые не проверялись W39d/QA-1: extras
 * строки (reply/reactions/embeds/edited), day divider, unread-полоса 34,
 * hover-панель 15 пунктов (R-16), композер (empty/typing/disabled, лимит 2000),
 * 6 групп участников (блок 42), search overlay (7 секций + 3 сортировки) и
 * честные счётчики «найдено / загружено».
 * ========================================================================== */

/** @param {GuiDom} app */
function api(app) {
  const instance = app.appModule?.currentApp?.();
  assert.ok(instance, "app.js не экспортирует currentApp() — нечего проверять");
  return instance;
}

/**
 * INT-07 (Pass 3): голосовые IPC-команды, которых нет в `generate_handler!`
 * `rust/gui-spike/src/main.rs`. Предварительная секция не должна их вызывать.
 */
const VOICE_IPC_COMMANDS = [
  "set_input_device",
  "set_output_device",
  "set_input_volume",
  "set_output_volume",
  "set_input_mode",
  "set_mute_state",
  "set_ptt_key",
];

/** @param {GuiDom} app @returns {number} */
function voiceIpcCalls(app) {
  return VOICE_IPC_COMMANDS.reduce((total, cmd) => total + app.callsOf(cmd).length, 0);
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

/**
 * @param {GuiDom} app
 * @param {ShimElement} node
 * @returns {void}
 */
function assertNamed(app, node) {
  const name = node.getAttribute("aria-label") ?? node.title ?? node.textContent.trim();
  assert.ok(String(name).length > 0, `интерактивный узел без доступного имени: ${node.className}`);
}

describe("чат-вью P3-02: сообщения и extras строки", () => {
  it("group start/continuation, day divider, reply-quote, reactions, embeds, edited", async () => {
    await withApp(async (app) => {
      await scrollChat(app, 0);
      const start = chatRow(app, 0);
      const cont = chatRow(app, 1);
      assert.ok(start && cont, "окно чата не содержит строк 0/1");
      assert.equal(start.getAttribute("data-group"), "start", "строка 0 не начало группы");
      assert.equal(cont.getAttribute("data-group"), "continuation", "строка 1 не продолжение");
      assert.equal(start.children.length, 2, "D-04/§7.1: у строки ровно 2 ребёнка (.author/.text)");

      // Day divider: отделитель внутри строки-«начала дня», а не ребёнок #chat-list.
      const divider = start.querySelector(".day-divider");
      assert.ok(divider, "у строки 0 нет .day-divider");
      assert.equal(divider.getAttribute("role"), "separator");
      assert.match(divider.getAttribute("aria-label") ?? "", /\d{4}/, "aria-label day divider без года");
      assert.match(divider.textContent, /\d/, "подпись day divider пустая");

      // Reply-quote: полоса + аватар + ник + текст, один ряд 22 (MSG-19…22).
      const quote = chatRow(app, 12)?.querySelector(".reply-quote");
      assert.ok(quote, "у строки 12 нет .reply-quote");
      assert.ok(quote.querySelector(".avatar"), "reply-quote без аватара");
      assert.ok((quote.querySelector(".reply-quote-author")?.textContent ?? "").length > 0);
      assert.ok((quote.querySelector(".reply-quote-text")?.textContent ?? "").length > 0);

      // Reactions-row: пилюли с aria-pressed и счётчиком (MSG-23…26).
      const pills = chatRow(app, 26)?.querySelectorAll(".reactions-row .reaction") ?? [];
      assert.ok(pills.length >= 1, "у строки 26 нет реакций");
      for (const pill of pills) {
        assert.equal(pill.tagName, "BUTTON", "реакция должна быть кнопкой");
        assert.match(pill.getAttribute("aria-pressed") ?? "", /^(true|false)$/);
        assert.ok((pill.getAttribute("aria-label") ?? "").length > 0, "реакция без имени");
        assert.match(pill.querySelector(".reaction-count")?.textContent ?? "", /^\d+$/);
      }

      // Edited-метка: суффикс в тексте и в aria-label строки (MSG-15).
      const edited = chatRow(app, 1)?.querySelector(".edited");
      assert.ok(edited, "у строки 1 нет .edited");
      assert.equal(edited.textContent, "(изменено)");
      assert.match(chatRow(app, 1)?.getAttribute("aria-label") ?? "", /изменено/);
    });
  });

  it("embeds: link-карточка и image-заглушка (без сети)", async () => {
    await withApp(async (app) => {
      await scrollChat(app, 9);
      const link = chatRow(app, 9)?.querySelector(".embed--link");
      assert.ok(link, "у строки 9 нет link-embed");
      assert.ok((link.querySelector(".embed-provider")?.textContent ?? "").length > 0);
      assert.ok((link.querySelector(".embed-title")?.textContent ?? "").length > 0);
      assert.ok(link.querySelector(".embed-description"), "у link-embed нет описания");

      await scrollChat(app, 400);
      const image = chatRow(app, 400)?.querySelector(".embed--image");
      assert.ok(image, "у строки 400 нет image-embed");
      const placeholder = image.querySelector(".embed-image");
      assert.ok(placeholder, "image-embed без контейнера картинки");
      assert.equal(placeholder.getAttribute("data-state"), "placeholder", "картинка не заглушка");
      assert.equal(placeholder.getAttribute("aria-hidden"), "true", "заглушка картинки шумит в AT");
      assert.ok((image.querySelector(".embed-caption")?.textContent ?? "").length > 0);
    });
  });

  it("link-embed: в href только http(s), javascript: не становится ссылкой (P0)", async () => {
    await withApp((app) => {
      const safe = createEmbed({
        kind: "link",
        provider: "docs.rust-lang.org",
        title: "Rust Book",
        url: "https://docs.rust-lang.org/book/",
      });
      const safeLink = safe.querySelector(".embed-title");
      assert.ok(safeLink, "у link-embed нет .embed-title");
      assert.equal(
        safeLink.getAttribute("href"),
        "https://docs.rust-lang.org/book/",
        "https: не попал в href",
      );
      assert.equal(safeLink.getAttribute("rel"), "noreferrer noopener");
      assert.equal(safeLink.getAttribute("aria-disabled"), null, "безопасная ссылка помечена disabled");

      // Красный путь до фикса: `href="javascript:alert(1)"` исполнялся по клику.
      for (const url of [
        "javascript:alert(document.domain)",
        "JavaScript:alert(1)",
        "data:text/html,<script>alert(1)</script>",
        "vbscript:msgbox(1)",
      ]) {
        const card = createEmbed({ kind: "link", title: "XSS", url });
        const link = card.querySelector(".embed-title");
        assert.ok(link, "у небезопасного link-embed нет .embed-title");
        const href = link.getAttribute("href");
        assert.ok(
          href === null || href === "#",
          `небезопасный URL попал в href: ${JSON.stringify(href)} (${url})`,
        );
        assert.doesNotMatch(String(href ?? ""), /javascript|vbscript|data:/i, `href сохранил схему: ${href}`);
        assert.equal(link.getAttribute("aria-disabled"), "true", `нет aria-disabled для ${url}`);
        assert.ok(
          String(link.getAttribute("title") ?? "").length > 0,
          `нет подписи (title) у недоступной ссылки для ${url}`,
        );
      }

      // Без url карточка остаётся честной заглушкой, а не ссылкой «#».
      const bare = createEmbed({ kind: "link", title: "Без адреса" });
      assert.equal(bare.querySelector(".embed-title")?.getAttribute("aria-disabled"), "true");
    });
  });

  it("unread-полоса 34 (D-13) — вне #chat-list, с честным счётчиком", async () => {
    await withApp((app) => {
      const bands = all(app, ".chat-unread-divider");
      assert.equal(bands.length, 1, `unread-полос ${bands.length}, ожидалась одна`);
      const [band] = bands;
      assert.ok(band, "нет .chat-unread-divider");
      assert.equal(band.getAttribute("data-height"), "34", "D-13: полоса 34 border-box");
      assert.equal(band.getAttribute("role"), "separator");
      assert.match(band.getAttribute("aria-label") ?? "", /нов/i);
      assert.match(band.textContent, /20\+ новых/);
      const list = byId(app, "chat-list");
      assert.equal(
        list.querySelectorAll(".chat-unread-divider").length,
        0,
        "полоса внутри #chat-list — нарушает §6.1 (у списка только sizer)",
      );
    });
  });

  it("hover-панель: 15 пунктов R-16, без мёртвых кнопок, реакция и reply работают", async () => {
    await withApp(async (app) => {
      await scrollChat(app, 0);
      const bars = all(app, ".msg-toolbar");
      assert.equal(bars.length, 1, "hover-панель должна быть одна (переиспользуется)");
      const [bar] = bars;
      assert.ok(bar, "нет .msg-toolbar");
      assert.equal(bar.getAttribute("data-open"), "false", "панель открыта без hover");
      assert.equal(bar.getAttribute("role"), "toolbar");

      const list = byId(app, "chat-list");
      const row = chatRow(app, 12);
      assert.ok(row, "нет строки 12 для hover");
      list.dispatchEvent("mouseover", { target: row });
      assert.equal(bar.getAttribute("data-open"), "true", "mouseover не открыл hover-панель");
      assert.equal(bar.getAttribute("data-for-index"), "12", "панель не привязана к строке");

      const buttons = bar.querySelectorAll("button");
      assert.equal(buttons.length, 15, `в панели ${buttons.length} пунктов, R-16 требует 15`);
      const reactions = buttons.filter((button) => button.getAttribute("data-emoji") !== null);
      assert.equal(reactions.length, 4, "в панели не 4 быстрые реакции");
      for (const button of buttons) {
        assertNamed(app, button);
        const disabled = button.getAttribute("aria-disabled") === "true";
        if (disabled) {
          assert.ok(
            (button.getAttribute("title") ?? "").length > 0,
            `aria-disabled без подсказки: «${button.getAttribute("aria-label")}»`,
          );
        } else {
          assert.ok(
            (button.getAttribute("data-command") ?? "").length > 0,
            `активный пункт без действия: «${button.getAttribute("aria-label")}»`,
          );
        }
      }

      // Рабочая реакция: состояние строки меняется, aria-pressed честный.
      const [like] = reactions;
      assert.ok(like, "в hover-панели нет быстрой реакции");
      like.click();
      const pill = chatRow(app, 12)?.querySelector(`.reaction[data-emoji="${like.getAttribute("data-emoji")}"]`);
      assert.ok(pill, "клик по реакции не добавил пилюлю в строку");
      assert.equal(pill.getAttribute("aria-pressed"), "true", "своя реакция не aria-pressed=true");

      // Рабочий reply: композер переходит в режим ответа, Esc отменяет.
      const reply = buttons.find((button) => button.getAttribute("data-command") === "reply");
      assert.ok(reply, "в панели нет пункта reply");
      reply.click();
      const form = byId(app, "composer");
      assert.equal(form.getAttribute("data-reply"), "true", "reply не включил режим ответа");
      const chip = form.querySelector(".composer-reply");
      assert.ok(chip, "нет чипа «ответ на»");
      assert.match(chip.textContent, /sasha|mira|kot|dev_null|lena|arty|north|zoe/);
    });
  });

  it("hover-панель: быстрая реакция уходит в IPC один раз и откатывается при 401", async () => {
    const { resetReactions } = await import("../../rust/gui-spike/ui/features/reactions.js");
    /** @type {((value: any) => void) | null} */
    let resolveToggle = null;
    const app = await loadApp({ invokeHandler: (cmd) => cmd === "toggle_reaction"
      ? new Promise((resolve) => { resolveToggle = resolve; })
      : undefined });
    try {
      resetReactions();
      await scrollChat(app, 0);
      const list = byId(app, "chat-list");
      const row = chatRow(app, 26); assert.ok(row);
      list.dispatchEvent("mouseover", { target: row });
      const bar = firstNode(app, ".msg-toolbar");
      const like = all(app, '.msg-toolbar [data-command="react"]')[0];
      assert.ok(like, "в панели нет быстрой реакции");
      const emoji = like.getAttribute("data-emoji") ?? "";
      like.click();
      assert.equal(app.callsOf("toggle_reaction").length, 1, "панель не отправила toggle_reaction");
      assert.deepEqual(app.callsOf("toggle_reaction")[0]?.args, {
        channelId: "news", messageId: "26", emoji, active: true,
      });
      assert.equal(like.getAttribute("aria-pressed"), "true", "панель не показала optimistic-состояние");
      const pill = chatRow(app, 26)?.querySelector(`.reaction[data-emoji="${emoji}"]`);
      assert.ok(pill, "optimistic-реакция не появилась в строке");
      assert.equal(pill.getAttribute("aria-pressed"), "true");

      // Двойной клик до ответа — по-прежнему один запрос.
      like.click();
      assert.equal(app.callsOf("toggle_reaction").length, 1, "двойной клик отправил второй запрос");
      assert.equal(bar.getAttribute("data-open"), "true");
      /** @type {any} */ (resolveToggle)(null);
      await app.settle(6);
      assert.equal(like.getAttribute("data-pending"), null, "pending не снят после ответа");

      // 401: optimistic-состояние откатывается, ошибка видна.
      resetReactions();
      app.setInvokeHandler(async (cmd) => { if (cmd === "toggle_reaction") throw { status: 401, message: "unauthorized" }; return null; });
      list.dispatchEvent("mouseover", { target: chatRow(app, 26) });
      like.click();
      assert.equal(like.getAttribute("aria-pressed"), "true", "нет optimistic-состояния перед откатом");
      await app.settle(6);
      assert.equal(like.getAttribute("aria-pressed"), "false", "401 не откатил реакцию панели");
      assert.equal(all(app, ".error-state").length, 1, "ошибка IPC не видна");
    } finally {
      resetReactions();
      app.dispose();
    }
  });

  it("B4b: edit сохраняет через IPC, оптимистично меняет текст и откатывает ошибку", async () => {
    await withApp(async (app) => {
      app.setInvokeHandler(async () => null);
      await scrollChat(app, 0);
      const list = byId(app, "chat-list");
      const row = chatRow(app, 12); assert.ok(row);
      list.dispatchEvent("mouseover", { target: row });
      const edit = all(app, ".msg-toolbar button").find((button) => button.getAttribute("aria-label") === "Изменить сообщение");
      assert.ok(edit);
      assert.equal(edit.getAttribute("aria-disabled"), null);
      edit.click();
      const editor = firstNode(app, '[data-action="edit-input"]');
      editor.value = "обновлено";
      editor.dispatchEvent("keydown", { key: "Enter" });
      assert.match(row.textContent, /обновлено/);
      await app.settle(2);
      assert.deepEqual(app.callsOf("edit_message")[0]?.args, {
        channelId: "news", messageId: "12", content: "обновлено",
      });

      app.setInvokeHandler(async (cmd) => { if (cmd === "edit_message") throw { status: 429, message: "rate limited" }; return null; });
      list.dispatchEvent("mouseover", { target: row });
      edit.click();
      const failedEditor = firstNode(app, '[data-action="edit-input"]');
      failedEditor.value = "сломано";
      failedEditor.dispatchEvent("keydown", { key: "Enter" });
      await app.settle(2);
      assert.match(row.textContent, /обновлено/);
      assert.match(app.document.body.textContent, /ограничение|rate limited|Ошибка/);
    });
  });

  it("B4b: повторный submit edit не отправляет второй edit_message до ответа", async () => {
    /** @type {((value: any) => void) | null} */
    let resolveEdit = null;
    const app = await loadApp({ invokeHandler: (cmd) => cmd === "edit_message"
      ? new Promise((resolve) => { resolveEdit = resolve; })
      : undefined });
    try {
      await scrollChat(app, 0);
      const list = byId(app, "chat-list");
      const row = chatRow(app, 12); assert.ok(row);
      list.dispatchEvent("mouseover", { target: row });
      const edit = all(app, ".msg-toolbar button").find((button) => button.getAttribute("aria-label") === "Изменить сообщение");
      assert.ok(edit);
      edit.click();
      const editor = firstNode(app, '[data-action="edit-input"]');
      editor.value = "одна правка";
      editor.dispatchEvent("keydown", { key: "Enter" });
      // Enter + клик «Сохранить» до ответа IPC — один запрос, а не два.
      editor.dispatchEvent("keydown", { key: "Enter" });
      assert.equal(app.callsOf("edit_message").length, 1, "повторный submit отправил второй edit_message");
      /** @type {any} */ (resolveEdit)(null);
      await app.settle(4);
      assert.equal(app.callsOf("edit_message").length, 1);
    } finally {
      app.dispose();
    }
  });

  it("B4b: delete требует подтверждение и возвращает строку при ошибке", async () => {
    await withApp(async (app) => {
      await scrollChat(app, 0);
      const list = byId(app, "chat-list");
      const row = chatRow(app, 12); assert.ok(row);
      list.dispatchEvent("mouseover", { target: row });
      const remove = all(app, ".msg-toolbar button").find((button) => button.getAttribute("aria-label") === "Удалить сообщение");
      assert.ok(remove);
      assert.equal(remove.getAttribute("aria-disabled"), null);
      remove.click();
      assert.equal(app.callsOf("delete_message").length, 0);
      assert.match(remove.textContent, /подтверд/i);
      remove.click();
      assert.equal(row.getAttribute("data-state"), "deleted");
      await app.settle(2);
      assert.deepEqual(app.callsOf("delete_message")[0]?.args, { channelId: "news", messageId: "12" });

      app.setInvokeHandler(async (cmd) => { if (cmd === "delete_message") throw { status: 401, message: "unauthorized" }; return null; });
      list.dispatchEvent("mouseover", { target: row });
      remove.click(); remove.click();
      await app.settle(2);
      assert.notEqual(row.getAttribute("data-state"), "deleted");
      assert.match(app.document.body.textContent, /сессия истекла|Ошибка/);
    });
  });

  it("B4b: удалённая строка не возвращается из PageStore после выгрузки и не оставляет пустого места", async () => {
    await withApp(async (app) => {
      app.setInvokeHandler(async () => null);
      await scrollChat(app, 0);
      const list = byId(app, "chat-list");
      const row = chatRow(app, 12); assert.ok(row);
      const beforeTop = Number.parseFloat(chatRow(app, 13)?.style.top ?? "NaN");
      list.dispatchEvent("mouseover", { target: row });
      const remove = all(app, ".msg-toolbar button").find((button) => button.getAttribute("aria-label") === "Удалить сообщение");
      assert.ok(remove);
      remove.click(); remove.click();
      await app.settle(2);
      assert.deepEqual(app.callsOf("delete_message")[0]?.args, { channelId: "news", messageId: "12" });
      assert.ok(chatRow(app, 12) === null, "удалённая строка осталась в DOM");

      // Соседние строки сомкнулись: строка 13 поднялась на высоту удалённой.
      const prev = chatRow(app, 11); const next = chatRow(app, 13);
      assert.ok(prev, "нет строки 11 после удаления");
      assert.ok(next, "нет строки 13 после удаления");
      const prevBottom = Number.parseFloat(prev.style.top) + Number.parseFloat(prev.style.height);
      assert.equal(Number.parseFloat(next.style.top), prevBottom, "между 11 и 13 осталось пустое место");
      assert.ok(Number.parseFloat(next.style.top) < beforeTop, "строки ниже удалённой не сдвинулись");

      // Выгружаем страницу 0 LRU-скроллом и возвращаемся: tombstone держится.
      for (const target of [1000, 2000, 3000, 4000, 5000]) await scrollChat(app, target);
      assert.equal(app.appModule.currentApp().chat.store.pages.has(0), false, "страница 0 не выгрузилась");
      await scrollChat(app, 0);
      assert.ok(chatRow(app, 12) === null, "удалённая строка вернулась из PageStore");
      const backPrev = chatRow(app, 11); const backNext = chatRow(app, 13);
      assert.ok(backPrev && backNext);
      assert.equal(
        Number.parseFloat(backNext.style.top),
        Number.parseFloat(backPrev.style.top) + Number.parseFloat(backPrev.style.height),
        "после перезагрузки страницы появилось пустое место",
      );
    });
  });

  it("B4b: reply отправляет reply_message с replyTo и отменяется Escape", async () => {
    await withApp(async (app) => {
      await scrollChat(app, 0);
      const list = byId(app, "chat-list");
      const row = chatRow(app, 12); assert.ok(row);
      list.dispatchEvent("mouseover", { target: row });
      firstNode(app, '.msg-toolbar [data-command="reply"]').click();
      const form = byId(app, "composer");
      assert.equal(form.getAttribute("data-reply"), "true");
      const input = byId(app, "composer-input");
      input.value = "ответ"; input.emit("input"); input.dispatchEvent("keydown", { key: "Enter" });
      await app.settle(2);
      assert.deepEqual(app.callsOf("reply_message")[0]?.args, { channelId: "news", content: "ответ", replyTo: "12" });
      assert.equal(form.getAttribute("data-reply"), "false");

      list.dispatchEvent("mouseover", { target: row });
      firstNode(app, '.msg-toolbar [data-command="reply"]').click();
      input.dispatchEvent("keydown", { key: "Escape" });
      assert.equal(form.getAttribute("data-reply"), "false");
    });
  });
});

describe("B3b: UI реакций", () => {
  it("отправляет toggle_reaction и оптимистично меняет счётчик/aria-pressed", async () => {
    await withApp(async (app) => {
      await scrollChat(app, 0);
      const pill = firstNode(app, '#chat-list .row[data-index="26"] .reaction[data-emoji="✅"]');
      assert.equal(pill.getAttribute("aria-pressed"), "false");
      assert.equal(pill.querySelector(".reaction-count")?.textContent, "1");
      pill.click();
      assert.equal(pill.getAttribute("aria-pressed"), "true");
      assert.equal(pill.querySelector(".reaction-count")?.textContent, "2");
      await app.settle(4);
      const calls = app.callsOf("toggle_reaction");
      assert.equal(calls.length, 1);
      assert.deepEqual(calls[0]?.args, { channelId: "news", messageId: "26", emoji: "✅", active: true });
    });
  });

  for (const failure of [
    { label: "401", error: { status: 401, message: "unauthorized" } },
    { label: "429", error: { status: 429, retry_after_ms: 250, message: "rate limited" } },
    { label: "сеть", error: new Error("offline") },
  ]) {
    it(`откатывает optimistic состояние при ${failure.label}`, async () => {
      const app = await loadApp({ invokeHandler: async (cmd) => {
        if (cmd === "toggle_reaction") throw failure.error;
        return null;
      } });
      try {
        await scrollChat(app, 0);
        const pill = firstNode(app, '#chat-list .row[data-index="26"] .reaction[data-emoji="✅"]');
        pill.click();
        await app.settle(6);
        const restored = firstNode(app, '#chat-list .row[data-index="26"] .reaction[data-emoji="✅"]');
        assert.equal(restored.getAttribute("aria-pressed"), "false");
        assert.equal(restored.querySelector(".reaction-count")?.textContent, "1");
        assert.equal(restored.parentNode?.parentNode?.getAttribute("data-index"), "26", "строка потеряна при ошибке");
        assert.equal(all(app, ".error-state").length, 1, "ошибка не видна");
      } finally {
        app.dispose();
      }
    });
  }

  it("двойной клик до ответа делает один запрос и не оставляет pending", async () => {
    /** @type {((value: any) => void) | null} */
    let resolveRequest = null;
    const app = await loadApp({ invokeHandler: (cmd) => cmd === "toggle_reaction"
      ? new Promise((resolve) => { resolveRequest = resolve; }) : null });
    try {
      await scrollChat(app, 0);
      const pill = firstNode(app, '#chat-list .row[data-index="26"] .reaction[data-emoji="✅"]');
      pill.click();
      pill.click();
      assert.equal(app.callsOf("toggle_reaction").length, 1);
      assert.equal(pill.getAttribute("data-pending"), "true");
      /** @type {any} */ (resolveRequest)?.(null);
      await app.settle(6);
      assert.equal(firstNode(app, '#chat-list .row[data-index="26"] .reaction[data-emoji="✅"]').getAttribute("data-pending"), null);
    } finally {
      app.dispose();
    }
  });

  it("неизвестная или пустая реакция не вызывает IPC и показывает подсказку", async () => {
    await withApp(async (app) => {
      await scrollChat(app, 0);
      const row = firstNode(app, '#chat-list .row[data-index="26"]');
      const before = row.querySelectorAll(".reaction").length;
      const { toggleReaction } = await import("../../rust/gui-spike/ui/features/reactions.js");
      assert.throws(() => toggleReaction(9, "", []), /пустая/);
      assert.throws(() => toggleReaction(9, "🧪", [{ emoji: "🔥", count: 2, me: false }]), /неизвестн/i);
      assert.equal(row.querySelectorAll(".reaction").length, before);
      assert.equal(app.callsOf("toggle_reaction").length, 0);
    });
  });
});

describe("чат-вью P3-02: композер", () => {
  it("empty → typing → empty, счётчик с 1800, лимит 2000", async () => {
    await withApp((app) => {
      const form = byId(app, "composer");
      const input = byId(app, "composer-input");
      const counter = form.querySelector(".composer-counter");
      assert.ok(counter, "нет .composer-counter");
      assert.equal(form.getAttribute("data-state"), "empty", "стартовое состояние не empty");
      assert.equal(counter.getAttribute("data-visible"), "false", "счётчик виден на пустом поле");

      input.value = "привет";
      input.emit("input");
      assert.equal(form.getAttribute("data-state"), "typing", "ввод не перевёл в typing");

      input.value = "а".repeat(1799);
      input.emit("input");
      assert.equal(counter.getAttribute("data-visible"), "false", "счётчик появился раньше 1800");

      input.value = "а".repeat(1800);
      input.emit("input");
      assert.equal(counter.getAttribute("data-visible"), "true", "счётчик не появился на 1800");
      assert.equal(counter.textContent, "1800/2000");

      // CMP-13: при превышении ввод блокируется — значение обрезается до 2000.
      input.value = "а".repeat(2100);
      input.emit("input");
      assert.equal(input.value.length, 2000, "лимит 2000 не обрезал ввод");
      assert.equal(form.getAttribute("data-limit"), "exceeded", "нет метки превышения лимита");
      assert.equal(counter.getAttribute("data-exceeded"), "true", "счётчик не покраснел");
      assert.equal(counter.textContent, "2000/2000");

      input.value = "";
      input.emit("input");
      assert.equal(form.getAttribute("data-state"), "empty", "очистка не вернула empty");
      assert.equal(form.getAttribute("data-limit"), "none", "метка лимита не снялась");
    });
  });

  it("вставка эмодзи в позицию курсора; лимит 2000 и disabled не ломаются", async () => {
    await withApp((app) => {
      const composer = api(app).composer;
      const form = byId(app, "composer");
      const input = byId(app, "composer-input");

      input.value = "привет мир";
      input.emit("input");
      input.selectionStart = 6;
      input.selectionEnd = 6;
      clickRail(app, "emoji");
      /** @type {ShimElement} */ (all(app, ".emoji-cell")[0]).click();
      const glyph = all(app, ".emoji-cell")[0]?.textContent ?? "";
      assert.equal(input.value, `привет${glyph} мир`, "эмодзи вставлен не в позицию курсора");
      assert.equal(form.getAttribute("data-state"), "typing", "вставка не обновила состояние");
      assert.equal(
        form.querySelector(".composer-counter")?.textContent,
        `${input.value.length}/2000`,
        "счётчик не учёл вставку",
      );

      // Выделение заменяется, а не дублируется (эмодзи в UTF-16 — 2 code unit).
      const wordStart = input.value.indexOf("мир");
      assert.ok(wordStart > 0, "в поле нет слова «мир»");
      input.selectionStart = wordStart;
      input.selectionEnd = wordStart + 3;
      composer.insertText("🌍");
      assert.equal(input.value, `привет${glyph} 🌍`, "выделение не заменено вставкой");

      // Лимит: вставка режется до 2000 без разрыва суррогатной пары.
      input.value = "а".repeat(1998);
      input.emit("input");
      input.selectionStart = 1998;
      input.selectionEnd = 1998;
      composer.insertText("😀");
      assert.equal(input.value.length, 2000, "вставка превысила лимит 2000");
      assert.equal(input.value.endsWith("😀"), true, "эмодзи потерялся при обрезке");
      assert.equal(form.getAttribute("data-limit"), "exceeded", "нет exceeded после вставки");

      // Если эмодзи не влезает целиком — он не рвётся на половину.
      input.value = "а".repeat(1999);
      input.emit("input");
      input.selectionStart = 1999;
      input.selectionEnd = 1999;
      composer.insertText("😀");
      assert.equal(input.value.length, 1999, "половина суррогатной пары попала в поле");
      assert.equal(input.value.endsWith("а"), true, "обрезка испортила текст");

      // Disabled: вставка не меняет поле и сообщает false.
      composer.setDisabled(true, "нет прав");
      const frozen = input.value;
      assert.equal(composer.insertText("🔥"), false, "вставка в disabled вернула true");
      assert.equal(input.value, frozen, "вставка в disabled изменила поле");
      composer.setDisabled(false);
    });
  });

  it("disabled: aria-disabled, поле в DOM, attach — affordance с подсказкой", async () => {
    await withApp((app) => {
      const form = byId(app, "composer");
      const input = byId(app, "composer-input");
      assert.ok(input, "поле исчезло из DOM");

      api(app).composer.setDisabled(true, "нет прав на запись в канал");
      assert.equal(form.getAttribute("data-state"), "disabled", "setDisabled не перевёл в disabled");
      assert.equal(input.getAttribute("aria-disabled"), "true", "у поля нет aria-disabled");
      assert.equal(input.getAttribute("disabled"), null, "CMP-07: поле остаётся фокусируемым");
      assert.match(form.getAttribute("data-disabled-reason") ?? "", /прав/i);

      input.value = "текст";
      input.emit("input");
      assert.equal(form.getAttribute("data-state"), "disabled", "ввод в disabled сменил состояние");

      api(app).composer.setDisabled(false);
      assert.equal(form.getAttribute("data-state"), "empty", "снятие disabled не вернуло empty");
      assert.equal(input.getAttribute("aria-disabled"), null, "aria-disabled не снят");

      const attach = action(app, "attach");
      assert.equal(attach.getAttribute("aria-disabled"), "true", "attach не помечен aria-disabled");
      assert.ok((attach.getAttribute("title") ?? "").length > 0, "у attach нет подсказки");
      const emoji = action(app, "emoji");
      assert.equal(emoji.getAttribute("aria-haspopup"), "dialog", "emoji без aria-haspopup=dialog");
    });
  });

  it("gift: явная заглушка Phase 3 — disabled-affordance, клик без ACTION_FAILED (P1)", async () => {
    await withApp((app) => {
      const gift = action(app, "gift");
      // Заглушка должна быть видимой для AT и мыши, а не «мёртвой» кнопкой.
      assert.equal(gift.getAttribute("aria-disabled"), "true", "gift не помечен aria-disabled");
      assert.ok((gift.getAttribute("title") ?? "").length > 0, "у gift нет подписи (title)");
      assert.match(String(gift.getAttribute("title") ?? ""), /Phase 3|спайк/i, "подпись gift не объясняет недоступность");
      assert.ok(
        String(gift.getAttribute("aria-label") ?? gift.textContent).length > 0,
        "gift без доступного имени",
      );

      // Красный путь до фикса: неизвестный data-action → reportError(ACTION_FAILED).
      gift.click();
      gift.click();
      assert.deepEqual(app.consoleErrors, [], "клик по gift дал ACTION_FAILED/console.error");
      assert.deepEqual(app.consoleWarns, [], "клик по gift дал console.warn");
      // Состояние остаётся честным: кнопка всё ещё disabled-заглушка, не «нажата».
      assert.equal(gift.getAttribute("aria-disabled"), "true", "клик снял disabled-состояние gift");
    });
  });

  it("Ctrl+E открывает пикер, Escape возвращает чат и не ломает черновик", async () => {
    await withApp((app) => {
      const input = byId(app, "composer-input");
      input.value = "черновик";
      input.emit("input");
      key(app, "e", { ctrlKey: true });
      assert.equal(app.document.body.getAttribute("data-active-screen"), "emoji", "Ctrl+E не открыл пикер");
      key(app, "Escape");
      assert.equal(app.document.body.getAttribute("data-active-screen"), "chat", "Escape не вернул чат");
      assert.equal(byId(app, "composer-input").value, "черновик", "черновик потерялся");
      assert.equal(byId(app, "composer").getAttribute("data-state"), "typing");
    });
  });
});

describe("чат-вью P3-02: участники — 6 групп (D-13)", () => {
  const ROLE_GROUPS = ["АДМИНИСТРАЦИЯ", "МОДЕРАТОРЫ", "УЧАСТНИКИ"];
  const STATUS_GROUPS = ["В СЕТИ", "НЕАКТИВНЫ", "НЕ В СЕТИ"];

  it("панель чата: 6 групп, заголовок 42, счётчики = строкам группы", async () => {
    await withApp(async (app) => {
      clickRail(app, "chat");
      await app.settle(8);
      const panel = byId(app, "member-panel-list");
      const groups = panel.querySelectorAll(".member-group");
      assert.equal(groups.length, 6, `групп ${groups.length}, D-13/MEM-02 требует 6`);

      const labels = groups.map((group) => {
        const title = group.querySelector(".member-group-title");
        assert.ok(title, "у группы нет .member-group-title");
        assert.equal(title.getAttribute("data-height"), "42", "D-13: блок заголовка группы 42");
        assert.match(title.getAttribute("aria-label") ?? "", /загруженн/i, "счётчик не помечен как оконный");
        return title.textContent;
      });
      for (const name of [...ROLE_GROUPS, ...STATUS_GROUPS]) {
        assert.ok(
          labels.some((label) => label.startsWith(name)),
          `нет группы «${name}» (есть: ${labels.join(" | ")})`,
        );
      }
      // Порядок MEM-04: сначала роли, затем статусы.
      assert.deepEqual(
        labels.map((label) => label.split(" — ")[0]?.trim()),
        [...ROLE_GROUPS, ...STATUS_GROUPS],
      );

      // Счётчик — по загруженному окну (может быть больше строк-превью), а сами
      // строки обязаны принадлежать своей группе. Независимая проверка состава —
      // по data-role/data-status, а не по порядку в разметке.
      /** @type {Record<string, string>} */
      const expectedRows = { "role-admin": "ADMIN", "role-mod": "MOD", "role-guest": "GUEST" };
      /** @type {Record<string, string[]>} */
      const expectedStatuses = {
        "status-online": ["online"],
        "status-idle": ["idle", "dnd"],
        "status-offline": ["offline"],
      };
      for (const group of groups) {
        const key = group.getAttribute("data-group-key") ?? "";
        const title = /** @type {ShimElement} */ (group.querySelector(".member-group-title"));
        const count = Number((title.textContent.split(" — ")[1] ?? "").trim());
        const rows = group.querySelectorAll(".member");
        assert.ok(count >= rows.length, `счётчик «${title.textContent}» меньше числа строк`);
        assert.ok(count <= 50, `счётчик группы ${count} > 50 (MEM-16)`);
        assert.ok(rows.length >= 1, `группа «${key}» пустая`);
        for (const row of rows) {
          assert.equal(row.style.height, "42px", "строка участника не 42");
          assert.equal(row.getAttribute("data-group"), key, "строка не из своей группы");
          if (key in expectedRows) {
            assert.equal(row.getAttribute("data-role"), expectedRows[key]);
          } else {
            assert.ok(
              expectedStatuses[key]?.includes(row.getAttribute("data-status") ?? ""),
              `в «${key}» попал статус ${row.getAttribute("data-status")}`,
            );
          }
        }
      }
      assert.ok(panel.querySelectorAll(".member").length <= 50, "в панели больше 50 строк (MEM-16)");
    });
  });

  it("экран участников: чипы 6 групп, roving tabindex, клик выбирает строку группы", async () => {
    await withApp(async (app) => {
      clickRail(app, "members");
      await app.settle(8);
      const chips = all(app, ".member-group-chip");
      assert.equal(chips.length, 6, `чипов ${chips.length}, ожидалось 6`);
      const bar = all(app, ".member-groups")[0];
      assert.ok(bar, "нет контейнера .member-groups");
      assert.equal(bar.getAttribute("role"), "toolbar");

      assert.equal(chips.filter((chip) => chip.getAttribute("tabindex") === "0").length, 1, "roving: не один tabindex=0");
      bar.dispatchEvent("keydown", { key: "ArrowRight" });
      assert.equal(chips[0]?.getAttribute("tabindex"), "-1", "стрелка не сняла tabindex со старого чипа");
      assert.equal(chips[1]?.getAttribute("tabindex"), "0", "стрелка не перевела tabindex");

      const online = chips.find((chip) => chip.getAttribute("data-group-key") === "status-online");
      assert.ok(online, "нет чипа группы «В СЕТИ»");
      online.click();
      assert.equal(online.getAttribute("aria-pressed"), "true", "чип не стал aria-pressed");
      assert.equal(
        chips.filter((chip) => chip.getAttribute("aria-pressed") === "true").length,
        1,
        "нажатых чипов больше одного",
      );
      const selected = all(app, "#member-list .member").filter(
        (row) => row.classList.contains("is-selected"),
      );
      assert.equal(selected.length, 1, "клик по чипу не выбрал строку");
      assert.match(selected[0]?.getAttribute("aria-label") ?? "", /в сети/);
    });
  });

  it("поиск участников по загруженному окну: честный счётчик", async () => {
    await withApp(async (app) => {
      clickRail(app, "members");
      await app.settle(8);
      const search = byId(app, "member-search");
      search.value = "member_1_";
      search.emit("input");
      const text = byId(app, "member-count").textContent.trim();
      const match = text.match(/найдено (\d+) из (\d+) загруженных/);
      assert.ok(match, `счётчик «${text}» не в форме «найдено N из M загруженных»`);
      const found = Number(match[1]);
      const loaded = Number(match[2]);
      assert.ok(found > 0, "поиск по member_1_ ничего не нашёл");
      assert.ok(loaded <= 500, `загруженных ${loaded} > 500 (ADR-1)`);
      // Независимый оракул: считаем по самим данным загруженного окна, а не по коду поиска.
      let expected = 0;
      for (const [, page] of api(app).members.store.pages) {
        for (const row of page.rows) {
          if (
            String(row.name ?? "").toLowerCase().includes("member_1_") ||
            String(row.activity ?? "").toLowerCase().includes("member_1_")
          ) expected += 1;
        }
      }
      assert.equal(found, expected, "счётчик поиска не совпадает с независимым подсчётом");
      assert.equal(loaded, api(app).members.store.stats().loadedRows, "«загружено» не равно окну стора");
    });
  });
});

describe("чат-вью P3-02: search overlay", () => {
  const SECTIONS = ["От", "В", "Содержит", "Упоминания", "Дата", "Тип автора", "Закреплено"];
  const SORTS = ["По новизне", "Самое старое", "Самое актуальное"];

  it("7 секций, 5 input, сортировка из 3 пунктов, честный статус", async () => {
    await withApp(async (app) => {
      const header = byId(app, "chat-search");
      header.value = "";
      header.emit("input");
      const panels = all(app, ".search-overlay");
      assert.equal(panels.length, 1, "нет .search-overlay");
      const panel = firstNode(app, ".search-overlay");
      assert.equal(panel.getAttribute("data-open"), "true", "ввод в #chat-search не открыл оверлей");

      const sections = panel.querySelectorAll(".search-section");
      assert.equal(sections.length, 7, `секций ${sections.length}, R-10 требует 7`);
      assert.deepEqual(
        sections.map((section) => section.querySelector(".search-section-title")?.textContent.trim()),
        SECTIONS,
      );
      assert.equal(panel.querySelectorAll(".search-section input").length, 5, "в секциях не 5 input");
      assert.equal(panel.querySelectorAll(".search-section select").length, 1, "нет select типа автора");
      assert.equal(panel.querySelectorAll('.search-section button[role="switch"]').length, 1, "нет switch «Закреплено»");
      assert.equal(panel.querySelectorAll(".search-option").length, 9, "«Содержит»: опций не 9");

      const trigger = panel.querySelector(".search-sort-trigger");
      assert.ok(trigger, "нет триггера сортировки");
      assert.equal(trigger.getAttribute("aria-haspopup"), "menu");
      assert.equal(trigger.getAttribute("aria-expanded"), "false", "меню открыто до клика");
      trigger.click();
      assert.equal(trigger.getAttribute("aria-expanded"), "true", "триггер не раскрыл меню");
      const items = panel.querySelectorAll('.search-sort-menu [role="menuitemradio"]');
      assert.equal(items.length, 3, "в меню сортировки не 3 пункта");
      assert.deepEqual(items.map((item) => item.textContent.trim()), SORTS);
      assert.equal(items.filter((item) => item.getAttribute("aria-checked") === "true").length, 1);

      const status = panel.querySelector(".search-status");
      assert.ok(status, "нет .search-status");
      assert.equal(status.getAttribute("aria-live"), "polite");
      assert.match(status.textContent, /найдено \d+ из \d+ загруженных/);
      assert.match(status.textContent, /100 000/, "статус не называет полный объём канала");

      const results = panel.querySelectorAll(".search-result");
      assert.ok(results.length > 0 && results.length <= 6, `результатов ${results.length} (срез 6)`);
      for (const result of results) {
        assert.ok(result.querySelector(".search-result-head"), "у результата нет шапки");
        assert.ok(result.querySelector(".search-result-text"), "у результата нет фрагмента");
        assertNamed(app, result);
      }
    });
  });

  it("фильтр по запросу и сортировка меняют выдачу; счётчик остаётся оконным", async () => {
    await withApp(async (app) => {
      const header = byId(app, "chat-search");
      header.value = "привет";
      header.emit("input");
      // B7b: первый запрос пробует `search_remote`; в DOM-шиме команды нет,
      // поэтому дожидаемся честного локального fallback.
      await api(app).search.whenIdle();
      const panel = firstNode(app, ".search-overlay");
      const status = panel.querySelector(".search-status");
      assert.ok(status, "нет .search-status");
      const found = Number(status.textContent.match(/найдено (\d+)/)?.[1] ?? NaN);
      assert.ok(found > 0, `запрос «привет» ничего не нашёл: ${status.textContent}`);
      assert.match(status.textContent, new RegExp(`найдено ${found} из`));

      const firstNew = Number(
        panel.querySelector(".search-result")?.getAttribute("data-index") ?? NaN,
      );
      const trigger = panel.querySelector(".search-sort-trigger");
      assert.ok(trigger, "нет триггера сортировки");
      trigger.click();
      /** @type {ShimElement} */ (
        panel.querySelectorAll('.search-sort-menu [role="menuitemradio"]')[1]
      ).click();
      const checked = panel.querySelectorAll('.search-sort-menu [role="menuitemradio"]');
      assert.equal(checked[1]?.getAttribute("aria-checked"), "true", "сортировка не переключилась");
      assert.equal(checked[0]?.getAttribute("aria-checked"), "false", "старая сортировка осталась выбранной");
      const firstOld = Number(
        panel.querySelector(".search-result")?.getAttribute("data-index") ?? NaN,
      );
      assert.ok(firstOld < firstNew, `«Самое старое» не перевернуло выдачу: ${firstNew} → ${firstOld}`);
    });
  });

  it("«Самое актуальное» — закреплённые выше, затем свежие; порядок ≠ «По новизне» (P2)", async () => {
    await withApp(async (app) => {
      // Окно с закреплённой строкой: 99893 % 523 === 0 (демо-флаг закрепления).
      await scrollChat(app, 99893);
      const pinnedRow = api(app).chat.store.getRow(99893);
      assert.equal(pinnedRow?.pinned, true, "в загруженном окне нет закреплённой строки 99893");

      const header = byId(app, "chat-search");
      header.value = "";
      header.emit("input");
      const panel = firstNode(app, ".search-overlay");
      /** @type {number[]} */
      const byNew = api(app).search.matches;
      assert.ok(byNew.length >= 2, `в окне ${byNew.length} совпадений — нужно ≥2`);
      // «По новизне»: только время, свежие сверху (индекс монотонен по времени).
      for (let i = 1; i < byNew.length; i += 1) {
        const prev = byNew[i - 1];
        const cur = byNew[i];
        assert.ok(prev !== undefined && cur !== undefined, `выдача короче индекса ${i}`);
        assert.ok(prev > cur, `«По новизне» не строго по времени: ${prev} → ${cur}`);
      }

      const trigger = panel.querySelector(".search-sort-trigger");
      assert.ok(trigger, "нет триггера сортировки");
      trigger.click();
      /** @type {ShimElement} */ (
        panel.querySelectorAll('.search-sort-menu [role="menuitemradio"]')[2]
      ).click();
      assert.equal(trigger.textContent, "Самое актуальное", "триггер не переключился на «Самое актуальное»");

      /** @type {number[]} */
      const byRecent = api(app).search.matches;
      assert.deepEqual(
        [...byRecent].sort((a, b) => a - b),
        [...byNew].sort((a, b) => a - b),
        "«Самое актуальное» потеряло/задвоило совпадения",
      );
      const first = byRecent[0];
      assert.ok(first !== undefined, "выдача «Самое актуальное» пуста");
      assert.equal(
        api(app).chat.store.getRow(first)?.pinned,
        true,
        `«Самое актуальное» не подняло закреплённое: первым ${first}`,
      );
      assert.notEqual(first, byNew[0], "«Самое актуальное» и «По новизне» дали одинаковый порядок");
      // Дальше — свежие сверху среди незакреплённых.
      const recentRest = byRecent.filter((index) => api(app).chat.store.getRow(index)?.pinned !== true);
      const newRest = byNew.filter((index) => api(app).chat.store.getRow(index)?.pinned !== true);
      assert.deepEqual(recentRest, newRest, "незакреплённые в «Самое актуальное» не по свежести");
    });
  });

  it("переход к сообщению: оверлей закрыт, список прокручен, строка подсвечена", async () => {
    await withApp(async (app) => {
      const header = byId(app, "chat-search");
      header.value = "привет";
      header.emit("input");
      await api(app).search.whenIdle();
      const panel = firstNode(app, ".search-overlay");
      const result = panel.querySelector(".search-result");
      assert.ok(result, "запрос «привет» не дал результата в загруженном окне");
      const index = Number(result.getAttribute("data-index"));
      result.click();
      assert.equal(panel.getAttribute("data-open"), "false", "оверлей не закрылся после перехода");
      const row = chatRow(app, index);
      assert.ok(row, `после перехода нет строки ${index}`);
      assert.equal(row.classList.contains("is-highlight"), true, "строка не подсвечена");
    });
  });

  it("ошибка фильтра — UIError в статусе, без падения оверлея", async () => {
    await withApp((app) => {
      const header = byId(app, "chat-search");
      header.value = "";
      header.emit("input");
      const panel = firstNode(app, ".search-overlay");
      const dateInput = panel.querySelectorAll(".search-section input")[4];
      assert.ok(dateInput, "нет поля даты");
      dateInput.value = "2026-99-99";
      dateInput.emit("input");
      const status = panel.querySelector(".search-status");
      assert.ok(status, "нет .search-status");
      assert.equal(status.getAttribute("data-state"), "error", "ошибка не отражена в статусе");
      assert.match(status.textContent, /Ошибка/);
      assert.equal(panel.getAttribute("data-open"), "true", "оверлей закрылся от ошибки");
      // Валидная дата снимает ошибку.
      dateInput.value = "";
      dateInput.emit("input");
      assert.equal(status.getAttribute("data-state"), "ok", "ошибка не снялась");
    });
  });

  it("Esc и крестик закрывают оверлей; состояние не теряется", async () => {
    await withApp((app) => {
      const header = byId(app, "chat-search");
      header.value = "тест";
      header.emit("input");
      const panel = firstNode(app, ".search-overlay");
      key(app, "Escape");
      assert.equal(panel.getAttribute("data-open"), "false", "Esc не закрыл оверлей");
      assert.equal(app.document.body.getAttribute("data-active-screen"), "chat", "Esc сменил экран");
      header.emit("input");
      assert.equal(panel.getAttribute("data-open"), "true", "повторный ввод не открыл оверлей");
      const close = panel.querySelector(".search-close");
      assert.ok(close, "нет кнопки закрытия");
      close.click();
      assert.equal(panel.getAttribute("data-open"), "false", "крестик не закрыл оверлей");
      assert.equal(header.value, "тест", "запрос потерялся при закрытии");
    });
  });
});

/* =============================================================================
 * B7b (RW-40): серверный поиск. Контракт — waves/RW-40/ATTACH-RE.md §1
 * (`content`, `channel_id`, `sort_by`, `sort_order`, `offset`; ответ
 * `messages`/`total_results`) и принятый backend B7a `search_remote`.
 * Локальный скан окна — только fallback, 401/429/сеть — видимая ошибка.
 * ========================================================================== */
describe("B7b: сетевой поиск (search_remote)", () => {
  /**
   * @param {(cmd: string, args: any) => any} invokeHandler
   * @param {(app: GuiDom) => void | Promise<void>} body
   * @returns {Promise<void>}
   */
  async function withRemoteApp(invokeHandler, body) {
    const app = await loadApp({ invokeHandler });
    try {
      await body(app);
    } finally {
      app.dispose();
    }
  }

  /**
   * @param {string} id
   * @param {string} content
   * @param {any} [extra]
   * @returns {any}
   */
  function serverMessage(id, content, extra = {}) {
    return {
      id,
      content,
      timestamp: "2026-10-20T10:05:00Z",
      channel_id: "123456789012345678",
      author: { id: "42", username: "network_alpha", bot: false },
      pinned: false,
      ...extra,
    };
  }

  /**
   * @param {GuiDom} app
   * @param {string} query
   * @returns {Promise<ShimElement>}
   */
  async function searchRemote(app, query) {
    const header = byId(app, "chat-search");
    header.value = query;
    header.emit("input");
    await api(app).search.whenIdle();
    return firstNode(app, ".search-overlay");
  }

  it("зовёт search_remote с guildId и подтверждёнными params; рисует total_results", async () => {
    /** @type {any[]} */
    const calls = [];
    await withRemoteApp(
      (cmd, args) => {
        if (cmd !== "search_remote") return undefined;
        calls.push(args);
        return {
          // Discord отдаёт messages массивом групп — проверяем разворачивание.
          messages: [[serverMessage("9001", "привет из сети")], [serverMessage("9002", "второе")]],
          total_results: 5,
          doing_deep_historical_index: false,
        };
      },
      async (app) => {
        const panel = firstNode(app, ".search-overlay");
        const channel = firstNode(app, '.search-section[data-key="in"] .search-control');
        channel.value = "123456789012345678";
        const header = byId(app, "chat-search");
        header.value = "привет";
        header.emit("input");
        const trigger = firstNode(app, ".search-sort-trigger");
        trigger.click();
        /** @type {ShimElement} */ (
          panel.querySelectorAll('.search-sort-menu [role="menuitemradio"]')[1]
        ).click();
        await api(app).search.whenIdle();
        await app.settle(2);

        assert.ok(calls.length >= 1, "search_remote не вызван");
        const args = calls[calls.length - 1];
        assert.equal(args.guildId, "1", "guildId не из выбранного сервера");
        assert.equal(args.params.content, "привет", "content не из запроса");
        assert.equal(args.params.channelId, "123456789012345678", "channelId не из фильтра «В»");
        assert.equal(args.params.sortBy, "timestamp");
        assert.equal(args.params.sortOrder, "asc", "«Самое старое» не ушло как sort_order=asc");
        assert.equal(args.params.offset, 0, "первая страница не с offset=0");

        const results = panel.querySelectorAll(".search-result");
        assert.equal(results.length, 2, "ответ messages не развёрнут");
        assert.deepEqual(
          results.map((node) => node.getAttribute("data-id")),
          ["9001", "9002"],
        );
        const status = firstNode(app, ".search-status");
        assert.equal(status.getAttribute("data-state"), "ok");
        assert.match(status.textContent, /на сервере найдено 5/, "total_results не показан");
        const more = panel.querySelector(".search-more");
        assert.ok(more, "нет «Показать ещё» при total_results > показанного");
        assert.match(more.textContent, /\(3\)/, "остаток страницы не из total_results");
      },
    );
  });

  it("offset-пагинация: вторая страница без дублей, offset растёт", async () => {
    /** @type {number[]} */
    const offsets = [];
    await withRemoteApp(
      (cmd, args) => {
        if (cmd !== "search_remote") return undefined;
        offsets.push(args.params.offset);
        if (args.params.offset === 0) {
          return {
            messages: [serverMessage("9101", "раз"), serverMessage("9102", "два")],
            total_results: 3,
            doing_deep_historical_index: false,
          };
        }
        return {
          // Пересечение с первой страницей — проверяем дедупликацию по id.
          messages: [serverMessage("9102", "два"), serverMessage("9103", "три")],
          total_results: 3,
          doing_deep_historical_index: false,
        };
      },
      async (app) => {
        const panel = await searchRemote(app, "волна");
        assert.equal(panel.querySelectorAll(".search-result").length, 2);
        const more = firstNode(app, ".search-more");
        more.click();
        await api(app).search.whenIdle();

        const ids = panel.querySelectorAll(".search-result").map((node) => node.getAttribute("data-id"));
        assert.deepEqual(ids, ["9101", "9102", "9103"], "догрузка потеряла/задвоила результаты");
        assert.equal(new Set(ids).size, 3, "в выдаче есть дубли");
        assert.deepEqual(offsets, [0, 2], "offset второй страницы не равен числу пропущенных");
        assert.equal(panel.querySelector(".search-more"), null, "пагинация не остановилась на total_results");
        assert.match(firstNode(app, ".search-status").textContent, /показано 3/);
      },
    );
  });

  it("401 — видимая ошибка, локального fallback нет", async () => {
    await withRemoteApp(
      (cmd) => {
        if (cmd !== "search_remote") return undefined;
        throw { kind: "unauthorized" };
      },
      async (app) => {
        const panel = await searchRemote(app, "привет");
        const status = firstNode(app, ".search-status");
        assert.equal(status.getAttribute("data-state"), "error", "401 не показан ошибкой");
        assert.match(status.textContent, /401|сессия истекла/);
        assert.equal(panel.querySelectorAll(".search-result").length, 0, "401 подменён локальной выдачей");
        assert.equal(api(app).search.matchesCount, 0, "сработал тихий локальный fallback");
        assert.doesNotMatch(status.textContent, /загруженных/, "статус остался оконным");
        assert.match(
          app.document.querySelector(".content")?.textContent ?? "",
          /Ошибка/,
          "ошибка не выведена на видимую поверхность",
        );
      },
    );
  });

  it("429 и сеть — разные видимые сообщения, без fallback", async () => {
    /** @type {Array<[string, RegExp]>} */
    const cases = [
      ["rateLimited", /429|много запросов/],
      ["network", /сеть недоступна/],
    ];
    for (const [kind, pattern] of cases) {
      await withRemoteApp(
        (cmd) => {
          if (cmd !== "search_remote") return undefined;
          throw { kind, retryAfterMs: 1500 };
        },
        async (app) => {
          const panel = await searchRemote(app, "привет");
          const status = firstNode(app, ".search-status");
          assert.equal(status.getAttribute("data-state"), "error", `${kind}: нет видимой ошибки`);
          assert.match(status.textContent, pattern, `${kind}: неверное сообщение`);
          assert.equal(panel.querySelectorAll(".search-result").length, 0, `${kind}: показана локальная выдача`);
        },
      );
    }
  });

  // G1-01: после `rest_failure` backend отдаёт `kind` = forbidden/notFound/
  // client/server. Это настоящие `SearchError`, поэтому локальный fallback
  // запрещён: иначе неполная выдача окна выдаётся за серверную, а пользователь
  // не узнаёт про «нет доступа»/«не найдено»/сбой сервера.
  it("G1-01: forbidden/notFound/client/server — видимая ошибка без fallback и свои тексты", async () => {
    /** @type {Array<{ name: string, error: any, pattern: RegExp }>} */
    const cases = [
      { name: "403", error: { kind: "forbidden" }, pattern: /нет доступа \(403\)/ },
      { name: "404", error: { kind: "notFound" }, pattern: /не найдено \(404\)/ },
      { name: "400", error: { kind: "client", status: 400 }, pattern: /ошибка запроса \(400\)/ },
      { name: "422", error: { kind: "client", status: 422 }, pattern: /ошибка запроса \(422\)/ },
      { name: "503", error: { kind: "server", status: 503 }, pattern: /ошибка сервера \(503\)/ },
      { name: "5xx без статуса", error: { kind: "server" }, pattern: /ошибка сервера \(5xx\)/ },
      // Не-регрессия прежнего контракта (RW-39/B7b).
      { name: "401", error: { kind: "unauthorized" }, pattern: /сессия истекла \(401\)/ },
      { name: "429", error: { kind: "rateLimited", retryAfterMs: 1500 }, pattern: /много запросов \(429\)/ },
      { name: "network", error: { kind: "network" }, pattern: /сеть недоступна/ },
    ];
    /** @type {Map<string, string>} */
    const texts = new Map();
    for (const testCase of cases) {
      await withRemoteApp(
        (cmd) => {
          if (cmd !== "search_remote") return undefined;
          throw testCase.error;
        },
        async (app) => {
          const panel = await searchRemote(app, "привет");
          const status = firstNode(app, ".search-status");
          assert.equal(
            status.getAttribute("data-state"),
            "error",
            `${testCase.name}: нет видимой ошибки (${status.textContent})`,
          );
          assert.match(
            status.textContent,
            testCase.pattern,
            `${testCase.name}: неверное сообщение «${status.textContent}»`,
          );
          // Fallback = локальная выдача + `mode: "local"`, для ошибки это ложь.
          assert.equal(
            panel.querySelectorAll(".search-result").length,
            0,
            `${testCase.name}: показана локальная выдача вместо ошибки`,
          );
          assert.equal(api(app).search.matchesCount, 0, `${testCase.name}: тихий локальный fallback`);
          assert.equal(api(app).search.mode, "remote", `${testCase.name}: режим откатился в локальный`);
          assert.doesNotMatch(
            status.textContent,
            /загруженных/,
            `${testCase.name}: статус остался оконным`,
          );
          assert.match(
            app.document.querySelector(".content")?.textContent ?? "",
            /Ошибка/,
            `${testCase.name}: ошибка не выведена на видимую поверхность`,
          );
          texts.set(testCase.name, status.textContent);
        },
      );
    }
    // «Разные сообщения»: ни один kind не должен схлопнуться в общий текст,
    // включая 400 vs 422 и 503 vs «5xx без статуса».
    assert.equal(
      new Set(texts.values()).size,
      cases.length,
      `сообщения схлопнулись: ${JSON.stringify([...texts])}`,
    );
  });

  // SEC-02: отказ keyring/blocking приходит отдельным `kind: "storage"` —
  // пользователь видит «хранилище», а не «сеть»; локального fallback нет.
  it("SEC-02: storage — видимая ошибка хранилища, не сеть и без fallback", async () => {
    await withRemoteApp(
      (cmd) => {
        if (cmd !== "search_remote") return undefined;
        throw { kind: "storage" };
      },
      async (app) => {
        const panel = await searchRemote(app, "привет");
        const status = firstNode(app, ".search-status");
        assert.equal(status.getAttribute("data-state"), "error", "storage не показан ошибкой");
        assert.match(status.textContent, /хранилище токена недоступно/);
        assert.doesNotMatch(status.textContent, /сеть недоступна/, "storage выдан за сетевую ошибку");
        assert.equal(
          panel.querySelectorAll(".search-result").length,
          0,
          "storage подменён локальной выдачей",
        );
        assert.equal(api(app).search.matchesCount, 0, "сработал тихий локальный fallback");
        assert.equal(api(app).search.mode, "remote", "режим откатился в локальный");
        assert.match(
          app.document.querySelector(".content")?.textContent ?? "",
          /Ошибка/,
          "ошибка хранилища не выведена на видимую поверхность",
        );
      },
    );
  });

  it("loading: до ответа сервера видно состояние загрузки, потом результаты", async () => {
    /** @type {() => void} */
    let release = () => {};
    const gate = new Promise((resolve) => {
      release = () => resolve(undefined);
    });
    await withRemoteApp(
      (cmd) => {
        if (cmd !== "search_remote") return undefined;
        return gate.then(() => ({
          messages: [serverMessage("9201", "готово")],
          total_results: 1,
          doing_deep_historical_index: false,
        }));
      },
      async (app) => {
        const header = byId(app, "chat-search");
        header.value = "привет";
        header.emit("input");
        const panel = firstNode(app, ".search-overlay");
        const status = firstNode(app, ".search-status");
        assert.equal(status.getAttribute("data-state"), "loading", "нет состояния загрузки");
        assert.match(status.textContent, /Ищем/);
        release();
        await api(app).search.whenIdle();
        assert.equal(status.getAttribute("data-state"), "ok", "loading не снялся");
        assert.equal(panel.querySelectorAll(".search-result").length, 1);
      },
    );
  });

  it("пустой ответ сервера — empty-состояние при total_results=0", async () => {
    await withRemoteApp(
      (cmd) => {
        if (cmd !== "search_remote") return undefined;
        return { messages: [], total_results: 0, doing_deep_historical_index: false };
      },
      async (app) => {
        const panel = await searchRemote(app, "ничего");
        assert.ok(panel.querySelector(".search-empty"), "нет empty-состояния");
        const status = firstNode(app, ".search-status");
        assert.match(status.textContent, /ничего не найдено/);
        assert.equal(status.getAttribute("data-state"), "ok");
        assert.equal(panel.querySelectorAll(".search-result").length, 0);
      },
    );
  });

  it("7 фильтров: content/channel/sort — сервер, остальные честно «локально»", async () => {
    await withApp((app) => {
      const panel = firstNode(app, ".search-overlay");
      assert.equal(
        firstNode(app, ".search-sort").getAttribute("data-scope"),
        "server",
        "сортировка не помечена серверной",
      );
      const channel = firstNode(app, '.search-section[data-key="in"]');
      assert.equal(channel.getAttribute("data-scope"), "server", "«В» не поддержан сервером");
      for (const key of ["from", "has", "mentions", "date", "author", "pinned"]) {
        const section = firstNode(app, `.search-section[data-key="${key}"]`);
        assert.equal(section.getAttribute("data-scope"), "local", `${key} выдан за серверный фильтр`);
        assert.equal(
          section.querySelector(".search-section-note")?.textContent,
          "локально",
          `${key}: нет честной пометки «локально»`,
        );
      }
      assert.equal(panel.querySelectorAll(".search-section-note").length, 6, "пометок не 6");
    });
  });

  it("локальные фильтры применяются к серверной выдаче и не уходят в params", async () => {
    /** @type {any[]} */
    const calls = [];
    await withRemoteApp(
      (cmd, args) => {
        if (cmd !== "search_remote") return undefined;
        calls.push(args);
        return {
          messages: [
            serverMessage("9301", "человек"),
            serverMessage("9302", "бот", { author: { id: "7", username: "helper_bot", bot: true } }),
          ],
          total_results: 2,
          doing_deep_historical_index: true,
        };
      },
      async (app) => {
        const panel = await searchRemote(app, "привет");
        assert.equal(panel.querySelectorAll(".search-result").length, 2);
        const author = firstNode(app, '.search-section[data-key="author"] .search-control');
        author.value = "Бот";
        author.emit("change");
        await api(app).search.whenIdle();
        const ids = panel.querySelectorAll(".search-result").map((node) => node.getAttribute("data-id"));
        assert.deepEqual(ids, ["9302"], "локальный фильтр «Бот» не применился к серверной выдаче");
        const status = firstNode(app, ".search-status");
        assert.match(status.textContent, /после локальных фильтров 1/);
        assert.match(status.textContent, /индекс ещё строится/, "флаг глубокого индекса не показан");
        const last = calls[calls.length - 1];
        assert.equal("author" in last.params, false, "локальный фильтр ушёл на сервер как параметр");
        assert.equal("channelId" in last.params, false, "channelId отправлен без фильтра «В»");
      },
    );
  });

  it("IPC-команды нет (демо) — локальный fallback с видимой пометкой", async () => {
    await withApp(async (app) => {
      const panel = await searchRemote(app, "привет");
      const status = firstNode(app, ".search-status");
      assert.equal(status.getAttribute("data-state"), "ok", "fallback ушёл в ошибку");
      assert.match(status.textContent, /найдено \d+ из \d+ загруженных/, "нет оконного счётчика");
      assert.match(status.textContent, /локально/, "fallback не помечен как локальный");
      assert.ok(panel.querySelectorAll(".search-result").length > 0, "локальный fallback пуст");
    });
  });
});

// RW-40 / B8b: гидратация и сохранение UI-настроек.
// Оракулы: `rust/gui-spike/src/settings_store.rs` (UiSettings: ровно 5 полей,
// version=1, белые списки cozy/compact · dark/light · ru/en), `ui/index.html`
// (`lang="ru" data-density="cozy"`), `ui/features/settings.js` (38 тумблеров).
describe("настройки: гидратация и сохранение (B8b)", () => {
  /** Дефолты хранилища (settings_store.rs `UiSettings::default`). */
  const DEFAULTS = { version: 1, toggles: {}, density: "cozy", theme: "dark", locale: "ru" };

  /**
   * @param {GuiDom} app
   * @param {string} id
   * @returns {ShimElement}
   */
  function toggleById(app, id) {
    return firstNode(app, `input[data-setting-id="${id}"]`);
  }

  /**
   * `settings_load` отдаёт `payload`, `settings_save` пишется в журнал harness.
   * @param {any} payload
   * @returns {(cmd: string, args: any) => any}
   */
  function loads(payload) {
    return (cmd) => (cmd === "settings_load" ? payload : undefined);
  }

  it("settings_load на старте: тумблеры по id, density/theme/locale и lang", async () => {
    const app = await loadApp({
      invokeHandler: loads({
        ...DEFAULTS,
        toggles: { "voice.suggest_devices": false, "voice.qos_priority": true },
        density: "compact",
        theme: "light",
        locale: "en",
      }),
    });
    try {
      assert.equal(app.callsOf("settings_load").length, 1, "settings_load не вызван ровно один раз");

      const suggest = toggleById(app, "voice.suggest_devices");
      assert.equal(suggest.checked, false, "тумблер не гидратирован по стабильному id");
      assert.equal(suggest.getAttribute("aria-checked"), "false", "aria-checked не синхронизирован при гидратации");
      assert.equal(toggleById(app, "voice.qos_priority").checked, true, "второй тумблер не гидратирован");

      const html = app.document.documentElement;
      assert.equal(html.getAttribute("data-density"), "compact", "density не применён");
      assert.equal(html.getAttribute("data-theme"), "light", "theme не применён");
      assert.equal(html.getAttribute("data-locale"), "en", "locale не применён");
      assert.equal(html.getAttribute("lang"), "en", "html[lang] не синхронизирован с locale");
      assert.equal(app.callsOf("settings_save").length, 0, "гидратация не должна вызывать settings_save");
    } finally {
      app.dispose();
    }
  });

  it("change: один settings_save после debounce, только белый список и без секретов", async () => {
    const app = await loadApp({ invokeHandler: loads(DEFAULTS) });
    try {
      const input = toggleById(app, "voice.qos_priority");
      assert.equal(input.checked, false, "стартовое состояние QoS не из дампа");
      input.click();
      input.emit("change");
      assert.equal(app.callsOf("settings_save").length, 0, "save вызван без debounce");

      app.flushTimers();
      const saves = app.callsOf("settings_save");
      assert.equal(saves.length, 1, `settings_save вызван ${saves.length} раз вместо одного`);

      const payload = saves[0]?.args?.settings;
      assert.ok(payload, "settings_save без аргумента settings");
      assert.deepEqual(
        Object.keys(payload).sort(),
        ["density", "locale", "theme", "toggles", "version"],
        "в файл уходят лишние поля (секреты/чужие ключи)",
      );
      assert.equal(payload.version, 1, "версия схемы не 1");
      assert.equal(payload.toggles["voice.qos_priority"], true, "изменённый тумблер не сохранён");
      assert.equal(Object.keys(payload.toggles).length, SETTINGS_TOGGLES, "сохранены не 38 тумблеров");
      for (const [key, value] of Object.entries(payload.toggles)) {
        assert.match(key, /^[a-z0-9_.]+$/, `нестабильный/не-ASCII id тумблера «${key}»`);
        assert.equal(typeof value, "boolean", `значение тумблера «${key}» не boolean`);
      }
      assert.equal(payload.density, "cozy");
      assert.equal(payload.theme, "dark");
      assert.equal(payload.locale, "ru");
    } finally {
      app.dispose();
    }
  });

  it("битые/чужие значения → дефолты cozy/dark/ru, без падения и без save", async () => {
    const app = await loadApp({
      invokeHandler: loads({
        version: 1,
        toggles: { "voice.qos_priority": "yes", "5": false, unknown_key: true },
        density: "neon",
        theme: 42,
        locale: "xx",
      }),
    });
    try {
      const html = app.document.documentElement;
      assert.equal(html.getAttribute("data-density"), "cozy", "плотность не откатилась к дефолту");
      assert.equal(html.getAttribute("data-theme"), "dark", "тема не откатилась к дефолту");
      assert.equal(html.getAttribute("data-locale"), "ru", "язык не откатился к дефолту");
      assert.equal(html.getAttribute("lang"), "ru");
      assert.equal(toggleById(app, "voice.qos_priority").checked, false, "невалидное значение тумблера применено");
      // Миграция старого формата: ключ-индекс 5 → шестой тумблер (эхоподавление).
      assert.equal(toggleById(app, "noise.echo_cancel").checked, false, "старый индексный ключ не мигрирован");
      assert.equal(app.document.querySelectorAll('input[data-setting-id]').length, SETTINGS_TOGGLES);
      assert.equal(app.callsOf("settings_save").length, 0, "загрузка мусора не должна сохраняться");
    } finally {
      app.dispose();
    }
  });

  it("settings_load упал → дефолты и отсутствие молчаливого падения", async () => {
    const app = await loadApp({
      invokeHandler: (cmd) => {
        if (cmd === "settings_load") throw { kind: "Parse", message: "битый JSON настроек" };
        return undefined;
      },
    });
    try {
      const html = app.document.documentElement;
      assert.equal(html.getAttribute("data-density"), "cozy");
      assert.equal(html.getAttribute("data-theme"), "dark");
      assert.equal(html.getAttribute("data-locale"), "ru");
      assert.equal(app.document.querySelectorAll(".toggle-input").length, SETTINGS_TOGGLES);
      assert.equal(app.callsOf("settings_save").length, 0, "сбой чтения не должен приводить к записи");
      // Ошибка чтения не глотается молча: видно, что работают дефолты.
      const status = firstNode(app, ".settings-save-status");
      assert.equal(status.getAttribute("hidden"), null, "ошибка чтения скрыта");
      assert.match(status.textContent, /по умолчанию/i, "нет пометки о дефолтах");
    } finally {
      app.dispose();
    }
  });

  it("debounce: несколько изменений (тумблер+density+theme) → один save с финальными значениями", async () => {
    const app = await loadApp({ invokeHandler: loads(DEFAULTS) });
    try {
      const input = toggleById(app, "voice.qos_priority");
      input.click();
      input.emit("change");
      const compact = firstNode(app, '.settings-density-option[aria-pressed="false"]');
      compact.click();
      const light = firstNode(app, '.settings-theme-option[aria-pressed="false"]');
      light.click();
      assert.equal(app.callsOf("settings_save").length, 0, "save до истечения debounce");

      app.flushTimers();
      const saves = app.callsOf("settings_save");
      assert.equal(saves.length, 1, `debounce не склеил изменения: ${saves.length} save`);
      const payload = saves[0]?.args?.settings;
      assert.equal(payload.toggles["voice.qos_priority"], true);
      assert.equal(payload.density, "compact", "финальная плотность не сохранена");
      assert.equal(payload.theme, "light", "финальная тема не сохранена");

      app.flushTimers();
      assert.equal(app.callsOf("settings_save").length, 1, "повторный flush дал дубль save");
    } finally {
      app.dispose();
    }
  });

  it("ошибка settings_save видна, состояние не откатывается", async () => {
    const app = await loadApp({
      invokeHandler: (cmd) => {
        if (cmd === "settings_load") return DEFAULTS;
        if (cmd === "settings_save") throw { kind: "Io", message: "нет прав на запись" };
        return undefined;
      },
    });
    try {
      const input = toggleById(app, "voice.qos_priority");
      input.click();
      input.emit("change");
      app.flushTimers();
      await app.settle(2);
      assert.equal(app.callsOf("settings_save").length, 1, "попытка сохранения не сделана");

      const status = firstNode(app, ".settings-save-status");
      assert.equal(status.getAttribute("hidden"), null, "ошибка сохранения скрыта от пользователя");
      assert.match(status.textContent, /Не удалось сохранить/, "нет текста ошибки");
      assert.match(status.textContent, /нет прав/, "причина ошибки потеряна");
      assert.equal(input.checked, true, "состояние тумблера откатилось молча");
      assert.equal(input.getAttribute("aria-checked"), "true");
      assert.equal(
        app.document.documentElement.getAttribute("data-density"),
        "cozy",
        "плотность изменилась при ошибке save",
      );
    } finally {
      app.dispose();
    }
  });

  it("id тумблеров стабильные ASCII; старый формат по индексу мигрируется без падения", async () => {
    const app = await loadApp({
      invokeHandler: loads({
        ...DEFAULTS,
        toggles: { "0": false, toggle_1: false, "4": false, "совершенно секретно": "токен" },
      }),
    });
    try {
      const inputs = all(app, "input[data-setting-id]");
      assert.equal(inputs.length, SETTINGS_TOGGLES, "не у всех 38 тумблеров есть data-setting-id");
      const ids = inputs.map((node) => node.getAttribute("data-setting-id") ?? "");
      assert.equal(new Set(ids).size, SETTINGS_TOGGLES, "id тумблеров не уникальны");
      for (const id of ids) assert.match(id, /^[a-z0-9_.]+$/, `id «${id}» не ASCII/стабильный`);

      // Индексы 0, 1, 4 — первые тумблеры спеки; старые ключи без id.
      assert.equal(ids[0], "voice.suggest_devices");
      assert.equal(toggleById(app, "voice.suggest_devices").checked, false, "индекс 0 не мигрирован");
      assert.equal(toggleById(app, "voice.warn_no_sound").checked, false, "ключ toggle_1 не мигрирован");
      assert.equal(toggleById(app, "input.auto_sensitivity").checked, false, "индекс 4 не мигрирован");

      // Чужой ключ не применён и не сохраняется.
      const qos = toggleById(app, "voice.qos_priority");
      qos.click();
      qos.emit("change");
      app.flushTimers();
      const payload = app.callsOf("settings_save")[0]?.args?.settings;
      assert.equal("совершенно секретно" in payload.toggles, false, "чужой ключ сохранён");
      assert.equal(Object.keys(payload.toggles).length, SETTINGS_TOGGLES);
    } finally {
      app.dispose();
    }
  });
});


/* =============================================================================
 * RW-40 / G2: три P1 из `waves/RW-40/QUALITY/G2-AUDIT.md` (G2-01, G2-02, G2-04).
 * Red-first characterization: поведение сформулировано по аудиту, не по коду.
 * ========================================================================== */

describe("G2 (RW-40): P1 — reply, error-overlay, auth focus-trap", () => {
  /**
   * @param {GuiDom} app
   * @param {string} query
   * @returns {Promise<void>}
   */
  async function remoteSearch(app, query) {
    const header = byId(app, "chat-search");
    header.value = query;
    header.emit("input");
    await api(app).search.whenIdle();
  }

  it("G2-01: набор текста не отменяет reply, отменяют Escape и крестик", async () => {
    await withApp((app) => {
      const form = byId(app, "composer");
      const input = byId(app, "composer-input");
      const composer = api(app).composer;
      composer.setReply({ author: "sasha", text: "привет", id: "12" });
      assert.equal(form.getAttribute("data-reply"), "true", "reply не включился");

      // G2-01: режим ответа переживает обычный ввод — это состояние, а не клавиша.
      for (const keydown of [{ key: "x" }, { key: "a" }, { key: "Backspace" }, { key: "ArrowLeft" }]) {
        const event = input.dispatchEvent("keydown", keydown);
        assert.equal(event.prevented, false, `keydown ${keydown.key} перехвачен композером`);
        assert.equal(
          form.getAttribute("data-reply"),
          "true",
          `keydown ${keydown.key} отменил reply (G2-01)`,
        );
      }
      input.value = "ответ";
      input.emit("input");
      assert.equal(form.getAttribute("data-reply"), "true", "ввод текста отменил reply (G2-01)");

      // Escape — единственная клавиша отмены из поля.
      input.dispatchEvent("keydown", { key: "Escape" });
      assert.equal(form.getAttribute("data-reply"), "false", "Escape не отменил reply");
      assert.equal(input.value, "ответ", "Escape стёр черновик вместе с reply");

      // Крестик чипа — второй путь отмены (остаётся кликом).
      composer.setReply({ author: "sasha", text: "привет", id: "12" });
      assert.equal(form.getAttribute("data-reply"), "true");
      const cancel = form.querySelector(".composer-reply-cancel");
      assert.ok(cancel, "нет кнопки отмены ответа");
      cancel.click();
      assert.equal(form.getAttribute("data-reply"), "false", "крестик не отменил reply");
    });
  });

  it("G2-02: error-overlay закрывается, снимается успехом и не выходит за .content", async () => {
    const message = {
      id: "9201",
      content: "готово",
      timestamp: "2026-10-20T10:05:00Z",
      channel_id: "123456789012345678",
      author: { id: "42", username: "network_alpha", bot: false },
      pinned: false,
    };
    let fail = true;
    const app = await loadApp({
      invokeHandler: (cmd) => {
        if (cmd !== "search_remote") return undefined;
        if (fail) throw { kind: "unauthorized" };
        return { messages: [message], total_results: 1, doing_deep_historical_index: false };
      },
    });
    try {
      const content = firstNode(app, ".content");
      // G2-02: `.error-state` абсолютен, значит containing block задаёт `.content`,
      // а не viewport (иначе ошибка накрывает всё приложение).
      assert.equal(
        content.style.position,
        "relative",
        ".content не является containing block для .error-state (G2-02)",
      );

      await remoteSearch(app, "привет");
      const first = all(app, ".error-state");
      assert.equal(first.length, 1, "ошибка не показана ровно один раз");
      const overlay = first[0];
      assert.ok(overlay, "нет .error-state");
      assert.equal(overlay.parentNode, content, "оверлей вынесен за пределы .content");
      const close = overlay.querySelector(".error-state-close");
      assert.ok(close, "у оверлея нет кнопки закрытия (G2-02)");
      assert.equal(close.getAttribute("type"), "button", "кнопка закрытия не button");
      const label = close.getAttribute("aria-label") ?? "";
      assert.match(label, /закрыть/i, `кнопка закрытия без честного aria-label: «${label}»`);

      // Повторная ошибка заменяет старую, а не копит оверлеи.
      await remoteSearch(app, "привет 2");
      assert.equal(all(app, ".error-state").length, 1, "повторная ошибка не заменила прошлую");

      // Ручное закрытие: recoverable-ошибка не требует перезапуска спайка.
      firstNode(app, ".error-state-close").click();
      assert.equal(all(app, ".error-state").length, 0, "кнопка закрытия не убрала оверлей");

      // Ошибка → retry → успешное действие снимает оверлей само.
      await remoteSearch(app, "ещё раз");
      assert.equal(all(app, ".error-state").length, 1, "ошибка не вернулась перед retry");
      fail = false;
      await remoteSearch(app, "успех");
      assert.equal(all(app, ".error-state").length, 0, "успешный поиск не снял оверлей (G2-02)");
      assert.equal(
        firstNode(app, ".search-status").getAttribute("data-state"),
        "ok",
        "поиск не завершился успешно — сценарий retry не воспроизведён",
      );
    } finally {
      app.dispose();
    }
  });

  it("G2-04: Tab заперт в auth-gate, Escape не уводит фокус в aria-hidden shell", async () => {
    const app = await loadApp({
      invokeHandler: (cmd) => {
        if (cmd === "auth_status") return { status: "no-session", stored: false };
        if (cmd === "auth_login_token") {
          return { kind: "ready", status: "ready", user: { id: "42", username: "sasha" } };
        }
        return undefined;
      },
    });
    try {
      const gate = byId(app, "auth-gate");
      const shell = firstNode(app, ".shell");
      assert.equal(gate.getAttribute("data-state"), "no-session", "гейт не показан без сессии");
      assert.equal(gate.getAttribute("hidden"), null, "гейт скрыт при no-session");
      assert.equal(gate.getAttribute("aria-modal"), "true");
      assert.equal(shell.getAttribute("aria-hidden"), "true", "shell не скрыт от AT при открытом гейте");

      /** @param {ShimElement | null} node @returns {boolean} */
      const insideGate = (node) => {
        let current = node;
        while (current) {
          if (current === gate) return true;
          current = current.parentNode;
        }
        return false;
      };

      // G2-04: модалка обязана удерживать Tab; сейчас Tab уходит в скрытый shell.
      const visited = new Set();
      for (let step = 1; step <= 30; step += 1) {
        const event = key(app, "Tab");
        assert.equal(event.prevented, true, `Tab #${step} не удержан модальным гейтом (G2-04)`);
        const active = app.document.activeElement;
        assert.ok(active, "Tab потерял фокус");
        assert.ok(insideGate(active), `Tab #${step} увёл фокус из #auth-gate`);
        visited.add(active);
      }
      assert.ok(visited.size >= 3, `Tab прошёл только по ${visited.size} узлам гейта — цикла нет`);

      // Escape в модалке не переключает экран и не отдаёт фокус в shell.
      const escape = key(app, "Escape");
      assert.equal(escape.prevented, true, "Escape не обработан модальным гейтом");
      assert.ok(insideGate(app.document.activeElement), "Escape увёл фокус из #auth-gate");

      // Вход: гейт закрывается, фокус возвращается в рабочую область.
      byId(app, "auth-token").value = "token-value";
      byId(app, "auth-token-submit").click();
      await app.settle(4);
      assert.equal(gate.getAttribute("data-state"), "ready", "гейт не перешёл в ready");
      assert.equal(gate.getAttribute("hidden"), "", "гейт не закрылся после входа");
      assert.equal(shell.getAttribute("aria-hidden"), null, "shell остался aria-hidden после входа");
      assert.equal(
        app.document.activeElement?.id,
        "chat-list",
        "после закрытия гейта фокус не вернулся в чат",
      );

      // Закрытая модалка больше не перехватывает Tab (settings/emoji не задеты).
      assert.equal(key(app, "Tab").prevented, false, "закрытый гейт продолжает перехватывать Tab");
    } finally {
      app.dispose();
    }
  });
});

/* ---------------------------------------------------------------------------
 * RW-40 / G2: меню сервера (G-05) и подтверждение выхода (G-04).
 * Контракт: waves/RW-40/DESIGN/SERVER-MENU.md §4.2, §4.4, §6, §7.2, §7.4.
 * Сети нет: `leave_guild` — фейк через harness `setInvokeHandler`; тест падает,
 * если код сходит в реальный IPC-дефолт (`gui-dom-harness: неизвестная IPC-команда`).
 * ------------------------------------------------------------------------- */

// D1-guild-menu-items.json: порядок и тексты 10 пунктов (дословно, с «...»).
const SERVER_MENU_LABELS = [
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

/**
 * @param {GuiDom} app
 * @returns {ShimElement[]}
 */
function serverMenuItems(app) {
  return all(app, '#server-menu [role="menuitem"], #server-menu [role="menuitemcheckbox"]');
}

/**
 * @param {GuiDom} app
 * @returns {ShimElement}
 */
function openServerMenu(app) {
  const opener = action(app, "guild-menu");
  opener.click();
  return byId(app, "server-menu");
}

/**
 * @param {GuiDom} app
 * @param {string} name
 * @returns {ShimElement}
 */
function menuAction(app, name) {
  const node = firstNode(app, `#server-menu [data-action="${name}"]`);
  return node;
}

/**
 * @param {GuiDom} app
 */
/**
 * Плитка гильдии по id: в DOM-шиме `dataset.guild` не виден селектору
 * `[data-guild="…"]`, поэтому ищем по коллекции (ограничение шима, не теста).
 * @param {GuiDom} app
 * @param {string} id
 * @returns {ShimElement | null}
 */
function guildTile(app, id) {
  return all(app, ".guild-item").find((node) => node.dataset.guild === id) ?? null;
}

/**
 * @param {GuiDom} app
 */
async function openLeaveDialog(app) {
  openServerMenu(app);
  menuAction(app, "guild-leave").click();
  const dialog = byId(app, "leave-server-dialog");
  assert.equal(dialog.getAttribute("hidden"), null, "confirm выхода не открылся");
  return dialog;
}

describe("G-05: поповер меню сервера (10 пунктов, роли, клавиатура)", () => {
  it("open: aria-haspopup/expanded, role=menu, 10 пунктов в порядке D1", async () => {
    await withApp((app) => {
      const opener = action(app, "guild-menu");
      assert.equal(opener.getAttribute("aria-haspopup"), "menu", "guild-menu без aria-haspopup=menu");
      assert.equal(opener.getAttribute("aria-expanded"), "false", "guild-menu стартует раскрытым");
      const menu = byId(app, "server-menu");
      assert.notEqual(menu.getAttribute("hidden"), null, "меню видно до клика");
      assert.equal(menu.getAttribute("role"), "menu", "#server-menu не role=menu");
      assert.equal(menu.getAttribute("aria-label"), "Меню сервера", "у меню нет доступного имени");

      openServerMenu(app);
      assert.equal(menu.getAttribute("hidden"), null, "клик не открыл поповер");
      assert.equal(opener.getAttribute("aria-expanded"), "true", "aria-expanded не синхронен открытию");

      const items = serverMenuItems(app);
      assert.equal(items.length, 10, `пунктов меню ${items.length}, а не 10`);
      assert.deepEqual(
        items.map((node) => node.querySelector(".menu-item-label")?.textContent),
        SERVER_MENU_LABELS,
        "тексты/порядок пунктов разошлись с D1-guild-menu-items.json",
      );
      const notifications = items[3];
      assert.ok(notifications, "нет пункта «Параметры уведомлений»");
      assert.equal(
        notifications.querySelector(".menu-item-subtitle")?.textContent,
        "Только @упоминания",
        "вторая строка пункта 4 потерялась",
      );

      // SM-13: подменю и toggle-пункт.
      for (const label of ["Заглушить сервер", "Параметры уведомлений", "Переместить..."]) {
        const item = items.find((node) => node.querySelector(".menu-item-label")?.textContent === label);
        assert.ok(item, `нет пункта «${label}»`);
        assert.equal(item.getAttribute("aria-haspopup"), "menu", `«${label}» без aria-haspopup=menu`);
      }
      const hideMuted = items.find(
        (node) => node.querySelector(".menu-item-label")?.textContent === "Скрыть заглушённые каналы",
      );
      assert.ok(hideMuted, "нет toggle-пункта");
      assert.equal(hideMuted.getAttribute("role"), "menuitemcheckbox", "toggle-пункт не menuitemcheckbox");
      assert.equal(hideMuted.getAttribute("aria-checked"), "false", "aria-checked не false");

      // SM-12 (с поправкой G-05): приглашения в спайке недоступны — неизвестная
      // capability трактуется как disabled; активны только leave и copy-id.
      const enabled = items.filter((node) => node.getAttribute("aria-disabled") !== "true");
      assert.deepEqual(
        enabled.map((node) => node.getAttribute("data-action")),
        ["guild-leave", "guild-copy-id"],
        "активные пункты меню не совпадают с доступными действиями",
      );
      const leave = menuAction(app, "guild-leave");
      assert.equal(leave.getAttribute("data-variant"), "danger", "leave не destructive");
      // Фокус — на первом доступном пункте (приглашение disabled по capability).
      assert.equal(app.document.activeElement, leave, "фокус не на первом доступном пункте");
    });
  });

  it("keyboard: ↑/↓/Home/End, Escape закрывает и возвращает фокус, клик вне тоже", async () => {
    await withApp((app) => {
      const opener = action(app, "guild-menu");
      let menu = openServerMenu(app);
      const leave = menuAction(app, "guild-leave");
      const copyId = menuAction(app, "guild-copy-id");
      assert.equal(app.document.activeElement, leave, "стартовый фокус не на leave");

      menu.dispatchEvent("keydown", { key: "ArrowDown", target: menu });
      assert.equal(app.document.activeElement, copyId, "ArrowDown не перешёл на copy-id");
      menu.dispatchEvent("keydown", { key: "ArrowDown", target: menu });
      assert.equal(app.document.activeElement, leave, "ArrowDown не зациклился на первом доступном");
      menu.dispatchEvent("keydown", { key: "ArrowUp", target: menu });
      assert.equal(app.document.activeElement, copyId, "ArrowUp не зациклился на последнем");
      menu.dispatchEvent("keydown", { key: "Home", target: menu });
      assert.equal(app.document.activeElement, leave, "Home не вернул первый пункт");
      menu.dispatchEvent("keydown", { key: "End", target: menu });
      assert.equal(app.document.activeElement, copyId, "End не перешёл на последний пункт");

      const escape = menu.dispatchEvent("keydown", { key: "Escape", target: menu });
      assert.equal(escape.prevented, true, "Escape в меню не предотвращён");
      assert.notEqual(menu.getAttribute("hidden"), null, "Escape не закрыл меню");
      assert.equal(opener.getAttribute("aria-expanded"), "false", "aria-expanded не сброшен");
      assert.equal(app.document.activeElement, opener, "фокус не вернулся на opener");

      menu = openServerMenu(app);
      app.document.body.click();
      assert.notEqual(menu.getAttribute("hidden"), null, "клик вне меню не закрыл поповер");
      assert.equal(opener.getAttribute("aria-expanded"), "false", "aria-expanded после клика вне");
      assert.equal(app.document.activeElement, opener, "клик вне не вернул фокус на opener");
    });
  });

  it("owner: leave disabled, подтверждение не открывается и IPC не уходит", async () => {
    await withApp((app) => {
      const tile = guildTile(app, "1");
      assert.ok(tile, "нет плитки гильдии 1");
      tile.setAttribute("data-owner", "true");
      openServerMenu(app);
      const leave = menuAction(app, "guild-leave");
      assert.equal(leave.getAttribute("aria-disabled"), "true", "leave владельца не disabled");
      leave.click();
      assert.notEqual(byId(app, "leave-server-dialog").getAttribute("hidden"), null, "диалог открылся владельцу");
      assert.equal(app.callsOf("leave_guild").length, 0, "владелец отправил leave_guild");
      assert.deepEqual(app.consoleErrors, [], "disabled-пункт дал ошибку в консоль");
    });
  });
});

describe("G-04: подтверждение выхода, leave_guild и видимые ошибки", () => {
  it("открытие confirm: alertdialog, безопасный фокус, отмена без IPC", async () => {
    await withApp(async (app) => {
      await openLeaveDialog(app);
      const card = firstNode(app, "#leave-server-dialog .dialog");
      assert.equal(card.getAttribute("role"), "alertdialog", "confirm не alertdialog");
      assert.equal(card.getAttribute("aria-modal"), "true", "confirm не модальный");
      assert.equal(card.getAttribute("aria-labelledby"), "leave-server-title", "нет aria-labelledby");
      assert.equal(card.getAttribute("aria-describedby"), "leave-server-text", "нет aria-describedby");
      assert.equal(
        byId(app, "leave-server-title").textContent,
        "Вы уверены, что хотите покинуть этот сервер?",
        "заголовок confirm разошёлся с AVazqJ",
      );
      const cancel = firstNode(app, '#leave-server-dialog [data-action="leave-cancel"]');
      assert.equal(cancel.getAttribute("data-autofocus"), "true", "фокус по умолчанию не на «Отмена»");
      assert.equal(firstNode(app, "#leave-confirm").getAttribute("data-autofocus"), null, "destructive получил autofocus");

      key(app, "Escape");
      assert.notEqual(byId(app, "leave-server-dialog").getAttribute("hidden"), null, "Escape не закрыл confirm");
      assert.equal(app.callsOf("leave_guild").length, 0, "отмена отправила leave_guild");
      assert.ok(guildTile(app, "1"), "отмена удалила плитку");
    });
  });

  it("2xx: один invoke(«leave_guild», {guildId, lurking}), удаление плитки, выбор предыдущей", async () => {
    await withApp(async (app) => {
      app.setInvokeHandler((cmd) => (cmd === "leave_guild" ? null : undefined));
      await openLeaveDialog(app);
      byId(app, "leave-confirm").click();
      await app.settle(3);

      const calls = app.callsOf("leave_guild");
      assert.equal(calls.length, 1, "на подтверждение ушло не одно IPC-сообщение");
      assert.deepEqual(calls[0]?.args, { guildId: "1", lurking: false }, "payload leave_guild не {guildId, lurking}");
      assert.equal(JSON.stringify(calls[0]?.args).includes("token"), false, "в payload попал токен");

      assert.notEqual(byId(app, "leave-server-dialog").getAttribute("hidden"), null, "диалог не закрылся после 2xx");
      assert.equal(guildTile(app, "1"), null, "плитка сервера осталась в рейле");
      const selected = all(app, ".guild-item.is-selected");
      assert.equal(selected.length, 1, `после выхода выбрано ${selected.length} плиток`);
      assert.equal(selected[0]?.dataset.guild, "2", "не выбрана предыдущая/соседняя гильдия");
      assert.match(byId(app, "server-status").textContent, /Вы покинули сервер/, "нет live-статуса успеха");
      assert.equal(action(app, "guild-menu").getAttribute("aria-expanded"), "false", "меню осталось раскрытым");
    });
  });

  it("lurking берётся из явного состояния плитки, а не хардкодится", async () => {
    await withApp(async (app) => {
      const tile = guildTile(app, "1");
      assert.ok(tile, "нет плитки гильдии 1");
      tile.setAttribute("data-lurking", "true");
      app.setInvokeHandler((cmd) => (cmd === "leave_guild" ? null : undefined));
      await openLeaveDialog(app);
      byId(app, "leave-confirm").click();
      await app.settle(3);
      assert.deepEqual(app.callsOf("leave_guild")[0]?.args, { guildId: "1", lurking: true }, "lurking не передан из состояния");
    });
  });

  it("двойной клик по «Покинуть сервер» даёт ровно один IPC", async () => {
    await withApp(async (app) => {
      app.setInvokeHandler((cmd) => (cmd === "leave_guild" ? null : undefined));
      await openLeaveDialog(app);
      const confirm = byId(app, "leave-confirm");
      confirm.click();
      confirm.click();
      confirm.click();
      await app.settle(3);
      assert.equal(app.callsOf("leave_guild").length, 1, "pending не блокирует повторный submit");
    });
  });

  it("401: видимая ошибка, диалог и плитка остаются", async () => {
    await withApp(async (app) => {
      app.setInvokeHandler((cmd) => {
        if (cmd === "leave_guild") throw { kind: "unauthorized" };
        return undefined;
      });
      await openLeaveDialog(app);
      byId(app, "leave-confirm").click();
      await app.settle(3);
      const error = byId(app, "leave-error");
      assert.equal(error.getAttribute("hidden"), null, "ошибка 401 не показана");
      assert.match(error.textContent, /Сессия истекла\. Войдите снова\./, "текст 401 не по §3.2");
      assert.equal(byId(app, "leave-server-dialog").getAttribute("hidden"), null, "диалог закрылся при 401");
      assert.ok(guildTile(app, "1"), "плитка удалена при 401");
      assert.equal(app.callsOf("leave_guild").length, 1);
    });
  });

  it("403: «Недостаточно прав», плитка на месте; 429: обратный отсчёт в тексте", async () => {
    await withApp(async (app) => {
      let kind = /** @type {any} */ ({ kind: "forbidden" });
      app.setInvokeHandler((cmd) => {
        if (cmd === "leave_guild") throw kind;
        return undefined;
      });
      await openLeaveDialog(app);
      byId(app, "leave-confirm").click();
      await app.settle(3);
      assert.match(byId(app, "leave-error").textContent, /Недостаточно прав, чтобы покинуть сервер\./, "текст 403");
      assert.ok(guildTile(app, "1"), "плитка удалена при 403");

      kind = { kind: "rateLimited", retry_after_ms: 3000 };
      byId(app, "leave-confirm").click();
      await app.settle(3);
      assert.match(byId(app, "leave-error").textContent, /Слишком много попыток\. Повторите через 3 с\./, "текст 429");

      // G05-REST-SPEC §3.9/D7 (блок из waves/RW-40/PM/g05-rest/REPORT.md §5):
      // 429 блокирует до 0 без автоповтора, затем ровно один ручной повтор.
      assert.notEqual(
        byId(app, "leave-confirm").getAttribute("disabled"),
        null,
        "кнопка осталась активной во время отсчёта 429",
      );
      assert.equal(app.callsOf("leave_guild").length, 2, "429 дал автоповтор");
      app.flushTimers();
      app.flushTimers();
      app.flushTimers();
      assert.equal(byId(app, "leave-error").textContent, "Слишком много попыток. Повторите через 0 с.");
      assert.equal(
        byId(app, "leave-confirm").getAttribute("disabled"),
        null,
        "кнопка осталась disabled после отсчёта 429",
      );
      byId(app, "leave-confirm").click();
      await app.settle(3);
      assert.equal(app.callsOf("leave_guild").length, 3, "ручной повтор после отсчёта не ушёл");
    });
  });

  it("нет мёртвых обработчиков: все data-action меню/диалога отрабатывают без ACTION_FAILED", async () => {
    await withApp((app) => {
      openServerMenu(app);
      // Disabled-пункты: клик не должен уходить в «неизвестный data-action».
      menuAction(app, "guild-invite").click();
      menuAction(app, "guild-copy-id").click();
      assert.deepEqual(app.consoleErrors, [], "клик по пункту меню дал ACTION_FAILED/console.error");
      assert.equal(all(app, ".error-state").length, 0, "клик по пункту меню показал оверлей ошибки");

      openServerMenu(app);
      menuAction(app, "guild-leave").click();
      firstNode(app, '#leave-server-dialog [data-action="leave-cancel"]').click();
      assert.deepEqual(app.consoleErrors, [], "cancel confirm дал ACTION_FAILED/console.error");
      for (const node of all(app, "#server-menu [data-action], #leave-server-dialog [data-action]")) {
        assert.ok(
          ["guild-invite", "guild-leave", "guild-copy-id", "leave-cancel", "leave-confirm"].includes(
            String(node.getAttribute("data-action")),
          ),
          `незарегистрированный data-action="${node.getAttribute("data-action")}"`,
        );
      }
    });
  });
});

/* =============================================================================
 * DP-02 / D-04: переменная высота строк (26 / 48 / +22, member 42).
 *
 * Оракул — `waves/RW-37/RE/layout.json` (RE-артефакт) + правила демо-строк
 * DOM-CONTRACT §7.1. Ожидание `--row-message-track: 44` / `--row-member-track: 40`
 * (приближение RW-38) здесь НЕ используется: оно противоречит SPEC §5.1/A-01/A-02.
 * ========================================================================== */
const uiDirD04 = fileURLToPath(new URL("../../rust/gui-spike/ui/", import.meta.url));
const layoutD04 = JSON.parse(
  readFileSync(fileURLToPath(new URL("../../waves/RW-37/RE/layout.json", import.meta.url)), "utf8"),
);
const RE_D04 = Object.freeze({
  CONTINUATION: layoutD04.row_heights.message_cozy,
  GROUP_START: layoutD04.row_heights_detail.message_cozy_group_start,
  LINE_EXTRA: layoutD04.row_heights_detail.message_extra_line,
  MEMBER: layoutD04.row_heights.member,
});
const tokensD04 = readFileSync(`${uiDirD04}tokens.css`, "utf8");

/** @param {number} index @returns {number} */
function oracleLinesD04(index) {
  return 1 + (index % 17 === 9 ? 1 : 0) + (index % 23 === 9 ? 1 : 0);
}

/** @param {number} index @returns {number} */
function oracleChatHeightD04(index) {
  const base = index % 4 === 0 ? RE_D04.GROUP_START : RE_D04.CONTINUATION;
  return base + (oracleLinesD04(index) - 1) * RE_D04.LINE_EXTRA;
}

/**
 * Ожидаемая высота `.text`: строки × 22 (+22 на строку автора для начала группы).
 * Это ровно высота строки минус вертикальные padding'и 2+2 (`--space-1`).
 * @param {number} index @returns {number}
 */
function oracleTextHeightD04(index) {
  return (oracleLinesD04(index) + (index % 4 === 0 ? 1 : 0)) * RE_D04.LINE_EXTRA;
}

describe("D-04: переменная высота строк в живом списке", () => {
  it("высоты строк чата 26/48/+22, .text по тексту/переносам (не 44-трек)", async () => {
    await withApp(async (app) => {
      await scrollChat(app, 0);
      const rows = all(app, "#chat-list .row");
      assert.ok(rows.length >= 10, `в окне ${rows.length} строк, ожидалось ≥10`);
      /** @type {Set<number>} */
      const seen = new Set();
      for (const row of rows) {
        const index = Number(row.getAttribute("data-index"));
        const expected = oracleChatHeightD04(index);
        assert.equal(row.style.height, `${expected}px`, `строка ${index}: высота не по оракулу D-04`);
        assert.notEqual(row.style.height, "44px", `строка ${index}: остался фиксированный трек 44`);
        const text = row.querySelector(".text");
        assert.ok(text, "у строки нет .text");
        assert.equal(
          text.style.height,
          `${oracleTextHeightD04(index)}px`,
          `строка ${index}: .text не по числу переносов (${oracleLinesD04(index)})`,
        );
        assert.equal(
          Number.parseFloat(row.style.height) - Number.parseFloat(text.style.height),
          4,
          `строка ${index}: .text не оставляет вертикальные padding'и 2+2`,
        );
        seen.add(expected);
      }
      assert.ok(seen.has(RE_D04.CONTINUATION), "в окне нет строки-продолжения 26");
      assert.ok(seen.has(RE_D04.GROUP_START), "в окне нет начала группы 48");
    });
  });

  it("renderChatRow считает высоту из текста, а не из строки/трека", async () => {
    await withApp(async (app) => {
      // Индекс 9: продолжение с двумя переносами (17 и 23) → 26 + 2×22.
      const row = app.document.createElement("div");
      renderChatRow(/** @type {any} */ (row), makeMessage(9), 9);
      const text = row.querySelector(".text");
      assert.ok(text, "renderChatRow не создал .text");
      assert.equal(row.style.height, "", "renderChatRow не должен задавать высоту строки (это делает HeightIndex)");
      assert.equal(text.style.height, `${oracleTextHeightD04(9)}px`, "высота .text считается не по тексту");
      assert.equal(
        text.style.height,
        `${messageHeight(9) - 4}px`,
        "высота .text не согласована с моделью высот",
      );

      // Начало группы с тремя строками: 48 + 2×22, .text = 4 строки по 22.
      const startRow = app.document.createElement("div");
      renderChatRow(/** @type {any} */ (startRow), makeMessage(400), 400);
      assert.equal(startRow.getAttribute("data-group"), "start", "400 не начало группы");
      assert.equal(oracleLinesD04(400), 3, "у 400 не три строки текста");
      assert.equal(
        startRow.querySelector(".text")?.style.height,
        `${oracleTextHeightD04(400)}px`,
        "высота .text начала группы не по тексту",
      );
    });
  });

  it("data-row-height моделей D-04 и отсутствие старых треков в коде", async () => {
    await withApp(async (app) => {
      assert.equal(
        byId(app, "chat-list").getAttribute("data-row-height"),
        `${RE_D04.CONTINUATION}/${RE_D04.GROUP_START}/+${RE_D04.LINE_EXTRA}`,
        "data-row-height чата не 26/48/+22",
      );
      assert.equal(
        byId(app, "member-list").getAttribute("data-row-height"),
        String(RE_D04.MEMBER),
        "data-row-height участников не 42",
      );
    });
    // Модель высот и рендер не читают замороженные треки RW-38.
    for (const file of ["model.js", "features/chat.js", "features/virtual-list.js", "features/members.js"]) {
      const source = readFileSync(`${uiDirD04}${file}`, "utf8");
      assert.doesNotMatch(
        source,
        /row-(?:message|member)-track/,
        `${file} ссылается на --row-*-track (приближение), а не на D-04`,
      );
    }
  });

  it("scrollToIndex/якорь: строка встаёт на свой top из prefix-sum", async () => {
    await withApp(async (app) => {
      const heights = createChatHeights(CHAT_ROWS);
      const list = byId(app, "chat-list");
      for (const index of [0, 1, 9, 12, 26, 1234]) {
        await scrollChat(app, index);
        const expectedTop = heights.top(index);
        assert.equal(list.scrollTop, expectedTop, `scrollTop ≠ top(${index})`);
        assert.equal(api(app).chat.list.firstVisibleIndex(), index, `первая видимая строка ≠ ${index}`);
        const row = chatRow(app, index);
        assert.ok(row, `строка ${index} не отрисована`);
        assert.equal(row.style.top, `${expectedTop}px`, `top строки ${index} не по префиксу`);
        assert.equal(row.style.height, `${oracleChatHeightD04(index)}px`, `высота ${index} не по оракулу`);
        assert.equal(row.style.top, `${row.style.top}`, "top не строка");
      }
    });
  });

  it("tombstone уменьшает totalHeight на высоту удалённой строки и не оставляет дыру", async () => {
    await withApp(async (app) => {
      app.setInvokeHandler(async () => null);
      await scrollChat(app, 0);
      const list = byId(app, "chat-list");
      const sizer = /** @type {ShimElement} */ (list.children[0]);
      const full = createChatHeights(CHAT_ROWS).totalHeight;
      assert.equal(Number.parseFloat(sizer.style.height), full, "sizer до удаления ≠ сумме высот");

      const row = chatRow(app, 12);
      assert.ok(row, "нет строки 12");
      const removed = oracleChatHeightD04(12);
      list.dispatchEvent("mouseover", { target: row });
      const remove = all(app, ".msg-toolbar button").find(
        (button) => button.getAttribute("aria-label") === "Удалить сообщение",
      );
      assert.ok(remove, "нет кнопки удаления");
      remove.click();
      remove.click();
      await app.settle(3);

      assert.ok(chatRow(app, 12) === null, "удалённая строка осталась в DOM");
      assert.equal(
        Number.parseFloat(/** @type {ShimElement} */ (list.children[0]).style.height),
        full - removed,
        `sizer не уменьшился на ${removed} (дыра tombstone)`,
      );
      const prev = chatRow(app, 11);
      const next = chatRow(app, 13);
      assert.ok(prev && next, "соседние строки пропали");
      assert.equal(
        Number.parseFloat(next.style.top),
        Number.parseFloat(prev.style.top) + Number.parseFloat(prev.style.height),
        "между строками 11 и 13 осталось пустое место",
      );
    });
  });

  it("участники: строка 42, шаг 43 (RE, JPEG ±1), панель и список", async () => {
    await withApp(async (app) => {
      clickRail(app, "members");
      await app.settle(4);
      const rows = all(app, "#member-list .member").slice(0, 8);
      assert.ok(rows.length >= 4, `в списке участников ${rows.length} строк`);
      /** @type {number[]} */
      const tops = [];
      for (const row of rows) {
        assert.equal(row.style.height, `${RE_D04.MEMBER}px`, "высота строки участника не 42");
        tops.push(Number.parseFloat(row.style.top));
      }
      for (let offset = 1; offset < tops.length; offset++) {
        assert.equal(
          /** @type {number} */ (tops[offset]) - /** @type {number} */ (tops[offset - 1]),
          RE_D04.MEMBER,
          "шаг виртуализованных строк участника не 42 (перекрытие/дыра)",
        );
      }
      // Превью-панель: те же 42, шаг RE-измерения 43 ±1 = --row-member-pitch.
      const preview = all(app, "#member-panel-list .member");
      assert.ok(preview.length > 0, "панель участников пуста");
      for (const row of preview) {
        assert.equal(row.style.height, `${RE_D04.MEMBER}px`, "высота строки панели не 42");
      }
    });

    assert.match(tokensD04, /--row-h-member:\s*42px;/, "нет --row-h-member: 42px (D-04)");
    assert.match(tokensD04, /--row-member-pitch:\s*43px;/, "нет шага 43 (--row-member-pitch)");
    // Legacy-имя `--row-member` обязано совпадать с D-04 (42), а не с треком 40.
    assert.match(tokensD04, /--row-member:\s*42px;/, "--row-member не 42 (legacy-имя разошлось с D-04)");
  });

  it("tokens.css: D-04 значения на месте, треки RW-38 помечены legacy", () => {
    assert.match(tokensD04, /--row-h-continuation:\s*26px;/, "нет --row-h-continuation: 26px");
    assert.match(tokensD04, /--row-h-group-start:\s*48px;/, "нет --row-h-group-start: 48px");
    assert.match(tokensD04, /--row-line-extra:\s*22px;/, "нет --row-line-extra: 22px");
    // Legacy-треки RW-38 остаются только ради waves/RW-39/PM/w39c/geometry-check.mjs
    // (31/31, вне скоупа DP-02) и обязаны быть помечены как не-модель.
    assert.match(
      tokensD04,
      /--row-message-track:\s*44px;\s*\/\*[^\n]*legacy[^\n]*\*\//,
      "--row-message-track не помечен legacy (44 — приближение, не паритет)",
    );
    assert.match(
      tokensD04,
      /--row-member-track:\s*40px;\s*\/\*[^\n]*legacy[^\n]*\*\//,
      "--row-member-track не помечен legacy (40 — приближение, не паритет)",
    );
    // Legacy-значения не должны подменять D-04 через var(): только явные числа.
    assert.doesNotMatch(tokensD04, /--row-h-[a-z-]+:\s*var\(--row-(?:message|member)-track\)/, "D-04 токен ссылается на legacy-трек");
  });
});

/* =============================================================================
 * L-01 (RW-40, PARITY-MATRIX): видимый reconnect/offline state.
 *
 * Оракул — Rust-контракт `rust/gui-spike/src/net.rs` (`GatewayStatus::Display`:
 * offline/connecting/ready/reconnecting/error), команды из `main.rs`
 * (`net_status`/`net_connect`) и UI-требования L-01: баннер «Переподключение…»
 * для connecting/reconnecting, «Нет соединения» + «Повторить» для offline/error,
 * без авто-retry net_connect. Реального IPC/сети/окна нет: ответы задаёт тест.
 * ========================================================================== */
const uiDirNet = fileURLToPath(new URL("../../rust/gui-spike/ui/", import.meta.url));
const NET_TOKEN_LEAK = "MTk4NjIyNDgzNDcxOTI1MjQ4.Cl2FMQ.fake-user-token-DO-NOT-LEAK";
const NET_USER = {
  id: "42",
  username: "tester",
  discriminator: "0",
  global_name: "Тестер",
  avatar: null,
  bot: false,
};

/** @typedef {{ status: string | (() => any), connect: string | (() => any), statusCalls: number }} NetScript */

/**
 * Загрузка `app.js` с уже пройденным гейтом входа: `auth_status` возвращает
 * `ready`, `net_status`/`net_connect` отвечают по сценарию теста.
 * @param {(app: GuiDom, script: NetScript) => void | Promise<void>} body
 * @param {{ status?: string | (() => any), connect?: string | (() => any) }} [script]
 * @returns {Promise<void>}
 */
async function withNetApp(body, script = {}) {
  /** @type {NetScript} */
  const state = { status: script.status ?? "ready", connect: script.connect ?? "connecting", statusCalls: 0 };
  const app = await loadApp({
    invokeHandler: (cmd) => {
      if (cmd === "auth_status") {
        return { status: "ready", stored: true, user: NET_USER, error: null };
      }
      if (cmd === "net_status") {
        state.statusCalls += 1;
        return typeof state.status === "function" ? state.status() : state.status;
      }
      if (cmd === "net_connect") {
        return typeof state.connect === "function" ? state.connect() : state.connect;
      }
      return undefined;
    },
  });
  try {
    await body(app, state);
  } finally {
    app.dispose();
  }
}

describe("L-01 (RW-40): видимый reconnect/offline state", () => {
  it("ready скрыт; connecting/reconnecting — «Переподключение…», retry только в offline", async () => {
    await withNetApp(async (app, state) => {
      const banner = byId(app, "net-status");
      assert.equal(banner.getAttribute("role"), "status", "баннер без role=status");
      assert.equal(banner.getAttribute("aria-live"), "polite", "баннер без aria-live");
      assert.notEqual(banner.getAttribute("hidden"), null, "ready: баннер видим");
      assert.equal(
        banner.parentNode,
        app.document.querySelector(".topbar"),
        "баннер вне .topbar — может перекрыть чат",
      );
      assert.equal(app.callsOf("net_status").length >= 1, true, "нет первичного net_status после ready");
      assert.equal(
        app.callsOf("net_status").every((call) => call.args === null),
        true,
        "net_status вызван с аргументами (токен/лишние поля)",
      );
      assert.equal(app.callsOf("net_connect").length, 0, "авто-connect без действия пользователя");
      const retry = byId(app, "net-retry");
      assert.notEqual(retry.getAttribute("hidden"), null, "retry виден при ready");

      state.status = "connecting";
      app.flushTimers();
      await app.settle(2);
      assert.equal(banner.getAttribute("hidden"), null, "connecting не показал баннер");
      assert.equal(banner.getAttribute("data-state"), "connecting");
      assert.match(banner.textContent, /Переподключение…/);
      assert.notEqual(retry.getAttribute("hidden"), null, "retry виден при connecting");

      state.status = "reconnecting";
      app.flushTimers();
      await app.settle(2);
      assert.equal(banner.getAttribute("data-state"), "reconnecting");
      assert.match(banner.textContent, /Переподключение…/);

      state.status = "ready";
      app.flushTimers();
      await app.settle(2);
      assert.notEqual(banner.getAttribute("hidden"), null, "ready не скрыл баннер");
    });
  });

  it("offline/error: «Нет соединения» + «Повторить»; retry — ровно один net_connect, без авто-цикла", async () => {
    await withNetApp(async (app, state) => {
      const banner = byId(app, "net-status");
      state.status = "offline";
      app.flushTimers();
      await app.settle(2);
      assert.equal(banner.getAttribute("hidden"), null, "offline не показал баннер");
      assert.equal(banner.getAttribute("data-state"), "offline");
      assert.match(banner.textContent, /Нет соединения/);
      const retry = byId(app, "net-retry");
      assert.equal(retry.getAttribute("hidden"), null, "retry скрыт в offline");
      assert.equal(retry.getAttribute("data-action"), "net-retry");

      state.status = "error";
      app.flushTimers();
      await app.settle(2);
      assert.equal(banner.getAttribute("data-state"), "error");
      assert.match(banner.textContent, /Нет соединения/);

      /** @type {((value: string) => void) | null} */
      let releaseConnect = null;
      state.connect = () => {
        state.status = "connecting";
        return new Promise((resolve) => { releaseConnect = resolve; });
      };
      // Тройной клик до ответа — всё равно один IPC (busy-защита), ручной retry.
      retry.click();
      retry.click();
      retry.click();
      await app.settle(2);
      assert.equal(app.callsOf("net_connect").length, 1, "повторные клики дали больше одного net_connect");
      assert.equal(app.callsOf("net_connect")[0]?.args, null, "net_connect вызван с аргументами");
      assert.ok(releaseConnect, "net_connect не вернул pending-promise");
      /** @type {(value: string) => void} */ (releaseConnect)("connecting");
      await app.settle(2);
      assert.equal(banner.getAttribute("data-state"), "connecting", "ответ net_connect не применён");

      // Соединение снова упало: статус-поллинг видит offline, но connect сам не повторяется.
      state.status = "offline";
      for (let round = 0; round < 5; round++) {
        app.flushTimers();
        await app.settle(1);
      }
      assert.equal(
        app.callsOf("net_connect").length,
        1,
        "net_connect повторяется автоматически (тихий retry-loop)",
      );
      assert.equal(retry.getAttribute("aria-disabled"), null, "retry не разблокирован после ответа");
    });
  });

  it("ошибка net_connect (401) видима, retry ручной, без тихого цикла и console.error", async () => {
    await withNetApp(
      async (app) => {
        const banner = byId(app, "net-status");
        const retry = byId(app, "net-retry");
        assert.equal(banner.getAttribute("hidden"), null, "offline не показал баннер");
        retry.click();
        await app.settle(3);
        assert.equal(banner.getAttribute("hidden"), null, "ошибка connect скрыла баннер");
        assert.equal(banner.getAttribute("data-state"), "error");
        assert.match(banner.textContent, /Сессия истекла|Войдите/i, "401 не показан видимым текстом");
        assert.equal(retry.getAttribute("aria-disabled"), null, "retry остался заблокирован");
        assert.equal(app.consoleErrors.length, 0, "ошибка connect ушла в console.error");

        for (let round = 0; round < 4; round++) {
          app.flushTimers();
          await app.settle(1);
        }
        assert.equal(app.callsOf("net_connect").length, 1, "ошибка connect породила авто-retry");
        assert.equal(app.callsOf("net_status").length > 1, true, "поллинг статуса остановился");
        assert.equal(
          JSON.stringify(app.invokeCalls).includes(NET_TOKEN_LEAK),
          false,
          "токен попал в IPC",
        );
      },
      {
        status: "offline",
        connect: () => Promise.reject({ kind: "Unauthorized", code: 401, message: "unauthorized" }),
      },
    );
  });

  it("dispose останавливает поллинг net_status, refresh после dispose — no-op", async () => {
    await withNetApp(async (app) => {
      assert.equal(app.callsOf("net_status").length >= 1, true);
      const net = app.appModule.currentApp().net;
      assert.ok(net, "currentApp().net не собран");
      net.dispose();
      const before = app.callsOf("net_status").length;
      for (let round = 0; round < 5; round++) {
        app.flushTimers();
        await app.settle(1);
      }
      assert.equal(app.callsOf("net_status").length, before, "таймер поллинга жив после dispose");
      await net.refresh();
      assert.equal(app.callsOf("net_status").length, before, "refresh после dispose шлёт IPC");
      assert.notEqual(byId(app, "net-status").getAttribute("hidden"), null);
    });
  });

  it("без auth ready нет поллинга net_status (гейт не пройден)", async () => {
    await withApp(async (app) => {
      app.flushTimers();
      await app.settle(2);
      assert.equal(app.callsOf("net_status").length, 0, "поллинг стартовал без ready");
      assert.equal(app.callsOf("net_connect").length, 0);
      assert.notEqual(byId(app, "net-status").getAttribute("hidden"), null);
    });
  });

  it("скрытое окно не опрашивает статус, после возврата поллинг продолжается", async () => {
    await withNetApp(async (app) => {
      const doc = /** @type {any} */ (app.document);
      const before = app.callsOf("net_status").length;
      doc.hidden = true;
      for (let round = 0; round < 3; round++) {
        app.flushTimers();
        await app.settle(1);
      }
      assert.equal(app.callsOf("net_status").length, before, "скрытое окно продолжило поллинг");
      doc.hidden = false;
      app.flushTimers();
      await app.settle(2);
      assert.equal(app.callsOf("net_status").length > before, true, "поллинг не возобновился");
    });
  });

  it("фокус не крадётся появлением баннера и возвращается в чат при ready", async () => {
    await withNetApp(async (app, state) => {
      const composer = byId(app, "composer-input");
      composer.focus();
      state.status = "offline";
      app.flushTimers();
      await app.settle(2);
      assert.equal(app.document.activeElement, composer, "баннер украл фокус");

      const retry = byId(app, "net-retry");
      retry.focus();
      state.status = "ready";
      app.flushTimers();
      await app.settle(2);
      assert.equal(byId(app, "net-status").getAttribute("hidden"), "", "ready не скрыл баннер");
      assert.equal(
        app.document.activeElement?.id,
        "chat-list",
        "фокус не вернулся из скрытого баннера в чат",
      );
    });
  });

  it("index.html подключает net-status.css; баннер в потоке, не поверх чата", () => {
    const html = readFileSync(`${uiDirNet}index.html`, "utf8");
    assert.match(html, /components\/net-status\.css/, "net-status.css не подключён в index.html");
    const css = readFileSync(`${uiDirNet}components/net-status.css`, "utf8");
    assert.doesNotMatch(css, /position:\s*fixed/, "баннер fixed — перекрывает чат");
    assert.match(css, /\.net-status\[hidden\]/, "нет правила скрытия баннера");
  });
});

/* =============================================================================
 * INT-07-UI-APP-MOUNT (RW-40): монтирование createVoiceDevicesSettings и
 * createNotificationSettingsController в rust/gui-spike/ui/app.js.
 * ========================================================================== */

describe("INT-07-UI-APP-MOUNT: контроллеры голосовых устройств и уведомлений в app.js", () => {
  it("aria-disabled контролы уведомлений на старте не выполняют действия мышью и клавиатурой", async () => {
    await withApp((app) => {
      const bell = action(app, "inbox");
      const serverItem = byId(app, "server-menu-notifications");
      const guildDialog = byId(app, "guild-notifications-dialog");
      const channelMenu = byId(app, "channel-notif-menu");

      assert.equal(bell.getAttribute("aria-disabled"), "true");
      assert.equal(serverItem.getAttribute("aria-disabled"), "true");
      // P1-1: до выбора гильдии пункт объявлен заглушкой (data-static-disabled,
      // tabindex=-1, причина через aria-describedby), а колокол — disabled VW-01.
      assert.equal(serverItem.getAttribute("data-static-disabled"), "true", "пункт не заглушка до rail.load");
      assert.equal(serverItem.getAttribute("tabindex"), "-1", "disabled-пункт в tab-порядке");
      assert.equal(serverItem.getAttribute("aria-haspopup"), "menu", "aria-haspopup заглушки не menu");
      assert.equal(bell.getAttribute("aria-haspopup"), null, "disabled-колокол обещает подменю");
      assert.equal(bell.getAttribute("aria-controls"), null, "disabled-колокол ссылается на меню");
      assert.equal(bell.getAttribute("title"), "Уведомления (Phase 3)");
      openServerMenu(app);
      assert.equal(
        serverItem.getAttribute("aria-describedby"),
        "menu-reason-not-implemented",
        "у disabled-пункта нет причины",
      );
      assert.equal(serverItem.getAttribute("aria-disabled"), "true", "refreshMenuState снял disabled до гильдии");
      bell.click();
      serverItem.click();
      for (const key of ["Enter", " "]) {
        serverItem.dispatchEvent("keydown", { key });
        bell.dispatchEvent("keydown", { key });
      }
      assert.notEqual(guildDialog.getAttribute("hidden"), null, "disabled пункт открыл диалог");
      assert.notEqual(channelMenu.getAttribute("hidden"), null, "disabled колокол открыл меню");
      assert.equal(app.callsOf("update_guild_notification_settings").length, 0);
      assert.equal(app.callsOf("update_channel_notification_override").length, 0);
    });
  });

  it("монтирует createVoiceDevicesSettings в предварительном режиме: состояние в памяти, без незарегистрированных IPC", async () => {
    await withApp((app) => {
      const runtime = api(app);
      // Сравнение через === : сама ссылка на контроллер содержит DOM-дерево, и diff
      // от `assert.equal(obj, null)` упирается в сериализацию циклической структуры.
      assert.equal(runtime.voiceDevices === null, true, "контроллер смонтирован до раскрытия секции");

      const settingsDialog = byId(app, "settings-dialog");
      const settingsModal = byId(app, "settings-modal");
      const voiceSection = byId(app, "voice-devices");
      assert.ok(
        settingsDialog.querySelector("#voice-devices"),
        "#voice-devices не смонтирован внутри #settings-dialog",
      );
      assert.ok(
        settingsModal.querySelector("#voice-devices"),
        "#voice-devices не смонтирован внутри #settings-modal",
      );

      // P1-1 (Pass 3): голосового IPC в main.rs нет. Секция живёт в памяти и явно
      // объявляет себя предварительной — иначе каждое изменение давало бы alert
      // «Не удалось сохранить настройку…» на незарегистрированной команде.
      const preliminary = byId(app, "voice-devices-preliminary");
      assert.ok(preliminary, "нет видимой пометки предварительного режима");
      assert.equal(preliminary.getAttribute("hidden"), null, "пометка предварительного режима скрыта");
      assert.equal(preliminary.getAttribute("role"), "status", "пометка не live-регион");
      assert.match(
        preliminary.textContent,
        /Предварительный режим/,
        "пометка не объявляет предварительный режим",
      );
      assert.match(
        preliminary.textContent,
        /voice-devices IPC/,
        "пометка не называет причину (нет voice-devices IPC)",
      );

      // AT-SPI RW-38: пока секция свёрнута, её тумблеры не попадают в диалог настроек.
      assert.notEqual(voiceSection.getAttribute("hidden"), null, "свёрнутая voice-секция видима");
      const expand = byId(app, "voice-devices-expand");
      assert.ok(expand, "нет кнопки раскрытия настроек устройств");
      assert.equal(expand.getAttribute("aria-expanded"), "false", "кнопка раскрытия объявлена раскрытой");
      assert.equal(expand.getAttribute("aria-controls"), "voice-devices");
      assert.match(expand.textContent, /Показать настройки устройств/);
      assert.equal(
        settingsDialog.querySelectorAll('[role="switch"]').length,
        SETTINGS_TOGGLES,
        "свёрнутая voice-секция добавила тумблеры в диалог настроек (AT-SPI: ожидалось 38)",
      );

      expand.click();
      assert.equal(expand.getAttribute("aria-expanded"), "true", "кнопка не объявила раскрытие");
      assert.equal(voiceSection.getAttribute("hidden"), null, "раскрытие не показало секцию");
      assert.ok(runtime.voiceDevices, "currentApp().voiceDevices не инициализирован после раскрытия");
      assert.equal(typeof runtime.voiceDevices.getState, "function");
      assert.equal(typeof runtime.voiceDevices.setDevices, "function");
      assert.equal(voiceSection.getAttribute("data-preliminary"), "true", "#voice-devices не помечен предварительным");
      assert.equal(
        settingsDialog.querySelectorAll('[role="switch"]').length,
        SETTINGS_TOGGLES + 3,
        "раскрытая секция должна добавить ровно 3 тумблера (PTT/mute/deafen)",
      );

      const inputSelect = byId(app, "voice-devices-input");
      const outputSelect = byId(app, "voice-devices-output");
      const outputVol = byId(app, "voice-devices-output-volume");
      const pttToggle = byId(app, "voice-devices-ptt");
      const muteToggle = byId(app, "voice-devices-user-mute");
      const deafToggle = byId(app, "voice-devices-self-deaf");
      const voiceCallsBefore = voiceIpcCalls(app);

      const initialState = runtime.voiceDevices.getState();
      assert.ok(initialState.selectedInput, "не выбрано входное устройство по умолчанию");
      assert.ok(initialState.selectedOutput, "не выбрано выходное устройство по умолчанию");
      assert.equal(inputSelect.getAttribute("aria-disabled"), null);
      assert.equal(outputSelect.getAttribute("aria-disabled"), null);

      outputSelect.value = "default-output";
      outputSelect.emit("change");
      inputSelect.value = "default-input";
      inputSelect.emit("change");
      assert.equal(runtime.voiceDevices.getState().selectedOutput, "default-output", "выбор вывода не сохранился в памяти");
      assert.equal(runtime.voiceDevices.getState().selectedInput, "default-input", "выбор ввода не сохранился в памяти");

      outputVol.value = "155";
      outputVol.emit("input");
      assert.equal(runtime.voiceDevices.getState().outputVolume, 155, "громкость вывода не обновилась");
      assert.equal(outputVol.getAttribute("aria-valuenow"), "155");
      const inputVol = byId(app, "voice-devices-input-volume");
      inputVol.value = "80";
      inputVol.emit("input");
      assert.equal(runtime.voiceDevices.getState().inputVolume, 80, "громкость ввода не обновилась в памяти");

      pttToggle.click();
      assert.equal(
        runtime.voiceDevices.getState().inputMode.kind,
        "push-to-talk",
        "режим PTT не переключился",
      );
      // P2-1 (Pass 2): PTT-клавиша должна доходить до адаптера, а не теряться молча.
      const pttKeyButton = byId(app, "voice-devices-ptt-key");
      pttKeyButton.click();
      pttKeyButton.dispatchEvent("keydown", { key: "K" });
      assert.equal(runtime.voiceDevices.getState().pttKey, "K", "PTT-клавиша не сохранена даже в памяти");

      deafToggle.click();
      assert.equal(runtime.voiceDevices.getState().selfDeaf, true);
      assert.equal(runtime.voiceDevices.getState().effectiveMute, true);
      assert.equal(voiceSection.getAttribute("data-effective-mute"), "true");
      assert.equal(muteToggle.getAttribute("aria-checked"), "false");

      // Ни одно действие в предварительной секции не уходит в IPC: команд
      // set_input_device…set_mute_state/set_ptt_key в Tauri-слое нет.
      assert.equal(
        voiceIpcCalls(app),
        voiceCallsBefore,
        "предварительная секция вызвала незарегистрированную voice-команду",
      );
      const notice = byId(app, "voice-devices-notice");
      assert.notEqual(notice.getAttribute("hidden"), null, "предварительный режим показал alert вместо пометки");
      assert.deepEqual(app.consoleErrors, [], "взаимодействие с voice-панелью дало console.error");

      const notifCss = readFileSync(`${uiDirNet}index.html`, "utf8");
      assert.match(notifCss, /\.notif-toggle\s*\{/);
      assert.match(notifCss, /\.notif-toggle-input\s*\{/);
      assert.match(notifCss, /\.notif-toggle-input:checked\s*\+\s*\.toggle-track/);
      // P2-3: ключевые правила строки тумблера продублированы, иначе в модалке
      // остаются голые чекбоксы (нет flex-строки, скрытия input, hover-трека).
      assert.match(notifCss, /\.notif-toggle\s*\{[^}]*display:\s*flex/s, "нет flex-строки .notif-toggle");
      assert.match(notifCss, /\.notif-toggle\s*\{[^}]*justify-content:\s*space-between/s);
      assert.match(notifCss, /\.notif-toggle:hover \.toggle-track/, "нет hover-трека");
      assert.match(notifCss, /\.notif-toggle:hover \.notif-toggle-input:checked \+ \.toggle-track/);
    });
  });

  it("P2-3: тумблеры модалки уведомлений живут на .notif-toggle и не раздувают инвариант 38", () => {
    return withApp((app) => {
      const dialog = byId(app, "guild-notifications-dialog");
      const settings = byId(app, "settings");
      const notifToggles = dialog.querySelectorAll(".notif-toggle");
      const notifInputs = dialog.querySelectorAll(".notif-toggle-input");
      assert.equal(notifToggles.length, 6, "в модалке не 6 тумблеров .notif-toggle");
      assert.equal(notifInputs.length, 6, "в модалке не 6 input .notif-toggle-input");
      assert.equal(dialog.querySelectorAll(".toggle").length, 0, "в модалке остались .toggle");
      assert.equal(dialog.querySelectorAll(".toggle-input").length, 0, "в модалке остались .toggle-input");
      assert.equal(settings.querySelectorAll(".toggle").length, SETTINGS_TOGGLES, "инвариант 38 .toggle сломан");
      assert.equal(
        settings.querySelectorAll(".toggle-input").length,
        SETTINGS_TOGGLES,
        "инвариант 38 .toggle-input сломан",
      );
    });
  });

  it("P2-1: панель голосовых устройств видна только на вкладке voice", async () => {
    await withApp((app) => {
      const settings = byId(app, "settings");
      const modal = byId(app, "settings-modal");
      const navItems = all(app, "#settings-nav .settings-nav-item");
      /** @param {string} section @returns {ShimElement} */
      const nav = (section) => {
        const node = navItems.find((item) => item.textContent === section);
        assert.ok(node, `нет вкладки ${section}`);
        return node;
      };

      // Стартовая панель D-17 — voice, значит панель устройств видима.
      assert.equal(settings.getAttribute("data-active-pane"), "voice");
      assert.equal(modal.getAttribute("hidden"), null, "voice-панель скрыта на старте");
      assert.equal(modal.getAttribute("role"), "region", "#settings-modal без role=region");

      nav("Внешний вид").click();
      assert.equal(settings.getAttribute("data-active-pane"), "appearance");
      assert.equal(modal.getAttribute("hidden"), "", "voice-панель видна на «Внешнем виде»");

      nav("Уведомления").click();
      assert.equal(settings.getAttribute("data-active-pane"), "notifications");
      assert.equal(modal.getAttribute("hidden"), "", "voice-панель видна на «Уведомлениях»");

      nav("Голос и видео").click();
      assert.equal(settings.getAttribute("data-active-pane"), "voice");
      assert.equal(modal.getAttribute("hidden"), null, "voice-панель не вернулась на своей вкладке");
    });
  });

  it("P2-5 (Pass 3): видимость voice-панели следует и за программной сменой вкладки", async () => {
    await withApp((app) => {
      const runtime = api(app);
      const settings = byId(app, "settings");
      const modal = byId(app, "settings-modal");
      assert.equal(typeof runtime.settings.showPane, "function", "settings.showPane не собран");

      // Не клик по #settings-nav, а прямой вызов — так переключает панели сам
      // settings.js и любой deep-link из другого экрана.
      runtime.settings.showPane("Внешний вид");
      assert.equal(settings.getAttribute("data-active-pane"), "appearance");
      assert.equal(
        modal.getAttribute("hidden"),
        "",
        "программная смена вкладки оставила voice-панель видимой",
      );

      runtime.settings.showPane("Голос и видео");
      assert.equal(settings.getAttribute("data-active-pane"), "voice");
      assert.equal(modal.getAttribute("hidden"), null, "программный возврат на voice не показал панель");

      runtime.settings.showPane("Уведомления");
      assert.equal(
        modal.getAttribute("hidden"),
        "",
        "переход на «Уведомления» программой оставил voice-панель видимой",
      );
    });
  });

  it("связывает #server-menu-notifications (#guild-notifications-dialog) и #channel-notif-menu с текущим состоянием гильдии и каналов", async () => {
    const guildChannels = [
      { id: "cat-10", type: 4, name: "Инфо", parentId: "", position: 0 },
      { id: "ch-101", type: 0, name: "общий-чат", parentId: "cat-10", position: 1 },
      { id: "ch-102", type: 0, name: "новости", parentId: "cat-10", position: 2 },
    ];
    const app = await loadApp({
      invokeHandler: (cmd, args) => {
        if (cmd === "ready") {
          return {
            user: { id: "42", username: "tester" },
            guilds: [
              { id: "777", name: "Гильдия Рубин" },
              { id: "888", name: "Гильдия Сапфир" },
            ],
          };
        }
        if (cmd === "channels") {
          if (args?.guildId === "777") return guildChannels;
          return [];
        }
        if (cmd === "messages_page") {
          return [];
        }
        if (cmd === "update_guild_notification_settings") {
          return args?.payload ?? {};
        }
        if (cmd === "update_channel_notification_override") {
          return {
            guild_id: args?.guildId,
            channel_overrides: {
              [args?.channelId]: {
                channel_id: args?.channelId,
                ...(args?.patch ?? {}),
              },
            },
          };
        }
        return undefined;
      },
    });
    try {
      const runtime = api(app);
      assert.ok(runtime.notifications, "currentApp().notifications не инициализирован");
      const dialogBackdrop = byId(app, "guild-notifications-dialog");
      const channelMenu = byId(app, "channel-notif-menu");
      assert.notEqual(dialogBackdrop.getAttribute("hidden"), null, "модалка уведомлений открыта до клика");
      assert.notEqual(channelMenu.getAttribute("hidden"), null, "меню канала открыто до клика");

      await runtime.rail.load();
      await app.settle(4);
      assert.equal(runtime.rail.selectedId, "777", "рейл не выбрал гильдию 777");
      assert.equal(runtime.tree.channels.length, 3, "каналы гильдии 777 не загружены в tree");

      const serverMenuNotif = byId(app, "server-menu-notifications");
      assert.equal(serverMenuNotif.getAttribute("aria-disabled"), null, "guild control остался disabled после rail.load");
      assert.equal(serverMenuNotif.getAttribute("data-action"), "guild-notifications");
      assert.equal(serverMenuNotif.getAttribute("aria-haspopup"), "dialog");
      assert.equal(serverMenuNotif.getAttribute("tabindex"), "0", "доступный пункт вне tab-порядка");
      assert.equal(serverMenuNotif.getAttribute("data-static-disabled"), null, "data-static-disabled не снят");
      assert.equal(serverMenuNotif.getAttribute("aria-describedby"), null, "aria-describedby не снят");
      assert.equal(serverMenuNotif.getAttribute("title"), null, "disabled title не снят");
      openServerMenu(app);
      assert.equal(serverMenuNotif.getAttribute("aria-disabled"), null, "refreshMenuState снова отключил пункт");
      const guildMenuButton = firstNode(app, '[data-action="guild-menu"]');

      // 1. Открытие модалки уведомлений гильдии через пункт меню сервера #server-menu-notifications
      openServerMenu(app);
      serverMenuNotif.click();

      assert.equal(
        dialogBackdrop.getAttribute("hidden"),
        null,
        "клик по #server-menu-notifications не открыл #guild-notifications-dialog",
      );
      assert.equal(runtime.notifications.getState().guildId, "777", "guildId в контроллере не 777");
      assert.notEqual(byId(app, "server-menu").getAttribute("hidden"), null, "меню сервера осталось открытым");
      assert.equal(guildMenuButton.getAttribute("aria-expanded"), "false", "aria-expanded не сброшен");
      assert.deepEqual(app.consoleErrors, [], "открытие настроек уведомлений создало ACTION_FAILED");
      assert.equal(
        runtime.notifications.getState().guildName,
        "Гильдия Рубин",
        "guildName в контроллере не соответствует выбранной гильдии",
      );
      assert.equal(
        byId(app, "guild-notif-subtitle").textContent,
        "Гильдия Рубин",
        "подзаголовок модалки (#guild-notif-subtitle) не содержит имя текущей гильдии",
      );

      const channelSelect = byId(app, "notif-override-channel-select");
      const optionValues = channelSelect.querySelectorAll("option").map((opt) => opt.getAttribute("value"));
      assert.ok(
        optionValues.includes("ch-101") && optionValues.includes("ch-102"),
        "селект каналов в модалке не заполнен каналами из tree.channels",
      );

      runtime.notifications.closeGuildModal();
      openServerMenu(app);
      serverMenuNotif.dispatchEvent("keydown", { key: "Enter" });
      assert.equal(dialogBackdrop.getAttribute("hidden"), null, "Enter не открыл настройки уведомлений");
      assert.notEqual(byId(app, "server-menu").getAttribute("hidden"), null, "Enter не закрыл меню сервера");
      runtime.notifications.closeGuildModal();

      // Изменение настройки гильдии отправляет IPC для текущей гильдии 777
      openServerMenu(app);
      serverMenuNotif.click();
      byId(app, "guild-notif-mute-switch").click();
      await app.settle(2);
      const guildCalls = app.callsOf("update_guild_notification_settings");
      assert.equal(guildCalls.length, 1, "не вызван update_guild_notification_settings");
      assert.equal(guildCalls[0]?.args?.guildId, "777", "guildId в update_guild_notification_settings не 777");
      assert.ok(
        guildCalls[0]?.args?.body?.guilds?.["777"],
        "update_guild_notification_settings отправлен не для текущей гильдии 777",
      );
      runtime.notifications.closeGuildModal();

      // 2. Открытие контекстного меню уведомлений канала (#channel-notif-menu) из дерева каналов
      const chButton = firstNode(app, '.channel-item[data-channel-id="ch-101"]');
      chButton.dispatchEvent("contextmenu", { target: chButton });
      assert.equal(
        channelMenu.getAttribute("hidden"),
        null,
        "contextmenu по каналу не открыл #channel-notif-menu",
      );
      assert.equal(
        channelMenu.getAttribute("data-channel-id"),
        "ch-101",
        "#channel-notif-menu не привязан к ch-101",
      );
      assert.equal(runtime.notifications.getState().activeMenuChannelId, "ch-101");

      const levelAllBtn = firstNode(
        app,
        '#channel-notif-menu [data-action="channel-notif-level"][data-level="0"]',
      );
      levelAllBtn.click();
      await app.settle(2);
      const chCalls = app.callsOf("update_channel_notification_override");
      assert.equal(chCalls.length, 1, "не вызван update_channel_notification_override");
      assert.equal(chCalls[0]?.args?.guildId, "777", "guildId в override канала не равен 777");
      assert.equal(chCalls[0]?.args?.channelId, "ch-101", "channelId в override канала не равен ch-101");
      runtime.notifications.closeChannelMenu();

      // 3. Открытие #channel-notif-menu через колокол шапки чата для открытого в чате канала
      await runtime.chat.openChannel({ id: "ch-102", name: "новости" });
      await app.settle(3);
      assert.equal(runtime.chat.channelId, "ch-102", "канал ch-102 не открылся в чате");
      const bell = action(app, "inbox");
      assert.equal(bell.getAttribute("aria-disabled"), null, "колокол остался disabled после открытия канала");
      assert.equal(bell.getAttribute("title"), null, "у доступного колокола остался disabled title");
      assert.equal(bell.getAttribute("aria-haspopup"), "menu");
      assert.equal(bell.getAttribute("aria-controls"), "channel-notif-menu");

      bell.click();
      assert.equal(
        channelMenu.getAttribute("hidden"),
        null,
        "клик по колоколу шапки не открыл #channel-notif-menu для активного канала",
      );
      assert.equal(
        channelMenu.getAttribute("data-channel-id"),
        "ch-102",
        "#channel-notif-menu при открытии из шапки чата не получил chat.channelId",
      );
    } finally {
      app.dispose();
    }
  });

  it("P2-6: пустой рейл не выдумывает гильдию и не шлёт уведомления в IPC", async () => {
    const app = await loadApp({
      invokeHandler: (cmd) => {
        if (cmd === "ready") return { user: { id: "42", username: "tester" }, guilds: [] };
        return undefined;
      },
    });
    try {
      const runtime = api(app);
      await runtime.rail.load();
      await app.settle(4);
      assert.equal(runtime.rail.selectedId, "", "рейл выбрал гильдию из пустого payload");

      const item = byId(app, "server-menu-notifications");
      assert.equal(item.getAttribute("aria-disabled"), "true", "пункт доступен без загруженной гильдии");
      openServerMenu(app);
      item.click();
      item.dispatchEvent("keydown", { key: "Enter" });
      assert.notEqual(
        byId(app, "guild-notifications-dialog").getAttribute("hidden"),
        null,
        "модалка открылась без гильдии",
      );
      assert.equal(app.callsOf("update_guild_notification_settings").length, 0);
      assert.equal(app.callsOf("update_channel_notification_override").length, 0);
      assert.equal(
        app.invokeCalls.some((call) => JSON.stringify(call.args ?? "").includes('"guildId":"1"')),
        false,
        "в IPC ушёл выдуманный guildId=1",
      );
      assert.deepEqual(app.consoleErrors, [], "disabled-пункт дал ACTION_FAILED");
    } finally {
      app.dispose();
    }
  });

  it("P2-5: contextmenu при закрытой модалке её не открывает; Escape возвращает фокус", async () => {
    const app = await loadApp({
      invokeHandler: (cmd, args) => {
        if (cmd === "ready") {
          return {
            user: { id: "42", username: "tester" },
            guilds: [{ id: "777", name: "Гильдия Рубин" }],
          };
        }
        if (cmd === "channels") {
          return [
            { id: "cat-20", type: 4, name: "Инфо", parentId: "", position: 0 },
            { id: "ch-201", type: 0, name: "общий-чат", parentId: "cat-20", position: 1 },
          ];
        }
        if (cmd === "update_guild_notification_settings") return args?.payload ?? {};
        if (cmd === "update_channel_notification_override") return {};
        return undefined;
      },
    });
    try {
      const runtime = api(app);
      await runtime.rail.load();
      await app.settle(4);
      const dialog = byId(app, "guild-notifications-dialog");
      const muteSwitch = byId(app, "guild-notif-mute-switch");
      assert.notEqual(dialog.getAttribute("hidden"), null, "модалка открыта до сценария");

      // 1. Контекстное меню канала не должно открывать закрытую модалку (P2-5).
      const chButton = firstNode(app, '.channel-item[data-channel-id="ch-201"]');
      chButton.dispatchEvent("contextmenu", { target: chButton });
      assert.notEqual(dialog.getAttribute("hidden"), null, "contextmenu открыл модалку уведомлений");
      assert.notEqual(app.document.activeElement, muteSwitch, "фокус ушёл в скрытую модалку");
      assert.equal(byId(app, "channel-notif-menu").getAttribute("hidden"), null, "меню канала не открылось");
      runtime.notifications.closeChannelMenu();

      // 2. Escape закрывает модалку и возвращает фокус на пункт меню-опенер.
      const item = byId(app, "server-menu-notifications");
      openServerMenu(app);
      item.click();
      assert.equal(dialog.getAttribute("hidden"), null, "модалка не открылась пунктом меню");
      assert.equal(app.document.activeElement, muteSwitch, "фокус не переведён в модалку");
      byId(app, "guild-notif-modal").dispatchEvent("keydown", { key: "Escape" });
      assert.notEqual(dialog.getAttribute("hidden"), null, "Escape не закрыл модалку");
      assert.equal(app.document.activeElement, item, "фокус не вернулся на #server-menu-notifications");
    } finally {
      app.dispose();
    }
  });
});

/* =============================================================================
 * INT-07 (Pass 3, P1-2/P2-7): durable guard «UI invoke ⊆ generate_handler!».
 * Пока такого теста не было, монтаж контроллера на незарегистрированные команды
 * (`set_input_device`…) проходил все гейты. Оракул — исходник `main.rs`, а не
 * список в тесте: набор команд читается из `generate_handler![…]`.
 * ========================================================================== */

/**
 * Рекурсивно собирает `.js`-файлы UI-слоя спайка.
 * @param {string} dir
 * @returns {string[]}
 */
function collectUiScripts(dir) {
  const files = [];
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const full = `${dir}${entry.name}`;
    if (entry.isDirectory()) files.push(...collectUiScripts(`${full}/`));
    else if (entry.name.endsWith(".js")) files.push(full);
  }
  return files;
}

describe("INT-07: UI invoke ⊆ generate_handler!", () => {
  it("каждая литеральная invoke-команда из rust/gui-spike/ui зарегистрирована в main.rs", () => {
    const uiFiles = collectUiScripts(uiDirNet);
    assert.ok(uiFiles.length > 0, "в rust/gui-spike/ui нет .js-файлов");

    /** @type {Map<string, string>} */
    const invokes = new Map();
    for (const file of uiFiles) {
      const source = readFileSync(file, "utf8");
      const pattern = /invoke\(\s*"([A-Za-z0-9_]+)"/g;
      /** @type {RegExpExecArray | null} */
      let match;
      while ((match = pattern.exec(source)) !== null) {
        const command = String(match[1]);
        const line = source.slice(0, match.index).split("\n").length;
        if (!invokes.has(command)) {
          invokes.set(command, `${file.slice(uiDirNet.length)}:${line}`);
        }
      }
    }
    assert.ok(invokes.size > 0, "в UI-слое не найдено ни одного литерального invoke");

    const mainSource = readFileSync(`${uiDirNet}../src/main.rs`, "utf8");
    const handlerBlock = mainSource.match(/generate_handler!\[([\s\S]*?)\]/);
    assert.ok(handlerBlock, "в rust/gui-spike/src/main.rs нет generate_handler![");
    const handlerBlockBody = String(handlerBlock?.[1] ?? "");
    const handlers = new Set(
      handlerBlockBody
        .split(",")
        .map((entry) => entry.trim())
        .filter((entry) => /^[a-z0-9_]+$/.test(entry)),
    );
    assert.ok(handlers.size > 0, "generate_handler![…] пуст — парсер сломан");

    const missing = [...invokes.entries()]
      .filter(([command]) => !handlers.has(command))
      .map(([command, where]) => `${command} (${where})`);
    assert.deepEqual(
      missing,
      [],
      `UI вызывает команды, которых нет в generate_handler!: ${missing.join(", ")}`,
    );
  });

  it("предварительная voice-секция не вызывает voice IPC, которого нет в Rust", () => {
    const appSource = readFileSync(`${uiDirNet}app.js`, "utf8");
    for (const command of VOICE_IPC_COMMANDS) {
      assert.doesNotMatch(
        appSource,
        new RegExp(`invoke\\(\\s*"${command}"`),
        `app.js всё ещё вызывает незарегистрированную команду ${command}`,
      );
    }
  });

  it("видимая пометка предварительного режима есть и в статической разметке index.html", () => {
    const html = readFileSync(`${uiDirNet}index.html`, "utf8");
    assert.match(html, /id="voice-devices-preliminary"/, "в index.html нет #voice-devices-preliminary");
    assert.match(html, /Предварительный режим/, "в index.html нет текста предварительного режима");
  });
});

/* =============================================================================
 * FE-P0-01 / task-133 (RW-40): монтирование createDmFriendsController в app.js
 * и хостов (#home-rail-btn, #dm-channel-list, #friends-view, #group-dm-dialog,
 * #stranger-confirm-dialog) в index.html + поддержка lastGuildId="@me".
 * Оракул: waves/RW-40/DESIGN/DM-FRIENDS-SPEC.md и waves/RW-40/DM-FRIENDS-RE.md.
 * ========================================================================== */

/**
 * Фикстура READY с пользователями, relationships и private_channels для Home/ЛС.
 */
function makeDmReadyPayload() {
  return {
    sessionId: "sess-dm-133",
    user: { id: "900000000000000001", username: "tester" },
    guilds: [
      { id: "100000000000000001", name: "Основная гильдия" },
      { id: "100000000000000002", name: "Вторая гильдия" },
    ],
    users: [
      { id: "200000000000000001", username: "alice", global_name: "Алиса", avatar: null },
      { id: "200000000000000002", username: "bob", global_name: "Боб", avatar: null },
      { id: "200000000000000003", username: "charlie", global_name: "Чарли", avatar: null },
      { id: "200000000000000004", username: "stranger", global_name: "Незнакомец", avatar: null },
    ],
    presences: {
      "200000000000000001": "online",
      "200000000000000002": "offline",
      "200000000000000003": "idle",
      "200000000000000004": "online",
    },
    relationships: [
      { id: "200000000000000001", type: 1 },
      { id: "200000000000000002", type: 1 },
      { id: "200000000000000003", type: 3 },
    ],
    private_channels: [
      {
        id: "300000000000000001",
        type: 1,
        recipients: [{ id: "200000000000000001", username: "alice", global_name: "Алиса" }],
        last_message_id: "400000000000000099",
        unread_count: 2,
      },
      {
        id: "300000000000000002",
        type: 3,
        name: "Штаб разработки",
        recipients: [
          { id: "200000000000000001", username: "alice", global_name: "Алиса" },
          { id: "200000000000000002", username: "bob", global_name: "Боб" },
        ],
        last_message_id: "400000000000000050",
      },
      {
        id: "300000000000000003",
        type: 1,
        recipients: [{ id: "200000000000000004", username: "stranger", global_name: "Незнакомец" }],
        last_message_id: "400000000000000010",
      },
    ],
  };
}

describe("FE-P0-01 / task-133: монтирование Home / ЛС / друзей в app.js и index.html", () => {
  it("1. В index.html и DOM смонтированы все 5 обязательных хостов (#home-rail-btn, #dm-channel-list, #friends-view, #group-dm-dialog, #stranger-confirm-dialog)", async () => {
    const html = readFileSync(`${uiDirNet}index.html`, "utf8");
    for (const hostId of [
      "home-rail-btn",
      "dm-channel-list",
      "friends-view",
      "group-dm-dialog",
      "stranger-confirm-dialog",
    ]) {
      assert.match(
        html,
        new RegExp(`id="${hostId}"`),
        `в index.html отсутствует обязательный хост #${hostId}`,
      );
    }

    await withApp(async (app) => {
      const homeBtn = byId(app, "home-rail-btn");
      const dmList = byId(app, "dm-channel-list");
      const friendsView = byId(app, "friends-view");
      const groupDialog = byId(app, "group-dm-dialog");
      const strangerDialog = byId(app, "stranger-confirm-dialog");

      assert.equal(homeBtn.getAttribute("data-guild-id"), "@me");
      assert.equal(homeBtn.getAttribute("data-action"), "nav-home");
      assert.ok(dmList, "нет #dm-channel-list в DOM");
      assert.ok(friendsView, "нет #friends-view в DOM");
      assert.ok(groupDialog, "нет #group-dm-dialog в DOM");
      assert.ok(strangerDialog, "нет #stranger-confirm-dialog в DOM");
      assert.ok(api(app).dmFriends, "currentApp().dmFriends не экспортирован");
    });
  });

  it("2. Клик по #home-rail-btn включает режим Home (@me), скрывает серверные каналы, показывает #dm-channel-list и #friends-view и сохраняет lastGuildId='@me'", async () => {
    /** @type {Array<Record<string, any>>} */
    const savedSettings = [];
    const app = await loadApp({
      invokeHandler: async (cmd, args) => {
        if (cmd === "ready") return makeDmReadyPayload();
        if (cmd === "channels") return [];
        if (cmd === "settings_save") {
          savedSettings.push(args?.settings ?? {});
          return null;
        }
        return undefined;
      },
    });
    try {
      await api(app).rail.load();
      await app.settle(6);

      const homeBtn = byId(app, "home-rail-btn");
      const tree = byId(app, "channel-tree");
      assert.notEqual(tree.getAttribute("data-nav-mode"), "home", "до клика не должен быть режим home");

      homeBtn.click();
      await app.settle(6);

      assert.equal(tree.getAttribute("data-nav-mode"), "home", "#channel-tree не переключился в data-nav-mode=home");
      assert.equal(homeBtn.classList.contains("is-selected"), true, "#home-rail-btn не получил .is-selected");
      assert.equal(homeBtn.getAttribute("aria-current"), "page", "#home-rail-btn не получил aria-current=page");
      assert.equal(api(app).rail.selectedId, "@me", "rail.selectedId должен быть '@me' в режиме Home");

      const guildTiles = app.document.querySelectorAll("#guild-rail .guild-item");
      for (const tile of guildTiles) {
        assert.equal(tile.classList.contains("is-selected"), false, "плитка гильдии осталась выбранной в Home");
        assert.equal(tile.getAttribute("aria-current"), null, "у невыбранной плитки гильдии остался aria-current");
      }

      assert.equal(byId(app, "dm-channel-list").getAttribute("hidden"), null, "#dm-channel-list скрыт в Home");
      assert.equal(byId(app, "friends-view").getAttribute("hidden"), null, "#friends-view скрыт в Home");
      assert.ok(
        savedSettings.some((s) => s.lastGuildId === "@me"),
        `settings_save не сохранил lastGuildId="@me": ${JSON.stringify(savedSettings)}`,
      );
      assert.deepEqual(app.consoleErrors, [], `ошибки консоли при входе в Home: ${app.consoleErrors.join("; ")}`);
    } finally {
      app.dispose();
    }
  });

  it("3. Возврат из Home на гильдию восстанавливает дерево каналов гильдии, скрывает #friends-view и сохраняет id гильдии", async () => {
    /** @type {Array<Record<string, any>>} */
    const savedSettings = [];
    const app = await loadApp({
      invokeHandler: async (cmd, args) => {
        if (cmd === "ready") return makeDmReadyPayload();
        if (cmd === "channels") return [];
        if (cmd === "settings_save") {
          savedSettings.push(args?.settings ?? {});
          return null;
        }
        return undefined;
      },
    });
    try {
      await api(app).rail.load();
      await app.settle(6);

      const homeBtn = byId(app, "home-rail-btn");
      homeBtn.click();
      await app.settle(6);
      assert.equal(byId(app, "channel-tree").getAttribute("data-nav-mode"), "home");

      const secondGuild = /** @type {ShimElement | null} */ (
        app.document.querySelector('#guild-rail .guild-item[data-guild-id="100000000000000002"]')
      );
      assert.ok(secondGuild, "не найдена вторая гильдия в рейле");
      secondGuild.click();
      await app.settle(6);

      assert.equal(byId(app, "channel-tree").getAttribute("data-nav-mode"), "guild");
      assert.equal(homeBtn.classList.contains("is-selected"), false);
      assert.equal(homeBtn.getAttribute("aria-current"), "false");
      assert.notEqual(byId(app, "friends-view").getAttribute("hidden"), null, "#friends-view не скрылся на гильдии");
      assert.notEqual(byId(app, "dm-channel-list").getAttribute("hidden"), null, "#dm-channel-list не скрылся на гильдии");
      assert.equal(secondGuild.classList.contains("is-selected"), true);
      assert.equal(secondGuild.getAttribute("aria-current"), "true");

      const treeGuild = app.document.querySelector(".tree-guild");
      assert.equal(treeGuild?.textContent, "Вторая гильдия", "заголовок гильдии в .tree-header не восстановился");
      assert.ok(
        savedSettings.some((s) => s.lastGuildId === "100000000000000002"),
        `settings_save не сохранил id выбранной гильдии: ${JSON.stringify(savedSettings)}`,
      );
    } finally {
      app.dispose();
    }
  });

  it("4. Восстановление режима Home (@me) при старте через readLastGuildId из settings_load (lastGuildId='@me')", async () => {
    const app = await loadApp({
      invokeHandler: async (cmd) => {
        if (cmd === "ready") return makeDmReadyPayload();
        if (cmd === "channels") return [];
        if (cmd === "settings_load") {
          return { version: 1, toggles: {}, density: "cozy", theme: "dark", locale: "ru", lastGuildId: "@me" };
        }
        return undefined;
      },
    });
    try {
      await api(app).rail.load();
      await app.settle(8);

      const homeBtn = byId(app, "home-rail-btn");
      const tree = byId(app, "channel-tree");
      assert.equal(tree.getAttribute("data-nav-mode"), "home", "режим Home не восстановился из lastGuildId='@me'");
      assert.equal(homeBtn.classList.contains("is-selected"), true, "#home-rail-btn не выбран после восстановления");
      assert.equal(homeBtn.getAttribute("aria-current"), "page");
      assert.equal(byId(app, "friends-view").getAttribute("hidden"), null, "#friends-view скрыт при старте в Home");
    } finally {
      app.dispose();
    }
  });

  it("5. Восстановление режима Home (@me) при старте через флаг homeMode: true в settings_load", async () => {
    const app = await loadApp({
      invokeHandler: async (cmd) => {
        if (cmd === "ready") return makeDmReadyPayload();
        if (cmd === "channels") return [];
        if (cmd === "settings_load") {
          return { version: 1, toggles: {}, density: "cozy", theme: "dark", locale: "ru", homeMode: true };
        }
        return undefined;
      },
    });
    try {
      await api(app).rail.load();
      await app.settle(8);

      assert.equal(
        byId(app, "channel-tree").getAttribute("data-nav-mode"),
        "home",
        "режим Home не восстановился из homeMode: true",
      );
      assert.equal(byId(app, "home-rail-btn").classList.contains("is-selected"), true);
    } finally {
      app.dispose();
    }
  });

  it("6. Проброс users, relationships и private_channels из ready() рендерит список ЛС и счётчики друзей", async () => {
    const app = await loadApp({
      invokeHandler: async (cmd) => {
        if (cmd === "ready") return makeDmReadyPayload();
        if (cmd === "channels") return [];
        return undefined;
      },
    });
    try {
      await api(app).rail.load();
      await app.settle(6);

      byId(app, "home-rail-btn").click();
      await app.settle(6);

      const dmRows = app.document.querySelectorAll("#dm-channel-list .dm-channel-row");
      assert.equal(dmRows.length, 3, "в #dm-channel-list должны отрендериться 3 приватных канала из ready()");
      // Отсортированы по last_message_id по убыванию: 300000000000000001 (Алиса), 300000000000000002 (Штаб), 300000000000000003 (Незнакомец)
      assert.equal(dmRows[0]?.getAttribute("data-channel-id"), "300000000000000001");
      assert.match(dmRows[0]?.textContent ?? "", /Алиса/);
      assert.equal(dmRows[1]?.getAttribute("data-channel-id"), "300000000000000002");
      assert.match(dmRows[1]?.textContent ?? "", /Штаб разработки/);
      assert.match(dmRows[1]?.textContent ?? "", /3 участника/);

      // Вкладка «В сети» по умолчанию показывает онлайн-друга Алису (type=1, online), но не Боба (offline)
      const friendRows = app.document.querySelectorAll("#friends-view .friend-row");
      assert.equal(friendRows.length, 1, "во вкладке 'В сети' должен быть ровно 1 онлайн-друг");
      assert.equal(friendRows[0]?.getAttribute("data-user-id"), "200000000000000001");

      // Бейдж «Ожидание» на вкладке показывает 1 входящий запрос (Чарли, type=3)
      const pendingBadge = byId(app, "friends-pending-badge");
      assert.equal(pendingBadge.textContent, "1");
      assert.equal(pendingBadge.getAttribute("hidden"), null);
    } finally {
      app.dispose();
    }
  });

  it("7. Клик по каналу в #dm-channel-list открывает сообщения этого ЛС в центральной панели", async () => {
    /** @type {string[]} */
    const openedChannelIds = [];
    const app = await loadApp({
      invokeHandler: async (cmd, args) => {
        if (cmd === "ready") return makeDmReadyPayload();
        if (cmd === "channels") return [];
        if (cmd === "messages_page") {
          const cid = String(args?.channelId ?? args?.channel_id ?? "");
          openedChannelIds.push(cid);
          return [
            {
              id: "500000000000000001",
              channelId: cid,
              author: "Алиса",
              content: "Привет из личных сообщений!",
              timestamp: "12:30",
            },
          ];
        }
        return undefined;
      },
    });
    try {
      await api(app).rail.load();
      await app.settle(6);

      byId(app, "home-rail-btn").click();
      await app.settle(6);

      const firstDm = /** @type {ShimElement | null} */ (
        app.document.querySelector('#dm-channel-list .dm-channel-item[data-channel-id="300000000000000001"]')
      );
      assert.ok(firstDm, "не найдена кнопка ЛС 300000000000000001");
      firstDm.click();
      await app.settle(8);

      assert.equal(firstDm.getAttribute("aria-current"), "page", "кнопка ЛС не получила aria-current=page");
      assert.equal(firstDm.classList.contains("is-selected"), true, "кнопка ЛС не получила .is-selected");
      assert.notEqual(byId(app, "friends-view").getAttribute("hidden"), null, "#friends-view должен скрыться при открытии ЛС");
      assert.equal(byId(app, "screen-chat").classList.contains("active"), true, "#screen-chat должен быть активен в ЛС");
      assert.ok(
        openedChannelIds.includes("300000000000000001"),
        `messages_page не был вызван для канала ЛС 300000000000000001: ${JSON.stringify(openedChannelIds)}`,
      );
    } finally {
      app.dispose();
    }
  });

  it("8. Клик по пункту 'Друзья' (#dm-nav-friends) после открытия ЛС возвращает центральную область на #friends-view", async () => {
    const app = await loadApp({
      invokeHandler: async (cmd) => {
        if (cmd === "ready") return makeDmReadyPayload();
        if (cmd === "channels" || cmd === "messages_page") return [];
        return undefined;
      },
    });
    try {
      await api(app).rail.load();
      await app.settle(6);

      byId(app, "home-rail-btn").click();
      await app.settle(6);

      const firstDm = /** @type {ShimElement} */ (
        app.document.querySelector('#dm-channel-list .dm-channel-item[data-channel-id="300000000000000001"]')
      );
      firstDm.click();
      await app.settle(6);
      assert.notEqual(byId(app, "friends-view").getAttribute("hidden"), null);

      const navFriends = byId(app, "dm-nav-friends");
      navFriends.click();
      await app.settle(6);

      assert.equal(byId(app, "friends-view").getAttribute("hidden"), null, "#friends-view не показался снова");
      const updatedFirstDm = /** @type {ShimElement} */ (
        app.document.querySelector('#dm-channel-list .dm-channel-item[data-channel-id="300000000000000001"]')
      );
      assert.equal(updatedFirstDm.getAttribute("aria-current"), null, "aria-current со строки ЛС не сбросился");
    } finally {
      app.dispose();
    }
  });

  it("9. Переключение вкладок #friends-view ('Все', 'Ожидание', 'Добавить в друзья') и фильтрация поиском", async () => {
    const app = await loadApp({
      invokeHandler: async (cmd) => {
        if (cmd === "ready") return makeDmReadyPayload();
        if (cmd === "channels") return [];
        return undefined;
      },
    });
    try {
      await api(app).rail.load();
      await app.settle(6);

      byId(app, "home-rail-btn").click();
      await app.settle(6);

      const allTab = byId(app, "friends-tab-all");
      allTab.click();
      await app.settle(4);
      assert.equal(allTab.getAttribute("aria-selected"), "true");
      assert.equal(
        app.document.querySelectorAll("#friends-view .friend-row").length,
        2,
        "во вкладке 'Все' должны быть оба друга (Алиса и Боб)",
      );

      const searchInput = byId(app, "friends-search-input");
      searchInput.value = "боб";
      searchInput.dispatchEvent("input");
      await app.settle(4);
      const filtered = app.document.querySelectorAll("#friends-view .friend-row");
      assert.equal(filtered.length, 1, "поиск по 'боб' должен оставить 1 строку");
      assert.equal(filtered[0]?.getAttribute("data-user-id"), "200000000000000002");

      searchInput.value = "";
      searchInput.dispatchEvent("input");
      const pendingTab = byId(app, "friends-tab-pending");
      pendingTab.click();
      await app.settle(4);
      const pendingRows = app.document.querySelectorAll("#friends-view .friend-row");
      assert.equal(pendingRows.length, 1, "во вкладке 'Ожидание' должен быть 1 запрос (Чарли)");
      assert.equal(pendingRows[0]?.getAttribute("data-user-id"), "200000000000000003");
    } finally {
      app.dispose();
    }
  });

  it("10. Вкладка 'Добавить в друзья' отправляет запрос и нормализует ошибки Discord на русском языке", async () => {
    const app = await loadApp({
      invokeHandler: async (cmd) => {
        if (cmd === "ready") return makeDmReadyPayload();
        if (cmd === "channels") return [];
        return undefined;
      },
    });
    try {
      await api(app).rail.load();
      await app.settle(6);

      byId(app, "home-rail-btn").click();
      await app.settle(6);

      const addTab = byId(app, "friends-tab-add");
      addTab.click();
      await app.settle(4);

      const input = byId(app, "add-friend-input");
      const submit = /** @type {any} */ (byId(app, "add-friend-submit"));
      assert.equal(submit.disabled, true, "кнопка отправки должна быть disabled при пустом поле");

      api(app).dmFriends.setRestHandler(async () => {
        const err = /** @type {any} */ (new Error("Friend request failed"));
        err.code = 80000;
        throw err;
      });

      input.value = "nobody#0001";
      input.dispatchEvent("input");
      assert.equal(submit.disabled, false, "кнопка должна активироваться при непустом вводе");

      const form = byId(app, "add-friend-form");
      form.dispatchEvent("submit");
      await app.settle(6);

      const feedback = byId(app, "add-friend-feedback");
      assert.equal(feedback.getAttribute("data-kind"), "error");
      assert.equal(feedback.getAttribute("hidden"), null);
      assert.ok(feedback.textContent.trim().length > 0, "сообщение об ошибке пустое");
    } finally {
      app.dispose();
    }
  });

  it("11. Диалог создания группового ЛС (#group-dm-dialog) и диалог подтверждения незнакомца (#stranger-confirm-dialog, код 80013) работают из смонтированного контроллера", async () => {
    const app = await loadApp({
      invokeHandler: async (cmd) => {
        if (cmd === "ready") return makeDmReadyPayload();
        if (cmd === "channels" || cmd === "messages_page") return [];
        return undefined;
      },
    });
    try {
      await api(app).rail.load();
      await app.settle(6);

      byId(app, "home-rail-btn").click();
      await app.settle(6);

      // Открытие #group-dm-dialog кнопкой +
      const createBtn = byId(app, "create-group-dm-btn");
      createBtn.click();
      await app.settle(4);
      const groupDialog = byId(app, "group-dm-dialog");
      assert.equal(groupDialog.getAttribute("hidden"), null, "#group-dm-dialog не открылся");

      const options = groupDialog.querySelectorAll('#group-dm-friend-list [role="option"]');
      assert.equal(options.length, 2, "в диалоге группового ЛС должны быть 2 друга (type=1)");
      options[0]?.click();
      options[1]?.click();
      byId(app, "group-dm-submit").click();
      await app.settle(6);

      assert.notEqual(groupDialog.getAttribute("hidden"), null, "#group-dm-dialog не закрылся после создания");
      assert.equal(
        app.document.querySelectorAll("#dm-channel-list .dm-channel-row").length,
        4,
        "новый групповой канал не добавился в #dm-channel-list",
      );

      // Код 80013 при принятии входящей заявки (Чарли, 200000000000000003) открывает #stranger-confirm-dialog
      let confirmCalls = 0;
      api(app).dmFriends.setRestHandler(async (/** @type {any} */ req) => {
        if (req.method === "PUT" && req.url.includes("/relationships/200000000000000003")) {
          confirmCalls += 1;
          if (!req.body?.confirm_stranger_request) {
            const err = /** @type {any} */ (new Error("Needs confirmation"));
            err.code = 80013;
            throw err;
          }
          return { status: 204, body: null };
        }
        return { status: 200, body: {} };
      });

      byId(app, "dm-nav-friends").click();
      byId(app, "friends-tab-pending").click();
      await app.settle(4);

      const acceptBtn = /** @type {ShimElement} */ (
        app.document.querySelector('#friends-view [data-action="accept-friend"][data-user-id="200000000000000003"]')
      );
      assert.ok(acceptBtn, "нет кнопки принятия заявки Чарли");
      acceptBtn.click();
      await app.settle(6);

      const strangerDialog = byId(app, "stranger-confirm-dialog");
      assert.equal(strangerDialog.getAttribute("hidden"), null, "#stranger-confirm-dialog не открылся при коде 80013");

      const confirmAcceptBtn = /** @type {ShimElement} */ (
        strangerDialog.querySelector('[data-action="accept-stranger-confirm"]')
      );
      confirmAcceptBtn.click();
      await app.settle(6);

      assert.notEqual(strangerDialog.getAttribute("hidden"), null, "#stranger-confirm-dialog не закрылся после подтверждения");
      assert.equal(confirmCalls, 2, "повторный PUT с confirm_stranger_request: true не был отправлен");
    } finally {
      app.dispose();
    }
  });

  it("12. Клавиатурная активация #home-rail-btn (Enter/Space) и отсутствие console.error/warn", async () => {
    const app = await loadApp({
      invokeHandler: async (cmd) => {
        if (cmd === "ready") return makeDmReadyPayload();
        if (cmd === "channels") return [];
        return undefined;
      },
    });
    try {
      await api(app).rail.load();
      await app.settle(6);

      const homeBtn = byId(app, "home-rail-btn");
      homeBtn.dispatchEvent("keydown", { key: "Enter" });
      await app.settle(6);

      assert.equal(byId(app, "channel-tree").getAttribute("data-nav-mode"), "home");
      assert.equal(homeBtn.classList.contains("is-selected"), true);
      assert.deepEqual(app.consoleErrors, []);
      assert.deepEqual(app.consoleWarns, []);
    } finally {
      app.dispose();
    }
  });
});

/* =============================================================================
 * FE-P0-03 / task-148 (RW-40): монтирование createNotificationsController в app.js,
 * бейджи непрочитанного (.guild-unread-pill) и счётчики упоминаний (1..99 и '99+'
 * с aria-label) в #guild-list и #channel-list, автоматический ackChannel
 * (ack_channel_message) при переключении текстового канала, контекстное меню
 * гильдии (Mute 15m/1h/8h/24h/indefinitely и All/Mentions/Nothing ->
 * update_guild_notification_settings) и очистка при destroy().
 * ========================================================================== */

function makeNotifMountReadyPayload() {
  return {
    user: { id: "42", username: "tester" },
    guilds: [
      { id: "7001", name: "Альфа Сервер", memberCount: 120 },
      { id: "7002", name: "Бета Мегаполис", memberCount: 3200 },
    ],
    read_state: {
      entries: [
        {
          id: "8001",
          guild_id: "7001",
          unread: true,
          mention_count: 5,
          last_message_id: "9001001",
          last_viewed: 4280,
          flags: 1,
        },
        {
          id: "8002",
          guild_id: "7001",
          unread: true,
          mention_count: 0,
          last_message_id: "9001002",
          last_viewed: 4280,
          flags: 1,
        },
        {
          id: "8003",
          guild_id: "7002",
          unread: true,
          mention_count: 120,
          last_message_id: "9002001",
          last_viewed: 4280,
          flags: 1,
        },
      ],
    },
    user_guild_settings: {
      entries: [
        {
          guild_id: "7001",
          message_notifications: 1,
          muted: false,
          mute_config: null,
          suppress_everyone: false,
          suppress_roles: false,
          notify_highlights: 0,
          hide_muted_channels: false,
          mute_scheduled_events: false,
          mobile_push: true,
          version: 10,
          channel_overrides: [],
        },
        {
          guild_id: "7002",
          message_notifications: 1,
          muted: false,
          mute_config: null,
          suppress_everyone: false,
          suppress_roles: false,
          notify_highlights: 0,
          hide_muted_channels: false,
          mute_scheduled_events: false,
          mobile_push: true,
          version: 11,
          channel_overrides: [],
        },
      ],
    },
  };
}

const NOTIF_MOUNT_CHANNELS = /** @type {Record<string, any[]>} */ ({
  "7001": [
    { id: "8000", type: 4, name: "ИНФО", parentId: "", position: 0 },
    {
      id: "8001",
      type: 0,
      name: "объявления",
      parentId: "8000",
      position: 1,
      last_message_id: "9001001",
    },
    {
      id: "8002",
      type: 0,
      name: "общий",
      parentId: "8000",
      position: 2,
      last_message_id: "9001002",
    },
  ],
  "7002": [
    {
      id: "8003",
      type: 0,
      name: "горячая-линия",
      parentId: "",
      position: 0,
      last_message_id: "9002001",
    },
  ],
});

describe("FE-P0-03 / task-148: монтирование createNotificationsController, бейджи, авто-ack и контекстное меню в app.js", () => {
  it("1. В app.js импортирован и инициализирован createNotificationsController при старте приложения, в DOM есть #guild-list и #channel-list", async () => {
    const appSource = readFileSync(`${uiDirNet}app.js`, "utf8");
    assert.match(
      appSource,
      /createNotificationsController/,
      "в app.js не найден createNotificationsController",
    );

    await withApp(async (app) => {
      assert.equal(
        typeof /** @type {any} */ (app.appModule)?.createNotificationsController,
        "function",
        "app.js не экспортирует createNotificationsController",
      );
      const runtime = api(app);
      assert.ok(runtime.notifications, "currentApp().notifications не инициализирован при старте");
      assert.equal(
        typeof /** @type {any} */ (runtime.notifications).ackChannel,
        "function",
        "у контроллера уведомлений нет метода ackChannel(channelId, lastMessageId)",
      );
      assert.equal(
        typeof /** @type {any} */ (runtime.notifications).openGuildContextMenu,
        "function",
        "у контроллера уведомлений нет метода openGuildContextMenu",
      );
      assert.equal(
        typeof runtime.notifications.destroy,
        "function",
        "у контроллера уведомлений нет метода destroy()",
      );
      assert.ok(byId(app, "guild-list"), "в DOM нет #guild-list");
      assert.ok(byId(app, "channel-list"), "в DOM нет #channel-list");
    });
  });

  it("2. Отрисовка бейджей непрочитанных сообщений (.guild-unread-pill) и счётчиков упоминаний (1..99 и '99+' с aria-label) в #guild-list и #channel-list", async () => {
    const app = await loadApp({
      invokeHandler: async (cmd, args) => {
        if (cmd === "ready") return makeNotifMountReadyPayload();
        if (cmd === "channels") return NOTIF_MOUNT_CHANNELS[String(args?.guildId ?? "")] ?? [];
        if (cmd === "messages_page") return [];
        return undefined;
      },
    });
    try {
      await api(app).rail.load();
      await app.settle(6);

      const guildList = byId(app, "guild-list");
      const g1Tile = /** @type {ShimElement} */ (
        guildList.querySelector('.guild-item[data-guild-id="7001"]')
      );
      const g2Tile = /** @type {ShimElement} */ (
        guildList.querySelector('.guild-item[data-guild-id="7002"]')
      );
      assert.ok(g1Tile, "нет плитки гильдии 7001 в #guild-list");
      assert.ok(g2Tile, "нет плитки гильдии 7002 в #guild-list");

      // Гильдия 7001: непрочитано + 5 упоминаний
      const g1Pill = g1Tile.querySelector(".guild-unread-pill");
      assert.ok(g1Pill, "на непрочитанной гильдии 7001 нет .guild-unread-pill");
      assert.equal(g1Pill.getAttribute("hidden"), null, ".guild-unread-pill гильдии 7001 скрыт");
      const g1Badge = g1Tile.querySelector(".guild-mention-badge");
      assert.ok(g1Badge, "на гильдии 7001 нет .guild-mention-badge");
      assert.equal(g1Badge.textContent, "5", "счётчик упоминаний гильдии 7001 не равен '5'");
      assert.match(
        g1Tile.getAttribute("aria-label") ?? "",
        /5 упоминаний/u,
        "aria-label гильдии 7001 не содержит '5 упоминаний'",
      );

      // Гильдия 7002: непрочитано + 120 упоминаний -> кэппинг '99+'
      const g2Pill = g2Tile.querySelector(".guild-unread-pill");
      assert.ok(g2Pill, "на непрочитанной гильдии 7002 нет .guild-unread-pill");
      const g2Badge = g2Tile.querySelector(".guild-mention-badge");
      assert.ok(g2Badge, "на гильдии 7002 нет .guild-mention-badge");
      assert.equal(g2Badge.textContent, "99+", "счётчик 120 упоминаний гильдии 7002 не округлён до '99+'");
      assert.match(
        g2Tile.getAttribute("aria-label") ?? "",
        /120 упоминаний/u,
        "aria-label гильдии 7002 не содержит точное число '120 упоминаний'",
      );

      // Каналы гильдии 7001 в #channel-list
      const channelList = byId(app, "channel-list");
      const ch1Btn = /** @type {ShimElement} */ (
        channelList.querySelector('.channel-item[data-channel-id="8001"]')
      );
      const ch2Btn = /** @type {ShimElement} */ (
        channelList.querySelector('.channel-item[data-channel-id="8002"]')
      );
      assert.ok(ch1Btn, "в #channel-list нет канала 8001");
      assert.ok(ch2Btn, "в #channel-list нет канала 8002");

      assert.equal(ch1Btn.classList.contains("is-unread"), true, "канал 8001 не помечен .is-unread");
      assert.ok(ch1Btn.querySelector(".guild-unread-pill"), "в канале 8001 нет .guild-unread-pill");
      assert.equal(ch1Btn.querySelector(".mention-badge")?.textContent, "5", "бейдж канала 8001 не '5'");
      assert.match(
        ch1Btn.getAttribute("aria-label") ?? "",
        /5 упоминаний/u,
        "aria-label канала 8001 не содержит '5 упоминаний'",
      );

      assert.equal(ch2Btn.classList.contains("is-unread"), true, "канал 8002 не помечен .is-unread");
      assert.ok(ch2Btn.querySelector(".guild-unread-pill"), "в канале 8002 нет .guild-unread-pill");
      assert.equal(ch2Btn.querySelector(".mention-badge"), null, "у канала 8002 без упоминаний есть .mention-badge");

      // Переключаемся на гильдию 7002 и проверяем '99+' в #channel-list
      g2Tile.click();
      await app.settle(6);
      const ch3Btn = /** @type {ShimElement} */ (
        byId(app, "channel-list").querySelector('.channel-item[data-channel-id="8003"]')
      );
      assert.ok(ch3Btn, "в #channel-list гильдии 7002 нет канала 8003");
      assert.equal(ch3Btn.querySelector(".mention-badge")?.textContent, "99+", "бейдж канала 8003 не '99+'");
      assert.match(
        ch3Btn.getAttribute("aria-label") ?? "",
        /120 упоминаний/u,
        "aria-label канала 8003 не содержит '120 упоминаний'",
      );
    } finally {
      app.dispose();
    }
  });

  it("3. Автоматический вызов ackChannel(channelId, lastMessageId) (ack_channel_message) при переключении активного текстового канала", async () => {
    const app = await loadApp({
      invokeHandler: async (cmd, args) => {
        if (cmd === "ready") return makeNotifMountReadyPayload();
        if (cmd === "channels") return NOTIF_MOUNT_CHANNELS[String(args?.guildId ?? "")] ?? [];
        if (cmd === "messages_page") return [];
        if (cmd === "ack_channel_message") return { ok: true };
        return undefined;
      },
    });
    try {
      await api(app).rail.load();
      await app.settle(6);

      const ch1Btn = /** @type {ShimElement} */ (
        byId(app, "channel-list").querySelector('.channel-item[data-channel-id="8001"]')
      );
      assert.ok(ch1Btn, "нет канала 8001");
      ch1Btn.click();
      await app.settle(6);

      const ackCalls1 = app.callsOf("ack_channel_message");
      assert.equal(ackCalls1.length, 1, "при открытии канала 8001 не вызван ack_channel_message");
      assert.equal(ackCalls1[0]?.args?.channelId, "8001");
      assert.equal(ackCalls1[0]?.args?.lastMessageId, "9001001");

      // Бейдж упоминаний и непрочитанности канала 8001 снят
      assert.equal(ch1Btn.classList.contains("is-unread"), false, "после ack канал 8001 остался .is-unread");
      assert.equal(ch1Btn.querySelector(".mention-badge"), null, "после ack у канала 8001 остался .mention-badge");
      assert.equal(ch1Btn.querySelector(".guild-unread-pill"), null, "после ack у канала 8001 остался .guild-unread-pill");

      // У гильдии 7001 упоминаний стало 0, но .guild-unread-pill ещё виден из-за непрочитанного канала 8002
      const g1Tile = /** @type {ShimElement} */ (
        byId(app, "guild-list").querySelector('.guild-item[data-guild-id="7001"]')
      );
      assert.equal(g1Tile.querySelector(".guild-mention-badge"), null, "у гильдии 7001 остался .guild-mention-badge после ack 8001");
      assert.ok(g1Tile.querySelector(".guild-unread-pill"), "у гильдии 7001 пропал .guild-unread-pill при непрочитанном 8002");

      // Переключаемся на второй канал 8002 -> второй ack_channel_message снимает и .guild-unread-pill у гильдии 7001
      const ch2Btn = /** @type {ShimElement} */ (
        byId(app, "channel-list").querySelector('.channel-item[data-channel-id="8002"]')
      );
      ch2Btn.click();
      await app.settle(6);

      const ackCalls2 = app.callsOf("ack_channel_message");
      assert.equal(ackCalls2.length, 2, "при переключении на канал 8002 не вызван второй ack_channel_message");
      assert.equal(ackCalls2[1]?.args?.channelId, "8002");
      assert.equal(ackCalls2[1]?.args?.lastMessageId, "9001002");
      assert.equal(g1Tile.querySelector(".guild-unread-pill"), null, "после прочтения всех каналов у гильдии 7001 остался .guild-unread-pill");
    } finally {
      app.dispose();
    }
  });

  it("4. Контекстное меню гильдии / настройки уведомлений (Mute 15m/1h/8h/24h/indefinitely и All/Mentions/Nothing -> update_guild_notification_settings) и очистка при destroy()", async () => {
    const app = await loadApp({
      invokeHandler: async (cmd, args) => {
        if (cmd === "ready") return makeNotifMountReadyPayload();
        if (cmd === "channels") return NOTIF_MOUNT_CHANNELS[String(args?.guildId ?? "")] ?? [];
        if (cmd === "messages_page") return [];
        if (cmd === "update_guild_notification_settings") return { ok: true };
        return undefined;
      },
    });
    try {
      await api(app).rail.load();
      await app.settle(6);

      const g1Tile = /** @type {ShimElement} */ (
        byId(app, "guild-list").querySelector('.guild-item[data-guild-id="7001"]')
      );
      assert.ok(g1Tile, "нет плитки гильдии 7001");

      // Открываем контекстное меню гильдии по правому клику (contextmenu)
      g1Tile.dispatchEvent("contextmenu", { target: g1Tile });
      const ctxMenu = byId(app, "guild-notif-context-menu");
      assert.equal(ctxMenu.getAttribute("hidden"), null, "contextmenu по гильдии не открыл #guild-notif-context-menu");
      assert.equal(ctxMenu.getAttribute("data-guild-id"), "7001");

      // Проверяем пресеты Mute: 15m (900), 1h (3600), 8h (28800), 24h (86400), indefinitely (-1)
      for (const [durationKey, windowSec] of /** @type {Array<[string, number]>} */ ([
        ["15m", 900],
        ["1h", 3600],
        ["8h", 28800],
        ["24h", 86400],
        ["indefinitely", -1],
      ])) {
        const presetBtn = /** @type {ShimElement} */ (
          ctxMenu.querySelector(`[data-action="guild-mute-preset"][data-mute-duration="${durationKey}"]`) ??
          ctxMenu.querySelector(`[data-action="guild-mute-preset"][data-window="${windowSec}"]`)
        );
        assert.ok(presetBtn, `нет пресета Mute ${durationKey} (${windowSec}) в контекстном меню гильдии`);
        presetBtn.click();
        await app.settle(3);
        const lastCall = app.callsOf("update_guild_notification_settings").at(-1);
        assert.ok(lastCall, `клик по Mute ${durationKey} не вызвал update_guild_notification_settings`);
        assert.equal(lastCall.args?.guildId, "7001");
        assert.equal(lastCall.args?.patch?.muted, true);
        assert.equal(lastCall.args?.patch?.mute_config?.selected_time_window, windowSec);
      }

      // При заглушённой гильдии пилюля непрочитанного скрыта, а data-muted="true"
      assert.equal(g1Tile.getAttribute("data-muted"), "true", "гильдия 7001 не получила data-muted=true после Mute");
      assert.equal(g1Tile.querySelector(".guild-unread-pill"), null, "у заглушённой гильдии 7001 остался .guild-unread-pill");

      // Проверяем уровни уведомлений: All (0), Mentions (1), Nothing (2)
      for (const [levelKey, levelNum] of /** @type {Array<[string, number]>} */ ([
        ["all", 0],
        ["mentions", 1],
        ["nothing", 2],
      ])) {
        const lvlBtn = /** @type {ShimElement} */ (
          ctxMenu.querySelector(`[data-action="guild-notif-level"][data-level="${levelNum}"]`)
        );
        assert.ok(lvlBtn, `нет уровня уведомлений ${levelKey} (${levelNum}) в контекстном меню гильдии`);
        lvlBtn.click();
        await app.settle(3);
        const lastCall = app.callsOf("update_guild_notification_settings").at(-1);
        assert.equal(lastCall?.args?.guildId, "7001");
        assert.equal(lastCall?.args?.patch?.message_notifications, levelNum);
      }

      // Проверяем очистку при destroy()
      api(app).notifications.destroy();
      assert.equal(app.document.getElementById("guild-notifications-dialog"), null, "destroy() не удалил #guild-notifications-dialog");
      assert.equal(app.document.getElementById("channel-notif-menu"), null, "destroy() не удалил #channel-notif-menu");
      assert.equal(app.document.getElementById("guild-notif-context-menu"), null, "destroy() не удалил #guild-notif-context-menu");
    } finally {
      app.dispose();
    }
  });
});

describe("FE-P1-06 / task-164: монтирование createUserPopoutController и createGuildModalsController в app.js", () => {
  it("1. В app.js импортированы и смонтированы createUserPopoutController и createGuildModalsController, возвращены в currentApp()", async () => {
    const appSource = readFileSync(`${uiDirNet}app.js`, "utf8");
    assert.match(
      appSource,
      /createUserPopoutController/,
      "в app.js не найден импорт createUserPopoutController",
    );
    assert.match(
      appSource,
      /createGuildModalsController/,
      "в app.js не найден импорт createGuildModalsController",
    );

    await withApp(async (app) => {
      const runtime = api(app);
      assert.ok(runtime.userPopout, "currentApp().userPopout не инициализирован при старте");
      assert.equal(
        typeof runtime.userPopout.openPopout,
        "function",
        "у userPopout нет метода openPopout",
      );
      assert.equal(
        typeof runtime.userPopout.closePopout,
        "function",
        "у userPopout нет метода closePopout",
      );
      assert.equal(
        typeof runtime.userPopout.openProfileModal,
        "function",
        "у userPopout нет метода openProfileModal",
      );
      assert.equal(
        typeof runtime.userPopout.attachToChatAndMembers,
        "function",
        "у userPopout нет метода attachToChatAndMembers",
      );

      assert.ok(runtime.guildModals, "currentApp().guildModals не инициализирован при старте");
      assert.equal(
        typeof runtime.guildModals.openContextMenu,
        "function",
        "у guildModals нет метода openContextMenu",
      );
      assert.equal(
        typeof runtime.guildModals.openInviteModal,
        "function",
        "у guildModals нет метода openInviteModal",
      );
      assert.equal(
        typeof runtime.guildModals.openLeaveModal,
        "function",
        "у guildModals нет метода openLeaveModal",
      );
      assert.equal(
        typeof runtime.guildModals.openJoinModal,
        "function",
        "у guildModals нет метода openJoinModal",
      );
      assert.equal(
        typeof runtime.guildModals.attachTrigger,
        "function",
        "у guildModals нет метода attachTrigger",
      );

      // DOM элементы в body
      assert.ok(byId(app, "user-popout"), "в DOM нет #user-popout");
      assert.ok(byId(app, "user-profile-modal"), "в DOM нет #user-profile-modal");
      assert.ok(byId(app, "guild-context-menu"), "в DOM нет #guild-context-menu");
      assert.ok(byId(app, "invite-modal"), "в DOM нет #invite-modal");
      assert.ok(byId(app, "leave-guild-modal"), "в DOM нет #leave-guild-modal");
      assert.ok(byId(app, "join-guild-modal"), "в DOM нет #join-guild-modal");

      // До первого взаимодействия модалки и поповеры скрыты
      assert.notEqual(byId(app, "user-popout").getAttribute("hidden"), null, "#user-popout не скрыт по умолчанию");
      assert.notEqual(byId(app, "guild-context-menu").getAttribute("hidden"), null, "#guild-context-menu не скрыт по умолчанию");
      assert.notEqual(byId(app, "invite-modal").getAttribute("hidden"), null, "#invite-modal не скрыт по умолчанию");
    });
  });

  it("2. userPopout открывается по клику на .author в чате и .member в списке участников с корректными данными", async () => {
    await withApp(async (app) => {
      const runtime = api(app);
      const chatList = byId(app, "chat-list");
      const memberList = byId(app, "member-list");

      // Создаём тестовую строку чата с автором
      const chatRow = app.document.createElement("div");
      chatRow.className = "row";
      chatRow.setAttribute("data-message-id", "msg-test-1");
      chatRow.setAttribute("data-author-id", "user-42");
      const authorSpan = app.document.createElement("span");
      authorSpan.className = "author";
      const authorName = app.document.createElement("span");
      authorName.className = "author-name";
      authorName.textContent = "TestUser";
      authorSpan.appendChild(authorName);
      chatRow.appendChild(authorSpan);
      chatList.appendChild(chatRow);

      // Клик по имени автора в чате
      chatList.dispatchEvent("click", { target: authorName });

      const popout = byId(app, "user-popout");
      assert.equal(popout.getAttribute("hidden"), null, "клик по .author-name не открыл #user-popout");
      assert.equal(
        popout.querySelector(".user-popout-display-name")?.textContent,
        "TestUser",
        "в поповере неверное имя пользователя",
      );

      // Закрываем через closePopout()
      runtime.userPopout.closePopout();
      assert.notEqual(popout.getAttribute("hidden"), null, "closePopout() не скрыл #user-popout");

      // Создаём тестовую строку участника
      const memberRow = app.document.createElement("div");
      memberRow.className = "member member-row";
      memberRow.setAttribute("data-user-id", "member-99");
      const memberName = app.document.createElement("span");
      memberName.className = "member-name";
      memberName.textContent = "CommanderShepard";
      memberRow.appendChild(memberName);
      memberList.appendChild(memberRow);

      // Клик по участнику в списке участников
      memberList.dispatchEvent("click", { target: memberName });
      assert.equal(popout.getAttribute("hidden"), null, "клик по .member-name не открыл #user-popout");
      assert.equal(
        popout.querySelector(".user-popout-display-name")?.textContent,
        "CommanderShepard",
        "в поповере неверное имя участника",
      );
    });
  });

  it("3. guildModals открывает #guild-context-menu по contextmenu на триггере сервера/рейла и обрабатывает Escape", async () => {
    await withApp(async (app) => {
      const runtime = api(app);
      const rail = byId(app, "guild-rail");
      assert.ok(rail, "нет #guild-rail");

      // Правый клик (contextmenu) по рейлу гильдий открывает #guild-context-menu
      rail.dispatchEvent("contextmenu", /** @type {any} */ ({ clientX: 120, clientY: 80 }));

      const ctxMenu = byId(app, "guild-context-menu");
      assert.equal(ctxMenu.getAttribute("hidden"), null, "contextmenu на #guild-rail не открыл #guild-context-menu");
      assert.equal(runtime.guildModals.getState().contextMenuOpen, true, "getState().contextMenuOpen !== true");

      // Escape закрывает контекстное меню
      ctxMenu.dispatchEvent("keydown", { key: "Escape" });
      assert.notEqual(ctxMenu.getAttribute("hidden"), null, "Escape не скрыл #guild-context-menu");
      assert.equal(runtime.guildModals.getState().contextMenuOpen, false, "getState().contextMenuOpen !== false");
    });
  });

  it("4. В #guild-context-menu пункт 'Настройки уведомлений' вызывает onOpenNotifications (sync/openModal)", async () => {
    let notifOpened = false;
    const app = await loadApp();
    try {
      const runtime = api(app);
      // Подменяем notifications.openModal
      runtime.notifications.openModal = () => {
        notifOpened = true;
      };

      runtime.guildModals.openContextMenu({ x: 50, y: 50, guildId: "g-test" });
      const ctxMenu = byId(app, "guild-context-menu");
      assert.equal(ctxMenu.getAttribute("hidden"), null, "контекстное меню не открылось");

      const notifItem = /** @type {ShimElement} */ (
        ctxMenu.querySelector('[data-action="notifications"]')
      );
      assert.ok(notifItem, "в #guild-context-menu нет кнопки с data-action='notifications'");
      notifItem.click();

      assert.equal(notifOpened, true, "клик по 'Настройки уведомлений' не вызвал notifications.openModal()");
      assert.notEqual(ctxMenu.getAttribute("hidden"), null, "клик по пункту меню не закрыл #guild-context-menu");
    } finally {
      app.dispose();
    }
  });

  it("5. Очистка ресурсов при destroy() контроллеров: удаление DOM узлов и снятие слушателей", async () => {
    await withApp(async (app) => {
      const runtime = api(app);
      assert.ok(byId(app, "user-popout"), "#user-popout отсутствует перед destroy()");
      assert.ok(byId(app, "guild-context-menu"), "#guild-context-menu отсутствует перед destroy()");

      runtime.userPopout.destroy();
      assert.equal(app.document.getElementById("user-popout"), null, "userPopout.destroy() не удалил #user-popout");
      assert.equal(app.document.getElementById("user-profile-modal"), null, "userPopout.destroy() не удалил #user-profile-modal");

      runtime.guildModals.destroy();
      // guildModals.destroy() закрывает модалки и снимает слушатели
      assert.equal(runtime.guildModals.getState().contextMenuOpen, false);
      assert.equal(runtime.guildModals.getState().inviteModalOpen, false);
    });
  });
});


