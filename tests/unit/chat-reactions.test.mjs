// @ts-check
/**
 * M-06 (RW-40): реакции в **реальном** канале, не в bench-окне «news».
 *
 * Оракул — контракт backend-проекции, а не текущая реализация UI:
 *   - `rust/gui-spike/src/data.rs` → `MessageRow` (camelCase):
 *     `id`, `channelId`, `reactions: [{ emoji, count, me }]` — ровно эти поля
 *     сериализует `MessagesPage.items[]`;
 *   - D-53 (`waves/RW-39/PM/DECISIONS.md`): Tauri `toggle_reaction` принимает
 *     `channelId`, `messageId`, `emoji`, `active`; `active=true` — PUT,
 *     `false` — DELETE. UI обязан передавать реальный snowflake канала и
 *     сообщения (`item.id`), а не синтетические `"news"`/индекс (G6-D-01);
 *   - `G6-BATCH-MERGE-01.md` §2.2/§5: в `renderChannelMessage` нет ни pills,
 *     ни быстрых реакций, `reactions.js` живёт на bench-id — это и есть
 *     регресс, который закрывает карточка;
 *   - MSG-23…26 (`DESIGN/DESIGN-MSG.md`): пилюля — `button.reaction` с
 *     `.reaction-count`, `aria-pressed`, непустым `aria-label`;
 *   - бенч не смешивается: `CHAT_ROWS=100_000` и `channelId:"news"` остаются
 *     только для синтетического окна (проверки B3b в `gui-spike-features`).
 *
 * Шим не всплывает события: `mouseover` адресуется `#chat-list`, как в
 * `actions.js`/`message-toolbar.js`.
 */
import { describe, it, beforeEach, afterEach } from "node:test";
import assert from "node:assert/strict";
import { loadApp } from "../support/gui-dom-harness.mjs";
import {
  reactionsFor,
  resetReactions,
  toggleReaction,
} from "../../rust/gui-spike/ui/features/reactions.js";

/** @typedef {import("../support/gui-dom-harness.mjs").GuiDom} GuiDom */
/** @typedef {import("../support/gui-dom-harness.mjs").ShimElement} ShimElement */

/** Гильдия и канал, которых нет в bench-окне. */
const GUILD_ID = "1014537851418980372";
const CHANNEL_ID = "1163408591315812392";
/** Реальные snowflake-и сообщений (19 цифр), а не индексы строк. */
const MSG_REACTED = "1163408592000000001";
const MSG_PLAIN = "1163408592000000002";
const MSG_BROKEN = "1163408592000000003";

/**
 * Строка `messages_page` в форме `MessageRow` (`data.rs`, camelCase).
 * @param {{
 *   id: string,
 *   content?: string,
 *   timestamp?: string,
 *   reactions?: unknown,
 *   channelId?: string,
 * }} input
 */
function channelMessage(input) {
  const timestamp = input.timestamp ?? "2026-10-23T20:05:00.000Z";
  return {
    id: input.id,
    channelId: input.channelId ?? CHANNEL_ID,
    guildId: GUILD_ID,
    author: {
      id: "42",
      username: "sasha",
      globalName: null,
      avatar: null,
      bot: false,
    },
    content: input.content ?? `сообщение ${input.id}`,
    timestamp,
    edited: false,
    editedAt: null,
    day: timestamp.slice(0, 10),
    group: "start",
    pinned: false,
    messageType: 0,
    embeds: [],
    attachments: [],
    reactions: input.reactions === undefined ? [] : input.reactions,
    replyTo: null,
    mentions: [],
    mentionEveryone: false,
  };
}

/**
 * Ответ `messages_page` из оракула.
 * @param {unknown[]} items
 */
function messagesPage(items) {
  return {
    channelId: CHANNEL_ID,
    items,
    nextCursor: null,
    hasMore: false,
    total: items.length,
    limit: 100,
    loadedRowsLimit: 500,
  };
}

