// @ts-check
/**
 * M2-02 (fixture browser): канал → send / reply / edit / delete + видимая ошибка.
 *
 * Орёл: `fixtures/gateway/by-type/002-READY.json` (каналы гильдии и их порядок —
 * независимый оракул `channels()`) и кадры `023/024/025-MESSAGE_CREATE.json`
 * (реальные `id`/`content`/`timestamp` канала `1163408591315812392`).
 *
 * Проверяется то, чего не доказывает DOM-шим: сообщения реально отрисованы в
 * настоящем DOM, а `send/reply/edit/delete` получают **настоящие** snowflake
 * (канал/сообщение), а не `"news"`/индекс (G6-D-01). Ошибка отправки обязана
 * быть видимой, а не проглоченной.
 */
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import {
  M2_IDS,
  m2Channels,
  m2Messages,
  openApp,
} from "../helpers/ui-fixture.mjs";

const GUILD = M2_IDS.guildWithMessages;
const CHANNEL = M2_IDS.channelWithMessages;
const CHANNEL_NAME = m2Channels(GUILD).find((channel) => channel.id === CHANNEL)?.name ?? "";
const MESSAGES = m2Messages(CHANNEL);

/**
 * Открывает канал и дожидается строк истории.
 * @param {import("../helpers/ui-fixture.mjs").UiFixture} ui
 */
async function openChannel(ui) {
  await ui.click(`#guild-rail button.guild-item[data-guild-id="${GUILD}"]`);
  await ui.waitFor(
    `Boolean(document.querySelector('.channel-item[data-channel-id="${CHANNEL}"]'))`,
    { label: `channel ${CHANNEL} в дереве` },
  );
  await ui.click(`.channel-item[data-channel-id="${CHANNEL}"]`);
  await ui.waitFor(
    `document.querySelectorAll('#chat-list [data-channel-message]').length === ${MESSAGES.length}`,
    { label: `история канала (${MESSAGES.length} строк)` },
  );
}

/**
 * Текст строк истории в DOM-порядке.
 * @param {import("../helpers/ui-fixture.mjs").UiFixture} ui
 * @returns {Promise<Array<{ id: string | null, text: string }>>}
 */
function readRows(ui) {
  return ui.eval(
    "[...document.querySelectorAll('#chat-list [data-channel-message]')].map((row) => ({"
      + " id: row.getAttribute('data-message-id'),"
      + " text: row.querySelector('.text-body') ? row.querySelector('.text-body').textContent : '',"
      + " }))",
  );
}

