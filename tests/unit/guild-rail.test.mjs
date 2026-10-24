// @ts-check
/**
 * G-01: рейл серверов из `ready()` (READY fixture) — список, выбор, клавиатура,
 * пустое/ошибочное/загрузочное состояние.
 *
 * Оракул — не текущая реализация, а реальный артефакт: `fixtures/gateway/by-type/002-READY.json`
 * (7 гильдий, имя в `guild.properties.name`) и контракт
 * `waves/RW-40/PARITY/G6-REPORT-RW40-02.md` §3.1:
 *   - источник списка только `ready()`, нового endpoint нет;
 *   - `button.guild-item[data-guild-id]`, `aria-label="Сервер <name>"`,
 *     `aria-current="true"` на выбранном, монограмма 2 буквы (иконки не склеиваем);
 *   - пустой список → empty-state, ошибка → `.error-state` + retry, загрузка → skeleton;
 *   - `ready()` вызывается только после `auth ready`.
 *
 * Шим не всплывает события, поэтому клик/клавиши адресуются самой плитке — как в
 * `features/actions.js` (прямые слушатели на `[data-action]`).
 */
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { loadApp } from "../support/gui-dom-harness.mjs";

/** @typedef {import("../support/gui-dom-harness.mjs").GuiDom} GuiDom */
/** @typedef {import("../support/gui-dom-harness.mjs").ShimElement} ShimElement */

const uiDir = fileURLToPath(new URL("../../rust/gui-spike/ui/", import.meta.url));
const html = readFileSync(`${uiDir}index.html`, "utf8");

/** READY fixture: `d.guilds[]`, имя — в `properties.name` (`data.rs parse_guild`). */
const FIXTURE = JSON.parse(
  readFileSync(
    fileURLToPath(new URL("../../fixtures/gateway/by-type/002-READY.json", import.meta.url)),
    "utf8",
  ),
);
/** @type {Array<{ id: string, name: string, icon: string | null }>} */
const GUILDS = (FIXTURE?.d?.guilds ?? []).map((/** @type {any} */ guild) => ({
  id: String(guild.id),
  name: String(guild?.properties?.name ?? ""),
  icon: typeof guild.icon === "string" ? guild.icon : null,
}));

/**
 * `ReadyPayload` в форме `data.rs` (camelCase). Поля гильдии, которые UI не
 * использует, всё равно отдаём: контракт `[id, name, icon, memberCount, channelCount]`.
 * @param {Array<{ id: string, name: string, icon: string | null }>} [guilds]
 */
function readyPayload(guilds = GUILDS) {
  return {
    user: {
      id: "42",
      username: "tester",
      globalName: "Тестер",
      avatar: null,
      email: null,
      bio: null,
      premium: false,
      mfaEnabled: false,
    },
    guilds: guilds.map((guild) => ({
      id: guild.id,
      name: guild.name,
      icon: guild.icon,
      memberCount: 3,
      channelCount: 2,
    })),
    guildCount: guilds.length,
    privateChannelCount: 0,
    relationshipCount: 0,
    readStateCount: 0,
    session: null,
  };
}

/**
 * Монограмма-оракул: инициалы первых двух слов (как «IS» у плитки-заглушки),
 * для одного слова — первые две буквы. Считается в тесте по имени, не по DOM.
 * @param {string} name
 * @returns {string}
 */
function expectedMonogram(name) {
  const words = name
    .split(/\s+/)
    .map((word) => word.replace(/^[^\p{L}]+/u, ""))
    .filter((word) => /^\p{L}/u.test(word));
  if (words.length >= 2) return `${[...(words[0] ?? "")][0] ?? ""}${[...(words[1] ?? "")][0] ?? ""}`.toUpperCase();
  return [...(words[0] ?? "")].slice(0, 2).join("").toUpperCase();
}

/**
 * `app.js` в шиме: вход уже пройден (`auth_status` → ready), ответы `ready()`
 * задаёт тест — так проверяется именно рейл, без окна и сети.
 * @param {(cmd: string, args: any) => any} handler
 * @returns {Promise<GuiDom>}
 */
function loadRailApp(handler) {
  return loadApp({
    invokeHandler: (cmd, args) => {
      if (cmd === "auth_status") {
        return { status: "ready", stored: true, user: { id: "42", username: "tester" } };
      }
      return handler(cmd, args);
    },
  });
}

/**
 * @param {GuiDom} app
 * @returns {import("../../rust/gui-spike/ui/app.js").App}
 */
