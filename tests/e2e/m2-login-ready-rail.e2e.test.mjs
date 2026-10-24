// @ts-check
/**
 * M2-01 (fixture browser): вход → READY → рейл серверов.
 *
 * Уровень: настоящий `rust/gui-spike/ui/index.html` в headless Chromium,
 * `__TAURI__` подменён детерминированным fake (`tests/helpers/fake-tauri.mjs`),
 * сети и живого Discord нет.
 *
 * Оракул (не реализация UI):
 * - `fixtures/gateway/by-type/002-READY.json` — 7 гильдий, их порядок и
 *   `properties.name` (по ним строится рейл);
 * - контракт auth: `auth_status` → `auth_login_token` → `ready`; до `ready`
 *   гейт входа видим, `.shell` скрыт от скринридеров (`aria-hidden`);
 * - токен не остаётся в DOM и не уходит ни в одну другую IPC-команду.
 *
 * Почему это важно: unit-тесты проверяют app.js в DOM-шиме, где статичные
 * плитки гильдий были заглушкой. Здесь разметка настоящая, поэтому проверяется
 * реальный порядок boot: рейл наполняется только после `ready()`.
 */
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { m2Commands, m2FixtureData, m2Guilds, openUiFixture } from "../helpers/ui-fixture.mjs";

describe("M2-01: login → READY → rail (реальный index.html)", () => {
  it("гейт → токен → ready: 7 гильдий из READY в порядке payload", async (t) => {
    const data = m2FixtureData();
    const guilds = m2Guilds();
    assert.equal(guilds.length, 7, "оракул 002-READY.json должен содержать 7 гильдий");

    const ui = await openUiFixture(t, {
      commands: m2Commands({
        auth_status: { ok: data.auth.noSession },
        auth_login_token: { ok: data.login.outcome },
      }),
    });

    // 1. До входа: гейт в состоянии no-session, оболочка скрыта от AT.
    await ui.waitFor(
      `(() => { const gate = document.querySelector('#auth-gate');`
        + ' return gate !== null && gate.getAttribute("data-state") === "no-session"; })()',
      { label: "auth: no-session" },
    );
    assert.equal(await ui.visible("#auth-gate"), true, "гейт входа не виден до ready");
    assert.equal(await ui.attr(".shell", "aria-hidden"), "true", ".shell открыт до входа");
    assert.equal(
      await ui.eval("document.querySelector('#auth-token').value"),
      "",
      "поле токена не пустое на старте",
    );
    assert.equal(await ui.callCount("ready"), 0, "ready() вызван до входа");

    // 2. Вход по токену: единственный путь, который передаёт токен в IPC.
    await ui.type("#auth-token", String(data.login.token));
    await ui.click("#auth-token-submit");
    await ui.waitFor(
      `(() => { const gate = document.querySelector('#auth-gate');`
        + ' return gate !== null && gate.getAttribute("data-state") === "ready"; })()',
      { label: "auth: ready" },
    );
    assert.equal(await ui.visible("#auth-gate"), false, "гейт остался видимым после ready");
    assert.equal(await ui.attr(".shell", "aria-hidden"), null, ".shell остался скрытым после ready");
    assert.equal(
      await ui.eval("document.querySelector('#auth-token').value"),
      "",
      "токен остался в DOM после входа",
    );

    // 3. Рейл: 7 плиток, порядок READY, первая выбрана, имя — из properties.name.
    await ui.waitFor(
      `(() => { const rail = document.querySelector('#guild-rail');`
        + ' return rail !== null && rail.getAttribute("data-state") === "ready"; })()',
      { label: "rail: ready" },
    );
    /** @type {Array<{ id: string | null, label: string | null, selected: boolean, current: string | null }>} */
    const tiles = await ui.eval(
      "[...document.querySelectorAll('#guild-rail button.guild-item')].map((node) => ({"
        + " id: node.getAttribute('data-guild-id'),"
        + " label: node.getAttribute('aria-label'),"
        + " selected: node.classList.contains('is-selected'),"
        + " current: node.getAttribute('aria-current'),"
        + " }))",
    );
    assert.equal(tiles.length, 7, `в рейле ${tiles.length} плиток вместо 7`);
    assert.deepEqual(
      tiles.map((tile) => tile.id),
      guilds.map((guild) => guild.id),
      "порядок/идентификаторы плиток не совпадают с READY",
    );
    assert.deepEqual(
      tiles.map((tile) => tile.label),
      guilds.map((guild) => `Сервер ${guild.name}`),
      "имена плиток не из properties.name",
    );
    assert.equal(tiles[0]?.selected, true, "первая гильдия READY не выбрана");
    assert.equal(tiles[0]?.current, "true");
    for (const tile of tiles.slice(1)) {
      assert.equal(tile.selected, false, `лишняя выбранная плитка ${tile.id}`);
      assert.equal(tile.current, null, `aria-current у невыбранной плитки ${tile.id}`);
    }

    // 4. Выбранная гильдия действительно запрашивает каналы, а ready() — после auth.
    await ui.waitForCall("channels", {
      check: (args) => assert.equal(args.guildId, guilds[0]?.id, "channels() не для выбранной гильдии"),
    });
    /** @type {string[]} */
    const journal = await ui.eval("window.__QA__.calls.map((call) => call.cmd)");
    const authAt = journal.indexOf("auth_login_token");
    const readyAt = journal.indexOf("ready");
    assert.ok(authAt >= 0, "auth_login_token не вызван");
    assert.ok(readyAt > authAt, `ready() до входа: auth@${authAt}, ready@${readyAt}`);

    // 5. Токен не протёк в другие команды (только auth_login_token его несёт).
    const withoutLogin = await ui.eval(
      "JSON.stringify(window.__QA__.calls.filter((call) => call.cmd !== 'auth_login_token'))",
    );
    assert.doesNotMatch(withoutLogin, /DO-NOT-LEAK/, "токен попал в чужой IPC-вызов");

    // 6. Страница не падала и не сыпала console.error при boot.
    assert.deepEqual(ui.pageExceptions(), [], "необработанные исключения страницы");
    assert.deepEqual(await ui.consoleErrors(), [], "console.error при входе/построении рейла");
  });
});
