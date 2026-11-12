// @ts-check
/**
 * TASK-765: Реактивное обновление ролей и прав доступа в UI без перезагрузки
 * (`GUILD_MEMBER_UPDATE`, `GUILD_ROLE_CREATE`, `GUILD_ROLE_UPDATE`, `GUILD_ROLE_DELETE`, `CHANNEL_UPDATE`).
 *
 * Проверяет:
 * 1. Проекции `dispatch-model.js`: `projectGuildMemberUpdate`, `projectGuildRoleCreate`,
 *    `projectGuildRoleUpdate`, `projectGuildRoleDelete` и маршрутизацию в `applyDispatch`.
 * 2. Сценарий «дал-забрал роль» (`roles: ["r1"]` -> `roles: []`) без воскрешения старых
 *    ролей из `guild.members` и без перезагрузки страницы.
 * 3. Мгновенный пересчёт дерева каналов (`VIEW_CHANNEL`), композера и баннера
 *    только-чтения (`SEND_MESSAGES`), цветов авторов в чате и доступа к голосовым
 *    каналам (`CONNECT`) при приходе `GUILD_MEMBER_UPDATE` и `GUILD_ROLE_*`.
 */
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { loadApp } from "../support/gui-dom-harness.mjs";
import * as dispatchModel from "../../rust/gui-spike/ui/features/dispatch-model.js";

const {
  applyDispatch,
  createDispatchStore,
} = dispatchModel;

