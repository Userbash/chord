// @ts-check
/**
 * M2-06 (fixture browser): видимый reconnect/offline и ручной retry.
 *
 * Оракул: `NET_STATUSES` (`offline|connecting|ready|reconnecting|error`) и
 * `NET_TEXT` из `app.js`; контракт `net_connect` — **ручной** (ровно один вызов
 * на клик), без авто-retry-loop; 401 даёт «Сессия истекла. Войдите снова.».
 *
 * Polling статуса в продукте — 2 с, поэтому ожидания здесь до 8 с: это проверка
 * реального таймера браузера, а не ускоренного шима.
 */
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { m2FixtureData, openApp } from "../helpers/ui-fixture.mjs";

const data = m2FixtureData();
const POLL_TIMEOUT = 8000;

describe("M2-06: reconnect/offline banner → retry (реальный index.html)", () => {
  it("ready скрыт → reconnecting/offline видимы → retry один, 401 видим, цикла нет", async (t) => {
    const ui = await openApp(t);

    // 1. До разрыва баннер скрыт, авто-connect не вызывался.
    assert.equal(await ui.visible("#net-status"), false, "баннер виден при ready");
    assert.equal(await ui.attr("#net-status", "data-state"), "ready");
    assert.equal(await ui.callCount("net_status") >= 1, true, "нет первичного net_status после ready");
    assert.equal(
      (await ui.calls("net_status")).every((call) => call.args === null),
      true,
      "net_status вызван с аргументами (токен/лишние поля)",
    );
    assert.equal(await ui.callCount("net_connect"), 0, "авто-connect без действия пользователя");

    // 2. reconnecting → «Переподключение…».
    await ui.setCommand("net_status", { ok: "reconnecting" });
    await ui.waitFor("document.querySelector('#net-status').getAttribute('data-state') === 'reconnecting'", {
      timeoutMs: POLL_TIMEOUT,
      label: "net-status=reconnecting",
    });
    assert.equal(await ui.visible("#net-status"), true, "баннер не виден при reconnecting");
    assert.match(String(await ui.text("#net-status")), new RegExp(String(data.net.reconnectingText)));
    assert.equal(await ui.visible("#net-retry"), false, "retry виден при reconnecting");

    // 3. offline → «Нет соединения» + видимая кнопка «Повторить».
    await ui.setCommand("net_status", { ok: "offline" });
    await ui.waitFor("document.querySelector('#net-status').getAttribute('data-state') === 'offline'", {
      timeoutMs: POLL_TIMEOUT,
      label: "net-status=offline",
    });
    assert.match(String(await ui.text("#net-status")), new RegExp(String(data.net.offlineText)));
    assert.equal(await ui.visible("#net-retry"), true, "нет кнопки «Повторить» при offline");

    // 4. Успешный retry: ровно один net_connect, баннер скрывается.
    await ui.setCommand("net_connect", { ok: "ready" });
    await ui.click("#net-retry");
    await ui.waitFor("document.querySelector('#net-status').hasAttribute('hidden')", {
      timeoutMs: POLL_TIMEOUT,
      label: "баннер скрыт после retry",
    });
    assert.equal(await ui.callCount("net_connect"), 1, "retry отправил не один net_connect");
    assert.equal((await ui.calls("net_connect"))[0]?.args, null, "net_connect вызван с аргументами");
    // Дальнейший polling не возвращает offline: статус снова ready.
    await ui.setCommand("net_status", { ok: "ready" });

    // 5. 401 при connect: видимый текст «Сессия истекла», без авто-цикла.
    await ui.setCommand("net_status", { ok: "offline" });
    await ui.waitFor("document.querySelector('#net-status').getAttribute('data-state') === 'offline'", {
      timeoutMs: POLL_TIMEOUT,
      label: "net-status=offline (второй раз)",
    });
    await ui.setCommand("net_connect", { err: data.net.unauthorized });
    await ui.click("#net-retry");
    await ui.waitFor(
      "(() => { const node = document.querySelector('#net-status');"
        + " return node !== null && (node.textContent || '').includes("
        + `${JSON.stringify(String(data.net.sessionExpiredText))}); })()`,
      { timeoutMs: POLL_TIMEOUT, label: "текст 401" },
    );
    assert.equal(await ui.callCount("net_connect"), 2, "после 401 число net_connect не равно 2");
    await new Promise((done) => setTimeout(done, 600));
    assert.equal(await ui.callCount("net_connect"), 2, "появился авто-retry-loop");
    assert.deepEqual(ui.pageExceptions(), [], "необработанные исключения страницы");
  });
});