describe("M2-02: канал → send/edit/delete/reply (реальный index.html)", () => {
  it("история из MESSAGE_CREATE; send/reply/edit/delete несут реальные id", async (t) => {
    assert.equal(MESSAGES.length, 3, "оракул: в канале должно быть 3 сообщения MESSAGE_CREATE");
    const ui = await openApp(t);
    await openChannel(ui);

    // 1. Заголовок и строки — из payload, а не из статичной разметки.
    assert.equal(await ui.text(".chat-title-text"), CHANNEL_NAME, "заголовок канала не из channels()");
    const rows = await readRows(ui);
    assert.deepEqual(
      rows.map((row) => row.id),
      MESSAGES.map((message) => message.id),
      "id строк не совпадают с MESSAGE_CREATE",
    );
    assert.deepEqual(
      rows.map((row) => row.text),
      MESSAGES.map((message) => message.content),
      "текст строк не совпадает с MESSAGE_CREATE",
    );

    // 2. Отправка: реальный channelId, текст, nonce; поле очищается.
    await ui.type("#composer-input", "M2 fixture send");
    await ui.press("#composer-input", "Enter");
    const send = await ui.waitForCall("send_message", {
      check: (args) => {
        assert.equal(args.channelId, CHANNEL, "send_message: не реальный channelId");
        assert.equal(args.content, "M2 fixture send");
        assert.equal(typeof args.nonce, "string", "send_message: нет nonce");
        assert.ok(String(args.nonce).length > 0, "send_message: пустой nonce");
      },
    });
    assert.equal(send.args.channelId, CHANNEL);
    assert.equal(await ui.eval("document.querySelector('#composer-input').value"), "", "поле не очищено");

    // 3. Reply: hover-панель → «Ответить» → цитата в композере → reply_message.
    const firstRow = `#chat-list [data-channel-message][data-index="0"]`;
    await ui.hover(firstRow);
    await ui.waitFor(`document.querySelector('.msg-toolbar').getAttribute('data-open') === "true"`, {
      label: "hover-панель сообщения",
    });
    await ui.click('.msg-toolbar [data-command="reply"]');
    assert.equal(
      await ui.attr("#composer", "data-reply"),
      "true",
      "композер не перешёл в режим ответа",
    );
    await ui.type("#composer-input", "M2 fixture reply");
    await ui.press("#composer-input", "Enter");
    await ui.waitForCall("reply_message", {
      check: (args) => {
        assert.equal(args.channelId, CHANNEL, "reply_message: не реальный channelId");
        assert.equal(args.content, "M2 fixture reply");
        assert.equal(args.replyTo, MESSAGES[0]?.id, "reply_message: replyTo не id выбранного сообщения");
      },
    });

    // 4. Edit: панель → «Изменить» → сохранение → edit_message с реальными id.
    await ui.hover(firstRow);
    await ui.waitFor(`document.querySelector('.msg-toolbar').getAttribute('data-open') === "true"`);
    await ui.click('.msg-toolbar [data-command="edit"]');
    await ui.waitFor("Boolean(document.querySelector('[data-action=\"edit-input\"]'))", {
      label: "поле правки",
    });
    await ui.type('[data-action="edit-input"]', "M2 fixture edited");
    await ui.click('[data-action="edit-save"]');
    await ui.waitForCall("edit_message", {
      check: (args) => {
        assert.equal(args.channelId, CHANNEL, "edit_message: не реальный channelId");
        assert.equal(args.messageId, MESSAGES[0]?.id, "edit_message: не реальный messageId");
        assert.equal(args.content, "M2 fixture edited");
        assert.ok(!String(args.messageId).match(/^\d{1,3}$/), "edit_message: index вместо snowflake");
      },
    });

    // 5. Delete: два клика (confirm) → один delete_message с реальными id.
    const lastIndex = (await ui.count("#chat-list [data-channel-message]")) - 1;
    const lastRow = `#chat-list [data-channel-message][data-index="${lastIndex}"]`;
    const lastId = await ui.attr(lastRow, "data-message-id");
    await ui.hover(lastRow);
    await ui.waitFor(`document.querySelector('.msg-toolbar').getAttribute('data-open') === "true"`);
    await ui.click('.msg-toolbar [data-command="delete"]');
    await ui.click('.msg-toolbar [data-command="delete"]');
    await ui.waitForCall("delete_message", {
      check: (args) => {
        assert.equal(args.channelId, CHANNEL, "delete_message: не реальный channelId");
        assert.equal(args.messageId, lastId, "delete_message: не id удаляемой строки");
      },
    });

    // 6. Ошибка отправки видима у самой строки (message-error), а не проглочена.
    await ui.setCommand("send_message", { err: { kind: "network" } });
    const before = await ui.callCount("send_message");
    await ui.type("#composer-input", "M2 fixture fail");
    await ui.press("#composer-input", "Enter");
    await ui.waitFor(
      "Boolean(document.querySelector('.chat-sent-messages .local-message[data-message-status=\"error\"]'))",
      { label: "видимая ошибка отправки" },
    );
    assert.match(
      String(await ui.text('.chat-sent-messages .local-message[data-message-status="error"] .message-error')),
      /сеть недоступна/,
      "ошибка отправки не показана текстом",
    );
    assert.equal(await ui.callCount("send_message"), before + 1, "ошибка отправила лишний IPC");
    assert.equal(await ui.visible(".chat-sent-messages .local-message[data-message-status=\"error\"]"), true);
    assert.deepEqual(ui.pageExceptions(), [], "необработанные исключения страницы");
  });
});
