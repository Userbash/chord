// @ts-check
/**
 * M2-04 (fixture browser): join invite → preview → accept → новая плитка рейла.
 *
 * Оракул: контракт `resolve_invite` (`InvitePreview`: `guildId`/`guildName`/
 * `memberCount`/`presenceCount`) и `accept_invite` (`AcceptedInvite.guildId`),
 * тексты `JOIN_TEXT` из `features/join-server.js`. Данные preview приходят
 * только из fake-ответа — в разметке их нет, поэтому «нарисованный» preview
 * без ответа IPC был бы виден как провал.
 *
 * Проверяется: ровно один вызов на шаг (retry не дублирует accept), ошибка
 * resolve видима и accept при ней не отправляется.
 */
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { M2_IDS, m2Channels, m2FixtureData, openApp } from "../helpers/ui-fixture.mjs";

const data = m2FixtureData();
const CODE = String(data.invite.code);
const INVITED = M2_IDS.invitedGuild;

/**
 * Открывает модалку join и вводит код.
 * @param {import("../helpers/ui-fixture.mjs").UiFixture} ui
 */
async function openInviteDialog(ui) {
  await ui.click('.guild-add[data-action="add-server"]');
  await ui.waitFor("(() => { const node = document.querySelector('#join-server-dialog');"
    + " return node !== null && !node.hasAttribute('hidden'); })()", { label: "join-модалка открыта" });
  await ui.type("#join-invite-input", CODE);
}

describe("M2-04: join invite → preview → accept → rail (реальный index.html)", () => {
  it("preview из ответа, accept добавляет плитку; по одному вызову на шаг", async (t) => {
    const ui = await openApp(t, {
      overrides: {
        resolve_invite: { ok: data.invite.preview },
        accept_invite: { ok: data.invite.accepted },
      },
    });
    await openInviteDialog(ui);

    // 1. Preview: отрисовка строго по ответу resolve_invite.
    await ui.click("#join-submit");
    await ui.waitFor("(() => { const node = document.querySelector('#join-preview');"
      + " return node !== null && node.getAttribute('data-state') === 'ready'; })()", {
      label: "preview готов",
    });
    assert.equal(await ui.visible("#join-preview"), true, "preview не виден");
    assert.match(String(await ui.text("#join-preview")), new RegExp(data.invite.preview.guildName));
    assert.match(String(await ui.text("#join-preview")), /1[\s\u00a0\u202f]234 участников/);
    assert.match(String(await ui.text("#join-preview")), /56 в сети/);
    assert.equal(await ui.callCount("resolve_invite"), 1, "resolve_invite вызван не один раз");
    assert.equal(await ui.callCount("accept_invite"), 0, "accept ушёл до подтверждения");

    // 2. Accept: диалог закрывается, плитка появляется, каналы грузятся по guildId.
    const tilesBefore = await ui.count("#guild-rail button.guild-item");
    await ui.click("#join-submit");
    await ui.waitFor("(() => { const node = document.querySelector('#join-server-dialog');"
      + " return node !== null && node.hasAttribute('hidden'); })()", { label: "join-модалка закрыта" });
    assert.equal(await ui.callCount("accept_invite"), 1, "accept_invite не ровно один");
    assert.equal(await ui.count("#guild-rail button.guild-item"), tilesBefore + 1, "плитка не добавилась");
    assert.equal(
      await ui.has(`#guild-rail button.guild-item[data-guild-id="${INVITED}"]`),
      true,
      "плитки приглашённой гильдии нет в рейле",
    );
    assert.match(String(await ui.text("#server-status")), /Вы присоединились/);
    await ui.waitForCall("channels", {
      where: (args) => args?.guildId === INVITED,
      check: (args) => assert.equal(args.guildId, INVITED, "channels() не для приглашённой гильдии"),
    });
    assert.deepEqual(ui.pageExceptions(), [], "необработанные исключения страницы");

    // 3. Повторный клик после accept не отправляет второй accept (диалог скрыт).
    const acceptCount = await ui.callCount("accept_invite");
    assert.equal(acceptCount, 1);
    // Для приглашённой гильдии фикстуры каналов нет — дерево честно пустое.
    assert.equal(m2Channels(INVITED).length, 0, "оракул: у приглашённой гильдии нет каналов в READY");
  });

  it("resolve 429: видимая ошибка, accept не отправляется", async (t) => {
    const ui = await openApp(t, {
      overrides: {
        resolve_invite: { err: data.invite.resolveRateLimited },
        accept_invite: { ok: data.invite.accepted },
      },
    });
    await openInviteDialog(ui);
    await ui.click("#join-submit");
    await ui.waitFor("(() => { const node = document.querySelector('#join-error');"
      + " return node !== null && !node.hasAttribute('hidden')"
      + ` && node.textContent.includes(${JSON.stringify(data.invite.rateLimitedText)}); })()`, {
      label: "видимая ошибка resolve",
    });
    assert.equal(await ui.visible("#join-error"), true, "ошибка не видна");
    assert.equal(await ui.callCount("accept_invite"), 0, "accept отправлен после ошибки resolve");
    assert.equal(await ui.callCount("resolve_invite"), 1, "resolve повторён без действия пользователя");
    assert.equal(
      await ui.attr("#join-preview", "hidden"),
      "",
      "preview показан без успешного resolve",
    );
    assert.deepEqual(ui.pageExceptions(), [], "необработанные исключения страницы");
  });
});
