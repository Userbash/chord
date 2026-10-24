// @ts-check
/**
 * M2-07 (fixture browser): голосовая панель в сборке без voice-сервиса.
 *
 * Оракул: `features/voice.js` — `service === null` даёт единственное честное
 * состояние `disabled`: `data-state=disabled`, `aria-disabled=true` на панели и
 * на кнопке входа, причина в тексте (`VOICE_TEXT.disabled`). Никаких имитаций
 * «подключено» без сессии.
 *
 * Дополнительно (то, что не проверить в DOM-шиме): клик «Войти» не порождает
 * IPC (`send_typing`, голосовые команды, реакции), не захватывает микрофон
 * (`getUserMedia`) и не создаёт участников.
 */
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { m2FixtureData, openApp } from "../helpers/ui-fixture.mjs";

const data = m2FixtureData();
const DISABLED_TEXT = String(data.voice.disabledText);

/** Команды, которые продукт имеет право слать в фоне (polling/замеры). */
const PERIODIC = new Set([
  "process_metrics",
  "net_status",
  "report_ui_snapshot",
  "settings_load",
  "read_state",
  "ack_message",
]);

describe("M2-07: voice panel disabled/idle (реальный index.html)", () => {
  it("disabled без сервиса: причина видима, join не создаёт IPC/audio", async (t) => {
    const ui = await openApp(t);

    // 1. Панель: disabled с честной причиной, вход заблокирован.
    assert.equal(await ui.attr("#voice-panel", "data-state"), "disabled", "панель не в disabled");
    assert.equal(await ui.attr("#voice-panel", "aria-disabled"), "true");
    assert.equal(await ui.attr("#voice-join", "aria-disabled"), "true", "кнопка входа доступна");
    assert.match(String(await ui.attr("#voice-join", "title")), /недоступен/i);
    assert.equal(await ui.text("#voice-status-label"), DISABLED_TEXT, "нет причины disabled");
    assert.equal(await ui.visible("#voice-participants"), false, "участники показаны без сессии");

    // 2. Ноль аудио-вызовов до действия пользователя.
    assert.equal(JSON.stringify(await ui.eval("window.__QA__.audio")), JSON.stringify({
      getUserMedia: 0,
      audioContext: 0,
      audioPlay: 0,
    }));
    assert.equal(await ui.callCount("send_typing"), 0, "send_typing до действия пользователя");

    // 3. Клик «Войти»: только видимая причина, без IPC и захвата устройств.
    const before = await ui.eval("window.__QA__.calls.length");
    await ui.click("#voice-join");
    await ui.waitFor(
      `(() => { const node = document.querySelector('#voice-a11y');`
        + " return node !== null && (node.textContent || '').trim() !== ''; })()",
      { label: "announce причины disabled" },
    );
    assert.equal(
      String(await ui.text("#voice-a11y")).trim(),
      DISABLED_TEXT,
      "причина не объявлена в live-регионе",
    );
    /** @type {Array<{ cmd: string }>} */
    const after = await ui.eval(`window.__QA__.calls.slice(${Number(before)})`);
    const unexpected = after.filter((call) => !PERIODIC.has(call.cmd));
    assert.deepEqual(
      unexpected.map((call) => call.cmd),
      [],
      "клик по disabled-панели породил IPC",
    );
    assert.equal(
      after.some((call) => /voice|typing|reaction/i.test(call.cmd)),
      false,
      "появилась голосовая/сетевая команда",
    );
    assert.equal(JSON.stringify(await ui.eval("window.__QA__.audio")), JSON.stringify({
      getUserMedia: 0,
      audioContext: 0,
      audioPlay: 0,
    }), "audio-вызов при disabled-панели");
    assert.equal(await ui.callCount("send_typing"), 0, "send_typing после клика join");
    assert.deepEqual(ui.pageExceptions(), [], "необработанные исключения страницы");
  });
});