function api(app) {
  const instance = /** @type {any} */ (app.appModule)?.currentApp?.();
  assert.ok(instance, "app.js не экспортирует currentApp()");
  return instance;
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
 * @returns {ShimElement[]}
 */
function tiles(app) {
  return app.document.querySelectorAll("button.guild-item");
}

describe("G-01: рейл серверов из ready()", () => {
  it("7 гильдий из READY fixture: data-guild-id, aria-label, aria-current, монограмма", async () => {
    const app = await loadRailApp((cmd) => (cmd === "ready" ? readyPayload() : undefined));
    try {
      const rail = firstNode(app, "#guild-rail");
      assert.equal(rail.getAttribute("data-state"), "ready", "рейл не перешёл в ready");
      assert.equal(app.callsOf("ready").length, 1, "ready() вызван не один раз");
      const navigation = firstNode(app, '#guild-rail [role="navigation"]');
      assert.equal(navigation.getAttribute("aria-label"), "Серверы", "нет навигации по серверам в рейле");

      assert.equal(GUILDS.length, 7, "фикстура READY изменилась: не 7 гильдий");
      const nodes = tiles(app);
      assert.equal(nodes.length, GUILDS.length, "в рейле не 7 плиток");

      for (const [index, guild] of GUILDS.entries()) {
        const tile = nodes[index];
        assert.ok(tile, `нет плитки #${index}`);
        assert.equal(tile.getAttribute("data-guild-id"), guild.id, "data-guild-id не snowflake из READY");
        assert.equal(tile.getAttribute("aria-label"), `Сервер ${guild.name}`, "aria-label не «Сервер <name>»");
        assert.equal(tile.textContent, expectedMonogram(guild.name), "монограмма не из имени");
        assert.equal(tile.textContent.length, 2, "монограмма не 2 буквы");
        assert.equal(tile.getAttribute("aria-current"), index === 0 ? "true" : null, "aria-current не только у выбранного");
        assert.ok(tile.classList.contains("is-selected") === (index === 0), "is-selected не у первого");
      }
      // Имена не выдуманы: в DOM только те, что в payload.
      assert.deepEqual(nodes.map((node) => node.getAttribute("aria-label")), GUILDS.map((guild) => `Сервер ${guild.name}`));
      // Иконка — хеш CDN, URL не склеиваем: картинок в рейле нет.
      assert.equal(rail.querySelectorAll("img").length, 0, "рейл склеил URL иконки");
      assert.equal(rail.querySelectorAll("[src]").length, 0, "в рейле появился src (неподтверждённый URL)");
    } finally {
      app.dispose();
    }
  });

  it("нет статичной заглушки data-guild=\"1\": ни в index.html, ни в готовом рейле", async () => {
    assert.equal(/data-guild="\d+"/.test(html), false, "в index.html осталась статичная плитка data-guild=\"N\"");
    const app = await loadRailApp((cmd) => (cmd === "ready" ? readyPayload() : undefined));
    try {
      assert.equal(app.document.querySelectorAll('[data-guild="1"]').length, 0, "в рейле живёт заглушка data-guild=\"1\"");
      assert.equal(app.document.querySelectorAll('[data-guild="2"]').length, 0, "в рейле живёт заглушка data-guild=\"2\"");
      assert.equal(app.document.querySelectorAll("[data-guild]").length, 0, "рейл держит legacy data-guild");
    } finally {
      app.dispose();
    }
  });

  it("клик меняет выбранный сервер (aria-current, is-selected, заголовок дерева)", async () => {
    const app = await loadRailApp((cmd) => (cmd === "ready" ? readyPayload() : undefined));
    try {
      const nodes = tiles(app);
      const target = nodes[4];
      assert.ok(target, "нет пятой плитки");
      target.click();

      assert.equal(target.getAttribute("aria-current"), "true", "клик не сделал плитку текущей");
      assert.equal(target.classList.contains("is-selected"), true, "клик не выделил плитку");
      assert.equal(tiles(app).filter((node) => node.getAttribute("aria-current") === "true").length, 1, "выбранных плиток не одна");
      assert.equal(api(app).rail.selectedId, GUILDS[4]?.id, "selectedId не совпал с кликом");
      assert.equal(
        firstNode(app, ".tree-guild").textContent,
        GUILDS[4]?.name,
        "заголовок дерева не следует за выбранным сервером",
      );
    } finally {
      app.dispose();
    }
  });

  it("клавиатура ↑/↓/Home/End двигает выбор и фокус", async () => {
    const app = await loadRailApp((cmd) => (cmd === "ready" ? readyPayload() : undefined));
    try {
      const nodes = tiles(app);
      const first = nodes[0];
      const last = nodes[nodes.length - 1];
      assert.ok(first && last, "нет плиток для клавиатуры");
      first.focus();

      const down = first.dispatchEvent("keydown", { key: "ArrowDown" });
      assert.equal(down.prevented, true, "ArrowDown не перехвачен");
      assert.equal(api(app).rail.selectedId, GUILDS[1]?.id, "ArrowDown не перешёл на следующую гильдию");
      assert.equal(app.document.activeElement, nodes[1], "фокус не остался в рейле");

      first.dispatchEvent("keydown", { key: "End" });
      assert.equal(api(app).rail.selectedId, GUILDS[6]?.id, "End не выбрал последнюю гильдию");

      last.dispatchEvent("keydown", { key: "Home" });
      assert.equal(api(app).rail.selectedId, GUILDS[0]?.id, "Home не выбрал первую гильдию");

      first.dispatchEvent("keydown", { key: "ArrowUp" });
      assert.equal(api(app).rail.selectedId, GUILDS[6]?.id, "ArrowUp с первой плитки не завернулся на последнюю");
    } finally {
      app.dispose();
    }
  });

  it("0 гильдий → empty-state, а не пустой рейл", async () => {
    const app = await loadRailApp((cmd) => (cmd === "ready" ? readyPayload([]) : undefined));
    try {
      const rail = firstNode(app, "#guild-rail");
      assert.equal(rail.getAttribute("data-state"), "empty", "рейл не в состоянии empty");
      assert.equal(tiles(app).length, 0, "empty-state оставил плитки");
      const empty = firstNode(app, ".guild-empty");
      assert.match(empty.textContent, /нет сервер/i, "empty-state без текста");
      assert.equal(rail.getAttribute("aria-busy"), null, "empty-state остался busy");
    } finally {
      app.dispose();
    }
  });

  it("ошибка ready() → .error-state + retry, повторный вызов отдаёт список", async () => {
    let calls = 0;
    const app = await loadRailApp((cmd) => {
      if (cmd !== "ready") return undefined;
      calls += 1;
      if (calls === 1) throw new Error("READY fixture недоступна");
      return readyPayload();
    });
    try {
      const rail = firstNode(app, "#guild-rail");
      assert.equal(rail.getAttribute("data-state"), "error", "рейл не в состоянии error");
      const state = firstNode(app, ".error-state");
      assert.equal(state.getAttribute("role"), "alert", "ошибка не объявлена AT");
      assert.equal(state.getAttribute("data-error-code"), "IPC_FAILED", "код ошибки не виден");
      assert.equal(tiles(app).length, 0, "при ошибке показаны выдуманные плитки");
      // Видимое состояние — последствие ошибки; шум в console.error не нужен.
      assert.deepEqual(app.consoleErrors, [], "ошибка ready() ушла в console.error");

      const retry = firstNode(app, ".error-state-retry");
      assert.equal(retry.textContent, "Повторить", "у retry нет подписи");
      retry.click();
      await app.settle(5);

      assert.equal(app.callsOf("ready").length, 2, "retry не повторил ready()");
      assert.equal(rail.getAttribute("data-state"), "ready", "после retry рейл не ready");
      assert.equal(app.document.querySelectorAll(".error-state").length, 0, "error-state не снят после успеха");
      assert.equal(tiles(app).length, GUILDS.length, "после retry нет 7 плиток");
    } finally {
      app.dispose();
    }
  });

  it("loading skeleton до ответа ready(), затем плитки", async () => {
    /** @type {(value: any) => void} */
    let resolveReady = () => {};
    const pending = new Promise((resolve) => { resolveReady = resolve; });
    const app = await loadRailApp((cmd) => (cmd === "ready" ? pending : undefined));
    try {
      const rail = firstNode(app, "#guild-rail");
      assert.equal(rail.getAttribute("data-state"), "loading", "нет loading-состояния");
      assert.equal(rail.getAttribute("aria-busy"), "true", "skeleton не помечен aria-busy");
      assert.ok(rail.querySelectorAll(".guild-skeleton").length > 0, "нет skeleton-плиток");
      assert.equal(tiles(app).length, 0, "skeleton показан как настоящие гильдии");

      resolveReady(readyPayload());
      await app.settle(5);

      assert.equal(rail.querySelectorAll(".guild-skeleton").length, 0, "skeleton не убран");
      assert.equal(rail.getAttribute("aria-busy"), null, "aria-busy не снят");
      assert.equal(tiles(app).length, GUILDS.length, "после загрузки нет 7 плиток");
    } finally {
      app.dispose();
    }
  });

  it("до auth ready ready() не вызывается (источник — только вход)", async () => {
    const app = await loadApp({
      invokeHandler: (cmd) => {
        if (cmd === "auth_status") return { status: "no-session", stored: false };
        return undefined;
      },
    });
    try {
      assert.equal(app.callsOf("ready").length, 0, "ready() дёрнули до входа");
    } finally {
      app.dispose();
    }
  });
});
