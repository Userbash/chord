// @ts-check
// W39d: оракул новых интеракций (не входит в сдаваемые tests/** — файловая матрица
// брифа запрещает новые test-файлы). Гоняет настоящий `ui/app.js` в DOM-шиме:
//   node waves/RW-39/PM/w39d/features-check.mjs
// Проверяет: body[data-active-screen]/aria-current, плоский listbox эмодзи (D-07),
// roving tabindex + стрелки, 38 канонических тумблеров и #settings-nav, панель
// участников 42px из model.js, отсутствие console.error/warn в штатном сценарии.
import assert from "node:assert/strict";
import { loadApp } from "../../../../tests/support/gui-dom-harness.mjs";

/**
 * @param {string} name
 * @param {() => void | Promise<void>} body
 */
async function check(name, body) {
  try {
    await body();
    console.log(`ok   | ${name}`);
  } catch (error) {
    console.log(`FAIL | ${name}`);
    throw error;
  }
}

const app = await loadApp();
try {
  const doc = app.document;

  /** @param {string} id */
  const byId = (id) => {
    const node = doc.getElementById(id);
    assert.ok(node, `нет #${id}`);
    return node;
  };
  /** @param {string} selector */
  const all = (selector) => doc.querySelectorAll(selector);

  await check("body[data-active-screen] + aria-current на 4 экранах", () => {
    assert.equal(doc.body.getAttribute("data-active-screen"), "chat", "стартовый экран не chat");
    for (const screen of ["emoji", "members", "settings", "chat"]) {
      const button = all(".rail-btn").find((node) => node.dataset.screen === screen);
      assert.ok(button, `нет rail-кнопки ${screen}`);
      button.click();
      assert.equal(doc.body.getAttribute("data-active-screen"), screen, `body не переключился на ${screen}`);
      assert.equal(button.getAttribute("aria-current"), "page", `aria-current у ${screen}`);
      const current = all(".rail-btn").filter((node) => node.getAttribute("aria-current") === "page");
      assert.equal(current.length, 1, `aria-current=page у ${current.length} кнопок`);
      assert.equal(all(".screen.active").length, 1, `активных экранов ${all(".screen.active").length}`);
    }
  });

  await check("эмодзи: плоский listbox (role=option, без .emoji-row) и 10 вкладок-глифов", () => {
    all(".rail-btn").find((node) => node.dataset.screen === "emoji")?.click();
    assert.equal(byId("emoji-grid").getAttribute("role"), "listbox", "#emoji-grid не listbox");
    assert.equal(all(".emoji-row").length, 0, "в плоском listbox остались .emoji-row");
    assert.equal(all(".tab").length, 10, "вкладок не 10");
    const activeTabs = all(".tab").filter((tab) => tab.getAttribute("aria-selected") === "true");
    assert.equal(activeTabs.length, 1, `aria-selected=true у ${activeTabs.length} вкладок`);
    for (const tab of all(".tab")) {
      assert.equal(tab.getAttribute("role"), "tab", "вкладка без role=tab");
      assert.ok((tab.title ?? "").length > 2, "у вкладки нет title с названием категории");
      assert.equal(tab.textContent.trim(), tab.textContent, "глиф вкладки с пробелами");
      assert.ok(tab.textContent.length <= 4, `во вкладке текст «${tab.textContent}», а не глиф`);
      assert.equal(tab.getAttribute("aria-controls"), "emoji-grid", "aria-controls вкладки");
    }
    const cells = all(".emoji-cell");
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

  await check("эмодзи: стрелки двигают aria-selected и roving tabindex", () => {
    const grid = byId("emoji-grid");
    const cells = all(".emoji-cell");
    assert.ok(cells.length > 2, "мало ячеек для roving");
    grid.dispatchEvent("keydown", { key: "ArrowRight" });
    const selected = all(".emoji-cell").filter((cell) => cell.getAttribute("aria-selected") === "true");
    assert.equal(selected.length, 1, `aria-selected=true у ${selected.length} ячеек`);
    assert.equal(selected[0], cells[1], "ArrowRight не перевёл выбор на вторую ячейку");
    assert.equal(cells[1]?.getAttribute("tabindex"), "0", "tabindex не переехал");
    assert.equal(cells[0]?.getAttribute("tabindex"), "-1", "старая ячейка осталась в табе");
    grid.dispatchEvent("keydown", { key: "End" });
    const lastCells = all(".emoji-cell");
    assert.equal(lastCells[lastCells.length - 1]?.getAttribute("aria-selected"), "true", "End не выбрал последнюю");
  });

  await check("настройки: 38 канонических тумблеров (switch/aria-checked) + #settings-nav", () => {
    all(".rail-btn").find((node) => node.dataset.screen === "settings")?.click();
    const settings = byId("settings");
    assert.equal(settings.querySelectorAll(".toggle").length, 38, "тумблеров не 38");
    assert.equal(settings.querySelectorAll('input[type="checkbox"]').length, 38, "чекбоксов не 38");
    for (const label of settings.querySelectorAll(".toggle")) {
      assert.ok(label.querySelector(".toggle-label"), "нет .toggle-label");
      const input = label.querySelector('input[type="checkbox"]');
      assert.ok(input, "нет input[type=checkbox]");
      assert.equal(input.getAttribute("class"), "toggle-input", "input без .toggle-input");
      assert.equal(input.getAttribute("role"), "switch", "input без role=switch");
      assert.equal(input.getAttribute("aria-checked"), input.checked ? "true" : "false", "aria-checked рассинхронизирован");
      assert.ok(label.querySelector(".toggle-track"), "нет .toggle-track");
      assert.ok(label.querySelector(".toggle-track .toggle-thumb"), "нет .toggle-thumb");
    }
    const nav = byId("settings-nav");
    assert.equal(nav.querySelectorAll("button").length, 3, "в #settings-nav не 3 кнопки");
    assert.equal(nav.querySelectorAll("h3").length, 0, "в #settings-nav попали h3 (счётчик групп)");
    assert.deepEqual(
      nav.querySelectorAll("button").map((button) => button.textContent),
      ["Голос и видео", "Интерфейс", "Уведомления"],
    );
    const first = settings.querySelector('input[type="checkbox"]');
    assert.equal(first.getAttribute("aria-checked"), "true");
    first.click();
    assert.equal(first.getAttribute("aria-checked"), "false", "click не синхронизировал aria-checked");
    first.checked = false;
    first.emit("change");
    assert.equal(first.getAttribute("aria-checked"), "false", "change не синхронизировал aria-checked");
  });

  await check("панель участников: ≤50 строк из model.js, высота 42, data-window=preview", async () => {
    all(".rail-btn").find((node) => node.dataset.screen === "chat")?.click();
    await app.settle(6);
    const list = byId("member-panel-list");
    const rows = list.querySelectorAll(".member");
    assert.ok(rows.length >= 10 && rows.length <= 50, `в панели ${rows.length} строк`);
    for (const row of rows) {
      assert.equal(row.getAttribute("data-window"), "preview", "нет data-window=preview");
      assert.equal(row.getAttribute("role"), "listitem", "строка панели без role=listitem");
      assert.equal(row.style.height, "42px", "высота строки панели не 42");
      assert.equal(row.children.length, 3, "в строке панели не 3 ребёнка");
      assert.match(row.children[1]?.textContent ?? "", /^member_\d+_/, "имя строки панели не member_<i>_");
    }
    const count = byId("member-online").textContent.trim();
    assert.ok(/^\d[\d\s]*$/.test(count), `#member-online = «${count}», а не число`);
  });

  await check("клавиши: Alt+1…4, Escape, Tab-trap верхнего слоя", () => {
    const key = (/** @type {string} */ value, /** @type {any} */ init = {}) =>
      doc.body.dispatchEvent("keydown", { key: value, altKey: false, ctrlKey: false, metaKey: false, shiftKey: false, ...init });
    key("2", { altKey: true });
    assert.equal(doc.body.getAttribute("data-active-screen"), "emoji", "Alt+2 не открыл эмодзи");
    const trapped = key("Tab");
    assert.equal(trapped.prevented, true, "Tab не удерживается в панели эмодзи");
    const escape = key("Escape");
    assert.equal(escape.prevented, true, "Escape не обработан");
    assert.equal(doc.body.getAttribute("data-active-screen"), "chat", "Escape не вернул в чат");
    key("4", { altKey: true });
    assert.equal(doc.body.getAttribute("data-active-screen"), "settings", "Alt+4 не открыл настройки");
    key("Escape");
    key("1", { altKey: true });
    assert.equal(doc.body.getAttribute("data-active-screen"), "chat", "Alt+1 не вернул чат");
  });

  await check("data-action: mute/deafen, emoji/emoji-close, settings/settings-close", () => {
    const action = (/** @type {string} */ name) => {
      const node = doc.querySelector(`[data-action="${name}"]`);
      assert.ok(node, `нет [data-action=${name}]`);
      return node;
    };
    const mute = action("mute");
    mute.click();
    assert.equal(mute.getAttribute("aria-pressed"), "true", "mute не переключил aria-pressed");
    mute.click();
    assert.equal(mute.getAttribute("aria-pressed"), "false", "mute не вернулся в false");
    action("emoji").click();
    assert.equal(doc.body.getAttribute("data-active-screen"), "emoji", "data-action=emoji не открыл пикер");
    action("emoji-close").click();
    assert.equal(doc.body.getAttribute("data-active-screen"), "chat", "emoji-close не вернул чат");
    action("settings").click();
    assert.equal(doc.body.getAttribute("data-active-screen"), "settings", "data-action=settings");
    action("settings-close").click();
    assert.equal(doc.body.getAttribute("data-active-screen"), "chat", "settings-close не вернул чат");
    action("all-members").click();
    assert.equal(doc.body.getAttribute("data-active-screen"), "members", "all-members не открыл экран");
    action("settings").click();
    action("settings-close").click();
  });

  await check("счётчики live (polite), FPS/p95 — не live", () => {
    assert.equal(byId("member-count").getAttribute("aria-live"), "polite", "member-count не counter-live");
    const metrics = doc.querySelector(".metrics");
    assert.ok(metrics, "нет .metrics");
    assert.equal(metrics.getAttribute("aria-live"), null, ".metrics (FPS) остался aria-live");
  });

  await check("штатная работа без console.error/warn", () => {
    assert.deepEqual(app.consoleErrors, [], "app.js дал console.error");
    assert.deepEqual(app.consoleWarns, [], "app.js дал console.warn");
  });

  console.log("\nfeatures: все проверки пройдены");
} finally {
  app.dispose();
}