/**
 * @param {{
 *   messages?: () => unknown,
 *   toggle?: (args: any) => any,
 * }} [options]
 * @returns {Promise<GuiDom>}
 */
function loadChatApp(options = {}) {
  return loadApp({
    invokeHandler: async (cmd, args) => {
      if (cmd === "auth_status") {
        return { status: "ready", stored: true, user: { id: "42", username: "sasha" } };
      }
      if (cmd === "ready") {
        return {
          user: {
            id: "42",
            username: "sasha",
            globalName: null,
            avatar: null,
            email: null,
            bio: null,
            premium: false,
            mfaEnabled: false,
          },
          guilds: [{ id: GUILD_ID, name: "Тестовая гильдия", icon: null, memberCount: 2, channelCount: 1 }],
          guildCount: 1,
          privateChannelCount: 0,
          relationshipCount: 0,
          readStateCount: 0,
          session: null,
        };
      }
      if (cmd === "channels") {
        return [{
          id: CHANNEL_ID,
          guildId: GUILD_ID,
          name: "general",
          type: 0,
          parentId: null,
          position: 0,
        }];
      }
      if (cmd === "messages_page") return options.messages ? options.messages() : messagesPage([]);
      if (cmd === "read_state") return [];
      if (cmd === "toggle_reaction") {
        if (options.toggle) return options.toggle(args);
        return null;
      }
      if (cmd === "ack_message" || cmd === "send_message" || cmd === "edit_message" || cmd === "delete_message") {
        return cmd === "send_message" ? { id: "1561627999999999999" } : null;
      }
      return undefined;
    },
  });
}

/**
 * Открыть реальный канал кликом в дереве (C-02 путь).
 * @param {GuiDom} app
 */
async function openRealChannel(app) {
  const item = app.document.querySelectorAll(".channel-item").find(
    (node) => node.getAttribute("data-channel-id") === CHANNEL_ID,
  );
  assert.ok(item, "в дереве каналов нет реального канала");
  item.click();
  await app.settle(12);
}

/**
 * @param {GuiDom} app
 * @param {string} messageId
 * @returns {ShimElement}
 */
function realRow(app, messageId) {
  const row = app.document.querySelectorAll("#chat-list .row").find(
    (node) => node.getAttribute("data-message-id") === messageId,
  );
  assert.ok(row, `в #chat-list нет строки ${messageId}`);
  return row;
}

/**
 * @param {ShimElement} row
 * @param {string} emoji
 * @returns {ShimElement}
 */
function pillOf(row, emoji) {
  const pill = row.querySelectorAll(".reactions-row .reaction").find(
    (node) => node.getAttribute("data-emoji") === emoji,
  );
  assert.ok(pill, `у строки ${row.getAttribute("data-message-id")} нет пилюли ${emoji}`);
  return pill;
}

/** @param {ShimElement} pill */
function pillCount(pill) {
  return pill.querySelector(".reaction-count")?.textContent ?? "";
}