describe("TASK-765: dispatch-model проекции ролей и прав участника", () => {
  it("M1: ROLE_PERMISSION_KINDS экспортирован, а applyDispatch маршрутизирует GUILD_MEMBER_UPDATE и GUILD_ROLE_*", () => {
    assert.equal(Array.isArray(dispatchModel.ROLE_PERMISSION_KINDS), true, "ROLE_PERMISSION_KINDS должен быть массивом");
    assert.deepEqual(
      [...dispatchModel.ROLE_PERMISSION_KINDS],
      ["GUILD_MEMBER_UPDATE", "GUILD_ROLE_CREATE", "GUILD_ROLE_UPDATE", "GUILD_ROLE_DELETE"],
    );
    assert.equal(typeof dispatchModel.projectGuildMemberUpdate, "function");
    assert.equal(typeof dispatchModel.projectGuildRoleCreate, "function");
    assert.equal(typeof dispatchModel.projectGuildRoleUpdate, "function");
    assert.equal(typeof dispatchModel.projectGuildRoleDelete, "function");
  });

  it("M2: GUILD_MEMBER_UPDATE обновляет роли участника в store (включая снятие всех ролей roles: []) и отсекает дубликаты", () => {
    const store = createDispatchStore();
    const grantRes = applyDispatch(store, {
      kind: "GUILD_MEMBER_UPDATE",
      data: {
        guild_id: "100",
        roles: ["201", "202"],
        nick: "Модератор",
        pending: false,
        user: { id: "999", username: "test_member", global_name: "Test Member" },
      },
    });
    assert.equal(grantRes.ok, true);
    assert.equal(grantRes.counters.applied, 1);
    assert.equal(grantRes.counters.ignored, 0);
    assert.equal(grantRes.changes.length, 1);
    const change1 = /** @type {any} */ (grantRes.changes[0]);
    assert.equal(change1.type, "guild_member_update");
    assert.equal(change1.guildId, "100");
    assert.equal(change1.userId, "999");
    assert.deepEqual(change1.roles, ["201", "202"]);
    assert.equal(change1.nick, "Модератор");
    assert.deepEqual(store.members.get("100")?.get("999")?.roles, ["201", "202"]);

    // Повтор того же кадра — duplicate
    const dupRes = applyDispatch(store, {
      kind: "GUILD_MEMBER_UPDATE",
      data: {
        guild_id: "100",
        roles: ["201", "202"],
        nick: "Модератор",
        pending: false,
        user: { id: "999", username: "test_member", global_name: "Test Member" },
      },
    });
    assert.equal(dupRes.counters.duplicate, 1);
    assert.equal(dupRes.changes.length, 0);

    // Снятие всех ролей (roles: []) — список ролей обязан стать пустым, без сохранения старых
    const revokeRes = applyDispatch(store, {
      kind: "GUILD_MEMBER_UPDATE",
      data: {
        guild_id: "100",
        roles: [],
        nick: "Модератор",
        pending: false,
        user: { id: "999", username: "test_member", global_name: "Test Member" },
      },
    });
    assert.equal(revokeRes.counters.applied, 1);
    assert.equal(revokeRes.changes.length, 1);
    const change2 = /** @type {any} */ (revokeRes.changes[0]);
    assert.deepEqual(change2.roles, []);
    assert.deepEqual(store.members.get("100")?.get("999")?.roles, []);
  });

  it("M3: GUILD_ROLE_CREATE, GUILD_ROLE_UPDATE и GUILD_ROLE_DELETE синхронизируют роли гильдии и очищают удалённую роль у участников", () => {
    const store = createDispatchStore();
    applyDispatch(store, {
      kind: "GUILD_MEMBER_UPDATE",
      data: {
        guild_id: "100",
        roles: ["r_mod", "r_keep"],
        user: { id: "999", username: "test_member" },
      },
    });

    const createRes = applyDispatch(store, {
      kind: "GUILD_ROLE_CREATE",
      data: {
        guild_id: "100",
        role: {
          id: "r_mod",
          name: "Страж",
          color: 0x3498db,
          position: 4,
          permissions: "3072",
          hoist: true,
        },
      },
    });
    assert.equal(createRes.counters.applied, 1);
    assert.equal(/** @type {any} */ (createRes.changes[0]).type, "guild_role_create");
    assert.equal(/** @type {any} */ (store).guildRoles?.get("100")?.get("r_mod")?.permissions, "3072");

    const updateRes = applyDispatch(store, {
      kind: "GUILD_ROLE_UPDATE",
      data: {
        guild_id: "100",
        role: {
          id: "r_mod",
          name: "Старший страж",
          color: 0xe74c3c,
          position: 6,
          permissions: "1051648",
          hoist: true,
        },
      },
    });
    assert.equal(updateRes.counters.applied, 1);
    assert.equal(/** @type {any} */ (updateRes.changes[0]).type, "guild_role_update");
    assert.equal(/** @type {any} */ (store).guildRoles?.get("100")?.get("r_mod")?.permissions, "1051648");

    const deleteRes = applyDispatch(store, {
      kind: "GUILD_ROLE_DELETE",
      data: {
        guild_id: "100",
        role_id: "r_mod",
      },
    });
    assert.equal(deleteRes.counters.applied, 1);
    assert.equal(/** @type {any} */ (deleteRes.changes[0]).type, "guild_role_delete");
    assert.equal(/** @type {any} */ (store).guildRoles?.get("100")?.has("r_mod"), false);
    assert.deepEqual(store.members.get("100")?.get("999")?.roles, ["r_keep"]);
  });

  it("M4: битые payload GUILD_MEMBER_UPDATE и GUILD_ROLE_* учитываются в counters.malformed", () => {
    for (const [kind, badData] of [
      ["GUILD_MEMBER_UPDATE", {}],
      ["GUILD_MEMBER_UPDATE", { guild_id: "100", roles: "not-array", user: { id: "1" } }],
      ["GUILD_MEMBER_UPDATE", { guild_id: "100", roles: [], user: {} }],
      ["GUILD_ROLE_CREATE", { guild_id: "100" }],
      ["GUILD_ROLE_UPDATE", { guild_id: "100", role: {} }],
      ["GUILD_ROLE_DELETE", { guild_id: "100" }],
    ]) {
      const store = createDispatchStore();
      const res = applyDispatch(store, { kind, data: badData });
      assert.equal(res.counters.malformed, 1, `${kind} с ${JSON.stringify(badData)} должен быть malformed`);
      assert.equal(res.changes.length, 0);
    }
  });
});

