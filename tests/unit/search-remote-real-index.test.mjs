// @ts-check
/**
 * QA-M2-03 / P1A: regression удалённого поиска на реальном `ui/index.html`.
 *
 * Старый false-green: `app.js` снимал `dataset.guild` на boot, а рейл G-01
 * строит плитки с `data-guild-id`; в реальном продукте рейл до `ready()` пуст,
 * поэтому `guildId === ""` и `search_remote` не вызывался. DOM-шим это
 * маскировал: в нём были статичные плитки с `data-guild`.
 *
 * Здесь шим-заглушки удаляются до импорта `app.js` — boot видит пустой рейл,
 * как настоящий `index.html` (в нём нет ни одного `data-guild`). Гильдии и их
 * snowflake берутся из независимого оракула `fixtures/gateway/by-type/002-READY.json`.
 * На старом коде тест обязан падать: `search_remote` не будет вызван вовсе.
 */
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath, pathToFileURL } from "node:url";
import { GuiDom } from "../support/gui-dom-harness.mjs";

/** @typedef {import("../support/gui-dom-harness.mjs").ShimElement} ShimElement */

const APP_PATH = fileURLToPath(new URL("../../rust/gui-spike/ui/app.js", import.meta.url));
const HTML = readFileSync(
  fileURLToPath(new URL("../../rust/gui-spike/ui/index.html", import.meta.url)),
  "utf8",
);
const FIXTURE = JSON.parse(
  readFileSync(
    fileURLToPath(new URL("../../fixtures/gateway/by-type/002-READY.json", import.meta.url)),
    "utf8",
  ),
);

/** READY fixture → гильдии в форме `data.rs` (id — snowflake, имя — properties.name). */
const GUILDS = (FIXTURE?.d?.guilds ?? []).map((/** @type {any} */ guild) => ({
  id: String(guild.id),
  name: String(guild?.properties?.name ?? ""),
}));

/** @returns {any} */
function readyPayload() {
  return {
    user: { id: "42", username: "tester" },
    guilds: GUILDS.map((/** @type {{ id: string, name: string }} */ guild) => ({
      id: guild.id,
      name: guild.name,
      icon: null,
      memberCount: 3,
      channelCount: 2,
    })),
    guildCount: GUILDS.length,
    privateChannelCount: 0,
    relationshipCount: 0,
    readStateCount: 0,
    session: null,
  };
}

/** @param {string} id @param {string} content @returns {any} */
function serverMessage(id, content) {
  return {
    id,
    content,
    timestamp: "2026-10-20T10:05:00Z",
    channel_id: "1163408591315812392",
    author: { id: "42", username: "network_alpha", bot: false },
    pinned: false,
  };
}

let importSeq = 0;

/**
 * Boot настоящего `app.js` без статичных плиток шима: вход уже пройден
 * (`auth_status` → ready), рейл наполняется только `ready()`.
 * @param {(cmd: string, args: any) => any} handler
 * @returns {Promise<GuiDom>}
 */
async function bootWithRealRail(handler) {
  const env = new GuiDom({
    appPath: APP_PATH,
    invokeHandler: (cmd, args) => {
      if (cmd === "auth_status") {
        return { status: "ready", stored: true, user: { id: "42", username: "tester" } };
      }
      if (cmd === "ready") return readyPayload();
      if (cmd === "channels") return [];
      return handler(cmd, args);
    },
  });
  // В `ui/index.html` нет статичных плиток; убираем и заглушки шима, чтобы
  // boot видел ровно то, что видит продукт — пустой рейл до `ready()`.
  for (const stub of env.document.querySelectorAll(".guild-item")) stub.remove();
  env.installGlobals();
  env.appModule = await import(`${pathToFileURL(APP_PATH).href}?real-rail=${++importSeq}`);
  await env.settle(30);
  return env;
}

/**
 * @param {GuiDom} env
 * @returns {import("../../rust/gui-spike/ui/app.js").App}
 */
function api(env) {
  const instance = /** @type {any} */ (env.appModule)?.currentApp?.();
  assert.ok(instance, "app.js не экспортирует currentApp()");
  return instance;
}

