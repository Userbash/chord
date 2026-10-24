// @ts-check
/**
 * C-02-UI (RW-40): реальные каналы из `channels(guildId)` + переключение и история.
 *
 * Оракул — реальные артефакты, не реализация:
 *   - `fixtures/gateway/by-type/002-READY.json` — 272 гильдейских канала, типы
 *     0/2/4/5/15, `position`, `parent_id`; фильтр по гильдии и порядок
 *     (`position` asc, затем `id` asc) считаются здесь независимо от UI;
 *   - кадры `*-MESSAGE_CREATE.json` — реальные message id/content канала
 *     `1163408591315812392` (3 сообщения);
 *   - `waves/RW-40/PARITY/G6-REPORT-RW40-02.md` §3.4:
 *     `#channel-tree .channel-item[data-channel-id]`, `aria-current="page"`,
 *     заголовок `#chat-header .chat-title-text`, `#chat-list` из `messages_page`,
 *     типы 0/5 открывают чат, 2/4/15 — нет (видимая причина), ошибки
 *     `NotFound|InvalidArg` видны (не пустой список), бенч `CHAT_ROWS=100_000`
 *     не ослаблен, G6-D-01 (`channelId:"news"`, `messageId=index`) закрыт.
 *
 * Шим не всплывает события: слушатели адресуются самому узлу, как в `actions.js`.
 */
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { loadApp } from "../support/gui-dom-harness.mjs";

/** @typedef {import("../support/gui-dom-harness.mjs").GuiDom} GuiDom */
/** @typedef {import("../support/gui-dom-harness.mjs").ShimElement} ShimElement */

const fixturesDir = fileURLToPath(new URL("../../fixtures/gateway/by-type/", import.meta.url));
const READY = JSON.parse(readFileSync(`${fixturesDir}002-READY.json`, "utf8"));

/** Гильдии READY в порядке рейла. @type {Array<{ id: string, name: string }>} */
const GUILDS = (READY?.d?.guilds ?? []).map((/** @type {any} */ guild) => ({
  id: String(guild.id),
  name: String(guild?.properties?.name ?? ""),
}));

/** Кадры MESSAGE_CREATE — независимый источник реальных сообщений. @type {any[]} */
const MESSAGE_FRAMES = readdirSync(fixturesDir)
  .filter((name) => name.endsWith(".json"))
  .map((name) => {
    try {
      return JSON.parse(readFileSync(`${fixturesDir}${name}`, "utf8"));
    } catch {
      return null;
    }
  })
  .filter((frame) => frame?.t === "MESSAGE_CREATE");

/** Реальный канал с сообщениями в фикстурах (guild 2 READY). */
const CHANNEL_WITH_MESSAGES = "1163408591315812392";
/** Гильдия этого канала. */
const GUILD_WITH_MESSAGES = "1014537851418980372";

/**
 * Каналы гильдии по READY — независимый оракул backend-проекции
 * (`data.rs channels`: порядок гильдий READY, внутри `position` asc, `id` asc).
 * @param {string} guildId
 * @returns {Array<{ id: string, guildId: string, name: string | null, type: number, parentId: string | null, position: number }>}
 */
function oracleChannels(guildId) {
  const guild = (READY?.d?.guilds ?? []).find(
    (/** @type {any} */ item) => String(item.id) === guildId,
  );
  if (!guild) return [];
  return /** @type {any[]} */ (guild.channels ?? [])
    .map((channel) => ({
      id: String(channel.id),
      guildId,
      name: typeof channel.name === "string" ? channel.name : null,
      type: Number(channel.type),
      parentId: channel.parent_id === "nil" || channel.parent_id === undefined
        ? null
        : String(channel.parent_id),
      position: Number(channel.position ?? 0),
    }))
    .sort((left, right) => {
      if (left.position !== right.position) return left.position - right.position;
      return left.id < right.id ? -1 : left.id > right.id ? 1 : 0;
    });
}

/**
 * Сообщения канала из MESSAGE_CREATE — независимый оракул `messages_page`
 * (`data.rs collect_messages`: порядок `timestamp`, затем `id`; `group` — по
 * соседям: новый автор/день начинает группу).
 * @param {string} channelId
 * @returns {Array<{ id: string, content: string, author: string, timestamp: string, group: string }>}
 */
