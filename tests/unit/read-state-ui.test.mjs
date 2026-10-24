// @ts-check
/**
 * M-07-UI (RW-40): ack/read-state в UI.
 *
 * Оракул — реальные артефакты, не реализация:
 *   - `fixtures/gateway/by-type/002-READY.json`:
 *     `d.read_state.entries[] = {id, flags, last_message_id, last_pin_timestamp,
 *     last_viewed, mention_count}`; канал `1163408591315812392` (гильдия
 *     `1014537851418980372`) — `last_viewed: 4287`, `mention_count: 0`,
 *     `last_message_id: 1553370750381990038`;
 *   - `waves/RW-40/PARITY/G6-REPORT-RW40-02.md` §3.5: IPC
 *     `ack_message {channelId, messageId, lastViewed, token?}`, тело запроса
 *     дословно `{token, last_viewed}`, `last_viewed` — **число-счётчик**, не
 *     snowflake; `token` nullable и никогда не bearer;
 *   - `waves/RW-40/PM/m07-be2/REPORT.md`: `read_state()` отдаёт camelCase
 *     `{channelId, flags, lastMessageId, lastPinTimestamp, lastViewed,
 *     mentionCount}`; отсутствующее/`null` поле — `null`, **не** 0;
 *   - политика триггера `[A]` — `CONTRACTS/read-state.md` (open/focus/scroll
 *     в низ, троттлинг, без ack на синтетическом бенче).
 *
 * Шим не всплывает события: слушатели адресуются самому `#chat-list`, как в
 * `actions.js`. Часы шима двигаются только кадрами rAF (`settle`).
 */
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { loadApp } from "../support/gui-dom-harness.mjs";
import * as chatApi from "../../rust/gui-spike/ui/features/chat.js";

/** @typedef {import("../support/gui-dom-harness.mjs").GuiDom} GuiDom */
/** @typedef {import("../support/gui-dom-harness.mjs").ShimElement} ShimElement */

const fixturesDir = fileURLToPath(new URL("../../fixtures/gateway/by-type/", import.meta.url));
const READY = JSON.parse(readFileSync(`${fixturesDir}002-READY.json`, "utf8"));

/** Реальный текстовый канал с read_state и сообщениями в фикстурах. */
const CHANNEL = "1163408591315812392";
/** Гильдия этого канала (индекс 5 в READY). */
const GUILD = "1014537851418980372";
/** Запись `read_state` канала из READY — независимый оракул курсора. */
const READY_ENTRY = (READY?.d?.read_state?.entries ?? []).find(
  (/** @type {any} */ entry) => String(entry.id) === CHANNEL,
);

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

/**
 * Каналы гильдии по READY (порядок `position` asc, затем `id` asc).
 * @param {string} guildId
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
 * Сообщения канала из MESSAGE_CREATE (порядок `timestamp`, затем `id`).
 * @param {string} channelId
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
  return [...byId.values()].sort((left, right) => {
    if (left.timestamp !== right.timestamp) return left.timestamp < right.timestamp ? -1 : 1;
    return left.id < right.id ? -1 : left.id > right.id ? 1 : 0;
  });
}

/**
 * `messages_page`-ответ в форме `MessagesPage` (camelCase).
 * @param {string} channelId
 */
function messagesPagePayload(channelId) {
  const rows = oracleMessages(channelId);
  /** @type {string | null} */
  let previousAuthor = null;
  /** @type {string | null} */
  let previousDay = null;
  return {
    channelId,
    items: rows.map((row) => {
      const day = row.timestamp.slice(0, 10);
      const group = previousAuthor === row.author && previousDay === day ? "continuation" : "start";
      previousAuthor = row.author;
      previousDay = day;
      return {
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
        day,
        group,
        pinned: false,
        messageType: 0,
        embeds: [],
        attachments: [],
        reactions: [],
        replyTo: null,
        mentions: [],
        mentionEveryone: false,
      };
    }),
    nextCursor: null,
    hasMore: false,
    total: rows.length,
    limit: 100,
    loadedRowsLimit: 500,
  };
}

/**
 * `read_state()` в camelCase-проекции backend'а (`m07-be2`): `null` остаётся
 * `null`, порядок и состав записей — как в READY.
 * @returns {any[]}
 */
function readStatePayload() {
  return (READY?.d?.read_state?.entries ?? []).map((/** @type {any} */ entry) => ({
    channelId: String(entry.id),
    flags: entry.flags ?? null,
    lastMessageId: entry.last_message_id ?? null,
    lastPinTimestamp: entry.last_pin_timestamp ?? null,
    lastViewed: entry.last_viewed ?? null,
    mentionCount: entry.mention_count ?? null,
  }));
}

