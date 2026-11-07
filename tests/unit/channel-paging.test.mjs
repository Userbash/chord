// @ts-check
/**
 * C-02-PAGE + C-02-UI-OLDER (RW-40): пагинация истории канала в обе стороны.
 *
 * Оракул — реальные артефакты, не реализация:
 *   - `fixtures/gateway/by-type/002-READY.json` — гильдии/каналы (имя канала,
 *     принадлежность гильдии);
 *   - `fixtures/gateway/by-type/0{23,24,25}-MESSAGE_CREATE.json` — реальные
 *     snowflake id, текст и timestamp канала `1163408591315812392`;
 *   - `rust/gui-spike/src/data.rs` (`MessagesPageRequest`, `messages_window`) и
 *     `waves/RW-40/PM/c02-before/REPORT.md` §1.2:
 *     `position:"latest"` — хвост канала; `before` — страница строго старше
 *     якоря; `hasOlder`/`olderCursor` — отдельная пара для верха; `cursor` —
 *     forward, строки строго после якоря, `hasMore`/`nextCursor` — отдельная
 *     пара для низа; `limit` 1..=100; `loadedRowsLimit` = 500 (окно UI ≤ 500).
 *
 * Направления курсоров **не смешиваются**: страница по `before` тоже возвращает
 * `nextCursor`/`hasMore` (forward-метаданные своего окна, C-02-BEFORE §6.2), и UI
 * обязан хранить `olderCursor` отдельно, иначе «вверх» превратится в «вниз».
 *
 * Шим не считает layout (`scrollHeight` = высота контейнера), поэтому пиксельный
 * сдвиг при вставке строк сверху здесь не измерим: проверяется сохранение
 * якорного узла/позиции и отсутствие сброса `scrollTop` на 0.
 */
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { loadApp } from "../support/gui-dom-harness.mjs";
import { createPinsThreadsPanels } from "../../rust/gui-spike/ui/features/pins-threads.js";

/** @typedef {import("../support/gui-dom-harness.mjs").GuiDom} GuiDom */
/** @typedef {import("../support/gui-dom-harness.mjs").ShimElement} ShimElement */

const fixturesDir = fileURLToPath(new URL("../../fixtures/gateway/by-type/", import.meta.url));
const READY = JSON.parse(readFileSync(`${fixturesDir}002-READY.json`, "utf8"));

/** Гильдии READY в порядке рейла. @type {Array<{ id: string, name: string }>} */
const GUILDS = (READY?.d?.guilds ?? []).map((/** @type {any} */ guild) => ({
  id: String(guild.id),
  name: String(guild?.properties?.name ?? ""),
}));

/** Кадры `MESSAGE_*` — независимый источник реальных сообщений. @type {any[]} */
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

/** Реальный канал с сообщениями в фикстурах. */
const CHANNEL = "1163408591315812392";
/** Гильдия этого канала. */
const GUILD = "1014537851418980372";

/**
 * История канала в порядке `data.rs` (`timestamp`, затем `id`) — оракул.
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

const HISTORY = oracleMessages(CHANNEL);

/**
 * Элемент `MessagesPage.items[]` (camelCase, как `MessageRow` в `data.rs`).
 * @param {string} channelId
 * @param {{ id: string, content: string, author: string, timestamp: string, group: string }} row
 */
function itemOf(channelId, row) {
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
  };
}

/**
 * Ответ `messages_page` (`data.rs MessagesPage`) из подмножества оракула.
 * @param {string} channelId
 * @param {Array<{ id: string, content: string, author: string, timestamp: string, group: string }>} rows
 * @param {{ hasMore: boolean, nextCursor: string | null, hasOlder?: boolean, olderCursor?: string | null, total?: number, limit?: number }} meta
 */
function pagePayload(channelId, rows, meta) {
  return {
    channelId,
    items: rows.map((row) => itemOf(channelId, row)),
    nextCursor: meta.nextCursor,
    hasMore: meta.hasMore,
    hasOlder: meta.hasOlder ?? false,
    olderCursor: meta.olderCursor ?? null,
    total: meta.total ?? HISTORY.length,
    limit: meta.limit ?? 100,
    loadedRowsLimit: 500,
  };
}

/**
 * Первая страница: первые две реальные строки канала, остаток — за курсором.
 * Мок отвечает не хвостом, а страницей с начала: forward-ветка обязана работать
 * независимо от того, какую страницу вернул backend (C-02-PAGE).
 * @param {string} channelId
 */
function firstPage(channelId) {
  return pagePayload(channelId, HISTORY.slice(0, 2), {
    hasMore: true,
    nextCursor: HISTORY[1]?.id ?? null,
  });
}

/**
 * Хвост канала (`position:"latest"`): последние две строки, ниже ничего нет,
 * выше — есть (`hasOlder`), `olderCursor` — первая строка хвоста.
 * @param {string} channelId
 */
function tailPage(channelId) {
  const rows = HISTORY.slice(-2);
  return pagePayload(channelId, rows, {
    hasMore: false,
    nextCursor: null,
    hasOlder: true,
    olderCursor: rows[0]?.id ?? null,
  });
}

/**
 * Страница по `before = olderCursor` хвоста: строки старше якоря + повтор
 * граничной строки (дедуп не должен создать дубль).
 * @param {string} channelId
 */
function olderPage(channelId) {
  const older = HISTORY.at(0);
  const boundary = HISTORY.at(1);
  assert.ok(older && boundary, "HISTORY: ожидались первые две строки");
  return pagePayload(channelId, [older, boundary], {
    hasMore: false,
    nextCursor: null,
    hasOlder: false,
    olderCursor: null,
  });
}

/**
 * Вторая страница: перекрытие с первой (дедуп) + последняя строка, конец истории.
 * @param {string} channelId
 */