function oracleMessages(channelId) {
  const byId = new Map();
  for (const frame of MESSAGE_FRAMES) {
    const data = frame?.d ?? {};
    if (String(data.channel_id ?? "") !== channelId) continue;
    const id = String(data.id ?? "");
    if (id === "") continue;
    byId.set(id, {
      id,
      content: String(data.content ?? ""),
      author: String(data.author?.username ?? ""),
      timestamp: String(data.timestamp ?? ""),
    });
  }
  const rows = [...byId.values()].sort((left, right) => {
    if (left.timestamp !== right.timestamp) return left.timestamp < right.timestamp ? -1 : 1;
    return left.id < right.id ? -1 : left.id > right.id ? 1 : 0;
  });
  /** @type {string | null} */
  let previousAuthor = null;
  /** @type {string | null} */
  let previousDay = null;
  return rows.map((row) => {
    const day = row.timestamp.slice(0, 10);
    const group = previousAuthor === row.author && previousDay === day ? "continuation" : "start";
    previousAuthor = row.author;
    previousDay = day;
    return { ...row, group };
  });
}

/**
 * `messages_page`-ответ в форме `MessagesPage` (camelCase) из оракула.
 * @param {string} channelId
 * @param {number} [limit]
 */
function messagesPagePayload(channelId, limit = 100) {
  const rows = oracleMessages(channelId);
  return {
    channelId,
    items: rows.map((row) => ({
      id: row.id,
      channelId,
      guildId: null,
      author: {
        id: row.author,
        username: row.author,
        globalName: null,
        avatar: null,
        bot: row.author.toLowerCase().includes("bot"),
      },
      content: row.content,
      timestamp: row.timestamp,
      edited: false,
      editedAt: null,
      day: row.timestamp.slice(0, 10),
      group: row.group,
      pinned: false,
      messageType: 0,
      embeds: [],
      attachments: [],
      reactions: [],
      replyTo: null,
      mentions: [],
      mentionEveryone: false,
    })),
    nextCursor: null,
    hasMore: false,
    total: rows.length,
    limit,
    loadedRowsLimit: 500,
  };
}

