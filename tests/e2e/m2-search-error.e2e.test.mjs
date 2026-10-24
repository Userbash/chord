// @ts-check
/**
 * M2-03 (fixture browser): ошибка серверного поиска — видимая, без fallback.
 *
 * Почему browser, а не только unit: старый unit был false-green — DOM-шим держал
 * статичную плитку `data-guild`, а в реальном `index.html` рейл до `ready()` пуст.
 * Здесь `search_remote` обязан получить **реальный** `guildId` выбранной плитки
 * (`data-guild-id`); иначе запрос вообще не уходит и сценарий краснеет.
 *
 * Оракул: таблица `SearchError kind → текст` из `features/search-overlay.js`
 * (`remoteErrorText`) и контракт B7a (`guildId`, `params.content`). Число вызовов
 * `search_remote` — ровно один на запрос: локальная выдача вместо ошибки запрещена.
 */
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { m2FixtureData, m2Guilds, openApp } from "../helpers/ui-fixture.mjs";

const data = m2FixtureData();
const QUERY = String(data.search.query);

/**
 * Вводит запрос и ждёт статус поиска, подходящий под `pattern`.
 * @param {import("../helpers/ui-fixture.mjs").UiFixture} ui
 * @param {string} query
 * @param {string} pattern regex-источник (как в data.search.errors)
 */
async function searchUntil(ui, query, pattern) {
  await ui.type("#chat-search", query);
  await ui.waitFor(
    `(() => { const node = document.querySelector('.search-status');`
      + ` if (!node) return false;`
      + ` return new RegExp(${JSON.stringify(pattern)}).test(node.textContent || ""); })()`,
    { label: `search status: ${pattern}` },
  );
}

describe("M2-03: search remote error → no fallback (реальный index.html)", () => {
  it("403: ошибка видима, локальных результатов нет, один вызов с реальным guildId", async (t) => {
    const guilds = m2Guilds();
    const forbidden = data.search.errors.find((/** @type {any} */ item) => item.name === "403");
    const ui = await openApp(t, { overrides: { search_remote: { err: forbidden.error } } });

    // Плитка выбрана до запроса: guildId берётся из DOM в момент поиска.
    assert.equal(
      await ui.attr(`#guild-rail button.guild-item[data-guild-id="${guilds[0]?.id}"]`, "data-guild-id"),
      guilds[0]?.id,
    );
    await searchUntil(ui, QUERY, forbidden.pattern);

    assert.equal(await ui.attr(".search-status", "data-state"), "error", "нет состояния ошибки");
    assert.equal(await ui.count(".search-result"), 0, "показана выдача вместо ошибки");
    assert.match(String(await ui.text(".search-overlay")), /Не удалось выполнить поиск/);
    assert.match(String(await ui.text(".content")), /Ошибка/, "ошибка не выведена на видимую поверхность");

    const calls = await ui.calls("search_remote");
    assert.equal(calls.length, 1, `search_remote вызван ${calls.length} раз(а) вместо одного`);
    assert.equal(calls[0]?.args?.guildId, guilds[0]?.id, "guildId не из выбранной плитки");
    assert.notEqual(calls[0]?.args?.guildId, "", "регрессия QA-M2-03: guildId пустой");
    assert.equal(calls[0]?.args?.params?.content, QUERY, "content не из запроса");
    assert.deepEqual(ui.pageExceptions(), [], "необработанные исключения страницы");
  });

  it("таблица kind: у каждой ошибки свой видимый текст, fallback нет", async (t) => {
    const ui = await openApp(t);
    /** @type {string[]} */
    const texts = [];
    for (const [index, item] of data.search.errors.entries()) {
      await ui.setCommand("search_remote", { err: item.error });
      const query = `${QUERY} ${index}`;
      await searchUntil(ui, query, item.pattern);
      assert.equal(
        await ui.attr(".search-status", "data-state"),
        "error",
        `${item.name}: нет data-state=error`,
      );
      assert.equal(await ui.count(".search-result"), 0, `${item.name}: показана локальная выдача`);
      texts.push(String(await ui.text(".search-status")));
      await ui.type("#chat-search", "");
    }
    assert.equal(new Set(texts).size, data.search.errors.length, "тексты ошибок схлопнулись");
    const calls = await ui.calls("search_remote");
    assert.equal(
      calls.length,
      data.search.errors.length,
      `на ${data.search.errors.length} запросов пришло ${calls.length} вызовов (fallback/повтор)`,
    );
    for (const call of calls) {
      assert.ok(String(call.args?.guildId ?? "") !== "", "search_remote вызван без guildId");
      assert.equal(typeof call.args?.params?.content, "string");
    }
    // Повторных запросов «на всякий случай» после ошибки нет.
    await new Promise((done) => setTimeout(done, 400));
    assert.equal(await ui.callCount("search_remote"), data.search.errors.length, "появился retry-loop");
  });
});