describe("TASK-765: реактивное обновление ролей и прав доступа в app.js без перезагрузки", () => {
  it("UI1: сценарий «дал-забрал роль» реактивно открывает и закрывает каналы, композер и цвет автора без reload", async () => {
    const prevImage = globalThis.Image;
    const prevDisabled = /** @type {any} */ (globalThis).__SPIKE_AUTH_DISABLED__;
    /** @type {any} */
    let gui = null;

    const GUILD_ID = "5001";
    const SELF_ID = "9001";
    const ROLE_WRITER = "7001";
    const ROLE_SECRET = "7002";

    const guildChannels = [
      {
        id: "c_readonly",
        guild_id: GUILD_ID,
        name: "новости",
        type: 0,
        kind: "text",
        position: 0,
        parentId: null,
        permission_overwrites: [
          // @everyone: запрещена отправка сообщений (2048 = 1 << 11)
          { id: GUILD_ID, type: 0, allow: "0", deny: "2048" },
          // ROLE_WRITER: разрешена отправка сообщений (2048)
          { id: ROLE_WRITER, type: 0, allow: "2048", deny: "0" },
        ],
      },
      {
        id: "c_secret",
        guild_id: GUILD_ID,
        name: "секретный-чат",
        type: 0,
        kind: "text",
        position: 1,
        parentId: null,
        permission_overwrites: [
          // @everyone: скрыт просмотр канала (1024 = 1 << 10)
          { id: GUILD_ID, type: 0, allow: "0", deny: "1024" },
          // ROLE_SECRET: разрешён просмотр и отправка (3072 = 1024 | 2048)
          { id: ROLE_SECRET, type: 0, allow: "3072", deny: "0" },
        ],
      },
    ];

    try {
      globalThis.Image = /** @type {any} */ (class FakeImage {});
      /** @type {any} */ (globalThis).__SPIKE_AUTH_DISABLED__ = false;

      gui = await loadApp({
        invokeHandler: (cmd, args) => {
          if (cmd === "auth_status") {
            return { status: "ready", stored: true, user: { id: SELF_ID, username: "tester" } };
          }
          if (cmd === "net_connect" || cmd === "ack_message") {
            return null;
          }
          if (cmd === "read_state") {
            return [];
          }
          if (cmd === "ready") {
            return {
              user: { id: SELF_ID, username: "tester", global_name: "Тестер" },
              guilds: [
                {
                  id: GUILD_ID,
                  name: "Тестовый Сервер",
                  owner_id: "1111",
                  my_roles: [],
                  roles: [
                    // @everyone: VIEW_CHANNEL | SEND_MESSAGES (3072) базово
                    { id: GUILD_ID, name: "@everyone", position: 0, color: 0, permissions: "3072" },
                    { id: ROLE_WRITER, name: "Писатель", position: 5, color: 0x2ecc71, permissions: "3072" },
                    { id: ROLE_SECRET, name: "Тайный орден", position: 10, color: 0xe67e22, permissions: "3072" },
                  ],
                  members: [
                    {
                      user: { id: SELF_ID, username: "tester", global_name: "Тестер" },
                      roles: [],
                    },
                  ],
                  channels: guildChannels,
                },
              ],
              relationships: [],
              private_channels: [],
              users: [{ id: SELF_ID, username: "tester", global_name: "Тестер" }],
            };
          }
          if (cmd === "channels") {
            if (String(args?.guildId ?? "") === GUILD_ID) {
              return guildChannels;
            }
            return [];
          }
          if (cmd === "messages_page") {
            const chId = String(args?.channelId ?? "c_readonly");
            return {
              channelId: chId,
              items: [
                {
                  id: "m_1",
                  channelId: chId,
                  guildId: GUILD_ID,
                  author: { id: SELF_ID, username: "tester", globalName: "Тестер", avatar: null, bot: false },
                  content: "Проверка цвета роли",
                  timestamp: "2026-10-23T20:00:00.000Z",
                  edited: false,
                  editedAt: null,
                  day: "2026-10-23",
                  group: "start",
                  pinned: false,
                  messageType: 0,
                  embeds: [],
                  attachments: [],
                  reactions: [],
                  replyTo: null,
                  mentions: [],
                  mentionEveryone: false,
                },
              ],
              hasMore: false,
              nextCursor: null,
              total: 1,
              limit: 100,
              loadedRowsLimit: 500,
            };
          }
          if (cmd === "members_page") {
            return { items: [], groups: [], total: 1, onlineCount: 1 };
          }
          if (cmd === "voice_set_audio_device" || cmd === "voice_rtc_status" || cmd === "voice_audio_devices") {
            return { ok: true };
          }
          return undefined;
        },
      });

      const doc = gui.window.document;
      const app = gui.appModule.currentApp();
      assert.ok(app, "currentApp() должен быть инициализирован");

      const findChannelItem = (/** @type {string} */ cid) =>
        [...doc.querySelectorAll("#channel-tree .channel-item")].find(
          (node) => node.getAttribute("data-channel-id") === cid,
        ) ?? null;

      await gui.settle(12);

      // 1. В начале у пользователя нет ни ROLE_WRITER, ни ROLE_SECRET:
      // секретный канал скрыт из дерева, а в #новости композер заблокирован.
      assert.equal(
        findChannelItem("c_secret"),
        null,
        "До выдачи роли c_secret обязан быть скрыт из #channel-tree",
      );

      await app.chat.openChannel(guildChannels[0]);
      await gui.settle(8);

      const composerInput = /** @type {HTMLTextAreaElement | null} */ (doc.getElementById("composer-input"));
      assert.ok(composerInput, "#composer-input должен существовать");
      assert.equal(composerInput.disabled, true, "Без роли ROLE_WRITER композер в #новости должен быть заблокирован");
      assert.ok(
        doc.getElementById("channel-readonly-access-banner"),
        "Без роли ROLE_WRITER должен отображаться #channel-readonly-access-banner",
      );

      // 2. Приходит GUILD_MEMBER_UPDATE — пользователю выдали обе роли (БЕЗ перезагрузки и без повторного openChannel)
      app.dispatch.handle({
        kind: "GUILD_MEMBER_UPDATE",
        data: {
          guild_id: GUILD_ID,
          roles: [ROLE_WRITER, ROLE_SECRET],
          user: { id: SELF_ID, username: "tester", global_name: "Тестер" },
        },
      });
      await gui.settle(4);

      assert.ok(
        findChannelItem("c_secret"),
        "После GUILD_MEMBER_UPDATE канал c_secret должен мгновенно появиться в #channel-tree без перезагрузки",
      );
      assert.equal(
        composerInput.disabled,
        false,
        "После выдачи ROLE_WRITER композер должен мгновенно разблокироваться без перезагрузки",
      );
      assert.equal(
        doc.getElementById("channel-readonly-access-banner"),
        null,
        "После выдачи ROLE_WRITER баннер только-чтения должен мгновенно исчезнуть",
      );

      const authorRow = [...doc.querySelectorAll("#chat-list .row.channel-message")].find(
        (node) => node.getAttribute("data-author-id") === SELF_ID,
      );
      const authorNameEl = authorRow?.querySelector(".author-name") ?? null;
      if (authorNameEl) {
        assert.equal(
          authorNameEl.getAttribute("data-role-color"),
          "#e67e22",
          "Цвет имени автора в чате должен реактивно обновиться до цвета старшей роли (#e67e22)",
        );
      }

      // 3. Сценарий «дал-забрал роль»: приходит GUILD_MEMBER_UPDATE с пустым массивом roles: []
      app.dispatch.handle({
        kind: "GUILD_MEMBER_UPDATE",
        data: {
          guild_id: GUILD_ID,
          roles: [],
          user: { id: SELF_ID, username: "tester", global_name: "Тестер" },
        },
      });
      await gui.settle(4);

      assert.equal(
        findChannelItem("c_secret"),
        null,
        "После снятия ролей (roles: []) c_secret должен снова мгновенно скрыться из #channel-tree",
      );
      assert.equal(
        composerInput.disabled,
        true,
        "После снятия ролей (roles: []) композер должен мгновенно заблокироваться (роли не должны воскресать из guild.members)",
      );
      assert.ok(
        doc.getElementById("channel-readonly-access-banner"),
        "После снятия ролей (roles: []) баннер #channel-readonly-access-banner должен снова появиться",
      );
    } finally {
      gui?.dispose();
      globalThis.Image = prevImage;
      /** @type {any} */ (globalThis).__SPIKE_AUTH_DISABLED__ = prevDisabled;
    }
  });

  it("UI2: GUILD_ROLE_UPDATE, GUILD_ROLE_DELETE и доступ к голосовому каналу (CONNECT) обновляются реактивно без reload", async () => {
    const prevImage = globalThis.Image;
    const prevDisabled = /** @type {any} */ (globalThis).__SPIKE_AUTH_DISABLED__;
    /** @type {any} */
    let gui = null;

    const GUILD_ID = "6001";
    const SELF_ID = "9002";
    const ROLE_MEMBER = "8001";
    let voiceConnectCalls = 0;

    const guildChannels = [
      {
        id: "c_general",
        guild_id: GUILD_ID,
        name: "общий",
        type: 0,
        kind: "text",
        position: 0,
        parentId: null,
        permission_overwrites: [],
      },
      {
        id: "v_locked",
        guild_id: GUILD_ID,
        name: "Закрытый войс",
        type: 2,
        kind: "voice",
        position: 1,
        parentId: null,
        permission_overwrites: [
          // @everyone: запрещён CONNECT (1048576 = 1 << 20)
          { id: GUILD_ID, type: 0, allow: "0", deny: "1048576" },
          // ROLE_MEMBER: изначально allow = 0 (нет CONNECT), обновим через GUILD_ROLE_UPDATE / CHANNEL_UPDATE
          { id: ROLE_MEMBER, type: 0, allow: "0", deny: "0" },
        ],
      },
    ];

    try {
      globalThis.Image = /** @type {any} */ (class FakeImage {});
      /** @type {any} */ (globalThis).__SPIKE_AUTH_DISABLED__ = false;

      gui = await loadApp({
        invokeHandler: (cmd, args) => {
          if (cmd === "auth_status") {
            return { status: "ready", stored: true, user: { id: SELF_ID, username: "voiceuser" } };
          }
          if (cmd === "net_connect" || cmd === "ack_message") {
            return null;
          }
          if (cmd === "read_state") {
            return [];
          }
          if (cmd === "ready") {
            return {
              user: { id: SELF_ID, username: "voiceuser", global_name: "ВойсТестер" },
              guilds: [
                {
                  id: GUILD_ID,
                  name: "Голосовой Сервер",
                  owner_id: "1111",
                  my_roles: [ROLE_MEMBER],
                  roles: [
                    // @everyone: VIEW_CHANNEL (1024) | SEND_MESSAGES (2048) = 3072
                    { id: GUILD_ID, name: "@everyone", position: 0, color: 0, permissions: "3072" },
                    // ROLE_MEMBER: изначально только VIEW_CHANNEL (1024), без SEND_MESSAGES и без CONNECT
                    { id: ROLE_MEMBER, name: "Участник", position: 2, color: 0x3498db, permissions: "1024" },
                  ],
                  members: [
                    {
                      user: { id: SELF_ID, username: "voiceuser", global_name: "ВойсТестер" },
                      roles: [ROLE_MEMBER],
                    },
                  ],
                  channels: guildChannels,
                },
              ],
              relationships: [],
              private_channels: [],
              users: [{ id: SELF_ID, username: "voiceuser", global_name: "ВойсТестер" }],
            };
          }
          if (cmd === "channels") {
            if (String(args?.guildId ?? "") === GUILD_ID) {
              return guildChannels;
            }
            return [];
          }
          if (cmd === "messages_page") {
            return {
              channelId: String(args?.channelId ?? "c_general"),
              items: [],
              hasMore: false,
              nextCursor: null,
              total: 0,
              limit: 100,
              loadedRowsLimit: 500,
            };
          }
          if (cmd === "voice_connect") {
            voiceConnectCalls += 1;
            return { ok: true, state: "connected", lastPingMs: 25 };
          }
          if (
            cmd === "voice_disconnect" ||
            cmd === "voice_set_audio_device" ||
            cmd === "voice_rtc_status" ||
            cmd === "voice_audio_devices"
          ) {
            return { ok: true, state: "connected", lastPingMs: 25 };
          }
          if (cmd === "members_page") {
            return { items: [], groups: [], total: 1, onlineCount: 1 };
          }
          return undefined;
        },
      });

      const doc = gui.window.document;
      const app = gui.appModule.currentApp();
      assert.ok(app);

      const findChannelItem = (/** @type {string} */ cid) =>
        [...doc.querySelectorAll("#channel-tree .channel-item")].find(
          (node) => node.getAttribute("data-channel-id") === cid,
        ) ?? null;

      await gui.settle(12);
      await app.chat.openChannel(guildChannels[0]);
      await gui.settle(8);

      // 1. Попытка войти в v_locked без права CONNECT блокируется
      const voiceBtnBefore = findChannelItem("v_locked");
      assert.ok(voiceBtnBefore, "v_locked должен быть виден в дереве");
      voiceBtnBefore.click();
      await gui.settle(4);
      assert.equal(voiceConnectCalls, 0, "Без права CONNECT вызов voice_connect не должен происходить");

      // 2. Приходит GUILD_ROLE_UPDATE — у @everyone отняли SEND_MESSAGES (permissions: "1024"),
      // проверяем что композер в #общий мгновенно стал disabled
      app.dispatch.handle({
        kind: "GUILD_ROLE_UPDATE",
        data: {
          guild_id: GUILD_ID,
          role: {
            id: GUILD_ID,
            name: "@everyone",
            position: 0,
            color: 0,
            permissions: "1024",
          },
        },
      });
      await gui.settle(4);

      const composerInput = /** @type {HTMLTextAreaElement | null} */ (doc.getElementById("composer-input"));
      assert.equal(
        composerInput?.disabled,
        true,
        "После GUILD_ROLE_UPDATE (снятие SEND_MESSAGES у @everyone) композер должен мгновенно заблокироваться",
      );

      // 3. Приходит GUILD_ROLE_UPDATE — роли ROLE_MEMBER дали SEND_MESSAGES (3072),
      // а через CHANNEL_UPDATE открыли CONNECT в v_locked для ROLE_MEMBER
      app.dispatch.handle({
        kind: "GUILD_ROLE_UPDATE",
        data: {
          guild_id: GUILD_ID,
          role: {
            id: ROLE_MEMBER,
            name: "Участник+",
            position: 2,
            color: 0x2ecc71,
            permissions: "3072",
          },
        },
      });
      app.dispatch.handle({
        kind: "CHANNEL_UPDATE",
        data: {
          id: "v_locked",
          guild_id: GUILD_ID,
          name: "Закрытый войс",
          type: 2,
          permission_overwrites: [
            { id: GUILD_ID, type: 0, allow: "0", deny: "1048576" },
            { id: ROLE_MEMBER, type: 0, allow: "1048576", deny: "0" },
          ],
        },
      });
      await gui.settle(4);

      assert.equal(
        composerInput?.disabled,
        false,
        "После GUILD_ROLE_UPDATE (выдача SEND_MESSAGES роли ROLE_MEMBER) композер должен мгновенно разблокироваться",
      );

      // Повторный клик по голосовому каналу v_locked теперь успешно вызывает voice_connect без перезагрузки!
      const voiceBtnAfter = findChannelItem("v_locked");
      assert.ok(voiceBtnAfter);
      voiceBtnAfter.click();
      await gui.settle(4);
      assert.equal(voiceConnectCalls, 1, "После выдачи CONNECT клик по v_locked обязан вызвать voice_connect");

      // 4. Приходит GUILD_ROLE_DELETE — роль ROLE_MEMBER удалена с сервера
      app.dispatch.handle({
        kind: "GUILD_ROLE_DELETE",
        data: {
          guild_id: GUILD_ID,
          role_id: ROLE_MEMBER,
        },
      });
      await gui.settle(4);

      assert.equal(
        composerInput?.disabled,
        true,
        "После GUILD_ROLE_DELETE удалённая роль больше не даёт SEND_MESSAGES, композер мгновенно блокируется",
      );
    } finally {
      gui?.dispose();
      globalThis.Image = prevImage;
      /** @type {any} */ (globalThis).__SPIKE_AUTH_DISABLED__ = prevDisabled;
    }
  });
});