function secondPage(channelId) {
  const rows = [HISTORY[1], HISTORY[2]].map((row) => {
    assert.ok(row, "HISTORY: ожидалась вторая/третья строка");
    return row;
  });
  return pagePayload(channelId, rows, {
    hasMore: false,
    nextCursor: null,
  });
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
 * Каналы гильдии из READY (порядок backend: `position`, затем `id`).
 * @param {string} guildId
 * @returns {any[]}
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
 * Приложение с управляемым `messages_page`.
 * @param {(channelId: string, args: any) => any} messages
 * @returns {Promise<GuiDom>}
 */
function loadPagingApp(messages) {
  return loadApp({
    invokeHandler: async (cmd, args) => {
      if (cmd === "auth_status") {
        return { status: "ready", stored: true, user: { id: "42", username: "tester" } };
      }
      if (cmd === "ready") return readyPayload();
      if (cmd === "channels") return oracleChannels(String(args?.guildId ?? ""));
      if (cmd === "messages_page") return messages(String(args?.channelId ?? ""), args);
      if (cmd === "read_state") return [];
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

/**
 * @param {GuiDom} app
 * @param {string} channelId
 */
async function openChannel(app, channelId) {
  const tile = all(app, ".channel-item").find(
    (node) => node.getAttribute("data-channel-id") === channelId,
  );
  assert.ok(tile, `в дереве нет канала ${channelId}`);
  tile.click();
  await app.settle(12);
}

/**
 * Скролл списка до нижнего края: подгрузка следующей страницы.
 * @param {GuiDom} app
 * @param {number} [scrollTop]
 */
async function scrollToEdge(app, scrollTop = 120) {
  const list = firstNode(app, "#chat-list");
  list.scrollTop = scrollTop;
  list.emit("scroll");
  await app.settle(10);
}

/**
 * Скролл списка до верхнего края: подгрузка более ранней истории.
 * @param {GuiDom} app
 * @param {number} [scrollTop]
 */
async function scrollToTop(app, scrollTop = 0) {
  const list = firstNode(app, "#chat-list");
  list.scrollTop = scrollTop;
  list.emit("scroll");
  await app.settle(10);
}

/**
 * Id строк истории в DOM-порядке.
 * @param {GuiDom} app
 */
function rowIds(app) {
  return all(app, "#chat-list .row[data-message-id]").map(
    (row) => row.getAttribute("data-message-id"),
  );
}

/**
 * Состояние верхней полосы истории (`#chat-list .chat-history-older`).
 * @param {GuiDom} app
 * @returns {ShimElement}
 */
function olderBar(app) {
  return firstNode(app, "#chat-list .chat-history-older");
}

describe("C-02-PAGE: первая страница", () => {
  it("открытие канала — position=latest, limit=100, без cursor/before/token", async () => {
    const app = await loadPagingApp((channelId) => firstPage(channelId));
    try {
      await selectGuild(app, GUILD);
      await openChannel(app, CHANNEL);
      const calls = app.callsOf("messages_page");
      assert.equal(calls.length, 1, "открытие канала не сделало ровно один запрос истории");
      assert.deepEqual(
        calls[0]?.args,
        { channelId: CHANNEL, position: "latest", limit: 100 },
        "первая страница ушла не как хвост канала (position=latest, limit=100)",
      );
      assert.equal("cursor" in (calls[0]?.args ?? {}), false, "первая страница отправила forward-cursor");
      assert.equal("before" in (calls[0]?.args ?? {}), false, "первая страница отправила older-курсор");
      assert.equal("token" in (calls[0]?.args ?? {}), false, "messages_page несёт token");
      assert.deepEqual(rowIds(app), HISTORY.slice(0, 2).map((row) => row.id), "строки не из первой страницы");
      assert.equal(
        firstNode(app, "#chat-list .chat-history-more").getAttribute("data-page-state"),
        "idle",
        "при hasMore=true нет доступной подгрузки",
      );
      assert.equal(app.snapshot.payload?.chatRows, 100_000, "CHAT_ROWS бенча ослаблен");
    } finally {
      app.dispose();
    }
  });
});

describe("C-02-PAGE: nextCursor / hasMore / дедуп", () => {
  it("скролл к краю грузит следующую страницу, дедуп по id, hasMore=false останавливает", async () => {
    const app = await loadPagingApp((channelId, args) => (
      args?.cursor === HISTORY[1]?.id ? secondPage(channelId) : firstPage(channelId)
    ));
    try {
      await selectGuild(app, GUILD);
      await openChannel(app, CHANNEL);
      await scrollToEdge(app);

      const calls = app.callsOf("messages_page");
      assert.equal(calls.length, 2, "подгрузка не сделала второй запрос истории");
      assert.deepEqual(calls[1]?.args, {
        channelId: CHANNEL,
        cursor: HISTORY[1]?.id,
        limit: 100,
      }, "вторая страница ушла не по nextCursor первой");

      const ids = rowIds(app);
      assert.deepEqual(ids, HISTORY.map((row) => row.id), "страницы склеены не в порядке backend");
      assert.equal(new Set(ids).size, ids.length, "подгрузка задвоила строки (нет дедупа по id)");
      assert.equal(
        firstNode(app, "#chat-list .chat-history-more").getAttribute("data-page-state"),
        "end",
        "hasMore=false не остановил пагинацию видимо",
      );
      assert.equal(firstNode(app, "#chat-list").getAttribute("aria-busy"), "false", "после подгрузки остался busy");

      // hasMore=false: повторные скроллы не шлют новые запросы.
      await scrollToEdge(app);
      await scrollToEdge(app);
      assert.equal(app.callsOf("messages_page").length, 2, "hasMore=false не остановил запросы");
    } finally {
      app.dispose();
    }
  });

  it("позиция и якорный узел сохраняются при подгрузке", async () => {
    const app = await loadPagingApp((channelId, args) => (
      args?.cursor === HISTORY[1]?.id ? secondPage(channelId) : firstPage(channelId)
    ));
    try {
      await selectGuild(app, GUILD);
      await openChannel(app, CHANNEL);
      const list = firstNode(app, "#chat-list");
      const anchor = all(app, "#chat-list .row[data-message-id]")[0];
      assert.ok(anchor, "в первой странице нет строк");
      const anchorId = anchor.getAttribute("data-message-id");
      list.scrollTop = 120;

      await scrollToEdge(app, 120);

      assert.equal(list.scrollTop, 120, "подгрузка сбросила позицию скролла");
      assert.equal(
        all(app, "#chat-list .row[data-message-id]")[0],
        anchor,
        "якорная строка пересоздана/потеряна при подгрузке",
      );
      assert.equal(rowIds(app)[0], anchorId, "якорь сместился на другое сообщение");
    } finally {
      app.dispose();
    }
  });

  it("cursor без новых строк и тот же nextCursor не крутит запросы", async () => {
    const app = await loadPagingApp((channelId, args) => {
      if (args?.cursor !== HISTORY[1]?.id) return firstPage(channelId);
      // Битый backend-ответ: hasMore=true и тот же курсор, строки только дубли.
      const anchor = HISTORY[1];
      assert.ok(anchor, "HISTORY[1] отсутствует");
      return pagePayload(channelId, [anchor], {
        hasMore: true,
        nextCursor: anchor.id,
      });
    });
    try {
      await selectGuild(app, GUILD);
      await openChannel(app, CHANNEL);
      await scrollToEdge(app);
      assert.equal(app.callsOf("messages_page").length, 2, "подгрузка без новых строк не остановилась");
      assert.deepEqual(rowIds(app), HISTORY.slice(0, 2).map((row) => row.id), "дубль попал в список");
      await scrollToEdge(app);
      await scrollToEdge(app);
      assert.equal(app.callsOf("messages_page").length, 2, "неподвижный nextCursor вызвал цикл запросов");
    } finally {
      app.dispose();
    }
  });
});

describe("C-02-PAGE: ошибки подгрузки видны", () => {
  for (const failure of [
    { label: "404", error: { kind: "NotFound", message: "курсор не найден" }, text: /не найден|недоступ/i },
    { label: "401", error: { status: 401, kind: "Unauthorized", message: "unauthorized" }, text: /сессия|войдите/i },
  ]) {
    it(`ошибка ${failure.label} второй страницы видна, строки первой не пропадают, retry работает`, async () => {
      let failed = false;
      const app = await loadPagingApp((channelId, args) => {
        if (args?.cursor !== HISTORY[1]?.id) return firstPage(channelId);
        if (!failed) {
          failed = true;
          throw failure.error;
        }
        return secondPage(channelId);
      });
      try {
        await selectGuild(app, GUILD);
        await openChannel(app, CHANNEL);
        await scrollToEdge(app);

        const footer = firstNode(app, "#chat-list .chat-history-more");
        assert.equal(footer.getAttribute("data-page-state"), "error", `ошибка ${failure.label} не видна`);
        assert.equal(footer.getAttribute("role"), "alert", "ошибка подгрузки не объявлена AT");
        assert.match(footer.textContent ?? "", failure.text, `текст ошибки ${failure.label} не виден`);
        assert.deepEqual(rowIds(app), HISTORY.slice(0, 2).map((row) => row.id), "при ошибке потеряны строки первой страницы");
        assert.equal(firstNode(app, "#chat-list").getAttribute("aria-busy"), "false", "ошибка оставила список busy");

        const retry = footer.querySelector(".chat-history-more-retry");
        assert.ok(retry, "у ошибки подгрузки нет повтора");
        retry.click();
        await app.settle(10);
        assert.deepEqual(rowIds(app), HISTORY.map((row) => row.id), "повтор не догрузил страницу");
        assert.equal(
          firstNode(app, "#chat-list .chat-history-more").getAttribute("data-page-state"),
          "end",
          "после успешного повтора hasMore=false не виден",
        );
      } finally {
        app.dispose();
      }
    });
  }
});

describe("C-02-PAGE: границы окна", () => {
  it("бенч-режим не грузит историю: CHAT_ROWS=100_000 остаётся", async () => {
    const app = await loadPagingApp((channelId) => firstPage(channelId));
    try {
      await scrollToEdge(app);
      assert.equal(app.callsOf("messages_page").length, 0, "скролл бенч-окна запросил историю канала");
      assert.equal(app.snapshot.payload?.chatRows, 100_000, "CHAT_ROWS бенч-окна ослаблен");
    } finally {
      app.dispose();
    }
  });

  it("окно истории не растёт сверх 500 строк (loaded_rows_limit)", async () => {
    const SYNTHETIC_TOTAL = 700;
    /**
     * Синтетическая страница 100 строк: граница окна — наша политика
     * (`data.rs UI_WINDOW_ROWS`), фикстура канала короче 500 строк.
     * @param {string} channelId
     * @param {number} page
     */
    const syntheticPage = (channelId, page) => {
      const start = page * 100;
      const rows = Array.from({ length: 100 }, (_, offset) => {
        const index = start + offset;
        return {
          id: String(7000000000000000000n + BigInt(index)),
          channelId,
          guildId: null,
          author: { id: "42", username: "tester", globalName: null, avatar: null, bot: false },
          content: `строка ${index}`,
          timestamp: "2026-10-19T07:00:00+00:00",
          edited: false,
          editedAt: null,
          day: "2026-10-19",
          group: index % 2 === 0 ? "start" : "continuation",
          pinned: false,
          messageType: 0,
          embeds: [],
          attachments: [],
          reactions: [],
          replyTo: null,
          mentions: [],
          mentionEveryone: false,
        };
      });
      return {
        channelId,
        items: rows,
        nextCursor: rows[rows.length - 1]?.id ?? null,
        hasMore: start + 100 < SYNTHETIC_TOTAL,
        total: SYNTHETIC_TOTAL,
        limit: 100,
        loadedRowsLimit: 500,
      };
    };
    let page = 0;
    const app = await loadPagingApp((channelId) => {
      const payload = syntheticPage(channelId, page);
      page += 1;
      return payload;
    });
    try {
      await selectGuild(app, GUILD);
      await openChannel(app, CHANNEL);
      for (let step = 0; step < 6; step++) await scrollToEdge(app);
      const calls = app.callsOf("messages_page").length;
      assert.ok(calls <= 5, `окно загрузило ${calls} страниц — больше 500 строк`);
      assert.equal(rowIds(app).length, Math.min(calls * 100, 500), "окно истории превысило 500 строк");
      assert.equal(
        firstNode(app, "#chat-list .chat-history-more").getAttribute("data-page-state"),
        "limit",
        "предел окна не показан видимо",
      );
    } finally {
      app.dispose();
    }
  });
});

describe("C-02-UI-OLDER: открытие на последнем сообщении", () => {
  it("хвост канала: position=latest, нижний подвал end, верхняя полоса idle", async () => {
    const app = await loadPagingApp((channelId, args) => {
      if (args?.position === "latest") return tailPage(channelId);
      throw new Error(`неожиданный запрос истории: ${JSON.stringify(args)}`);
    });
    try {
      await selectGuild(app, GUILD);
      await openChannel(app, CHANNEL);

      const calls = app.callsOf("messages_page");
      assert.equal(calls.length, 1, "открытие хвоста сделало не один запрос");
      assert.deepEqual(
        calls[0]?.args,
        { channelId: CHANNEL, position: "latest", limit: 100 },
        "открытие канала ушло не за хвостом (position=latest)",
      );
      assert.deepEqual(
        rowIds(app),
        HISTORY.slice(-2).map((row) => row.id),
        "показаны не последние сообщения канала",
      );
      assert.equal(
        firstNode(app, "#chat-list .chat-history-more").getAttribute("data-page-state"),
        "end",
        "при hasMore=false нижний подвал не остановлен",
      );
      assert.equal(
        olderBar(app).getAttribute("data-page-state"),
        "idle",
        "при hasOlder=true верхняя полоса не предлагает более ранние",
      );
      assert.match(olderBar(app).textContent ?? "", /вверх|ранн/i, "верхняя полоса без подсказки");
    } finally {
      app.dispose();
    }
  });
});

describe("C-02-UI-OLDER: скролл вверх (before/olderCursor)", () => {
  it("before=olderCursor, prepend с дедупом, scrollTop сохранён, hasOlder=false — стоп", async () => {
    /** @type {string | null} */
    let olderCursorSeen = null;
    const app = await loadPagingApp((channelId, args) => {
      if (args?.position === "latest") return tailPage(channelId);
      if (typeof args?.before === "string") {
        olderCursorSeen = args.before;
        return olderPage(channelId);
      }
      throw new Error(`неожиданный запрос истории: ${JSON.stringify(args)}`);
    });
    try {
      await selectGuild(app, GUILD);
      await openChannel(app, CHANNEL);
      const tailIds = HISTORY.slice(-2).map((row) => row.id);
      assert.deepEqual(rowIds(app), tailIds, "хвост не открылся");

      const list = firstNode(app, "#chat-list");
      list.scrollTop = 64;
      await scrollToTop(app, 64);

      const calls = app.callsOf("messages_page");
      assert.equal(calls.length, 2, "скролл вверх не запросил более раннюю историю");
      assert.equal(olderCursorSeen, HISTORY[1]?.id, "запрос ушёл не по olderCursor хвоста");
      assert.deepEqual(
        calls[1]?.args,
        { channelId: CHANNEL, before: HISTORY[1]?.id, limit: 100 },
        "older-страница ушла не как {before, limit}",
      );
      assert.equal("cursor" in (calls[1]?.args ?? {}), false, "older-страница отправила forward-cursor");

      const ids = rowIds(app);
      assert.deepEqual(ids, HISTORY.map((row) => row.id), "older-страница не встала перед хвостом");
      assert.equal(new Set(ids).size, ids.length, "older-страница задвоила строки (нет дедупа)");
      assert.equal(list.scrollTop, 64, "prepend сбросил позицию скролла");
      assert.equal(
        olderBar(app).getAttribute("data-page-state"),
        "end",
        "hasOlder=false не остановил подгрузку вверх",
      );
      assert.match(olderBar(app).textContent ?? "", /начало истории/i, "конец истории вверх не объяснён");

      // hasOlder=false: повторные скроллы вверх не шлют запросы.
      await scrollToTop(app);
      await scrollToTop(app);
      assert.equal(app.callsOf("messages_page").length, 2, "hasOlder=false не остановил запросы");
    } finally {
      app.dispose();
    }
  });

  it("направления не смешиваются: nextCursor older-страницы не подменяет forward", async () => {
    const app = await loadPagingApp((channelId, args) => {
      if (args?.position === "latest") {
        return pagePayload(channelId, HISTORY.slice(1, 3), {
          hasMore: true,
          nextCursor: HISTORY[2]?.id ?? null,
          hasOlder: true,
          olderCursor: HISTORY[1]?.id ?? null,
        });
      }
      if (args?.before === HISTORY[1]?.id) {
        // Backend отдаёт forward-метаданные и для older-страницы (C-02-BEFORE §6.2) —
        // UI не должен принять их за новый курсор «вниз».
        return pagePayload(channelId, [HISTORY[0], HISTORY[1]].map((row) => {
          assert.ok(row, "HISTORY: ожидались первые две строки");
          return row;
        }), {
          hasMore: true,
          nextCursor: HISTORY[1]?.id ?? null,
          hasOlder: false,
          olderCursor: null,
        });
      }
      if (args?.cursor === HISTORY[2]?.id) {
        return pagePayload(channelId, [], { hasMore: false, nextCursor: null });
      }
      throw new Error(`неожиданный запрос истории: ${JSON.stringify(args)}`);
    });
    try {
      await selectGuild(app, GUILD);
      await openChannel(app, CHANNEL);
      assert.deepEqual(
        rowIds(app),
        HISTORY.slice(1, 3).map((row) => row.id),
        "хвост открылся не с последних строк",
      );

      await scrollToTop(app);
      assert.deepEqual(rowIds(app), HISTORY.map((row) => row.id), "older-страница не склеилась");
      assert.equal(
        firstNode(app, "#chat-list .chat-history-more").getAttribute("data-page-state"),
        "idle",
        "forward hasMore=true потерялся после older-страницы",
      );

      await scrollToEdge(app);
      const calls = app.callsOf("messages_page");
      assert.equal(calls.length, 3, "скролл вниз не запросил forward-страницу");
      assert.deepEqual(
        calls[2]?.args,
        { channelId: CHANNEL, cursor: HISTORY[2]?.id, limit: 100 },
        "forward-курсор подменён nextCursor older-страницы",
      );
      assert.notEqual(
        calls[2]?.args?.cursor,
        HISTORY[1]?.id,
        "forward-запрос ушёл по курсору older-страницы",
      );
      assert.equal("before" in (calls[2]?.args ?? {}), false, "forward-страница отправила before");
    } finally {
      app.dispose();
    }
  });
});

describe("C-02-UI-OLDER: ошибки подгрузки вверх", () => {
  for (const failure of [
    { label: "404", error: { kind: "NotFound", message: "якорь не найден" }, text: /не найден|недоступ/i },
    { label: "401", error: { status: 401, kind: "Unauthorized", message: "unauthorized" }, text: /сессия|войдите/i },
  ]) {
    it(`ошибка ${failure.label}: alert сверху, строки целы, retry догружает`, async () => {
      let failed = false;
      const app = await loadPagingApp((channelId, args) => {
        if (args?.position === "latest") return tailPage(channelId);
        if (typeof args?.before === "string") {
          if (!failed) {
            failed = true;
            throw failure.error;
          }
          return olderPage(channelId);
        }
        throw new Error(`неожиданный запрос истории: ${JSON.stringify(args)}`);
      });
      try {
        await selectGuild(app, GUILD);
        await openChannel(app, CHANNEL);
        await scrollToTop(app);

        const bar = olderBar(app);
        assert.equal(bar.getAttribute("data-page-state"), "error", `ошибка ${failure.label} не видна`);
        assert.equal(bar.getAttribute("role"), "alert", "ошибка подгрузки вверх не объявлена AT");
        assert.match(bar.textContent ?? "", failure.text, `текст ошибки ${failure.label} не виден`);
        assert.deepEqual(
          rowIds(app),
          HISTORY.slice(-2).map((row) => row.id),
          "при ошибке потеряны строки хвоста",
        );
        assert.equal(firstNode(app, "#chat-list").getAttribute("aria-busy"), "false", "ошибка оставила список busy");

        const retry = bar.querySelector(".chat-history-older-retry");
        assert.ok(retry, "у ошибки подгрузки вверх нет повтора");
        retry.click();
        await app.settle(10);
        assert.deepEqual(rowIds(app), HISTORY.map((row) => row.id), "повтор не догрузил более раннюю страницу");
        assert.equal(
          olderBar(app).getAttribute("data-page-state"),
          "end",
          "после успешного повтора hasOlder=false не виден",
        );
      } finally {
        app.dispose();
      }
    });
  }
});

describe("C-02-UI-OLDER: границы окна", () => {
  it("окно истории не растёт сверх 500 строк при подгрузке вверх", async () => {
    const SYNTHETIC_TOTAL = 700;
    /** @param {number} index */
    const idAt = (index) => String(7000000000000000000n + BigInt(index));
    /**
     * Синтетическая строка канала: id растёт с индексом, порядок — `id`.
     * @param {string} channelId
     * @param {number} index
     */
    const syntheticRow = (channelId, index) => ({
      id: idAt(index),
      channelId,
      guildId: null,
      author: { id: "42", username: "tester", globalName: null, avatar: null, bot: false },
      content: `строка ${index}`,
      timestamp: "2026-10-19T07:00:00+00:00",
      edited: false,
      editedAt: null,
      day: "2026-10-19",
      group: "start",
      pinned: false,
      messageType: 0,
      embeds: [],
      attachments: [],
      reactions: [],
      replyTo: null,
      mentions: [],
      mentionEveryone: false,
    });
    /**
     * Ответ `MessagesPage` из подмножества синтетических строк.
     * @param {string} channelId
     * @param {number} start
     * @param {number} end
     * @param {{ hasMore: boolean, nextCursor: string | null, hasOlder: boolean, olderCursor: string | null }} meta
     */
    const slicePage = (channelId, start, end, meta) => ({
      channelId,
      items: Array.from({ length: Math.max(0, end - start) }, (_, offset) =>
        syntheticRow(channelId, start + offset)),
      total: SYNTHETIC_TOTAL,
      limit: 100,
      loadedRowsLimit: 500,
      ...meta,
    });
    const app = await loadPagingApp((channelId, args) => {
      if (args?.position === "latest") {
        return slicePage(channelId, SYNTHETIC_TOTAL - 100, SYNTHETIC_TOTAL, {
          hasMore: false,
          nextCursor: null,
          hasOlder: true,
          olderCursor: idAt(SYNTHETIC_TOTAL - 100),
        });
      }
      if (typeof args?.before !== "string") {
        throw new Error(`неожиданный запрос истории: ${JSON.stringify(args)}`);
      }
      const index = Number(BigInt(args.before) - 7000000000000000000n);
      const start = Math.max(0, index - 100);
      return slicePage(channelId, start, index, {
        hasMore: false,
        nextCursor: null,
        hasOlder: start > 0,
        olderCursor: start > 0 ? idAt(start) : null,
      });
    });
    try {
      await selectGuild(app, GUILD);
      await openChannel(app, CHANNEL);
      for (let step = 0; step < 6; step++) await scrollToTop(app);
      const calls = app.callsOf("messages_page").length;
      assert.ok(calls <= 5, `подгрузка вверх сделала ${calls} запросов — больше 500 строк`);
      const ids = rowIds(app);
      assert.equal(ids.length, Math.min(calls * 100, 500), "окно истории превысило 500 строк");
      assert.equal(new Set(ids).size, ids.length, "подгрузка вверх задвоила строки");
      assert.equal(
        olderBar(app).getAttribute("data-page-state"),
        "limit",
        "предел окна при подгрузке вверх не показан видимо",
      );
    } finally {
      app.dispose();
    }
  });
});

/* =============================================================================
 * STUB-10 (M-10 и M-11, RW-40): выпадающие панели пинов и веток в хедере чата.
 * waves/RW-40/DESIGN/PINS-THREADS-PROFILE-SPEC.md (§2 и §3).
 * ========================================================================== */

describe("STUB-10-PINS-THREADS-PANELS-UI: M-10 (pins) и M-11 (threads) выпадающие панели", () => {
  it("прогрессивное создание #chat-pins-panel и #chat-threads-panel с корректными ролями и ARIA-атрибутами", async () => {
    const app = await loadPagingApp(tailPage);
    try {
      assert.equal(app.document.getElementById("chat-pins-panel"), null);
      assert.equal(app.document.getElementById("chat-threads-panel"), null);

      const pinsBtn = app.document.querySelector('[data-action="pins"]');
      const threadsBtn = app.document.querySelector('[data-action="threads"]');
      assert.ok(pinsBtn, "кнопка pins должна присутствовать в хедере чата");
      assert.ok(threadsBtn, "кнопка threads должна присутствовать в хедере чата");

      const ctrl = createPinsThreadsPanels({
        document: app.document,
        pinsButton: pinsBtn,
        threadsButton: threadsBtn,
      });

      assert.equal(pinsBtn.getAttribute("aria-haspopup"), "dialog");
      assert.equal(pinsBtn.getAttribute("aria-expanded"), "false");
      assert.equal(pinsBtn.getAttribute("aria-controls"), "chat-pins-panel");

      assert.equal(threadsBtn.getAttribute("aria-haspopup"), "dialog");
      assert.equal(threadsBtn.getAttribute("aria-expanded"), "false");
      assert.equal(threadsBtn.getAttribute("aria-controls"), "chat-threads-panel");

      const pinsPanel = app.document.getElementById("chat-pins-panel");
      const threadsPanel = app.document.getElementById("chat-threads-panel");
      assert.ok(pinsPanel, "#chat-pins-panel должен быть создан");
      assert.ok(threadsPanel, "#chat-threads-panel должен быть создан");

      assert.equal(pinsPanel.getAttribute("role"), "dialog");
      assert.equal(pinsPanel.getAttribute("aria-label"), "Закреплённые сообщения");
      assert.equal(pinsPanel.getAttribute("aria-modal"), "false");
      assert.equal(pinsPanel.getAttribute("data-state"), "empty");
      assert.equal(pinsPanel.getAttribute("tabindex"), "-1");
      assert.ok(pinsPanel.hasAttribute("hidden"), "закрытая панель пинов скрыта");
      assert.ok(pinsPanel.querySelector('[data-action="close-pins"]'), "кнопка закрытия close-pins");
      assert.ok(pinsPanel.querySelector(".pins-content"), "контейнер .pins-content");

      assert.equal(threadsPanel.getAttribute("role"), "dialog");
      assert.equal(threadsPanel.getAttribute("aria-label"), "Ветки");
      assert.equal(threadsPanel.getAttribute("aria-modal"), "false");
      assert.equal(threadsPanel.getAttribute("data-state"), "empty");
      assert.equal(threadsPanel.getAttribute("tabindex"), "-1");
      assert.ok(threadsPanel.hasAttribute("hidden"), "закрытая панель веток скрыта");
      assert.ok(threadsPanel.querySelector('[data-action="close-threads"]'), "кнопка закрытия close-threads");
      assert.ok(threadsPanel.querySelector(".threads-content"), "контейнер .threads-content");
      assert.ok(threadsPanel.querySelector('[role="tablist"]'), "табы активные/архив");
      assert.ok(threadsPanel.querySelector(".threads-search-input"), "поле поиска веток");
    } finally {
      app.dispose();
    }
  });

  it("взаимное исключение: открытие панели пинов автоматически закрывает панель веток и наоборот", async () => {
    const app = await loadPagingApp(tailPage);
    try {
      const pinsBtn = /** @type {ShimElement} */ (app.document.querySelector('[data-action="pins"]'));
      const threadsBtn = /** @type {ShimElement} */ (app.document.querySelector('[data-action="threads"]'));
      const ctrl = createPinsThreadsPanels({
        document: app.document,
        pinsButton: pinsBtn,
        threadsButton: threadsBtn,
      });

      const pinsPanel = ctrl.pinsPanel;
      const threadsPanel = ctrl.threadsPanel;

      pinsBtn.click();
      assert.equal(pinsBtn.getAttribute("aria-expanded"), "true");
      assert.equal(pinsPanel.hasAttribute("hidden"), false, "панель пинов открыта");
      assert.equal(threadsBtn.getAttribute("aria-expanded"), "false");
      assert.equal(threadsPanel.hasAttribute("hidden"), true, "панель веток закрыта");

      threadsBtn.click();
      assert.equal(threadsBtn.getAttribute("aria-expanded"), "true");
      assert.equal(threadsPanel.hasAttribute("hidden"), false, "панель веток открыта");
      assert.equal(pinsBtn.getAttribute("aria-expanded"), "false");
      assert.equal(pinsPanel.hasAttribute("hidden"), true, "панель пинов закрыта");

      pinsBtn.click();
      assert.equal(pinsBtn.getAttribute("aria-expanded"), "true");
      assert.equal(pinsPanel.hasAttribute("hidden"), false);
      assert.equal(threadsBtn.getAttribute("aria-expanded"), "false");
      assert.equal(threadsPanel.hasAttribute("hidden"), true);

      pinsBtn.click();
      assert.equal(pinsBtn.getAttribute("aria-expanded"), "false");
      assert.equal(pinsPanel.hasAttribute("hidden"), true);
    } finally {
      app.dispose();
    }
  });

  it("панель пинов: 4 состояния (loading со скелетонами, empty, error с role='alert' и populated с переходом jump)", async () => {
    const app = await loadPagingApp(tailPage);
    try {
      const pinsBtn = /** @type {ShimElement} */ (app.document.querySelector('[data-action="pins"]'));
      const threadsBtn = /** @type {ShimElement} */ (app.document.querySelector('[data-action="threads"]'));
      let retried = false;
      /** @type {string | null} */
      let jumpedMessageId = null;

      const ctrl = createPinsThreadsPanels({
        document: app.document,
        pinsButton: pinsBtn,
        threadsButton: threadsBtn,
        onRetryPins: () => { retried = true; },
        onJumpToMessage: (id) => { jumpedMessageId = id; },
      });

      const panel = ctrl.pinsPanel;
      const content = /** @type {ShimElement} */ (panel.querySelector(".pins-content"));

      // 1. loading
      ctrl.setPinsState("loading");
      assert.equal(panel.getAttribute("data-state"), "loading");
      assert.equal(content.getAttribute("aria-busy"), "true");
      const skeletons = panel.querySelectorAll(".pin-skeleton");
      assert.equal(skeletons.length, 3, "должно быть 3 скелетона в состоянии loading");
      assert.ok(content.textContent.includes("Загрузка"), "screen reader текст загрузки");

      // 2. empty
      ctrl.setPinsState("empty");
      assert.equal(panel.getAttribute("data-state"), "empty");
      assert.equal(content.getAttribute("aria-busy"), "false");
      assert.ok(panel.querySelector(".dropdown-empty-state"), "контейнер пустого состояния");
      assert.ok(content.textContent.includes("нет закреплённых сообщений"));

      // 3. error
      ctrl.setPinsState("error");
      assert.equal(panel.getAttribute("data-state"), "error");
      assert.equal(content.getAttribute("aria-busy"), "false");
      const errorBox = panel.querySelector(".dropdown-error-state");
      assert.ok(errorBox, "блок ошибки");
      assert.equal(errorBox.getAttribute("role"), "alert");
      const retryBtn = /** @type {ShimElement} */ (panel.querySelector('[data-action="retry-pins"]'));
      assert.ok(retryBtn, "кнопка повтора");
      retryBtn.click();
      assert.equal(retried, true, "клик по retry-pins вызывает onRetryPins");

      // 4. populated
      const samplePins = [
        { id: "1001", author: "Alice", content: "Первый важный пин", timestamp: "12:00" },
        { id: "1002", author: "Bob", content: "Второй важный пин", timestamp: "12:30" },
      ];
      ctrl.setPinsState("populated", samplePins);
      assert.equal(panel.getAttribute("data-state"), "populated");
      assert.equal(content.getAttribute("aria-busy"), "false");
      const pinItems = panel.querySelectorAll(".pin-item");
      assert.equal(pinItems.length, 2, "2 карточки пинов");
      assert.ok(pinItems[0]?.textContent.includes("Alice"));
      assert.ok(pinItems[0]?.textContent.includes("Первый важный пин"));
      assert.ok(pinItems[1]?.textContent.includes("Bob"));

      const jumpBtn = /** @type {ShimElement} */ (panel.querySelector('[data-action="jump-to-message"][data-message-id="1001"]'));
      assert.ok(jumpBtn, "кнопка перехода к сообщению 1001");
      jumpBtn.click();
      assert.equal(jumpedMessageId, "1001", "клик по кнопке перехода передаёт id сообщения");
    } finally {
      app.dispose();
    }
  });

  it("панель веток: 4 состояния (loading, empty, error, populated), переход к ветке и фильтрация поиска", async () => {
    const app = await loadPagingApp(tailPage);
    try {
      const pinsBtn = /** @type {ShimElement} */ (app.document.querySelector('[data-action="pins"]'));
      const threadsBtn = /** @type {ShimElement} */ (app.document.querySelector('[data-action="threads"]'));
      let retriedThreads = false;
      /** @type {string | null} */
      let openedThreadId = null;

      const ctrl = createPinsThreadsPanels({
        document: app.document,
        pinsButton: pinsBtn,
        threadsButton: threadsBtn,
        onRetryThreads: () => { retriedThreads = true; },
        onOpenThread: (id) => { openedThreadId = id; },
      });

      const panel = ctrl.threadsPanel;
      const content = /** @type {ShimElement} */ (panel.querySelector(".threads-content"));

      // 1. loading
      ctrl.setThreadsState("loading");
      assert.equal(panel.getAttribute("data-state"), "loading");
      assert.equal(content.getAttribute("aria-busy"), "true");
      const skeletons = panel.querySelectorAll(".thread-skeleton");
      assert.equal(skeletons.length, 4, "должно быть 4 скелетона в состоянии loading веток");

      // 2. empty
      ctrl.setThreadsState("empty");
      assert.equal(panel.getAttribute("data-state"), "empty");
      assert.equal(content.getAttribute("aria-busy"), "false");
      assert.ok(panel.querySelector(".dropdown-empty-state"));
      assert.ok(content.textContent.includes("нет активных веток"));

      // 3. error
      ctrl.setThreadsState("error");
      assert.equal(panel.getAttribute("data-state"), "error");
      assert.equal(content.getAttribute("aria-busy"), "false");
      const errorBox = panel.querySelector(".dropdown-error-state");
      assert.ok(errorBox);
      assert.equal(errorBox.getAttribute("role"), "alert");
      const retryBtn = /** @type {ShimElement} */ (panel.querySelector('[data-action="retry-threads"]'));
      assert.ok(retryBtn);
      retryBtn.click();
      assert.equal(retriedThreads, true, "клик по retry-threads вызывает onRetryThreads");

      // 4. populated
      const sampleThreads = [
        { id: "t-1", name: "Обсуждение релиза", replyCount: 15, lastMessageTime: "10:15", archived: false },
        { id: "t-2", name: "Архитектура Rust", replyCount: 42, lastMessageTime: "11:20", archived: false },
        { id: "t-3", name: "Старый архивный тред", replyCount: 3, lastMessageTime: "вчера", archived: true },
      ];
      ctrl.setThreadsState("populated", sampleThreads);
      assert.equal(panel.getAttribute("data-state"), "populated");
      assert.equal(content.getAttribute("aria-busy"), "false");

      let items = panel.querySelectorAll(".thread-item");
      assert.equal(items.length, 2, "2 активные ветки в списке");
      assert.ok(items[0]?.textContent.includes("Обсуждение релиза"));
      assert.ok(items[0]?.textContent.includes("15 ответов"));

      const firstItem = /** @type {ShimElement} */ (items[0]);
      firstItem.click();
      assert.equal(openedThreadId, "t-1");

      const searchInput = /** @type {ShimElement} */ (panel.querySelector(".threads-search-input"));
      assert.ok(searchInput);
      searchInput.value = "Rust";
      searchInput.emit("input");
      items = panel.querySelectorAll(".thread-item");
      assert.equal(items.length, 1, "только 1 ветка совпала с 'Rust'");
      assert.ok(items[0]?.textContent.includes("Архитектура Rust"));

      const archiveTab = /** @type {ShimElement} */ (panel.querySelector("#tab-archived-threads"));
      const activeTab = /** @type {ShimElement} */ (panel.querySelector("#tab-active-threads"));
      assert.ok(archiveTab && activeTab);
      archiveTab.click();
      assert.equal(archiveTab.getAttribute("aria-selected"), "true");
      assert.equal(activeTab.getAttribute("aria-selected"), "false");
      searchInput.value = "";
      searchInput.emit("input");
      items = panel.querySelectorAll(".thread-item");
      assert.equal(items.length, 1, "1 архивная ветка");
      assert.ok(items[0]?.textContent.includes("Старый архивный тред"));
    } finally {
      app.dispose();
    }
  });

  it("закрытие по Escape, клику вне панели и кнопкам закрытия с возвратом фокуса на триггер", async () => {
    const app = await loadPagingApp(tailPage);
    try {
      const pinsBtn = /** @type {ShimElement} */ (app.document.querySelector('[data-action="pins"]'));
      const threadsBtn = /** @type {ShimElement} */ (app.document.querySelector('[data-action="threads"]'));
      const ctrl = createPinsThreadsPanels({
        document: app.document,
        pinsButton: pinsBtn,
        threadsButton: threadsBtn,
      });

      const pinsPanel = ctrl.pinsPanel;
      const threadsPanel = ctrl.threadsPanel;

      // 1. Закрытие pins кнопкой [data-action="close-pins"]
      pinsBtn.click();
      assert.equal(pinsPanel.hasAttribute("hidden"), false);
      const closePinsBtn = /** @type {ShimElement} */ (pinsPanel.querySelector('[data-action="close-pins"]'));
      assert.ok(closePinsBtn);
      closePinsBtn.click();
      assert.equal(pinsPanel.hasAttribute("hidden"), true, "панель пинов скрыта");
      assert.equal(pinsBtn.getAttribute("aria-expanded"), "false");
      assert.equal(app.document.activeElement, pinsBtn, "фокус вернулся на кнопку pins");

      // 2. Закрытие threads кнопкой [data-action="close-threads"]
      threadsBtn.click();
      assert.equal(threadsPanel.hasAttribute("hidden"), false);
      const closeThreadsBtn = /** @type {ShimElement} */ (threadsPanel.querySelector('[data-action="close-threads"]'));
      assert.ok(closeThreadsBtn);
      closeThreadsBtn.click();
      assert.equal(threadsPanel.hasAttribute("hidden"), true, "панель веток скрыта");
      assert.equal(threadsBtn.getAttribute("aria-expanded"), "false");
      assert.equal(app.document.activeElement, threadsBtn, "фокус вернулся на кнопку threads");

      // 3. Закрытие pins по клавише Escape
      pinsBtn.click();
      assert.equal(pinsPanel.hasAttribute("hidden"), false);
      app.document.body.dispatchEvent("keydown", { key: "Escape" });
      assert.equal(pinsPanel.hasAttribute("hidden"), true, "панель закрыта по Escape");
      assert.equal(pinsBtn.getAttribute("aria-expanded"), "false");
      assert.equal(app.document.activeElement, pinsBtn, "фокус вернулся на кнопку pins после Escape");

      // 4. Закрытие threads по клавише Escape
      threadsBtn.click();
      assert.equal(threadsPanel.hasAttribute("hidden"), false);
      app.document.body.dispatchEvent("keydown", { key: "Escape" });
      assert.equal(threadsPanel.hasAttribute("hidden"), true, "панель веток закрыта по Escape");
      assert.equal(threadsBtn.getAttribute("aria-expanded"), "false");
      assert.equal(app.document.activeElement, threadsBtn, "фокус вернулся на кнопку threads после Escape");

      // 5. Закрытие по клику вне панели (outside click)
      pinsBtn.click();
      assert.equal(pinsPanel.hasAttribute("hidden"), false);
      app.document.body.dispatchEvent("click", { target: app.document.body });
      assert.equal(pinsPanel.hasAttribute("hidden"), true, "панель пинов закрыта по клику вне");
      assert.equal(pinsBtn.getAttribute("aria-expanded"), "false");
      assert.equal(app.document.activeElement, pinsBtn, "фокус вернулся на кнопку pins после outside click");
    } finally {
      app.dispose();
    }
  });
});