/** `ready()` в форме `data.rs ReadyPayload`. */
function readyPayload() {
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
    guilds: GUILDS.map((guild) => ({
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

/**
 * Приложение с управляемым IPC: вход пройден, `ready()` и `channels()` — из
 * оракула, `messages_page` можно переопределить (loading/ошибка/пусто).
 * @param {{
 *   messages?: (channelId: string, args: any) => any,
 *   channels?: (guildId: string, args: any) => any,
 * }} [overrides]
 * @returns {Promise<GuiDom>}
 */
function loadSwitchApp(overrides = {}) {
  return loadApp({
    invokeHandler: async (cmd, args) => {
      if (cmd === "auth_status") {
        return { status: "ready", stored: true, user: { id: "42", username: "tester" } };
      }
      if (cmd === "ready") return readyPayload();
      if (cmd === "channels") {
        const guildId = String(args?.guildId ?? "");
        if (overrides.channels) return overrides.channels(guildId, args);
        return oracleChannels(guildId);
      }
      if (cmd === "messages_page") {
        const channelId = String(args?.channelId ?? "");
        if (overrides.messages) return overrides.messages(channelId, args);
        return messagesPagePayload(channelId);
      }
      // Штатный успех команд сообщений: сервер отвечает 2xx без тела (кроме send).
      if (cmd === "send_message") {
        return {
          id: "1561627999999999999",
          channel_id: args?.channelId,
          content: args?.content,
          nonce: args?.nonce,
        };
      }
      if (cmd === "edit_message" || cmd === "delete_message" || cmd === "reply_message") {
        return null;
      }
      return undefined;
    },
  });
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
 * @param {string} selector
 * @returns {ShimElement[]}
 */
function all(app, selector) {
  return app.document.querySelectorAll(selector);
}

/**
 * @param {GuiDom} app
 * @param {string} channelId
 * @returns {ShimElement}
 */
function channelButton(app, channelId) {
  const node = all(app, ".channel-item").find(
    (item) => item.getAttribute("data-channel-id") === channelId,
  );
  assert.ok(node, `в дереве нет канала ${channelId}`);
  return node;
}

/**
 * @param {GuiDom} app
 * @param {string} guildId
 */
async function selectGuild(app, guildId) {
  const tile = all(app, ".guild-item").find(
    (node) => node.getAttribute("data-guild-id") === guildId,
  );
  assert.ok(tile, `в рейле нет гильдии ${guildId}`);
  tile.click();
  await app.settle(12);
}

/** Типы, которые открывают чат (G6 §3.4). */
const CHAT_TYPES = new Set([0, 5]);
/** Типы, которые не открывают чат, но видны. */
const NON_CHAT_TYPES = new Set([2, 4, 15]);
/** Порядок групп дерева (контракт C-02: текст/объявления, затем служебные). */
const GROUP_ORDER = [0, 5, 15, 2, 4];

/**
 * Ожидаемый DOM-порядок: группы по типам в порядке `GROUP_ORDER`, внутри
 * группы — порядок `channels()` (position, id).
 * @param {ReturnType<typeof oracleChannels>} rows
 * @returns {ReturnType<typeof oracleChannels>}
 */
function expectedTreeOrder(rows) {
  const present = [...new Set(rows.map((row) => row.type))];
  const types = [
    ...GROUP_ORDER.filter((type) => present.includes(type)),
    ...present.filter((type) => !GROUP_ORDER.includes(type)),
  ];
  return types.flatMap((type) => rows.filter((row) => row.type === type));
}

describe("C-02-UI: дерево каналов из channels(guildId)", () => {
  it("выбор гильдии → channels(guildId), дерево по типам, 0/5 кликабельны, 2/4/15 — нет", async () => {
    const app = await loadSwitchApp();
    try {
      const guildId = GUILDS[0]?.id ?? "";
      const oracle = oracleChannels(guildId);
      assert.ok(oracle.length > 20, "в READY у первой гильдии подозрительно мало каналов");

      const calls = app.callsOf("channels");
      assert.equal(calls.length, 1, "при выборе гильдии channels() не вызван ровно один раз");
      assert.deepEqual(calls[0]?.args, { guildId }, "channels() вызван без guildId выбранной гильдии");

      const tree = firstNode(app, "#channel-tree");
      assert.equal(tree.getAttribute("data-state"), "ready", "дерево не перешло в ready");

      const items = all(app, "#channel-tree .channel-item[data-channel-id]");
      const expectedOrder = expectedTreeOrder(oracle);
      assert.equal(items.length, oracle.length, "в дереве не все каналы гильдии");
      assert.deepEqual(
        items.map((item) => item.getAttribute("data-channel-id")),
        expectedOrder.map((channel) => channel.id),
        "порядок/id каналов не из READY (группы по типам, внутри — position/id)",
      );
      for (const [index, channel] of expectedOrder.entries()) {
        const item = /** @type {ShimElement} */ (items[index]);
        assert.equal(item.getAttribute("data-channel-type"), String(channel.type), "тип канала не из READY");
        assert.equal(
          item.querySelector(".channel-name")?.textContent,
          channel.name ?? channel.id,
          "имя канала не из READY",
        );
        const openable = CHAT_TYPES.has(channel.type);
        assert.equal(item.getAttribute("data-openable"), openable ? "true" : "false", "data-openable не по типу канала");
        if (NON_CHAT_TYPES.has(channel.type)) {
          assert.equal(item.getAttribute("aria-disabled"), "true", "некликабельный канал не помечен aria-disabled");
          assert.ok(
            (item.querySelector(".channel-reason")?.textContent ?? "").length > 0,
            "у voice/category/forum нет видимой причины",
          );
        }
      }
      // Группы по типам: заголовки видны, каждый тип из READY имеет свою группу.
      const titles = all(app, "#channel-tree .channel-group-title").map((node) => node.textContent ?? "");
      assert.ok(titles.length >= 4, "каналы не сгруппированы по типам");
      for (const type of [0, 2, 4]) {
        assert.ok(
          all(app, "#channel-tree .channel-item[data-channel-type=\"" + type + "\"]").length > 0,
          `в дереве нет группы типа ${type}, хотя он есть в READY`,
        );
      }

      // Бенч-путь не ослаблен: снапшот по-прежнему 100 000 синтетических строк.
      assert.equal(app.snapshot.payload?.chatRows, 100_000, "CHAT_ROWS бенча ослаблен");

      // Некликабельные каналы не открывают чат и объясняют причину.
      const samples = [
        { type: 2, reason: /голос/i },
        { type: 4, reason: /категор/i },
        { type: 15, reason: /форум|ветк/i },
      ];
      for (const sample of samples) {
        const item = all(app, "#channel-tree .channel-item").find(
          (node) => node.getAttribute("data-channel-type") === String(sample.type),
        );
        if (!item) continue;
        const titleBefore = firstNode(app, ".chat-title-text").textContent;
        item.click();
        await app.settle(4);
        assert.equal(app.callsOf("messages_page").length, 0, `клик по типу ${sample.type} открыл чат`);
        assert.equal(firstNode(app, ".chat-title-text").textContent, titleBefore, "заголовок сменился без открытия чата");
        assert.match(
          firstNode(app, ".channel-note").textContent ?? "",
          sample.reason,
          `нет видимой причины для типа ${sample.type}`,
        );
      }

      // Текстовый канал открывает реальную историю.
      const textChannel = oracle.find((channel) => channel.type === 0);
      assert.ok(textChannel, "в READY нет текстового канала");
      channelButton(app, textChannel.id).click();
      await app.settle(12);

      const page = app.callsOf("messages_page");
      assert.equal(page.length, 1, "клик по текстовому каналу не вызвал messages_page");
      assert.equal(page[0]?.args?.channelId, textChannel.id, "messages_page без реального channelId");
      assert.equal(
        firstNode(app, ".chat-title-text").textContent,
        textChannel.name,
        "заголовок чата не имя выбранного канала",
      );
      assert.equal(
        channelButton(app, textChannel.id).getAttribute("aria-current"),
        "page",
        "выбранный канал не aria-current=page",
      );
      assert.equal(
        all(app, '#channel-tree .channel-item[aria-current="page"]').length,
        1,
        "aria-current стоит не на одном канале",
      );
      // Синтетика в реальном канале не используется: строки — из messages_page.
      const rows = all(app, "#chat-list .row[data-message-id]");
      const oracleText = oracleMessages(textChannel.id);
      assert.equal(rows.length, oracleText.length, "строки не из messages_page (синтетическое окно)");
      assert.deepEqual(
        rows.map((row) => row.getAttribute("data-message-id")),
        oracleText.map((message) => message.id),
        "message id строк не реальные",
      );
      if (oracleText.length > 0) {
        assert.match(
          firstNode(app, "#chat-list .row .text-body").textContent ?? "",
          /./,
          "у реальной строки нет текста",
        );
      }
    } finally {
      app.dispose();
    }
  });

  it("переключение гильдии перезагружает дерево по channels() новой гильдии", async () => {
    const app = await loadSwitchApp();
    try {
      const secondGuild = GUILDS[1]?.id ?? "";
      const oracle = oracleChannels(secondGuild);
      await selectGuild(app, secondGuild);

      const calls = app.callsOf("channels");
      assert.equal(calls.length, 2, "смена гильдии не вызвала channels() второй раз");
      assert.deepEqual(calls[1]?.args, { guildId: secondGuild }, "channels() не по новой гильдии");
      assert.deepEqual(
        all(app, "#channel-tree .channel-item[data-channel-id]").map((node) => node.getAttribute("data-channel-id")),
        expectedTreeOrder(oracle).map((channel) => channel.id),
        "дерево не перерисовано под новую гильдию",
      );
      assert.equal(firstNode(app, ".tree-guild").textContent, GUILDS[1]?.name, "заголовок дерева не сменился");
    } finally {
      app.dispose();
    }
  });
});

describe("C-02-UI: переключение канала, окно и позиция", () => {
  it("каждый канал помнит своё окно и позицию; возврат не перезапрашивает историю", async () => {
    const app = await loadSwitchApp();
    try {
      await selectGuild(app, GUILD_WITH_MESSAGES);
      const guildChannels = oracleChannels(GUILD_WITH_MESSAGES);
      const withHistory = guildChannels.find((channel) => channel.id === CHANNEL_WITH_MESSAGES);
      const emptyChannel = guildChannels.find(
        (channel) => channel.type === 0 && oracleMessages(channel.id).length === 0,
      );
      assert.ok(withHistory, "в READY нет канала с сообщениями");
      assert.ok(emptyChannel, "в READY нет текстового канала без сообщений");

      channelButton(app, withHistory.id).click();
      await app.settle(12);
      const rows = all(app, "#chat-list .row[data-message-id]");
      assert.ok(rows.length > 0, "канал с историей открылся пустым");
      const list = firstNode(app, "#chat-list");
      list.scrollTop = 120;
      const historyCalls = () => app.callsOf("messages_page").filter((call) => call.args?.channelId === withHistory.id).length;
      assert.equal(historyCalls(), 1, "история канала запрошена не один раз");

      channelButton(app, emptyChannel.id).click();
      await app.settle(12);
      assert.equal(firstNode(app, ".chat-title-text").textContent, emptyChannel.name, "заголовок не сменился на второй канал");
      assert.equal(
        all(app, "#chat-list .row[data-message-id]").length,
        0,
        "у канала без сообщений остались строки прошлого канала",
      );
      assert.equal(
        firstNode(app, "#chat-list [data-state]").getAttribute("data-state"),
        "empty",
        "пустой канал без empty-состояния",
      );

      channelButton(app, withHistory.id).click();
      await app.settle(12);
      assert.equal(historyCalls(), 1, "возврат в канал перезапросил историю (окно не сохранено)");
      assert.equal(
        all(app, "#chat-list .row[data-message-id]").length,
        rows.length,
        "возврат не восстановил окно канала",
      );
      assert.equal(firstNode(app, "#chat-list").scrollTop, 120, "возврат не восстановил позицию канала");

      // Пустой канал тоже помнит своё окно: второй вызов не нужен.
      channelButton(app, emptyChannel.id).click();
      await app.settle(12);
      assert.equal(
        app.callsOf("messages_page").filter((call) => call.args?.channelId === emptyChannel.id).length,
        1,
        "повторный вход в пустой канал перезапросил историю",
      );
    } finally {
      app.dispose();
    }
  });
});

describe("C-02-UI: G6-D-01 — реальные id в send/edit/delete/reply", () => {
  it("send/edit/delete/reply несут реальный channelId и messageId, а не news/index", async () => {
    const app = await loadSwitchApp();
    try {
      await selectGuild(app, GUILD_WITH_MESSAGES);
      channelButton(app, CHANNEL_WITH_MESSAGES).click();
      await app.settle(12);
      const oracle = oracleMessages(CHANNEL_WITH_MESSAGES);
      assert.equal(oracle.length, 3, "оракул канала изменился: не 3 сообщения");

      const input = firstNode(app, "#composer-input");
      input.value = "реальное сообщение";
      input.emit("input");
      input.dispatchEvent("keydown", { key: "Enter" });
      await app.settle(4);
      const sent = app.callsOf("send_message");
      assert.equal(sent.length, 1, "композер не отправил send_message");
      assert.equal(sent[0]?.args?.channelId, CHANNEL_WITH_MESSAGES, "send_message ушёл в несуществующий канал");
      assert.notEqual(sent[0]?.args?.channelId, "news", "send_message всё ещё на «news»");

      const list = firstNode(app, "#chat-list");
      const rowAt = (/** @type {number} */ index) => {
        const row = all(app, "#chat-list .row[data-message-id]")[index];
        assert.ok(row, `нет строки ${index}`);
        return row;
      };

      // edit: реальный id сообщения, не индекс.
      list.dispatchEvent("mouseover", { target: rowAt(0) });
      const edit = all(app, ".msg-toolbar button").find(
        (button) => button.getAttribute("aria-label") === "Изменить сообщение",
      );
      assert.ok(edit, "в панели нет «Изменить сообщение»");
      edit.click();
      const editor = firstNode(app, '[data-action="edit-input"]');
      editor.value = "правка реального сообщения";
      editor.dispatchEvent("keydown", { key: "Enter" });
      await app.settle(4);
      const edited = app.callsOf("edit_message");
      assert.equal(edited.length, 1, "edit_message не отправлен");
      assert.deepEqual(edited[0]?.args, {
        channelId: CHANNEL_WITH_MESSAGES,
        messageId: oracle[0]?.id,
        content: "правка реального сообщения",
      }, "edit_message с индексом вместо реального id");
      assert.match(rowAt(0).textContent ?? "", /правка реального сообщения/, "правка не видна в строке");

      // delete: реальный id последнего сообщения.
      list.dispatchEvent("mouseover", { target: rowAt(2) });
      const remove = all(app, ".msg-toolbar button").find(
        (button) => button.getAttribute("aria-label") === "Удалить сообщение",
      );
      assert.ok(remove, "в панели нет «Удалить сообщение»");
      remove.click();
      remove.click();
      await app.settle(4);
      const deleted = app.callsOf("delete_message");
      assert.equal(deleted.length, 1, "delete_message не отправлен");
      assert.deepEqual(deleted[0]?.args, {
        channelId: CHANNEL_WITH_MESSAGES,
        messageId: oracle[2]?.id,
      }, "delete_message с индексом вместо реального id");
      assert.equal(
        all(app, "#chat-list .row[data-message-id]").length,
        oracle.length - 1,
        "удалённая строка осталась в реальном канале",
      );

      // reply: replyTo — реальный id сообщения.
      list.dispatchEvent("mouseover", { target: rowAt(1) });
      const reply = all(app, ".msg-toolbar button").find(
        (button) => button.getAttribute("data-command") === "reply",
      );
      assert.ok(reply, "в панели нет ответа");
      reply.click();
      input.value = "ответ в реальном канале";
      input.emit("input");
      input.dispatchEvent("keydown", { key: "Enter" });
      await app.settle(4);
      const replied = app.callsOf("reply_message");
      assert.equal(replied.length, 1, "reply_message не отправлен");
      assert.deepEqual(replied[0]?.args, {
        channelId: CHANNEL_WITH_MESSAGES,
        content: "ответ в реальном канале",
        replyTo: oracle[1]?.id,
      }, "reply_message с индексом вместо реального id");

      // Ни один live-запрос не ушёл в «news»/индекс.
      for (const call of [...sent, ...edited, ...deleted, ...replied]) {
        assert.notEqual(call.args?.channelId, "news", `${call.cmd}: канал «news»`);
        if ("messageId" in (call.args ?? {})) {
          assert.ok(
            oracle.some((message) => message.id === call.args.messageId),
            `${call.cmd}: messageId не из реальной истории`,
          );
        }
      }
    } finally {
      app.dispose();
    }
  });
});

describe("C-02-UI: состояния реального канала", () => {
  it("loading (aria-busy, role=status) → реальные строки, без синтетики", async () => {
    /** @type {(value: any) => void} */
    let resolvePage = () => {};
    const pending = new Promise((resolve) => { resolvePage = resolve; });
    const app = await loadSwitchApp({
      messages: (channelId) => (channelId === CHANNEL_WITH_MESSAGES ? pending : messagesPagePayload(channelId)),
    });
    try {
      await selectGuild(app, GUILD_WITH_MESSAGES);
      channelButton(app, CHANNEL_WITH_MESSAGES).click();
      await app.settle(6);
      const list = firstNode(app, "#chat-list");
      assert.equal(list.getAttribute("aria-busy"), "true", "загрузка истории не помечена aria-busy");
      assert.equal(
        firstNode(app, "#chat-list [data-state]").getAttribute("data-state"),
        "loading",
        "нет видимого loading-состояния",
      );
      assert.equal(
        firstNode(app, "#chat-list [data-state]").getAttribute("role"),
        "status",
        "loading не объявлен AT",
      );

      resolvePage(messagesPagePayload(CHANNEL_WITH_MESSAGES));
      await app.settle(10);
      assert.equal(list.getAttribute("aria-busy"), "false", "после ответа aria-busy не снят");
      assert.equal(
        all(app, "#chat-list .row[data-message-id]").length,
        3,
        "после ответа не отрисованы реальные строки",
      );
      assert.equal(all(app, "#chat-list [data-state]").length, 0, "loading-состояние осталось после ответа");
    } finally {
      app.dispose();
    }
  });

  it("пустой канал → empty, а не пустой список без объяснения", async () => {
    const app = await loadSwitchApp();
    try {
      await selectGuild(app, GUILD_WITH_MESSAGES);
      const emptyChannel = oracleChannels(GUILD_WITH_MESSAGES).find(
        (channel) => channel.type === 0 && oracleMessages(channel.id).length === 0,
      );
      assert.ok(emptyChannel);
      channelButton(app, emptyChannel.id).click();
      await app.settle(12);
      const state = firstNode(app, "#chat-list [data-state]");
      assert.equal(state.getAttribute("data-state"), "empty", "нет empty-состояния");
      assert.match(state.textContent ?? "", /нет сообщений|пока нет/i, "empty без текста");
      assert.equal(all(app, "#chat-list .row[data-message-id]").length, 0, "empty показал чужие строки");
    } finally {
      app.dispose();
    }
  });

  for (const failure of [
    { label: "404", error: { kind: "NotFound", message: "канал не найден" }, text: /не найден|недоступ/i },
    { label: "401", error: { status: 401, kind: "Unauthorized", message: "unauthorized" }, text: /сессия|войдите/i },
  ]) {
    it(`ошибка ${failure.label} видна как alert, а не пустой список`, async () => {
      const app = await loadSwitchApp({
        messages: (channelId) => {
          if (channelId === CHANNEL_WITH_MESSAGES) throw failure.error;
          return messagesPagePayload(channelId);
        },
      });
      try {
        await selectGuild(app, GUILD_WITH_MESSAGES);
        channelButton(app, CHANNEL_WITH_MESSAGES).click();
        await app.settle(12);
        const error = firstNode(app, "#chat-list [data-state]");
        assert.equal(error.getAttribute("data-state"), "error", `ошибка ${failure.label} не показана`);
        assert.equal(error.getAttribute("role"), "alert", "ошибка не объявлена AT");
        assert.match(error.textContent ?? "", failure.text, `текст ошибки ${failure.label} не виден`);
        assert.equal(all(app, "#chat-list .row[data-message-id]").length, 0, "при ошибке нарисованы выдуманные строки");
        assert.equal(firstNode(app, "#chat-list").getAttribute("aria-busy"), "false", "ошибка осталась busy");
        assert.equal(
          firstNode(app, ".chat-title-text").textContent,
          oracleChannels(GUILD_WITH_MESSAGES).find((channel) => channel.id === CHANNEL_WITH_MESSAGES)?.name,
          "заголовок потерялся при ошибке",
        );
      } finally {
        app.dispose();
      }
    });
  }

  it("клавиатура дерева: ↓ двигает фокус, Enter открывает канал и ставит aria-current", async () => {
    const app = await loadSwitchApp();
    try {
      await selectGuild(app, GUILD_WITH_MESSAGES);
      const openable = all(app, '#channel-tree .channel-item[data-openable="true"]');
      assert.ok(openable.length > 1, "в гильдии меньше двух открываемых каналов");
      const first = /** @type {ShimElement} */ (openable[0]);
      const second = /** @type {ShimElement} */ (openable[1]);
      first.focus();
      const down = first.dispatchEvent("keydown", { key: "ArrowDown" });
      assert.equal(down.prevented, true, "ArrowDown не перехвачен деревом");
      assert.equal(app.document.activeElement, second, "ArrowDown не перевёл фокус на следующий канал");
      second.dispatchEvent("keydown", { key: "Enter" });
      await app.settle(12);
      assert.equal(
        app.callsOf("messages_page").at(-1)?.args?.channelId,
        second.getAttribute("data-channel-id"),
        "Enter не открыл канал под фокусом",
      );
      assert.equal(second.getAttribute("aria-current"), "page", "открытый с клавиатуры канал не aria-current");
      assert.equal(first.getAttribute("aria-current"), null, "старый aria-current не снят");
    } finally {
      app.dispose();
    }
  });
});

describe("C-02-UI: ошибка channels()", () => {
  it("channels() падает → видимая ошибка дерева с retry, до победы", async () => {
    let attempts = 0;
    const app = await loadSwitchApp({
      channels: (guildId) => {
        attempts += 1;
        if (attempts === 1) throw { kind: "Parse", message: "READY без объекта d" };
        return oracleChannels(guildId);
      },
    });
    try {
      const tree = firstNode(app, "#channel-tree");
      assert.equal(tree.getAttribute("data-state"), "error", "ошибка channels() не видна в дереве");
      const error = firstNode(app, "#channel-tree [role=\"alert\"]");
      assert.match(error.textContent ?? "", /канал/i, "ошибка без объяснения");
      assert.equal(all(app, "#channel-tree .channel-item").length, 0, "при ошибке показаны выдуманные каналы");

      firstNode(app, "#channel-tree .channel-tree-retry").click();
      await app.settle(12);
      assert.equal(attempts, 2, "retry не повторил channels()");
      assert.equal(tree.getAttribute("data-state"), "ready", "после retry дерево не ready");
      assert.equal(
        all(app, "#channel-tree .channel-item[data-channel-id]").length,
        oracleChannels(GUILDS[0]?.id ?? "").length,
        "после retry не все каналы",
      );
    } finally {
      app.dispose();
    }
  });
});