/**
 * @param {GuiDom} env
 * @param {string} query
 * @returns {Promise<ShimElement>}
 */
async function searchRemote(env, query) {
  const header = /** @type {ShimElement | null} */ (env.document.getElementById("chat-search"));
  assert.ok(header, "нет #chat-search");
  header.value = query;
  header.emit("input");
  await api(env).search.whenIdle();
  const panel = /** @type {ShimElement | null} */ (env.document.querySelector(".search-overlay"));
  assert.ok(panel, "нет .search-overlay");
  return panel;
}

describe("QA-M2-03: search_remote на реальном index.html (data-guild-id)", () => {
  it("после ready шлёт ровно один search_remote с guildId из data-guild-id", async () => {
    assert.doesNotMatch(HTML, /data-guild="\d+"/, "в index.html осталась статичная плитка data-guild");
    assert.equal(GUILDS.length, 7, "READY fixture изменилась: не 7 гильдий");
    const first = GUILDS[0];
    assert.ok(first && first.id !== "" && first.name !== "", "в READY нет первой гильдии");

    /** @type {any[]} */
    const calls = [];
    const env = await bootWithRealRail((cmd, args) => {
      if (cmd !== "search_remote") return undefined;
      calls.push(args);
      return {
        messages: [serverMessage("9001", "привет из сети")],
        total_results: 1,
        doing_deep_historical_index: false,
      };
    });
    try {
      const tiles = env.document.querySelectorAll(".guild-item");
      assert.equal(tiles.length, GUILDS.length, "рейл после ready не из payload");
      assert.equal(tiles[0]?.getAttribute("data-guild-id"), first.id, "плитка не из data-guild-id");
      assert.equal(tiles[0]?.getAttribute("data-guild"), null, "плитка несёт legacy data-guild");
      assert.equal(
        env.document.querySelectorAll("[data-guild]").length,
        0,
        "в DOM есть legacy data-guild (оракул подменён шимом)",
      );

      const panel = await searchRemote(env, "привет");
      await env.settle(2);

      assert.equal(calls.length, 1, `search_remote вызван ${calls.length} раз(а), нужен ровно один`);
      assert.equal(calls[0]?.guildId, first.id, "guildId не из выбранной плитки data-guild-id");
      assert.equal(calls[0]?.params?.content, "привет", "content не из запроса");
      assert.equal(panel.querySelectorAll(".search-result").length, 1, "серверная выдача не отрисована");
      assert.equal(panel.querySelector(".search-status")?.getAttribute("data-state"), "ok");
      assert.equal(api(env).search.mode, "remote", "режим не переключился на сервер");
    } finally {
      env.dispose();
    }
  });

  it("unauthorized/forbidden/notFound — видимая ошибка без локального fallback", async () => {
    for (const kind of ["unauthorized", "forbidden", "notFound"]) {
      /** @type {any[]} */
      const calls = [];
      const env = await bootWithRealRail((cmd, args) => {
        if (cmd !== "search_remote") return undefined;
        calls.push(args);
        throw { kind };
      });
      try {
        const panel = await searchRemote(env, "привет");
        const status = /** @type {ShimElement | null} */ (panel.querySelector(".search-status"));
        assert.ok(status, `${kind}: нет .search-status`);
        assert.equal(calls.length, 1, `${kind}: search_remote вызван ${calls.length} раз(а)`);
        assert.equal(calls[0]?.guildId, GUILDS[0]?.id, `${kind}: guildId не из data-guild-id`);
        assert.equal(status.getAttribute("data-state"), "error", `${kind}: ошибка не видна`);
        assert.equal(panel.querySelectorAll(".search-result").length, 0, `${kind}: локальный fallback`);
        assert.equal(api(env).search.matchesCount, 0, `${kind}: сработал тихий локальный fallback`);
        assert.equal(api(env).search.mode, "remote", `${kind}: режим откатился в локальный`);
        assert.doesNotMatch(status.textContent ?? "", /загруженных/, `${kind}: остался оконный счётчик`);
      } finally {
        env.dispose();
      }
    }
  });
});
