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
 * `UiSettings` в форме `settings_store.rs` (G-01b): `lastGuildId` — camelCase и
 * опционален. Остальные поля — валидные значения вне дефолтов, чтобы сохранение
 * с рейла нельзя было сделать «пустой заготовкой» и потерять настройки (форма
 * строгая: `deny_unknown_fields`, без полей Rust вернёт ошибку разбора).
 * @param {string | null} [lastGuildId] `null` — ключа в файле нет (не «выбора не было»).
 */
function settingsPayload(lastGuildId = null) {
  return {
    version: 1,
    toggles: { "voice.qos_priority": true },
    density: "compact",
    theme: "light",
    locale: "en",
    ...(lastGuildId === null ? {} : { lastGuildId }),
  };
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
  return /** @type {ShimElement} */ (node);
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

describe("G-01b: последний сервер из settings_load восстанавливается и сохраняется", () => {
  /**
   * Boot с сохранёнными настройками: `settings_load` отдаёт `UiSettings`
   * (G-01b ACCEPT), `ready()` — READY fixture.
   * @param {Record<string, any>} settings
   * @returns {Promise<GuiDom>}
   */
  function bootWithSettings(settings) {
    return loadApp({
      invokeHandler: (cmd) => {
        if (cmd === "auth_status") {
          return { status: "ready", stored: true, user: { id: "42", username: "tester" } };
        }
        if (cmd === "settings_load") return settings;
        if (cmd === "ready") return readyPayload();
        return undefined;
      },
    });
  }

  /**
   * @param {GuiDom} app
   * @param {string} id
   * @returns {ShimElement[]}
   */
  function markedCurrent(app, id) {
    return tiles(app).filter(
      (node) => node.getAttribute("data-guild-id") === id && node.getAttribute("aria-current") === "true",
    );
  }

  it("boot: выбирается гильдия с id == lastGuildId из settings_load", async () => {
    const stored = GUILDS[3]?.id ?? "";
    assert.notEqual(stored, "", "фикстура READY без четвёртой гильдии");
    const app = await bootWithSettings(settingsPayload(stored));
    try {
      assert.ok(app.callsOf("settings_load").length > 0, "settings_load не прочитан на boot");
      assert.equal(api(app).rail.selectedId, stored, "выбран не последний сервер из настроек");
      assert.equal(markedCurrent(app, stored).length, 1, "у восстановленной гильдии нет aria-current");
      assert.equal(
        tiles(app).filter((node) => node.getAttribute("aria-current") === "true").length,
        1,
        "выбранных плиток не одна",
      );
      assert.equal(
        firstNode(app, ".tree-guild").textContent,
        GUILDS[3]?.name,
        "заголовок дерева не следует за восстановленной гильдией",
      );
      assert.deepEqual(
        app.callsOf("channels").map((call) => call.args?.guildId),
        [stored],
        "каналы запрошены не для восстановленной гильдии",
      );
      assert.equal(app.callsOf("settings_save").length, 0, "восстановление выбора пишет настройки на boot");
    } finally {
      app.dispose();
    }
  });

  it("boot: lastGuildId вне READY или без поля → первая гильдия", async () => {
    const missing = await bootWithSettings(settingsPayload("999999999999999999"));
    try {
      assert.equal(api(missing).rail.selectedId, GUILDS[0]?.id, "чужой lastGuildId не откатился на первую гильдию");
      assert.equal(markedCurrent(missing, GUILDS[0]?.id ?? "").length, 1, "первая гильдия не помечена текущей");
      assert.equal(missing.callsOf("channels")[0]?.args?.guildId, GUILDS[0]?.id, "каналы не первой гильдии");
    } finally {
      missing.dispose();
    }
    const absent = await bootWithSettings(settingsPayload());
    try {
      assert.equal(api(absent).rail.selectedId, GUILDS[0]?.id, "без lastGuildId выбрана не первая гильдия");
    } finally {
      absent.dispose();
    }
  });

  it("клик по плитке сохраняет lastGuildId, не теряя остальные настройки", async () => {
    const app = await bootWithSettings(settingsPayload());
    const target = GUILDS[2]?.id ?? "";
    try {
      assert.equal(app.callsOf("settings_save").length, 0, "settings_save вызван до клика");
      const tile = tiles(app)[2];
      assert.ok(tile, "нет третьей плитки");
      tile.click();
      await app.settle(6);

      const saves = app.callsOf("settings_save");
      assert.equal(saves.length, 1, "клик по гильдии не дал ровно один settings_save");
      /** @type {Record<string, any>} */
      const saved = saves[0]?.args?.settings ?? {};
      assert.equal(saved.lastGuildId, target, "lastGuildId не обновлён кликом");
      // Сохранение идёт целым `UiSettings`, а не заготовкой: поля файла не теряются.
      assert.equal(saved.version, 1, "version потерян при сохранении с рейла");
      assert.deepEqual(saved.toggles, { "voice.qos_priority": true }, "тумблеры потеряны при сохранении с рейла");
      assert.equal(saved.density, "compact", "density потерян при сохранении с рейла");
      assert.equal(saved.theme, "light", "theme потерян при сохранении с рейла");
      assert.equal(saved.locale, "en", "locale потерян при сохранении с рейла");
    } finally {
      app.dispose();
    }
  });

  it("выбор гильдии -> переключение настройки -> settings_save сохраняет lastGuildId -> повторный boot выбирает её", async () => {
    let currentStore = settingsPayload();
    /** @returns {Promise<GuiDom>} */
    function bootInstance() {
      return loadApp({
        invokeHandler: (cmd, args) => {
          if (cmd === "auth_status") {
            return { status: "ready", stored: true, user: { id: "42", username: "tester" } };
          }
          if (cmd === "settings_load") return currentStore;
          if (cmd === "settings_save") {
            currentStore = args?.settings;
            return undefined;
          }
          if (cmd === "ready") return readyPayload();
          return undefined;
        },
      });
    }

    const app1 = await bootInstance();
    const target = GUILDS[2]?.id ?? "";
    assert.notEqual(target, "", "фикстура READY без третьей гильдии");

    try {
      const tile = tiles(app1)[2];
      assert.ok(tile, "нет третьей плитки");
      tile.click();
      await app1.settle(6);

      assert.equal(currentStore.lastGuildId, target, "клик по гильдии не сохранил target lastGuildId");

      // Переключаем тумблер в настройках
      const toggle = firstNode(app1, 'input[data-setting-id="voice.qos_priority"]');
      toggle.click();
      toggle.emit("change");
      app1.flushTimers();
      await app1.settle(2);

      const saves = app1.callsOf("settings_save");
      assert.ok(saves.length >= 2, "нет settings_save от изменения настроек");
      const lastSave = saves[saves.length - 1]?.args?.settings ?? {};
      assert.equal(lastSave.lastGuildId, target, "переключение тумблера стёрло lastGuildId");
      assert.equal(currentStore.lastGuildId, target, "хранилище настроек потеряло lastGuildId после toggle");
    } finally {
      app1.dispose();
    }

    // Повторный boot выбирает сохранённую гильдию
    const app2 = await bootInstance();
    try {
      assert.equal(api(app2).rail.selectedId, target, "повторный boot открыл не сохранённую гильдию");
      assert.equal(markedCurrent(app2, target).length, 1, "у сохранённой гильдии нет aria-current после повторного boot");
    } finally {
      app2.dispose();
    }
  });

  it("быстрые клики серверов A -> B с задержкой первого settings_load/settings_save гарантированно сохраняют последний выбор B", async () => {
    let currentStore = settingsPayload();
    /** @type {(val?: any) => void} */
    let resolveFirstLoad = () => {};
    const firstLoadDeferred = new Promise((resolve) => {
      resolveFirstLoad = resolve;
    });
    let loadCount = 0;

    const app = await loadApp({
      invokeHandler: (cmd, args) => {
        if (cmd === "auth_status") {
          return { status: "ready", stored: true, user: { id: "42", username: "tester" } };
        }
        if (cmd === "ready") return readyPayload();
        if (cmd === "settings_load") {
          loadCount++;
          // Первый вызов settings_load — на boot приложения.
          // Второй вызов settings_load — при клике на сервер A.
          if (loadCount === 2) {
            return firstLoadDeferred.then(() => currentStore);
          }
          return currentStore;
        }
        if (cmd === "settings_save") {
          currentStore = args?.settings;
          return undefined;
        }
        return undefined;
      },
    });

    try {
      const allTiles = tiles(app);
      const targetA = GUILDS[1]?.id ?? "";
      const targetB = GUILDS[2]?.id ?? "";
      assert.ok(targetA && targetB, "нет гильдий A и B");
      const tileA = allTiles[1];
      const tileB = allTiles[2];
      assert.ok(tileA && tileB, "нет плиток A и B");

      // Клик по плитке A — её settings_load задерживается в firstLoadDeferred
      tileA.click();

      // Быстрый клик по плитке B до завершения загрузки A
      tileB.click();
      await app.settle(6);

      // Разрешаем отложенный settings_load первого клика
      resolveFirstLoad();
      await app.settle(6);

      // В хранилище настроек должен сохраниться последний выбор B, а не устаревший A
      assert.equal(
        currentStore.lastGuildId,
        targetB,
        `гонка при быстрых кликах A -> B: сохранён ${currentStore.lastGuildId}, ожидался последний выбор ${targetB}`
      );
    } finally {
      app.dispose();
    }
  });

  it("быстрые клики серверов A -> B с задержкой завершения settings_save от A сохраняют итоговый выбор B", async () => {
    let currentStore = settingsPayload();
    /** @type {(val?: any) => void} */
    let resolveFirstSave = () => {};
    const firstSaveDeferred = new Promise((resolve) => {
      resolveFirstSave = resolve;
    });
    let saveCount = 0;
    /** @type {string[]} */
    const saveEvents = [];

    const app = await loadApp({
      invokeHandler: (cmd, args) => {
        if (cmd === "auth_status") {
          return { status: "ready", stored: true, user: { id: "42", username: "tester" } };
        }
        if (cmd === "ready") return readyPayload();
        if (cmd === "settings_load") {
          return currentStore;
        }
        if (cmd === "settings_save") {
          saveCount++;
          const targetId = args?.settings?.lastGuildId;
          saveEvents.push(`start:${targetId}`);
          if (saveCount === 1) {
            return firstSaveDeferred.then(() => {
              currentStore = args?.settings;
              saveEvents.push(`finish:${targetId}`);
            });
          }
          currentStore = args?.settings;
          saveEvents.push(`finish:${targetId}`);
          return undefined;
        }
        return undefined;
      },
    });

    try {
      const allTiles = tiles(app);
      const targetA = GUILDS[1]?.id ?? "";
      const targetB = GUILDS[2]?.id ?? "";
      assert.ok(targetA && targetB, "нет гильдий A и B");
      const tileA = allTiles[1];
      const tileB = allTiles[2];
      assert.ok(tileA && tileB, "нет плиток A и B");

      // 1. Клик по плитке A — settings_load выполняется, settings_save от A начинается и ждёт firstSaveDeferred
      tileA.click();
      await app.settle(6);
      assert.equal(saveCount, 1, "settings_save для A не начался");
      assert.deepEqual(saveEvents, [`start:${targetA}`], "A должен только начать сохранение");

      // 2. Клик по плитке B до завершения сохранения A
      tileB.click();
      await app.settle(6);

      // 3. Разрешаем отложенное завершение settings_save от A
      resolveFirstSave();
      await app.settle(6);

      // Итоговый store должен содержать выбор B, а устаревший save от A не должен завершаться после B
      assert.equal(
        currentStore.lastGuildId,
        targetB,
        `гонка при отложенном settings_save от A: сохранён ${currentStore.lastGuildId}, ожидался последний выбор ${targetB}`
      );
      assert.deepEqual(
        saveEvents,
        [`start:${targetA}`, `finish:${targetA}`, `start:${targetB}`, `finish:${targetB}`],
        `нарушен порядок очереди сохранения: ${saveEvents.join(" -> ")}`
      );
    } finally {
      app.dispose();
    }
  });

  it("STUB-10 / STUB-05: currentApp() экспортирует рабочие контроллеры pinsThreads и inviteModal", async () => {
    const app = await bootWithSettings(settingsPayload());
    try {
      const current = api(app);
      assert.ok(current.pinsThreads, "pinsThreads не экспортирован в currentApp()");
      assert.equal(typeof current.pinsThreads.openPins, "function", "pinsThreads.openPins не функция");
      assert.equal(typeof current.pinsThreads.openThreads, "function", "pinsThreads.openThreads не функция");
      assert.ok(current.inviteModal, "inviteModal не экспортирован в currentApp()");
      assert.equal(typeof current.inviteModal.open, "function", "inviteModal.open не функция");
    } finally {
      app.dispose();
    }
  });

  it("PINS-THREADS-SPEC §1–§3: server-menu.css содержит поповер-правила .header-dropdown-panel, .pins-panel, .threads-panel", () => {
    const cssText = readFileSync(fileURLToPath(new URL("../../rust/gui-spike/ui/components/server-menu.css", import.meta.url)), "utf8");
    assert.match(cssText, /\.header-dropdown-panel\s*\{[^}]*position:\s*absolute/, "нет position: absolute");
    assert.match(cssText, /\.header-dropdown-panel\s*\{[^}]*z-index:\s*var\(--z-popover\)/, "нет z-index: var(--z-popover)");
    assert.match(cssText, /\.pins-panel\s*\{[^}]*width:\s*360px/, "нет ширины pins 360px");
    assert.match(cssText, /\.threads-panel\s*\{[^}]*width:\s*380px/, "нет ширины threads 380px");
  });
});
