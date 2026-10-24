// @ts-check
/**
 * P-03-UI (RW-40): «Печатает…» в композере и исходящий `send_typing`.
 *
 * Оракулы — артефакты, не реализация:
 *   - `rust/crates/discord-core/src/state/typing/types.rs`:
 *     `TYPING_TTL_MS = 10_000`, активность — `now_ms < expires_at_ms`;
 *   - `waves/RW-40/PARITY/G6-REPORT-RW40-02.md` §3.6: IPC
 *     `send_typing { channelId }` → `void | {messageSendCooldownMs?,
 *     threadCreateCooldownMs?}`, `TYPING_RESEND_MS` — именованная политика `[A]`
 *     (тест проверяет правило `now > prev + interval`, а не число Discord),
 *     guard ≤ 5 каналов, индикатор `aria-live="polite"`, «Печатает…» /
 *     «Несколько человек печатают…», себя не показывать;
 *   - `rust/gui-spike/src/typing.rs`: тело POST пустое, токен берётся из keyring —
 *     из фронтенда уходят только `channelId`;
 *   - `fixtures/gateway/by-type/002-READY.json` + кадры `MESSAGE_CREATE` —
 *     реальный канал/гильдия C-02/M-07 (не bench-«news»).
 *
 * Шим не всплывает события: `input` эмитируется самому `#composer-input`.
 * Часы шима двигаются кадрами rAF; для правила интервала/ TTL модуль получает
 * инъецированные часы, поэтому тест не зависит от числа кадров.
 */
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { loadApp } from "../support/gui-dom-harness.mjs";
import {
  TYPING_MAX_CHANNELS,
  TYPING_RESEND_MS,
  TYPING_TTL_MS,
  createTypingIndicator,
  createTypingSender,
} from "../../rust/gui-spike/ui/features/typing-indicator.js";
import { CHAT_ROWS } from "../../rust/gui-spike/ui/features/chat.js";

/** @typedef {import("../support/gui-dom-harness.mjs").GuiDom} GuiDom */
/** @typedef {import("../support/gui-dom-harness.mjs").ShimElement} ShimElement */

/* ---------- независимые оракулы ---------- */

const CORE_TYPES = readFileSync(
  new URL("../../rust/crates/discord-core/src/state/typing/types.rs", import.meta.url),
  "utf8",
);
/** TTL из core, а не из UI-константы (сравнение значений, не источника). */
const CORE_TTL = Number(
  /pub const TYPING_TTL_MS:\s*u64\s*=\s*([\d_]+)\s*;/.exec(CORE_TYPES)?.[1]?.replaceAll("_", ""),
);
assert.equal(CORE_TTL, 10_000, "оракул core TYPING_TTL_MS изменился — перечитать G6 §3.6");

const fixturesDir = fileURLToPath(new URL("../../fixtures/gateway/by-type/", import.meta.url));
const READY = JSON.parse(readFileSync(`${fixturesDir}002-READY.json`, "utf8"));

/** Реальный канал с сообщениями (фикстуры `MESSAGE_CREATE`). */
const CHANNEL = "1163408591315812392";
/** Гильдия этого канала. */
const GUILD = "1014537851418980372";
/** `user.id` из READY/`auth_status` — себя индикатор не показывает. */
const SELF_ID = "42";

/** Гильдии READY в порядке рейла. @type {Array<{ id: string, name: string }>} */
const GUILDS = (READY?.d?.guilds ?? []).map((/** @type {any} */ guild) => ({
  id: String(guild.id),
  name: String(guild?.properties?.name ?? ""),
}));

/** Кадры MESSAGE_CREATE — реальные сообщения канала. @type {any[]} */
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
 * `messages_page` в форме `MessagesPage` (camelCase) из кадров фикстур.
 * @param {string} channelId
 */