/** `ready()` в форме `data.rs ReadyPayload` (для рейла и заголовков). */
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
 * Приложение с управляемым `read_state`/`ack_message`.
 * @param {{
 *   readState?: any[],
 *   readStateError?: unknown,
 *   ack?: (args: any) => any,
 * }} [options]
 */
async function loadReadStateApp(options = {}) {
  const state = {
    readState: options.readState ?? readStatePayload(),
    readStateCalls: 0,
    ackCalls: [],
  };
  const app = await loadApp({
    invokeHandler: async (cmd, args) => {
      if (cmd === "auth_status") {
        return { status: "ready", stored: true, user: { id: "42", username: "tester" } };
      }
      if (cmd === "ready") return readyPayload();
      if (cmd === "channels") {
        return oracleChannels(String(args?.guildId ?? ""));
      }
      if (cmd === "messages_page") {
        return messagesPagePayload(String(args?.channelId ?? ""));
      }
      if (cmd === "read_state") {
        state.readStateCalls += 1;
        if (options.readStateError !== undefined) throw options.readStateError;
        return state.readState;
      }
      if (cmd === "ack_message") {
        /** @type {any[]} */ (state.ackCalls).push(args);
        return options.ack ? options.ack(args) : null;
      }
      if (cmd === "send_message") {
        return { id: "1561627999999999999", channel_id: args?.channelId, content: args?.content };
      }
      if (cmd === "edit_message" || cmd === "delete_message" || cmd === "reply_message") {
        return null;
      }
      return undefined;
    },
  });
  return { app, state };
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

/** @param {GuiDom} app */
function unreadBand(app) {
  return firstNode(app, ".chat-unread-divider");
}

/** @param {GuiDom} app @param {string} guildId */
async function selectGuild(app, guildId) {
  const tile = app.document.querySelectorAll(".guild-item").find(
    (node) => node.getAttribute("data-guild-id") === guildId,
  );
  assert.ok(tile, `в рейле нет гильдии ${guildId}`);
  tile.click();
  await app.settle(12);
}

/** @param {GuiDom} app @param {string} channelId */
function channelTile(app, channelId) {
  const node = app.document.querySelectorAll(".channel-item").find(
    (item) => item.getAttribute("data-channel-id") === channelId,
  );
  assert.ok(node, `в дереве нет канала ${channelId}`);
  return node;
}

/**
 * Открыть реальный канал кликом по дереву и дождаться IPC.
 * @param {GuiDom} app
 */
async function openChannelByClick(app) {
  await selectGuild(app, GUILD);
  channelTile(app, CHANNEL).click();
  await app.settle(12);
}

/**
 * «Скролл в низ» в шиме: у строк канала нет inline-высот, поэтому контент
 * задаётся тестом через `style.height` первой строки.
 * @param {GuiDom} app
 */
async function scrollToBottom(app) {
  const list = firstNode(app, "#chat-list");
  list.boxHeight = 400;
  const first = list.children[0];
  if (first) first.style.height = "1200px";
  list.scrollTop = 800;
  list.emit("scroll");
  await app.settle(12);
}

/** Кадров rAF, заведомо больше окна троттлинга (≈16.7 мс на кадр). */
const THROTTLE_FRAMES = 220;

describe("M-07-UI: read-state/ack в реальном канале", () => {
  it("триггер ack зафиксирован как [A] и окно троттлинга — именованная константа", () => {
    assert.equal(
      /** @type {any} */ (chatApi).ACK_THROTTLE_MS,
      3_000,
      "ACK_THROTTLE_MS — политика [A] (CONTRACTS/read-state.md), а не литерал в вызове",
    );
  });

  it("открытие канала → ровно один ack_message с lastViewed из READY (число, не snowflake)", async () => {
    assert.ok(READY_ENTRY, "в READY нет read_state канала-оракула");
    const { app, state } = await loadReadStateApp();
    try {
      await openChannelByClick(app);
      const calls = app.callsOf("ack_message");
      assert.equal(calls.length, 1, `ack_message вызван ${calls.length} раз вместо одного`);
      const args = /** @type {any} */ (calls[0]?.args ?? {});
      assert.deepEqual(
        Object.keys(args).sort(),
        ["channelId", "lastViewed", "messageId", "token"],
        "аргументы ack не совпадают с контрактом IPC",
      );
      assert.equal(args.channelId, CHANNEL, "ack ушёл не в открытый канал");
      assert.equal(
        args.messageId,
        String(READY_ENTRY.last_message_id),
        "messageId не из READY.last_message_id",
      );
      assert.equal(
        args.lastViewed,
        Number(READY_ENTRY.last_viewed),
        "lastViewed не из READY.last_viewed",
      );
      assert.equal(typeof args.lastViewed, "number", "lastViewed обязан быть числом, не snowflake-строкой");
      assert.ok(
        String(args.lastViewed) !== String(args.messageId),
        "lastViewed подменён id сообщения",
      );
      // Контракт §3.5: token nullable и никогда не bearer-токен.
      assert.equal(args.token, null, "token в args должен быть null");
      for (const [key, value] of Object.entries(args)) {
        assert.ok(
          value === null || typeof value === "string" || typeof value === "number",
          `неожиданный тип аргумента ${key}`,
        );
      }
      assert.doesNotMatch(
        JSON.stringify(args),
        /token"\s*:\s*"[^"]+"/,
        "в ack-args попал непустой токен",
      );
      assert.ok(state.readStateCalls >= 1, "lastViewed взят не из read_state()");
    } finally {
      app.dispose();
    }
  });

  it("null/badge-запись read_state не даёт ack и не превращается в 0", async () => {
    const badge = [{
      channelId: CHANNEL,
      flags: null,
      lastMessageId: null,
      lastPinTimestamp: null,
      lastViewed: null,
      mentionCount: null,
    }];
    const { app } = await loadReadStateApp({ readState: badge });
    try {
      await openChannelByClick(app);
      assert.equal(app.callsOf("ack_message").length, 0, "ack без числового lastViewed — выдумка");
      const band = unreadBand(app);
      assert.equal(band.getAttribute("data-source"), "read-state", "полоса не переведена в режим read-state");
      assert.ok(band.hasAttribute("hidden"), "полоса видима без источника данных");
      assert.equal(band.getAttribute("data-mentions"), null, "badge/nullable превращён в счётчик");
      assert.doesNotMatch(band.textContent, /\b0\b/, "null превращён в 0");
    } finally {
      app.dispose();
    }
  });

  it("троттлинг: фокус/скролл в низ не дублируют ack, новый курсор — только после окна", async () => {
    const { app, state } = await loadReadStateApp();
    try {
      await openChannelByClick(app);
      assert.equal(app.callsOf("ack_message").length, 1, "open не дал ровно один ack");

      // Курсор вырос в READY, но окно троттлинга ещё не прошло.
      state.readState = state.readState.map((entry) => (
        entry.channelId === CHANNEL ? { ...entry, lastViewed: 4290 } : entry
      ));
      const list = firstNode(app, "#chat-list");
      list.emit("focus");
      list.emit("focusin");
      await app.settle(2);
      assert.equal(
        app.callsOf("ack_message").length,
        1,
        "троттлинг не удержал повторный ack внутри окна",
      );

      // Окно прошло: скролл в низ отправляет ровно один ack с новым курсором.
      await app.settle(THROTTLE_FRAMES);
      await scrollToBottom(app);
      const calls = app.callsOf("ack_message");
      assert.equal(calls.length, 2, `после окна ack не отправлен (${calls.length})`);
      assert.equal(/** @type {any} */ (calls[1]?.args)?.lastViewed, 4290, "новый курсор не из READY");

      // Тот же курсор повторно не дублируется ни фокусом, ни скроллом.
      await app.settle(THROTTLE_FRAMES);
      list.emit("focus");
      list.emit("scroll");
      await app.settle(2);
      assert.equal(app.callsOf("ack_message").length, 2, "повтор с тем же lastViewed продублирован");
    } finally {
      app.dispose();
    }
  });

  it("unread/mention-полоса: цитата из READY и сброс после успешного ack", async () => {
    const mention = [{
      channelId: CHANNEL,
      flags: 1,
      lastMessageId: String(READY_ENTRY.last_message_id),
      lastPinTimestamp: String(READY_ENTRY.last_pin_timestamp),
      lastViewed: Number(READY_ENTRY.last_viewed),
      mentionCount: 5,
    }];
    // Медленный сервер: полоса видна до ответа ack, затем сбрасывается.
    /** @type {(value: any) => void} */
    let resolveAck = () => {};
    const pendingAck = new Promise((resolve) => { resolveAck = resolve; });
    const { app } = await loadReadStateApp({ readState: mention, ack: () => pendingAck });
    try {
      await selectGuild(app, GUILD);
      const tile = channelTile(app, CHANNEL);
      tile.classList.add("is-unread");

      tile.click();
      await app.settle(12);
      const band = unreadBand(app);
      assert.equal(band.getAttribute("data-source"), "read-state", "полоса не из read_state");
      assert.equal(band.getAttribute("data-mentions"), "5", "mentionCount не из READY");
      assert.equal(band.hasAttribute("hidden"), false, "полоса упоминаний скрыта");
      assert.notEqual(band.style.display, "none", "полоса упоминаний не показана");
      assert.match(band.textContent, /5/, "в полосе нет честного счётчика упоминаний");
      assert.equal(app.callsOf("ack_message").length, 1, "ack при открытии не отправлен");
      assert.equal(tile.classList.contains("is-unread"), true, "точка сброшена до ответа ack");

      // Успешный ack — полоса и точка в дереве сбрасываются.
      resolveAck(null);
      await app.settle(8);
      assert.ok(band.hasAttribute("hidden"), "полоса не сброшена после ack");
      assert.equal(band.style.display, "none", "полоса осталась видимой после ack");
      assert.equal(band.getAttribute("data-cleared"), "true", "сброс не помечен");
      assert.equal(band.getAttribute("data-mentions"), null, "счётчик упоминаний остался после сброса");
      assert.equal(tile.classList.contains("is-unread"), false, "точка непрочитанного осталась в дереве");
    } finally {
      app.dispose();
    }
  });

  it("401 Unauthorized: ошибка ack видима, без цикла и без дублей", async () => {
    const unauthorized = Object.assign(new Error("401: Unauthorized"), {
      kind: "Unauthorized",
      status: 401,
    });
    const { app } = await loadReadStateApp({ ack: () => Promise.reject(unauthorized) });
    try {
      await openChannelByClick(app);
      const calls = app.callsOf("ack_message");
      assert.equal(calls.length, 1, "ack не отправлен");
      const note = firstNode(app, ".chat-ack-note");
      assert.equal(note.getAttribute("role"), "alert", "ошибка ack не объявлена как alert");
      assert.equal(note.getAttribute("data-state"), "error", "нет видимого состояния ошибки");
      assert.equal(note.getAttribute("data-kind"), "Unauthorized", "тип ошибки не разобран");
      assert.match(note.textContent, /сесси/i, "401 не объяснён пользователю");

      // Без цикла: фокус/скролл после окна не повторяют неудачный ack.
      await app.settle(THROTTLE_FRAMES);
      const list = firstNode(app, "#chat-list");
      list.emit("focus");
      await scrollToBottom(app);
      assert.equal(app.callsOf("ack_message").length, 1, "неудачный ack повторяется в цикле");
    } finally {
      app.dispose();
    }
  });

  it("429 RateLimited: ошибка ack видима и не повторяется", async () => {
    const limited = Object.assign(new Error("429: Too Many Requests"), {
      kind: "RateLimited",
      status: 429,
      retry_after_ms: 5_000,
    });
    const { app } = await loadReadStateApp({ ack: () => Promise.reject(limited) });
    try {
      await openChannelByClick(app);
      assert.equal(app.callsOf("ack_message").length, 1, "ack не отправлен");
      const note = firstNode(app, ".chat-ack-note");
      assert.equal(note.getAttribute("data-kind"), "RateLimited", "429 не разобран");
      assert.match(note.textContent, /част|позже/i, "429 не объяснён пользователю");
      await app.settle(THROTTLE_FRAMES);
      firstNode(app, "#chat-list").emit("focus");
      assert.equal(app.callsOf("ack_message").length, 1, "429 повторяется в цикле");
    } finally {
      app.dispose();
    }
  });

  it("синтетический бенч (CHAT_ROWS) не ack'ает: ни read_state, ни ack_message", async () => {
    const { app, state } = await loadReadStateApp();
    try {
      const list = firstNode(app, "#chat-list");
      list.boxHeight = 400;
      const sizer = list.children[0];
      if (sizer) sizer.style.height = "1200px";
      list.scrollTop = 800;
      list.emit("scroll");
      list.emit("focus");
      list.emit("focusin");
      await app.settle(12);
      assert.equal(app.callsOf("ack_message").length, 0, "ack ушёл из синтетического бенча");
      assert.equal(app.callsOf("read_state").length, 0, "read_state запрошен на бенче");
      assert.equal(state.readStateCalls, 0, "read_state вызван на бенче");
    } finally {
      app.dispose();
    }
  });
});