describe("M-06: реакции в реальном канале", () => {
  beforeEach(() => resetReactions());
  afterEach(() => resetReactions());

  it("messages_page.reactions → pills: emoji, count, active, data-message-id=item.id", async () => {
    const app = await loadChatApp({
      messages: () => messagesPage([
        channelMessage({
          id: MSG_REACTED,
          reactions: [
            { emoji: "🔥", count: 3, me: true },
            { emoji: "✅", count: 1, me: false },
          ],
        }),
        channelMessage({ id: MSG_PLAIN, content: "без реакций", reactions: [] }),
      ]),
    });
    try {
      await openRealChannel(app);
      const rows = app.document.querySelectorAll("#chat-list .row");
      assert.deepEqual(
        rows.map((row) => row.getAttribute("data-message-id")),
        [MSG_REACTED, MSG_PLAIN],
        "data-message-id строк не равен item.id из messages_page",
      );

      const reacted = realRow(app, MSG_REACTED);
      const pills = reacted.querySelectorAll(".reactions-row .reaction");
      assert.equal(pills.length, 2, "реакции из messages_page не отрисованы пилюлями");
      const fire = pillOf(reacted, "🔥");
      assert.equal(fire.tagName, "BUTTON", "пилюля реакции должна быть кнопкой");
      assert.equal(fire.getAttribute("type"), "button");
      assert.equal(pillCount(fire), "3", "счётчик реакции не из messages_page");
      assert.equal(fire.getAttribute("aria-pressed"), "true", "me=true не даёт aria-pressed=true");
      assert.equal(fire.getAttribute("data-me"), "true");
      assert.match(
        fire.getAttribute("aria-label") ?? "",
        /Реакция 🔥, 3/,
        "aria-label пилюли без emoji/счётчика",
      );

      const done = pillOf(reacted, "✅");
      assert.equal(pillCount(done), "1");
      assert.equal(done.getAttribute("aria-pressed"), "false", "me=false не даёт aria-pressed=false");

      assert.equal(
        realRow(app, MSG_PLAIN).querySelectorAll(".reaction").length,
        0,
        "пустые reactions нарисовали пилюли",
      );

      // Бенч-окно не подмешано: синтетические реакции строки 9/26 и «news» не в DOM.
      assert.equal(app.callsOf("messages_page")[0]?.args?.channelId, CHANNEL_ID);
      assert.equal(app.snapshot.payload?.chatRows, 100_000, "CHAT_ROWS бенча ослаблен");
    } finally {
      app.dispose();
    }
  });

  it("клик ставит и снимает через toggle_reaction с реальными channelId/messageId", async () => {
    const app = await loadChatApp({
      messages: () => messagesPage([
        channelMessage({
          id: MSG_REACTED,
          reactions: [
            { emoji: "🔥", count: 3, me: true },
            { emoji: "✅", count: 1, me: false },
          ],
        }),
      ]),
    });
    try {
      await openRealChannel(app);

      // Поставить: ✅ 1/me=false → optimistic 2/true, IPC active=true.
      const add = pillOf(realRow(app, MSG_REACTED), "✅");
      add.click();
      assert.equal(add.getAttribute("aria-pressed"), "true", "нет optimistic aria-pressed до ответа IPC");
      assert.equal(pillCount(add), "2", "optimistic счётчик не вырос");
      await app.settle(6);
      const addCalls = app.callsOf("toggle_reaction");
      assert.equal(addCalls.length, 1, "клик по пилюле не отправил ровно один toggle_reaction");
      assert.deepEqual(addCalls[0]?.args, {
        channelId: CHANNEL_ID,
        messageId: MSG_REACTED,
        emoji: "✅",
        active: true,
      }, "toggle_reaction ушёл с bench-id или не тем active");

      // Снять: 🔥 3/me=true → optimistic 2/false, IPC active=false.
      const remove = pillOf(realRow(app, MSG_REACTED), "🔥");
      remove.click();
      assert.equal(remove.getAttribute("aria-pressed"), "false", "снятие не сбросило aria-pressed сразу");
      assert.equal(pillCount(remove), "2", "снятие не уменьшило счётчик");
      await app.settle(6);
      const calls = app.callsOf("toggle_reaction");
      assert.equal(calls.length, 2, "снятие реакции не отправило запрос");
      assert.deepEqual(calls[1]?.args, {
        channelId: CHANNEL_ID,
        messageId: MSG_REACTED,
        emoji: "🔥",
        active: false,
      }, "active=false не ушёл при снятии реакции");

      // Ни один запрос не ушёл в синтетический канал/индекс (G6-D-01).
      for (const call of calls) {
        assert.notEqual(call.args?.channelId, "news", "реакция ушла в bench-канал «news»");
        assert.notEqual(call.args?.messageId, "0", "реакция ушла с индексом строки вместо item.id");
      }
      // Состояние пережило перерисовку строки после ответа IPC.
      assert.equal(pillOf(realRow(app, MSG_REACTED), "✅").getAttribute("aria-pressed"), "true");
      assert.equal(pillCount(pillOf(realRow(app, MSG_REACTED), "🔥")), "2");
    } finally {
      app.dispose();
    }
  });

  for (const failure of [
    { label: "401", error: { status: 401, message: "unauthorized" } },
    { label: "429", error: { status: 429, retry_after_ms: 250, message: "rate limited" } },
    { label: "сеть", error: new Error("offline") },
  ]) {
    it(`откатывает optimistic состояние при ${failure.label} и не теряет строку`, async () => {
      const app = await loadChatApp({
        messages: () => messagesPage([
          channelMessage({
            id: MSG_REACTED,
            reactions: [{ emoji: "✅", count: 1, me: false }],
          }),
        ]),
        toggle: () => { throw failure.error; },
      });
      try {
        await openRealChannel(app);
        const pill = pillOf(realRow(app, MSG_REACTED), "✅");
        pill.click();
        assert.equal(pill.getAttribute("aria-pressed"), "true", "нет optimistic-состояния перед откатом");
        await app.settle(6);
        const restored = pillOf(realRow(app, MSG_REACTED), "✅");
        assert.equal(restored.getAttribute("aria-pressed"), "false", `${failure.label} не откатил aria-pressed`);
        assert.equal(pillCount(restored), "1", `${failure.label} не откатил счётчик`);
        assert.equal(
          realRow(app, MSG_REACTED).getAttribute("data-message-id"),
          MSG_REACTED,
          "строка потеряла реальный id после отката",
        );
        assert.equal(app.document.querySelectorAll(".error-state").length, 1, "ошибка IPC не видна");
      } finally {
        app.dispose();
      }
    });
  }

  it("двойной клик до ответа IPC — один toggle_reaction", async () => {
    /** @type {((value: any) => void) | null} */
    let resolveToggle = null;
    const app = await loadChatApp({
      messages: () => messagesPage([
        channelMessage({
          id: MSG_REACTED,
          reactions: [{ emoji: "✅", count: 1, me: false }],
        }),
      ]),
      toggle: () => new Promise((resolve) => { resolveToggle = resolve; }),
    });
    try {
      await openRealChannel(app);
      const pill = pillOf(realRow(app, MSG_REACTED), "✅");
      pill.click();
      pill.click();
      assert.equal(app.callsOf("toggle_reaction").length, 1, "двойной клик отправил второй запрос");
      assert.equal(pill.getAttribute("data-pending"), "true", "нет индикатора pending");
      /** @type {any} */ (resolveToggle)?.(null);
      await app.settle(6);
      assert.equal(
        pillOf(realRow(app, MSG_REACTED), "✅").getAttribute("data-pending"),
        null,
        "pending не снят после ответа",
      );
    } finally {
      app.dispose();
    }
  });

  it("hover-панель: быстрые реакции не скрыты и уходят с реальными id строки", async () => {
    const app = await loadChatApp({
      messages: () => messagesPage([
        channelMessage({ id: MSG_PLAIN, reactions: [] }),
      ]),
    });
    try {
      await openRealChannel(app);
      const list = app.document.getElementById("chat-list");
      assert.ok(list, "нет #chat-list");
      list.dispatchEvent("mouseover", { target: realRow(app, MSG_PLAIN) });
      const bar = app.document.querySelector(".msg-toolbar");
      assert.ok(bar, "нет hover-панели");
      assert.equal(bar.getAttribute("data-open"), "true", "hover не открыл панель");
      const quick = bar.querySelectorAll('[data-command="react"]');
      assert.equal(quick.length, 4, "в hover-панели не 4 быстрые реакции");
      for (const button of quick) {
        assert.equal(button.getAttribute("hidden"), null, "быстрая реакция скрыта в реальном канале");
      }
      const like = quick.find((button) => button.getAttribute("data-emoji") === "👍");
      assert.ok(like, "в панели нет 👍");
      like.click();
      assert.equal(like.getAttribute("aria-pressed"), "true", "панель не показала optimistic-состояние");
      await app.settle(6);
      const calls = app.callsOf("toggle_reaction");
      assert.equal(calls.length, 1, "быстрая реакция не отправила запрос");
      assert.deepEqual(calls[0]?.args, {
        channelId: CHANNEL_ID,
        messageId: MSG_PLAIN,
        emoji: "👍",
        active: true,
      }, "быстрая реакция ушла не в реальный канал/сообщение");
      const added = pillOf(realRow(app, MSG_PLAIN), "👍");
      assert.equal(added.getAttribute("aria-pressed"), "true", "пилюля не появилась в реальной строке");
      assert.equal(pillCount(added), "1");
    } finally {
      app.dispose();
    }
  });

  it("пустые и битые reactions не паникуют, счётчик остаётся числом", async () => {
    const app = await loadChatApp({
      messages: () => messagesPage([
        channelMessage({ id: MSG_PLAIN, reactions: [] }),
        channelMessage({ id: "1163408592000000004", reactions: null }),
        channelMessage({ id: "1163408592000000005", reactions: "битые" }),
        channelMessage({
          id: MSG_BROKEN,
          reactions: [
            null,
            { count: 4 },
            { emoji: "", count: 1, me: true },
            { emoji: "😮", count: "4", me: 1 },
          ],
        }),
      ]),
    });
    try {
      await openRealChannel(app);
      const rows = app.document.querySelectorAll("#chat-list .row[data-message-id]");
      assert.equal(rows.length, 4, "битые данные сломали окно реального канала");
      for (const id of [MSG_PLAIN, "1163408592000000004", "1163408592000000005"]) {
        assert.equal(
          realRow(app, id).querySelectorAll(".reaction").length,
          0,
          `из непустых/невалидных reactions появились пилюли у ${id}`,
        );
      }
      const broken = realRow(app, MSG_BROKEN);
      const pills = broken.querySelectorAll(".reactions-row .reaction");
      assert.equal(pills.length, 1, "валидная реакция потерялась среди битых записей");
      const emojiPill = /** @type {ShimElement} */ (pills[0]);
      assert.ok(emojiPill);
      assert.equal(emojiPill.getAttribute("data-emoji"), "😮");
      assert.match(pillCount(emojiPill), /^\d+$/, "счётчик битой реакции не число");
      assert.match(emojiPill.getAttribute("aria-label") ?? "", /Реакция 😮/, "нет aria-label у пилюли");
      // Клик по восстановленной пилюле не бросает исключение и уходит с item.id.
      emojiPill.click();
      await app.settle(6);
      assert.equal(app.callsOf("toggle_reaction").length, 1);
      assert.equal(app.callsOf("toggle_reaction")[0]?.args?.messageId, MSG_BROKEN);
    } finally {
      app.dispose();
    }
  });

  it("состояние реального канала не смешивается с bench-индексом", () => {
    const benchBase = [{ emoji: "🔥", count: 1, me: false }];
    const realKey = `channel:${CHANNEL_ID}:${MSG_REACTED}`;
    toggleReaction(0, "🔥", benchBase);
    assert.equal(reactionsFor(0, benchBase)[0]?.count, 2, "bench-состояние не изменилось");
    assert.deepEqual(reactionsFor(realKey, []), [], "bench-переключение протекло в реальный канал");

    toggleReaction(realKey, "🔥", benchBase);
    assert.equal(reactionsFor(realKey, benchBase)[0]?.count, 2, "реальная реакция не переключилась");
    assert.equal(reactionsFor(0, benchBase)[0]?.count, 2, "реальная реакция перезаписала bench-состояние");
  });
});