function messagesPagePayload(channelId) {
  /** @type {Map<string, any>} */
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

/** `ready()` в форме `data.rs ReadyPayload` (рейл/дерево/self id). */
function readyPayload() {
  return {
    user: {
      id: SELF_ID,
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
 * Приложение с управляемым `send_typing`.
 * @param {{
 *   typing?: (args: any) => any,
 *   readState?: any[],
 * }} [options]
 * @returns {Promise<GuiDom>}
 */
function loadTypingApp(options = {}) {
  return loadApp({
    invokeHandler: async (cmd, args) => {
      if (cmd === "auth_status") {
        return { status: "ready", stored: true, user: { id: SELF_ID, username: "tester" } };
      }
      if (cmd === "ready") return readyPayload();
      if (cmd === "channels") return oracleChannels(String(args?.guildId ?? ""));
      if (cmd === "messages_page") return messagesPagePayload(String(args?.channelId ?? ""));
      if (cmd === "read_state") return options.readState ?? [];
      if (cmd === "ack_message") return null;
      if (cmd === "send_typing") {
        return options.typing ? options.typing(args) : null;
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

/** @param {GuiDom} app @param {string} selector @returns {ShimElement[]} */
function all(app, selector) {
  return /** @type {ShimElement[]} */ ([...app.document.querySelectorAll(selector)]);
}

/**
 * Открыть реальный канал кликом по дереву (C-02/M-07).
 * @param {GuiDom} app
 */
async function openChannel(app) {
  const tile = all(app, ".guild-item").find(
    (node) => node.getAttribute("data-guild-id") === GUILD,
  );
  assert.ok(tile, `в рейле нет гильдии ${GUILD}`);
  tile.click();
  await app.settle(12);
  const button = all(app, ".channel-item").find(
    (node) => node.getAttribute("data-channel-id") === CHANNEL,
  );
  assert.ok(button, `в дереве нет канала ${CHANNEL}`);
  button.click();
  await app.settle(12);
}

/**
 * Ввести текст в композер (событие `input` адресуется самому полю).
 * @param {GuiDom} app
 * @param {string} value
 */
async function type(app, value) {
  const input = /** @type {any} */ (firstNode(app, "#composer-input"));
  input.value = value;
  input.emit("input");
  await app.settle(4);
}

/**
 * Индикатор внутри приложения (единственный экземпляр).
 * @param {GuiDom} app
 */
function appIndicator(app) {
  const nodes = all(app, "#typing-indicator");
  assert.equal(nodes.length, 1, `#typing-indicator продублирован (${nodes.length})`);
  return nodes[0];
}

/** @param {GuiDom} app */
function appApi(app) {
  const instance = app.appModule?.currentApp?.();
  assert.ok(instance, "app.js не экспортирует currentApp()");
  return instance;
}

/* =============================================================================
 * Константы/политика (G6 §3.6)
 * ========================================================================== */

describe("P-03-UI: константы и политика [A]", () => {
  it("TTL индикатора равен core TYPING_TTL_MS (10 с), а не локальному литералу", () => {
    assert.equal(TYPING_TTL_MS, CORE_TTL, "TTL разошёлся с core state/typing");
  });

  it("интервал повтора — именованная конфигурация в диапазоне 8–10 с", () => {
    assert.equal(typeof TYPING_RESEND_MS, "number");
    assert.ok(
      TYPING_RESEND_MS >= 8_000 && TYPING_RESEND_MS <= 10_000,
      `TYPING_RESEND_MS=${TYPING_RESEND_MS} вне политики [A] (8–10 с)`,
    );
  });

  it("guard ≤ 5 каналов и бенч CHAT_ROWS=100_000 не ослаблены", () => {
    assert.equal(TYPING_MAX_CHANNELS, 5, "guard каналов не 5");
    assert.equal(CHAT_ROWS, 100_000, "синтетическое окно бенча ослаблено");
  });
});

/* =============================================================================
 * Индикатор: TTL, дедуп, self, доступность
 * ========================================================================== */

/**
 * Индикатор на инъецированных часах внутри свежего шим-документа.
 * @param {GuiDom} app
 * @param {{ channelId?: string, selfUserId?: string }} [options]
 */
function makeIndicator(app, options = {}) {
  const host = app.document.createElement("div");
  app.document.body.append(host);
  /** @type {{ now: number }} */
  const clock = { now: 0 };
  const indicator = createTypingIndicator({
    host: /** @type {any} */ (host),
    getChannelId: () => options.channelId ?? CHANNEL,
    selfUserId: options.selfUserId ?? SELF_ID,
    now: () => clock.now,
  });
  return { host, clock, indicator };
}

describe("P-03-UI: индикатор «Печатает…»", () => {
  it("один пользователь — «Печатает…», несколько — «Несколько человек печатают…»", async () => {
    const app = await loadApp();
    try {
      const { indicator } = makeIndicator(app);
      const node = indicator.node;
      assert.equal(node.getAttribute("aria-live"), "polite", "индикатор без aria-live=polite");
      assert.equal(node.getAttribute("role"), "status", "индикатор без role=status");
      assert.ok(node.hasAttribute("hidden"), "пустой индикатор не скрыт");

      assert.equal(
        indicator.start({ channelId: CHANNEL, userId: "777", name: "Аня" }),
        true,
        "событие чужого пользователя не принято",
      );
      assert.equal(node.getAttribute("data-visible"), "true", "индикатор не показан");
      assert.equal(node.hasAttribute("hidden"), false, "скрытый узел при активном наборе");
      assert.equal(node.getAttribute("data-count"), "1");
      assert.equal(
        /** @type {any} */ (node.querySelector(".typing-indicator-text")).textContent,
        "Печатает…",
      );
    } finally {
      app.dispose();
    }
  });

  it("повтор пользователя не дублирует индикатор; двое дают множественную форму", async () => {
    const app = await loadApp();
    try {
      const { indicator } = makeIndicator(app);
      const node = indicator.node;
      indicator.start({ channelId: CHANNEL, userId: "777", name: "Аня" });
      indicator.start({ channelId: CHANNEL, userId: "777", name: "Аня" });
      assert.equal(node.getAttribute("data-count"), "1", "повтор пользователя удвоил счётчик");
      assert.equal(
        /** @type {any} */ (node.querySelector(".typing-indicator-text")).textContent,
        "Печатает…",
      );
      indicator.start({ channelId: CHANNEL, userId: "778", name: "Борис" });
      assert.equal(node.getAttribute("data-count"), "2");
      assert.equal(
        /** @type {any} */ (node.querySelector(".typing-indicator-text")).textContent,
        "Несколько человек печатают…",
      );
    } finally {
      app.dispose();
    }
  });

  it("себя не показывает: self-флаг и selfUserId игнорируются", async () => {
    const app = await loadApp();
    try {
      const { indicator } = makeIndicator(app);
      assert.equal(
        indicator.start({ channelId: CHANNEL, userId: SELF_ID, name: "Тестер" }),
        false,
        "свой user id принят индикатором",
      );
      assert.equal(
        indicator.start({ channelId: CHANNEL, userId: "900", self: true }),
        false,
        "self-флаг не отфильтрован",
      );
      assert.equal(indicator.node.getAttribute("data-count"), "0");
      assert.ok(indicator.node.hasAttribute("hidden"), "индикатор показан для себя");
    } finally {
      app.dispose();
    }
  });

  it("TTL 10 с: на 9 999 мс видим, на 10 000 мс — уже нет (граница core)", async () => {
    const app = await loadApp();
    try {
      const { indicator, clock } = makeIndicator(app);
      indicator.start({ channelId: CHANNEL, userId: "777" });
      clock.now = TYPING_TTL_MS - 1;
      indicator.sweep();
      assert.equal(indicator.node.getAttribute("data-count"), "1", "индикатор истёк раньше TTL");
      clock.now = TYPING_TTL_MS;
      indicator.sweep();
      assert.equal(indicator.node.getAttribute("data-count"), "0", "TTL не истёк на границе");
      assert.ok(indicator.node.hasAttribute("hidden"), "истёкший индикатор остался видимым");
    } finally {
      app.dispose();
    }
  });

  it("событие другого канала не показывается в текущем", async () => {
    const app = await loadApp();
    try {
      const { indicator } = makeIndicator(app);
      indicator.start({ channelId: "999", userId: "777" });
      assert.equal(indicator.node.getAttribute("data-count"), "0", "чужой канал попал в индикатор");
      indicator.start({ channelId: CHANNEL, userId: "777" });
      assert.equal(indicator.node.getAttribute("data-count"), "1");
    } finally {
      app.dispose();
    }
  });
});

/* =============================================================================
 * Исходящий typing: интервал, guard, ошибки
 * ========================================================================== */

/**
 * Отправитель на инъецированных часах.
 * @param {{
 *   invoke?: (cmd: string, args?: any) => Promise<any>,
 *   now?: () => number,
 *   resendMs?: number,
 * }} [options]
 */
function makeSender(options = {}) {
  /** @type {{ now: number }} */
  const clock = { now: 0 };
  /** @type {Array<{ cmd: string, args: any }>} */
  const calls = [];
  /** @type {Array<{ text: string, kind: string }>} */
  const notices = [];
  const sender = createTypingSender({
    invoke: options.invoke ?? (async (cmd, args) => {
      calls.push({ cmd, args });
      return null;
    }),
    now: options.now ?? (() => clock.now),
    resendMs: options.resendMs,
    onNotice: (text, kind) => notices.push({ text, kind }),
  });
  return { clock, calls, notices, sender };
}

describe("P-03-UI: исходящий send_typing", () => {
  it("первый ввод — один POST {channelId} без токена; повтор — только после интервала", async () => {
    const app = await loadApp();
    try {
      const { sender, calls, clock } = makeSender();
      const first = await sender.note(CHANNEL);
      assert.equal(first.sent, true, "первый typing не отправлен");
      assert.equal(calls.length, 1, "не ровно один send_typing");
      assert.equal(calls[0]?.cmd, "send_typing");
      assert.deepEqual(Object.keys(calls[0]?.args ?? {}), ["channelId"], "в args лишние поля/токен");
      assert.equal(calls[0]?.args?.channelId, CHANNEL);

      clock.now = TYPING_RESEND_MS;
      assert.equal((await sender.note(CHANNEL)).sent, false, "повтор на границе интервала");
      assert.equal(calls.length, 1, "send_typing отправлен внутри интервала");

      clock.now = TYPING_RESEND_MS + 1;
      assert.equal((await sender.note(CHANNEL)).sent, true, "правило now > prev + interval не работает");
      assert.equal(calls.length, 2, "второй typing не ушёл после интервала");
    } finally {
      app.dispose();
    }
  });

  it("синтетический bench (news) и не-снежинки не отправляют typing", async () => {
    const app = await loadApp();
    try {
      const { sender, calls } = makeSender();
      for (const id of ["news", "", "abc", "12x"]) {
        const result = await sender.note(id);
        assert.equal(result.sent, false, `typing ушёл для ${id}`);
      }
      assert.equal(calls.length, 0, "bench/мусорный id вызвал IPC");
    } finally {
      app.dispose();
    }
  });

  it("guard: не более 5 каналов в состоянии typing", async () => {
    const app = await loadApp();
    try {
      const { sender, calls, clock } = makeSender();
      const ids = ["1", "2", "3", "4", "5", "6"];
      for (const id of ids) {
        clock.now += TYPING_RESEND_MS + 1;
        await sender.note(id);
      }
      assert.equal(calls.length, TYPING_MAX_CHANNELS, `guard пропустил ${calls.length} каналов`);
      const sixth = await sender.note("6");
      assert.equal(sixth.sent, false);
      assert.equal(sixth.reason, "guard", "шестой канал не отсечён guard'ом");
    } finally {
      app.dispose();
    }
  });

  it("401: видимое сообщение и никакого автоматического цикла", async () => {
    const app = await loadApp();
    try {
      let calls = 0;
      const { sender, notices, clock } = makeSender({
        invoke: async () => {
          calls += 1;
          throw { kind: "Unauthorized", status: 401 };
        },
      });
      const result = await sender.note(CHANNEL);
      assert.equal(result.sent, false);
      assert.equal(calls, 1, "401 не дал ровно один запрос");
      assert.equal(sender.blocked, true, "401 не заблокировал отправку");
      const notice = notices.at(-1);
      assert.ok(notice, "401 без видимого сообщения");
      assert.equal(notice?.kind, "Unauthorized");
      assert.match(notice?.text ?? "", /Сессия истекла/);

      // Имитация продолжения набора: новых IPC быть не должно.
      for (let index = 1; index <= 4; index += 1) {
        clock.now += TYPING_RESEND_MS + 1;
        const repeat = await sender.note(CHANNEL);
        assert.equal(repeat.sent, false, "после 401 снова ушёл typing");
      }
      assert.equal(calls, 1, "после 401 запустился retry-цикл");
    } finally {
      app.dispose();
    }
  });

  it("429: видимое сообщение с retry_after и пауза без цикла", async () => {
    const app = await loadApp();
    try {
      let calls = 0;
      const { sender, notices, clock } = makeSender({
        invoke: async () => {
          calls += 1;
          if (calls === 1) throw { kind: "RateLimited", retryAfterMs: 1_500 };
          return null;
        },
      });
      const result = await sender.note(CHANNEL);
      assert.equal(result.sent, false);
      assert.equal(calls, 1);
      const notice = notices.at(-1);
      assert.equal(notice?.kind, "RateLimited");
      assert.match(notice?.text ?? "", /Слишком часто/);

      clock.now += 1_000;
      assert.equal((await sender.note(CHANNEL)).sent, false, "typing ушёл внутри retry_after");
      assert.equal(calls, 1, "429 спровоцировал повтор до retry_after");

      clock.now += 1_000;
      assert.equal((await sender.note(CHANNEL)).sent, true, "после retry_after typing не восстановился");
      assert.equal(calls, 2);
    } finally {
      app.dispose();
    }
  });

  it("slowmode-кулдаун из 200 — видимое сообщение и пауза до истечения", async () => {
    const app = await loadApp();
    try {
      let calls = 0;
      const { sender, notices, clock } = makeSender({
        invoke: async () => {
          calls += 1;
          return { messageSendCooldownMs: 5_000, threadCreateCooldownMs: 10_000 };
        },
      });
      const result = await sender.note(CHANNEL);
      assert.equal(result.sent, true);
      assert.equal(result.cooldownMs, 5_000);
      const notice = notices.at(-1);
      assert.equal(notice?.kind, "slowmode");
      assert.match(notice?.text ?? "", /Медленный режим/);
      assert.match(notice?.text ?? "", /\b5\b/, "в сообщении нет секунд кулдауна");

      clock.now += 4_000;
      assert.equal((await sender.note(CHANNEL)).sent, false, "typing ушёл во время slowmode");
      assert.equal(calls, 1, "slowmode запустил цикл запросов");
    } finally {
      app.dispose();
    }
  });

  it("токен/тело ответа не утекают в видимое сообщение и консоль", async () => {
    const app = await loadApp();
    try {
      const secret = "Bearer super-secret-token-value";
      const { sender, notices } = makeSender({
        invoke: async () => {
          throw new Error(secret);
        },
      });
      await sender.note(CHANNEL);
      const text = notices.at(-1)?.text ?? "";
      assert.doesNotMatch(text, /super-secret-token-value/, "сообщение ошибки попало в UI");
      assert.equal(app.consoleErrors.length, 0, "typing пишет в console.error");
      assert.equal(app.consoleWarns.length, 0, "typing пишет в console.warn");
    } finally {
      app.dispose();
    }
  });
});

/* =============================================================================
 * Интеграция с приложением (реальный канал из C-02, bench не шлёт)
 * ========================================================================== */

describe("P-03-UI: интеграция с композером", () => {
  it("реальный ввод в канале — один send_typing с реальным channelId и без токена", async () => {
    const app = await loadTypingApp();
    try {
      await openChannel(app);
      assert.equal(appApi(app).chat.mode, "channel", "канал не открылся");
      await type(app, "привет");
      const calls = app.callsOf("send_typing");
      assert.equal(calls.length, 1, `send_typing вызван ${calls.length} раз вместо одного`);
      assert.deepEqual(Object.keys(calls[0]?.args ?? {}), ["channelId"], "args содержат лишнее/токен");
      assert.equal(calls[0]?.args?.channelId, CHANNEL);
      assert.notEqual(calls[0]?.args?.channelId, "news", "typing ушёл в bench-канал");
      assert.match(String(calls[0]?.args?.channelId), /^\d{1,20}$/, "channelId не снежинка");

      // Один интервал: без движения часов повторный ввод не шлёт второй POST.
      await type(app, "привет ещё");
      assert.equal(app.callsOf("send_typing").length, 1, "ввод внутри интервала продублировал typing");

      // Удаление всего текста — не повод отправлять typing.
      await type(app, "");
      assert.equal(app.callsOf("send_typing").length, 1, "пустой композер отправил typing");
      assert.equal(app.consoleErrors.length, 0, "typing дал console.error");
    } finally {
      app.dispose();
    }
  });

  it("синтетический bench typing не отправляет (CHAT_ROWS не ослаблен)", async () => {
    const app = await loadTypingApp();
    try {
      assert.equal(appApi(app).chat.mode, "bench", "до открытия канала режим не bench");
      assert.equal(appApi(app).chat.store.totalRows, 100_000, "окно бенча ослаблено");
      await type(app, "в бенче");
      assert.equal(app.callsOf("send_typing").length, 0, "bench отправил send_typing");
    } finally {
      app.dispose();
    }
  });

  it("индикатор один, aria-live polite, себя не показывает, чужого — показывает", async () => {
    const app = await loadTypingApp();
    try {
      await openChannel(app);
      const indicator = /** @type {any} */ (appIndicator(app));
      assert.equal(indicator.getAttribute("aria-live"), "polite");
      assert.ok(indicator.hasAttribute("hidden"), "индикатор виден без событий");

      const typing = appApi(app).typing;
      assert.ok(typing, "currentApp().typing не собран");
      assert.equal(
        typing.start({ channelId: CHANNEL, userId: SELF_ID }),
        false,
        "свой user id из ready() не отфильтрован",
      );
      assert.ok(indicator.hasAttribute("hidden"), "индикатор показал себя");

      typing.start({ channelId: CHANNEL, userId: "777", name: "Аня" });
      assert.equal(indicator.getAttribute("data-count"), "1");
      assert.equal(indicator.hasAttribute("hidden"), false, "чужой набор не показан");
      assert.equal(
        /** @type {any} */ (indicator.querySelector(".typing-indicator-text")).textContent,
        "Печатает…",
      );
    } finally {
      app.dispose();
    }
  });

  it("slowmode из ответа send_typing виден над композером", async () => {
    const app = await loadTypingApp({
      typing: () => ({ messageSendCooldownMs: 3_000 }),
    });
    try {
      await openChannel(app);
      await type(app, "текст");
      assert.equal(app.callsOf("send_typing").length, 1);
      const indicator = /** @type {any} */ (appIndicator(app));
      const notice = firstNode(app, "#typing-indicator .typing-indicator-notice");
      assert.equal(indicator.hasAttribute("hidden"), false, "сообщение slowmode скрыто");
      assert.match(notice.textContent ?? "", /Медленный режим/);
      assert.equal(notice.getAttribute("data-kind"), "slowmode");
      assert.doesNotMatch(String(notice.textContent), /token/i, "в сообщении упомянут токен");
    } finally {
      app.dispose();
    }
  });

  it("401 от send_typing: видимое сообщение, новых запросов нет", async () => {
    let calls = 0;
    const app = await loadTypingApp({
      typing: () => {
        calls += 1;
        throw { kind: "Unauthorized", status: 401 };
      },
    });
    try {
      await openChannel(app);
      await type(app, "первый");
      assert.equal(calls, 1, "401 не дал ровно один запрос");
      const notice = firstNode(app, "#typing-indicator .typing-indicator-notice");
      assert.match(notice.textContent ?? "", /Сессия истекла/);
      assert.equal(notice.getAttribute("data-kind"), "Unauthorized");

      await type(app, "второй");
      await type(app, "третий");
      assert.equal(calls, 1, "после 401 отправка продолжилась (цикл)");
      assert.equal(app.consoleErrors.length, 0, "401 залогирован в console.error");
    } finally {
      app.dispose();
    }
  });
});
