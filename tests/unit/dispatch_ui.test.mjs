// @ts-check
/**
 * L-02-UI (RW-40): подписка UI на `gateway://dispatch` и проекции 6 kind.
 *
 * Оракулы — артефакты, а не текущая реализация:
 *   - `rust/gui-spike/src/net.rs`: `DISPATCH_EVENT` и ровно 6
 *     `FORWARDED_DISPATCHES`, конверт `DispatchPayload` = `{kind, data}`;
 *   - `fixtures/gateway/by-type/*`: READY (7 гильдий), GUILD_MEMBERS_CHUNK
 *     (009/022), GUILD_MEMBER_LIST_UPDATE (005 SYNC + 079/086 по 4/2 op),
 *     PRESENCE_UPDATE (010); `MESSAGE_REACTION_*` — форма `[A]` (синтетика);
 *   - `waves/RW-40/PM/l02-ui/PLAN.md` §3–§8: ключи идемпотентности, negative
 *     cases N1–N13, DOM-контракт `.member[data-user-id]`, пилюли реакций;
 *   - `CONTRACTS/dispatch.md` §3–§8: секреты и честный `data-dispatch-source`.
 *
 * Только fake events: сеть/окно/браузер/cargo не используются. Интеграционные
 * кейсы идут через настоящий `app.js` в DOM-шиме (`loadApp`), путь события —
 * `currentApp().dispatch.handle(envelope)`, тот же, что у колбэка `listen`.
 */
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { loadApp } from "../support/gui-dom-harness.mjs";
import {
  DISPATCH_EVENT,
  FORWARDED_KINDS,
  applyDispatch,
  asArray,
  asBool,
  asNumber,
  asRecord,
  asString,
  createDispatchStore,
  nilish,
  normalizeEnvelope,
  setDispatchSelfUserId,
} from "../../rust/gui-spike/ui/features/dispatch-model.js";
import {
  createDispatchBridge,
  resolveDispatchListen,
} from "../../rust/gui-spike/ui/features/dispatch-bridge.js";
import { formatNumber } from "../../rust/gui-spike/ui/features/dom.js";

/** @typedef {import("../support/gui-dom-harness.mjs").GuiDom} GuiDom */
/** @typedef {import("../support/gui-dom-harness.mjs").ShimElement} ShimElement */

const NET_RS = readFileSync(
  new URL("../../rust/gui-spike/src/net.rs", import.meta.url),
  "utf8",
);
const fixturesDir = fileURLToPath(new URL("../../fixtures/gateway/by-type/", import.meta.url));
/** @param {string} name */
function frame(name) {
  return JSON.parse(readFileSync(`${fixturesDir}${name}`, "utf8"));
}

const READY_FRAME = frame("002-READY.json");
const LIST_SYNC_FRAME = frame("005-GUILD_MEMBER_LIST_UPDATE.json");
const LIST_4OPS_FRAME = frame("079-GUILD_MEMBER_LIST_UPDATE.json");
const LIST_2OPS_FRAME = frame("086-GUILD_MEMBER_LIST_UPDATE.json");
const CHUNK_FRAME = frame("009-GUILD_MEMBERS_CHUNK.json");
const CHUNK4_FRAME = frame("022-GUILD_MEMBERS_CHUNK.json");
const PRESENCE_FRAME = frame("010-PRESENCE_UPDATE.json");

/** Гильдия 005/009 (в READY её нет — добавляем копией, PLAN §8.3). */
const GUILD_MEMBERS = "-1467272931325252608";
/** Гильдия READY с реальным каналом C-02/M-06. */
const GUILD_MAIN = "1014537851418980372";
/** Реальный канал с сообщениями в фикстурах. */
const CHANNEL = "1163408591315812392";
/** Сообщение без реакций (синтетическая строка `messages_page`). */
const MESSAGE_ID = "1163408592000000001";
/** `user.id` вошедшего пользователя (`ready()`), а не из READY-фикстуры. */
const SELF_ID = "42";
/** Имя добавленной гильдии (копия READY + 005, поля не выдумываются). */
const ADD_GUILD_NAME = "Участники L-02";

/** @param {string} kind @param {any} data */
function envelope(kind, data) {
  return { kind, data };
}

/** @param {string} name @param {number} [count] */
function cloneFrame(name, count = 1) {
  return JSON.parse(JSON.stringify(frame(name)));
}

/** Копия `d` READY с добавленной гильдией из LIST_UPDATE (PLAN §8.3). */
function readyWithMembersGuild() {
  const d = JSON.parse(JSON.stringify(READY_FRAME.d));
  d.guilds.push({
    id: GUILD_MEMBERS,
    properties: { name: ADD_GUILD_NAME },
    member_count: Number(LIST_SYNC_FRAME.d.member_count),
  });
  return d;
}

/**
 * @param {any} store
 * @param {string} kind
 * @param {any} data
 */
function apply(store, kind, data) {
  return applyDispatch(store, envelope(kind, data));
}

/** Независимый пересчёт окна по `ops[]` (оракул B7, без чтения UI-кода). */
function oracleOrder(initial, ops) {
  const ids = [...initial];
  for (const op of ops) {
    const type = String(op.op).toUpperCase();
    if (type === "SYNC") {
      ids.length = 0;
      for (const item of op.items ?? []) {
        if (!item.member) continue;
        const id = String(item.member.user?.id ?? "");
        if (id !== "") ids.push(id);
      }
      continue;
    }
    const index = Number(op.index);
    if (!Number.isInteger(index) || index < 0) continue;
    if (type === "DELETE") {
      if (index >= ids.length) continue;
      ids.splice(index, 1);
      continue;
    }
    if (op.item?.group) continue;
    const id = String(op.item?.member?.user?.id ?? "");
    if (id === "") continue;
    if (type === "INSERT") {
      if (ids.includes(id)) continue;
      ids.splice(Math.min(index, ids.length), 0, id);
      continue;
    }
    if (type === "UPDATE") {
      if (index >= ids.length) continue;
      ids[index] = id;
    }
  }
  return ids;
}

/** Синтетический кадр реакции (`[A]`-форма, PLAN §3.5). */
function reactionFrame(action, input = {}) {
  return {
    channel_id: input.channelId ?? CHANNEL,
    message_id: input.messageId ?? MESSAGE_ID,
    user_id: input.userId ?? "777",
    emoji: { id: input.emojiId ?? "nil", name: input.emoji ?? "🔥", animated: "false" },
    guild_id: GUILD_MAIN,
  };
}

/* =============================================================================
 * A. Контракт (Rust — оракул)
 * ========================================================================== */

describe("L-02-UI: контракт dispatch (A1–A3)", () => {
  it("A1: DISPATCH_EVENT совпадает с net.rs::DISPATCH_EVENT", () => {
    const expected = /pub const DISPATCH_EVENT:\s*&str\s*=\s*"([^"]+)";/.exec(NET_RS)?.[1];
    assert.equal(expected, "gateway://dispatch", "оракул net.rs изменился — перечитать CONTRACTS/dispatch.md");
    assert.equal(DISPATCH_EVENT, expected, "имя события в UI разошлось с ядром");
  });

  it("A2: FORWARDED_KINDS — ровно 6 имён из net.rs::FORWARDED_DISPATCHES", () => {
    const block = /pub const FORWARDED_DISPATCHES:\s*\[&str;\s*6\]\s*=\s*\[([\s\S]*?)\];/.exec(NET_RS)?.[1] ?? "";
    const names = [...block.matchAll(/"([^"]+)"/g)].map((match) => match[1]);
    assert.equal(names.length, 6, "в net.rs не 6 форвардимых имён");
    assert.equal(FORWARDED_KINDS.length, 6, "в UI не 6 kind");
    assert.deepEqual(new Set(FORWARDED_KINDS), new Set(names), "набор kind разошёлся с ядром");
  });

  it("A3: конверт — только {kind,data}; seq принимается, если есть, и не выдумывается", () => {
    const body = /pub struct DispatchPayload\s*\{([\s\S]*?)\n\}/.exec(NET_RS)?.[1] ?? "";
    const fields = [...body.matchAll(/pub\s+(\w+)\s*:/g)].map((match) => match[1]);
    assert.deepEqual(fields, ["kind", "data"], "DispatchPayload перестал быть ровно {kind,data}");

    const plain = normalizeEnvelope({ kind: "READY", data: { guilds: [] }, extra: 1 });
    assert.equal(plain.ok, true);
    assert.equal("seq" in plain, false, "seq выдуман при отсутствии в конверте");
    const withSeq = normalizeEnvelope({ kind: "READY", data: { guilds: [] }, seq: 17 });
    assert.equal(withSeq.ok, true);
    assert.equal(/** @type {any} */ (withSeq).seq, 17, "лишнее поле seq не принято как дополнительный ключ");

    for (const bad of [null, [], "x", 7, { data: {} }, { kind: null, data: {} }, { kind: "READY", data: [] }]) {
      assert.equal(normalizeEnvelope(bad).ok, false, `битый конверт принят: ${JSON.stringify(bad)}`);
    }
  });

  it("нормализация nil/чисел/строк — общая для дампа и живого кадра", () => {
    assert.equal(nilish(null), true);
    assert.equal(nilish(undefined), true);
    assert.equal(nilish("nil"), true);
    assert.equal(nilish(0), false);
    assert.equal(asString(" nil "), "");
    // Идентификаторы — только строки: snowflake не проходит через Number().
    assert.equal(asString("-13390286217619177728"), "-13390286217619177728");
    assert.equal(asString(-177376723249664), "-177376723249664");
    assert.equal(asString(["x"]), "");
    assert.equal(asNumber("1792391357994"), 1792391357994);
    assert.equal(asNumber("-177376723249664"), -177376723249664);
    assert.equal(asNumber(" 12 "), null, "число-строка с пробелами не должна терять строгость");
    assert.equal(asNumber("nil"), null);
    assert.equal(asBool("true"), true);
    assert.equal(asBool("false"), false);
    assert.equal(asBool(1), true);
    assert.equal(asBool("1"), true);
    assert.deepEqual(asArray("x"), []);
    assert.deepEqual(asRecord(null), {});
    assert.deepEqual(asRecord([]), {});
  });
});

/* =============================================================================
 * B. Чистые проекции (без DOM)
 * ========================================================================== */

describe("L-02-UI: проекции dispatch (B)", () => {
  it("B1: READY → гильдии/имена/member_count/selfUserId по фикстуре", () => {
    const store = createDispatchStore();
    const result = apply(store, "READY", READY_FRAME.d);
    assert.equal(result.changes.length, 1);
    const change = /** @type {any} */ (result.changes[0]);
    assert.equal(change.type, "guilds");
    const expected = READY_FRAME.d.guilds.map((/** @type {any} */ guild) => ({
      id: String(guild.id),
      name: String(guild.properties?.name ?? ""),
      memberCount: Number(guild.member_count),
    }));
    assert.deepEqual(change.guilds, expected, "гильдии не из READY");
    assert.deepEqual([...store.guilds.values()], expected, "store.live.guilds не равен READY");
    assert.deepEqual(store.order, expected.map((guild) => guild.id), "порядок плиток не из payload");
    assert.equal(store.source, "dispatch", "факт данных без reload не отмечен");
    // `selfUserId`: авторитетный ready() не перебит фикстурой другого аккаунта.
    assert.equal(store.selfUserId, String(READY_FRAME.d.user.id), "fallback selfUserId не применён");
  });

  it("B2: дамп с \"nil\" и тот же d с null дают равную проекцию", () => {
    const raw = createDispatchStore();
    apply(raw, "READY", READY_FRAME.d);
    const normalized = JSON.parse(JSON.stringify(READY_FRAME.d).replace(/"nil"/g, "null"));
    const other = createDispatchStore();
    const result = apply(other, "READY", normalized);
    assert.equal(result.changes.length, 1, "вариант с null не применён");
    assert.deepEqual(
      JSON.parse(JSON.stringify(other.guilds)),
      JSON.parse(JSON.stringify(raw.guilds)),
      "nil/null дают разные проекции гильдий",
    );
  });

  it("B3: READY дважды — тот же store, второй раз changes пустые", () => {
    const store = createDispatchStore();
    assert.equal(apply(store, "READY", READY_FRAME.d).changes.length, 1);
    const before = JSON.stringify(store);
    const second = apply(store, "READY", READY_FRAME.d);
    assert.deepEqual(second.changes, [], "повтор READY пересобрал UI");
    assert.equal(JSON.stringify(store), before, "повтор READY изменил store");
    assert.equal(second.counters.duplicate, 1, "повтор READY не учтён как duplicate");
  });

  it("B4: CHUNK — участники по user.id, имя по global_name→nick→username→id", () => {
    const store = createDispatchStore();
    const result = apply(store, "GUILD_MEMBERS_CHUNK", CHUNK_FRAME.d);
    assert.equal(result.changes.length, 1);
    const guild = store.members.get(String(CHUNK_FRAME.d.guild_id));
    assert.ok(guild, "участники 009 не попали в store");
    const member = guild.get(String(CHUNK_FRAME.d.members[0].user.id));
    assert.ok(member, "участник 009 не найден по user.id");
    assert.equal(member.name, "Новости", "имя не по приоритету nick (global_name=nil, username=MonitoRSS)");
    assert.equal(member.bot, true, "bot не из user.bot");

    const store2 = createDispatchStore();
    apply(store2, "GUILD_MEMBERS_CHUNK", CHUNK4_FRAME.d);
    const four = /** @type {Map<string, any>} */ (store2.members.get(String(CHUNK4_FRAME.d.guild_id)));
    assert.equal(four.size, 4, "в чанке 022 не 4 участника");
    assert.equal(four.get("159985870458322944")?.name, "MEE6", "global_name=nil → username");
    assert.equal(four.get("465601078914711572")?.name, "lgReaper", "global_name должен иметь приоритет");
    assert.equal(four.get("566215798759096322")?.name, "Nomadkg / АРА");
  });

  it("B5: CHUNK дважды — duplicate, store равен", () => {
    const store = createDispatchStore();
    apply(store, "GUILD_MEMBERS_CHUNK", CHUNK_FRAME.d);
    const before = JSON.stringify(store);
    const again = apply(store, "GUILD_MEMBERS_CHUNK", CHUNK_FRAME.d);
    assert.deepEqual(again.changes, [], "повтор чанка изменил UI");
    assert.equal(again.counters.duplicate, 1);
    assert.equal(JSON.stringify(store), before, "повтор чанка изменил store");
  });

  it("B6: LIST_UPDATE SYNC — order/counts из ops[0].items, range=null на мусоре", () => {
    const store = createDispatchStore();
    const d = LIST_SYNC_FRAME.d;
    const result = apply(store, "GUILD_MEMBER_LIST_UPDATE", d);
    assert.equal(result.changes.length, 1);
    const list = store.lists.get(String(d.guild_id));
    assert.ok(list, "окно списка не создано");
    const expected = oracleOrder([], [d.ops[0]]);
    assert.deepEqual(list.order, expected, "order не равен независимому пересчёту SYNC");
    assert.equal(list.onlineCount, Number(d.online_count));
    assert.equal(store.onlineCountKnown, true, "online_count не помечен как известный");
    assert.equal(list.memberCount, Number(d.member_count));
    assert.equal(list.range, null, "мусорный range (\"\\0c\") попал в store");
    assert.equal(list.listId, String(d.id));
  });

  it("B7: INSERT/UPDATE/DELETE применяются в порядке провода (2 и 4 op кадры)", () => {
    const store = createDispatchStore();
    const sync = LIST_SYNC_FRAME.d;
    apply(store, "GUILD_MEMBER_LIST_UPDATE", sync);
    let expected = oracleOrder([], [sync.ops[0]]);

    for (const source of [LIST_4OPS_FRAME.d, LIST_2OPS_FRAME.d]) {
      const d = JSON.parse(JSON.stringify(source));
      d.guild_id = String(sync.guild_id);
      const result = apply(store, "GUILD_MEMBER_LIST_UPDATE", d);
      expected = oracleOrder(expected, d.ops);
      const actual = /** @type {any} */ (store.lists.get(String(sync.guild_id))).order;
      assert.deepEqual(actual, expected, `кадр из ${d.ops.length} op применён не целиком/не по порядку`);
      assert.ok(result.counters.applied === 1 || result.changes.length === 1, "кадр не отмечен применённым");
    }
  });

  it("B8: INSERT существующего userId — без дубля, duplicate+1", () => {
    const store = createDispatchStore();
    apply(store, "GUILD_MEMBER_LIST_UPDATE", LIST_SYNC_FRAME.d);
    const list = /** @type {any} */ (store.lists.get(String(LIST_SYNC_FRAME.d.guild_id)));
    const userId = list.order[3];
    const d = {
      guild_id: String(LIST_SYNC_FRAME.d.guild_id),
      id: "everyone",
      ops: [{ op: "INSERT", index: 0, item: { member: { user: { id: userId, username: "dup" } } } }],
    };
    const result = apply(store, "GUILD_MEMBER_LIST_UPDATE", d);
    assert.equal(result.counters.duplicate, 1, "повторный INSERT не учтён");
    assert.equal(list.order.filter((/** @type {string} */ id) => id === userId).length, 1, "появился дубль userId");
    assert.equal(new Set(list.order).size, list.order.length, "в order есть дубли");
  });

  it("B9: DELETE вне окна — outOfRange, порядок не меняется", () => {
    const store = createDispatchStore();
    apply(store, "GUILD_MEMBER_LIST_UPDATE", LIST_SYNC_FRAME.d);
    const list = /** @type {any} */ (store.lists.get(String(LIST_SYNC_FRAME.d.guild_id)));
    const before = [...list.order];
    const d = {
      guild_id: String(LIST_SYNC_FRAME.d.guild_id),
      id: "everyone",
      ops: [{ op: "DELETE", index: 9_999 }],
    };
    const result = apply(store, "GUILD_MEMBER_LIST_UPDATE", d);
    assert.equal(result.counters.outOfRange, 1);
    assert.deepEqual(list.order, before, "DELETE вне окна изменил порядок");
  });

  it("B10: PRESENCE — LWW по ts, равный ts применяется, отрицательный/строковый ts", () => {
    const store = createDispatchStore();
    const base = PRESENCE_FRAME.d;
    const first = apply(store, "PRESENCE_UPDATE", base);
    assert.equal(first.changes.length, 1);
    const key = `${base.guild_id}:${base.user.id}`;
    const stored = /** @type {any} */ (store.presences.get(key));
    assert.equal(stored.status, "online");
    assert.equal(stored.activity, "bg-capture");
    assert.equal(stored.ts, Number(base.processed_at_timestamp), "ts-строка не распознана");

    const older = { ...base, processed_at_timestamp: String(Number(base.processed_at_timestamp) - 1), status: "dnd" };
    const stale = apply(store, "PRESENCE_UPDATE", older);
    assert.equal(stale.counters.stale, 1, "старое событие не отброшено");
    assert.equal(/** @type {any} */ (store.presences.get(key)).status, "online");

    const same = { ...base, status: "idle" };
    apply(store, "PRESENCE_UPDATE", same);
    assert.equal(/** @type {any} */ (store.presences.get(key)).status, "idle", "равный ts не применился");

    // Отрицательный ts — валидное число: проверяем на свежем store (иначе LWW
    // справедливо отбросит его как старое событие).
    const negativeStore = createDispatchStore();
    const negative = { ...base, processed_at_timestamp: "-1", status: "offline" };
    const negativeResult = apply(negativeStore, "PRESENCE_UPDATE", negative);
    assert.equal(negativeResult.changes.length, 1, "отрицательный ts отброшен");
    assert.equal(/** @type {any} */ (negativeStore.presences.get(key)).ts, -1);

    const noTs = { ...base, processed_at_timestamp: "nil", status: "dnd" };
    apply(store, "PRESENCE_UPDATE", noTs);
    assert.equal(/** @type {any} */ (store.presences.get(key)).status, "dnd", "ts=null не применился без сравнения");
  });

  it("B11: REACTION ADD/REMOVE — ±1, me по selfUserId, повтор ADD — no-op", () => {
    const store = createDispatchStore();
    store.selfUserId = SELF_ID;
    const add = apply(store, "MESSAGE_REACTION_ADD", reactionFrame("ADD", { userId: "777" }));
    assert.equal(add.changes.length, 1);
    let change = /** @type {any} */ (add.changes[0]);
    assert.equal(change.type, "reaction");
    assert.equal(change.count, 1);
    assert.equal(change.me, false);
    assert.equal(change.removed, false);

    const repeat = apply(store, "MESSAGE_REACTION_ADD", reactionFrame("ADD", { userId: "777" }));
    assert.deepEqual(repeat.changes, [], "повтор ADD создал вторую пилюлю");
    assert.equal(repeat.counters.duplicate, 1);

    const self = apply(store, "MESSAGE_REACTION_ADD", reactionFrame("ADD", { userId: SELF_ID }));
    change = /** @type {any} */ (self.changes[0]);
    assert.equal(change.count, 2);
    assert.equal(change.me, true, "me не по selfUserId");

    const remove = apply(store, "MESSAGE_REACTION_REMOVE", reactionFrame("REMOVE", { userId: "777" }));
    change = /** @type {any} */ (remove.changes[0]);
    assert.equal(change.count, 1);
    assert.equal(change.removed, true);
  });

  it("B12: REMOVE без ADD — счётчик не уходит в минус", () => {
    const store = createDispatchStore();
    const result = apply(store, "MESSAGE_REACTION_REMOVE", reactionFrame("REMOVE", { userId: "555" }));
    assert.deepEqual(result.changes, [], "REMOVE неизвестного пользователя изменил счётчик");
    assert.equal(result.counters.ignored, 1);
  });

  it("B13: malformed-таблица (null/[]/\"x\"/{}) для каждого kind", () => {
    for (const kind of FORWARDED_KINDS) {
      for (const data of [null, [], "x", {}]) {
        const store = createDispatchStore();
        const result = apply(store, kind, data);
        assert.deepEqual(result.changes, [], `${kind} принял ${JSON.stringify(data)}`);
        assert.equal(result.counters.malformed, 1, `${kind}: ${JSON.stringify(data)} не malformed`);
        assert.equal(result.counters.applied, 0, `${kind}: битые данные отмечены applied`);
      }
    }
  });

  it("B14: неизвестный kind — ignored, изменений нет", () => {
    for (const kind of ["TYPING_START", "MESSAGE_CREATE", "CHANNEL_INFO", "READY_SUPPLEMENTAL", "X", ""]) {
      const store = createDispatchStore();
      const result = apply(store, kind, { anything: true });
      assert.deepEqual(result.changes, [], `${kind} дал изменения`);
      assert.equal(result.counters.ignored, 1, `${kind} не ignored`);
    }
  });

  it("B15: секреты READY не попадают в store/changes, redactedKeys учтён", () => {
    const store = createDispatchStore();
    const result = apply(store, "READY", READY_FRAME.d);
    const dump = JSON.stringify({ store, changes: result.changes });
    for (const key of ["session_id", "qos_token", "resume_gateway_url", "auth_session_id_hash", "analytics_token"]) {
      const value = READY_FRAME.d[key];
      assert.equal(typeof value, "string", `оракул ${key} изменился`);
      assert.equal(dump.includes(value), false, `значение ${key} попало в store/changes`);
    }
    assert.ok(result.counters.redactedKeys >= 5, "redactedKeys не увидел секреты READY");
  });

  it("B16: непустой READY → пустой READY — полный снапшот очищает store", () => {
    const store = createDispatchStore();
    assert.equal(apply(store, "READY", READY_FRAME.d).changes.length, 1);
    const empty = { ...READY_FRAME.d, guilds: [] };

    const result = apply(store, "READY", empty);
    assert.equal(result.changes.length, 1, "пустой READY не применён как полный снапшот");
    const change = /** @type {any} */ (result.changes[0]);
    assert.equal(change.type, "guilds");
    assert.deepEqual(change.guilds, [], "в снапшоте остались гильдии");
    assert.equal(store.guilds.size, 0, "store.guilds не очищен");
    assert.deepEqual(store.order, [], "store.order не очищен");
    assert.equal(store.source, "dispatch", "пустой READY не отмечен как данные из dispatch");

    const again = apply(store, "READY", empty);
    assert.deepEqual(again.changes, [], "повтор пустого READY пересобрал UI");
    assert.equal(again.counters.duplicate, 1, "повтор пустого READY не учтён как duplicate");
  });

  it("B16b: первый пустой READY на чистом store — снапшот, а не duplicate", () => {
    // Store пуст и до dispatch, но рейл мог наполнить `ready()` IPC: первый
    // пустой READY обязан дойти до view (P2 REVIEW-L02-UI).
    const store = createDispatchStore();
    const result = apply(store, "READY", { ...READY_FRAME.d, guilds: [] });
    assert.equal(result.changes.length, 1, "первый пустой READY проглочен как duplicate");
    assert.deepEqual(/** @type {any} */ (result.changes[0]).guilds, [], "снапшот непустой");
    assert.equal(store.source, "dispatch", "источник не отмечен");
  });

  it("B17: self ID — string точно, safe integer допустим, unsafe integer отвергнут", () => {
    const unsafe = 384299970632876032;
    const snowflake = "384299970632876032";
    // Оракул: JS теряет точность ещё до String() — принять такой number нельзя.
    assert.notEqual(String(unsafe), snowflake, "оракул не подтверждает потерю точности");

    const store = createDispatchStore();
    setDispatchSelfUserId(store, unsafe);
    assert.equal(store.selfUserId, "", "unsafe integer превращён в строку вслепую");
    setDispatchSelfUserId(store, 42);
    assert.equal(store.selfUserId, "42", "safe integer отвергнут");
    setDispatchSelfUserId(store, snowflake);
    assert.equal(store.selfUserId, snowflake, "string snowflake потерял точность");

    // READY dispatch fallback: `d.user.id` тоже не стринговать вслепую.
    const fallback = createDispatchStore();
    const data = { ...READY_FRAME.d, user: { ...READY_FRAME.d.user, id: unsafe } };
    apply(fallback, "READY", data);
    assert.equal(fallback.selfUserId, "", "READY fallback стринговал unsafe user.id");
  });
});

/* =============================================================================
 * D. Транспорт (юнит, fake listen)
 * ========================================================================== */

/**
 * @param {{
 *   listen?: ((event: string, handler: (payload: any) => void) => Promise<() => void>) | null,
 *   view?: any,
 * }} [options]
 */
function makeBridge(options = {}) {
  /** @type {{ handler: ((payload: any) => void) | null, unlisten: number, calls: string[] }} */
  const transport = { handler: null, unlisten: 0, calls: [] };
  const listen = options.listen === undefined
    ? (event, handler) => {
      transport.calls.push(event);
      transport.handler = handler;
      return Promise.resolve(() => { transport.unlisten += 1; });
    }
    : options.listen;
  const view = options.view ?? {
    calls: [],
    setSource() {},
    /** @param {any[]} changes */
    apply(changes) {
      this.calls.push(changes);
      return { applied: changes.length, ignored: 0, failed: 0, notOpen: 0, notLoaded: 0 };
    },
  };
  const bridge = createDispatchBridge({ listen, view });
  return { bridge, view, transport };
}

describe("L-02-UI: транспорт bridge (D)", () => {
  it("D1: start() подписывается на gateway://dispatch, колбэк применяет конверт", async () => {
    const { bridge, view, transport } = makeBridge();
    await bridge.start();
    assert.deepEqual(transport.calls, [DISPATCH_EVENT], "подписка не на gateway://dispatch");
    assert.equal(typeof transport.handler, "function", "listen не получил колбэк");
    /** @type {(payload: any) => void} */ (transport.handler)({
      event: DISPATCH_EVENT,
      payload: envelope("PRESENCE_UPDATE", PRESENCE_FRAME.d),
    });
    assert.equal(view.calls.length, 1, "Tauri-конверт {event,payload} не разобран");
    assert.equal(bridge.stats.received, 1);
    assert.equal(bridge.stats.applied, 1);
    assert.equal(bridge.source, "live");
  });

  it("D2: stop() вызывает unlisten; повторный start() не создаёт вторую подписку", async () => {
    const { bridge, transport } = makeBridge();
    await bridge.start();
    await bridge.start();
    assert.equal(transport.calls.length, 1, "повторный start() подписался второй раз");
    await bridge.stop();
    assert.equal(transport.unlisten, 1, "stop() не вызвал unlisten");
    await bridge.start();
    assert.equal(transport.calls.length, 2, "start() после stop() не подписался заново");
  });

  it("D3: ошибка view.apply — failed+lastErrorKind, следующее событие обрабатывается", async () => {
    let failures = 1;
    const view = {
      calls: [],
      setSource() {},
      /** @param {any[]} changes */
      apply(changes) {
        if (failures > 0) {
          failures -= 1;
          throw new Error("boom");
        }
        this.calls.push(changes);
        return { applied: changes.length, ignored: 0, failed: 0, notOpen: 0, notLoaded: 0 };
      },
    };
    const { bridge } = makeBridge({ view });
    await bridge.start();
    bridge.handle(envelope("PRESENCE_UPDATE", PRESENCE_FRAME.d));
    assert.equal(bridge.stats.failed, 1, "исключение view не учтено");
    assert.match(bridge.stats.lastErrorKind, /apply/i, "lastErrorKind не назвал сбой");
    bridge.handle(envelope("PRESENCE_UPDATE", PRESENCE_FRAME.d));
    assert.equal(bridge.stats.failed, 1, "второе событие тоже упало");
    assert.equal(bridge.stats.applied, 1, "после сбоя событие не обработалось");
  });

  it("D4: resolveDispatchListen без __TAURI__.event → null, с функцией — она же", () => {
    assert.equal(resolveDispatchListen(/** @type {any} */ ({})), null);
    assert.equal(resolveDispatchListen(/** @type {any} */ ({ __TAURI__: { core: {} } })), null);
    const listen = () => Promise.resolve(() => {});
    assert.equal(resolveDispatchListen(/** @type {any} */ ({ __TAURI__: { event: { listen } } })), listen);
  });

  it("D5: listen упал — source=unavailable, ошибка учтена, rejection не всплывает", async () => {
    const view = { calls: [], sources: [], setSource(source) { this.sources.push(source); }, apply() {
      return { applied: 0, ignored: 0, failed: 0, notOpen: 0, notLoaded: 0 };
    } };
    const { bridge } = makeBridge({ listen: () => Promise.reject(new Error("no ipc")), view });
    await bridge.start();
    assert.equal(bridge.source, "unavailable");
    assert.equal(bridge.stats.failed, 1);
    assert.match(bridge.stats.lastErrorKind, /listen/i);
    assert.equal(view.sources.at(-1), "unavailable", "view не узнал о недоступном транспорте");
  });
});

/* =============================================================================
 * C. Интеграция через настоящий app.js (fake events → DOM, без reload)
 * ========================================================================== */

/** `ready()` в форме `data.rs ReadyPayload`: 7 гильдий READY и вход `42`. */
function readyPayload() {
  const guilds = READY_FRAME.d.guilds.map((/** @type {any} */ guild) => ({
    id: String(guild.id),
    name: String(guild.properties?.name ?? ""),
    icon: null,
    memberCount: Number(guild.member_count),
    channelCount: 2,
  }));
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
    guilds,
    guildCount: guilds.length,
    privateChannelCount: 0,
    relationshipCount: 0,
    readStateCount: 0,
    session: null,
  };
}

/** Одна строка `messages_page` (`data.rs MessageRow`, camelCase). */
function messagesPagePayload(reactions = []) {
  return {
    channelId: CHANNEL,
    items: [{
      id: MESSAGE_ID,
      channelId: CHANNEL,
      guildId: GUILD_MAIN,
      author: { id: "777", username: "mira", globalName: null, avatar: null, bot: false },
      content: "строка для live-реакций",
      timestamp: "2026-10-23T20:05:00.000Z",
      edited: false,
      editedAt: null,
      day: "2026-10-23",
      group: "start",
      pinned: false,
      messageType: 0,
      embeds: [],
      attachments: [],
      reactions,
      replyTo: null,
      mentions: [],
      mentionEveryone: false,
    }],
    nextCursor: null,
    hasMore: false,
    total: 1,
    limit: 100,
    loadedRowsLimit: 500,
  };
}

/**
 * Приложение с управляемым IPC (вход пройден, канал C-02 доступен).
 * `readyUserId` подменяет `user.id` в ответе `ready()` (граница IPC в C16).
 * @param {{ reactions?: any[], readyUserId?: any }} [options]
 * @returns {Promise<GuiDom>}
 */
function loadDispatchApp(options = {}) {
  const ready = readyPayload();
  if ("readyUserId" in options) ready.user.id = options.readyUserId;
  return loadApp({
    invokeHandler: async (cmd, args) => {
      if (cmd === "auth_status") {
        return { status: "ready", stored: true, user: { id: SELF_ID, username: "tester" } };
      }
      if (cmd === "ready") return ready;
      if (cmd === "channels") {
        const guildId = String(args?.guildId ?? "");
        if (guildId === GUILD_MAIN) {
          return [{ id: CHANNEL, guildId, name: "general", type: 0, parentId: null, position: 0 }];
        }
        return [];
      }
      if (cmd === "messages_page") return messagesPagePayload(options.reactions ?? []);
      if (cmd === "read_state") return [];
      if (cmd === "ack_message" || cmd === "edit_message" || cmd === "delete_message" || cmd === "reply_message") {
        return null;
      }
      if (cmd === "send_message") {
        return { id: "1561627999999999999", channel_id: args?.channelId, content: args?.content };
      }
      return undefined;
    },
  });
}

/** @param {GuiDom} app @returns {any} */
function appApi(app) {
  const instance = app.appModule?.currentApp?.();
  assert.ok(instance, "app.js не экспортирует currentApp()");
  assert.ok(instance.dispatch, "currentApp().dispatch не собран (L-02-UI не подключён)");
  return instance;
}

/**
 * @param {GuiDom} app
 * @param {string} kind
 * @param {any} data
 */
function send(app, kind, data) {
  return appApi(app).dispatch.handle({ kind, data });
}

/** @param {GuiDom} app @param {string} id @returns {ShimElement} */
function byId(app, id) {
  const node = app.document.getElementById(id);
  assert.ok(node, `нет #${id}`);
  return node;
}

/** @param {GuiDom} app @param {string} selector @returns {ShimElement[]} */
function all(app, selector) {
  return /** @type {ShimElement[]} */ ([...app.document.querySelectorAll(selector)]);
}

/** @param {GuiDom} app @param {string} selector @returns {ShimElement} */
function first(app, selector) {
  const node = app.document.querySelector(selector);
  assert.ok(node, `нет ${selector}`);
  return node;
}

/**
 * @param {GuiDom} app
 * @param {string} guildId
 */
async function selectGuild(app, guildId) {
  const tile = all(app, "#guild-rail .guild-item[data-guild-id]").find(
    (node) => node.getAttribute("data-guild-id") === guildId,
  );
  assert.ok(tile, `в рейле нет гильдии ${guildId}`);
  tile.click();
  await app.settle(8);
}

/** @param {GuiDom} app */
async function openMainChannel(app) {
  await selectGuild(app, GUILD_MAIN);
  const button = all(app, ".channel-item").find(
    (node) => node.getAttribute("data-channel-id") === CHANNEL,
  );
  assert.ok(button, `в дереве нет канала ${CHANNEL}`);
  button.click();
  await app.settle(10);
  assert.equal(appApi(app).chat.mode, "channel", "реальный канал не открылся");
}

/** @param {GuiDom} app @param {string} messageId @returns {ShimElement} */
function chatRow(app, messageId) {
  const row = all(app, "#chat-list .row").find(
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
  const pill = row.querySelectorAll(".reaction").find(
    (node) => node.getAttribute("data-emoji") === emoji,
  );
  assert.ok(pill, `нет пилюли ${emoji}`);
  return pill;
}

/** Всё текстовое и атрибутное содержимое документа (оракул N7). */
function domDump(doc) {
  /** @type {string[]} */
  const parts = [];
  for (const node of doc.querySelectorAll("*")) {
    parts.push(node.textContent ?? "");
    for (const [name, value] of node.attributes) parts.push(`${name}=${value}`);
  }
  return parts.join("\n");
}

describe("L-02-UI: интеграция dispatch → DOM (C1–C14)", () => {
  it("C1: READY upsert-ит рейл без смены выбора/фокуса и без IPC", async () => {
    const app = await loadDispatchApp();
    try {
      const before = {
        ready: app.callsOf("ready").length,
        channels: app.callsOf("channels").length,
        messages: app.callsOf("messages_page").length,
      };
      const selected = first(app, ".guild-item.is-selected");
      const focus = app.document.activeElement;
      const data = readyWithMembersGuild();

      const result = send(app, "READY", data);
      assert.equal(result.changes.length, 1, "READY не применён");

      const tiles = all(app, "#guild-rail .guild-item[data-guild-id]");
      assert.equal(tiles.length, data.guilds.length, "число плиток не равно снапшоту READY");
      assert.deepEqual(
        tiles.map((tile) => tile.getAttribute("data-guild-id")),
        data.guilds.map((/** @type {any} */ guild) => String(guild.id)),
        "порядок плиток не из payload",
      );
      for (const [index, tile] of tiles.entries()) {
        const guild = /** @type {any} */ (data.guilds[index]);
        assert.equal(
          tile.getAttribute("aria-label"),
          `Сервер ${guild.properties.name}`,
          "aria-label плитки не из READY",
        );
        assert.equal(tile.textContent, tile.textContent?.toUpperCase(), "монограмма не верхний регистр");
      }
      assert.equal(all(app, ".guild-item.is-selected").length, 1, "выбрана не одна плитка");
      assert.equal(first(app, ".guild-item.is-selected"), selected, "READY пересоздал выбранную плитку (фокус потерян)");
      assert.equal(selected.getAttribute("aria-current"), "true", "aria-current снят с выбранной плитки");
      assert.equal(app.document.activeElement, focus, "READY перевёл фокус");
      assert.deepEqual(
        {
          ready: app.callsOf("ready").length,
          channels: app.callsOf("channels").length,
          messages: app.callsOf("messages_page").length,
        },
        before,
        "READY вызвал reload-IPC",
      );
      assert.equal(byId(app, "member-online").textContent, formatNumber(Number(data.guilds[0].member_count)));
    } finally {
      app.dispose();
    }
  });

  it("C2: LIST_UPDATE → live-панель, порядок/высота/счётчик без reload", async () => {
    const app = await loadDispatchApp();
    try {
      send(app, "READY", readyWithMembersGuild());
      await selectGuild(app, GUILD_MEMBERS);
      const before = {
        ready: app.callsOf("ready").length,
        channels: app.callsOf("channels").length,
        messages: app.callsOf("messages_page").length,
      };
      const result = send(app, "GUILD_MEMBER_LIST_UPDATE", LIST_SYNC_FRAME.d);
      assert.equal(result.changes.length, 1);

      const panel = byId(app, "member-panel-list");
      assert.equal(panel.getAttribute("data-source"), "live", "панель не переведена в live");
      assert.equal(panel.getAttribute("aria-label"), "Участники сервера", "live-панель с aria-label bench");
      const expected = oracleOrder([], [LIST_SYNC_FRAME.d.ops[0]]);
      const rows = all(app, "#member-panel-list .member[data-user-id]");
      assert.equal(rows.length, expected.length, "число live-строк не равно окну order");
      assert.deepEqual(
        rows.map((row) => row.getAttribute("data-user-id")),
        expected,
        "порядок live-строк не order",
      );
      for (const [index, row] of rows.entries()) {
        assert.equal(row.getAttribute("role"), "listitem", "live-строка без role=listitem");
        assert.equal(row.style.height, "42px", "высота live-строки не 42 (D-04)");
        assert.equal(row.getAttribute("aria-setsize"), String(expected.length), "aria-setsize не размер окна");
        assert.equal(row.getAttribute("aria-posinset"), String(index + 1));
        assert.ok(row.getAttribute("data-status"), "у live-строки нет data-status");
        assert.ok(row.querySelector(".status-dot"), "у live-строки нет .status-dot");
        assert.match(row.getAttribute("aria-label") ?? "", /, /, "aria-label без статуса (MEM-18)");
        assert.equal(row.getAttribute("data-group"), null, "live-строка попала в bench-группу");
      }
      assert.equal(
        byId(app, "member-online").textContent,
        formatNumber(Number(LIST_SYNC_FRAME.d.online_count)),
        "#member-online не online_count",
      );
      assert.equal(panel.getAttribute("data-live-order"), String(expected.length));
      // READY после LIST_UPDATE не перебивает точный online_count (PLAN §3.1).
      const repeated = readyWithMembersGuild();
      const added = repeated.guilds.find((/** @type {any} */ guild) => String(guild.id) === GUILD_MEMBERS);
      if (added) added.member_count = 999;
      send(app, "READY", repeated);
      assert.equal(
        byId(app, "member-online").textContent,
        formatNumber(Number(LIST_SYNC_FRAME.d.online_count)),
        "повторный READY перебил online_count из LIST_UPDATE",
      );
      assert.deepEqual(
        {
          ready: app.callsOf("ready").length,
          channels: app.callsOf("channels").length,
          messages: app.callsOf("messages_page").length,
        },
        before,
        "LIST_UPDATE вызвал reload-IPC",
      );
    } finally {
      app.dispose();
    }
  });

  it("C3: CHUNK после LIST_UPDATE — имена у тех же строк, без роста списка", async () => {
    const app = await loadDispatchApp();
    try {
      send(app, "READY", readyWithMembersGuild());
      await selectGuild(app, GUILD_MEMBERS);
      send(app, "GUILD_MEMBER_LIST_UPDATE", LIST_SYNC_FRAME.d);
      const order = oracleOrder([], [LIST_SYNC_FRAME.d.ops[0]]);
      const targetId = /** @type {string} */ (order[0]);
      const before = all(app, "#member-panel-list .member[data-user-id]");
      const target = /** @type {ShimElement} */ (
        before.find((row) => row.getAttribute("data-user-id") === targetId)
      );

      const chunk = cloneFrame("009-GUILD_MEMBERS_CHUNK.json");
      chunk.d.members[0].user.id = targetId;
      send(app, "GUILD_MEMBERS_CHUNK", chunk.d);

      const after = all(app, "#member-panel-list .member[data-user-id]");
      assert.equal(after.length, before.length, "CHUNK изменил число строк окна");
      const same = after.find((row) => row.getAttribute("data-user-id") === targetId);
      assert.equal(same, target, "CHUNK пересоздал строку (потеря идентичности узла)");
      assert.equal(
        target.querySelector(".member-label")?.textContent,
        "Новости",
        "имя не обновилось по приоритету global_name→nick→username",
      );
      assert.match(target.getAttribute("aria-label") ?? "", /^Новости, /, "aria-label не пересобран");
    } finally {
      app.dispose();
    }
  });

  it("C4: PRESENCE online→idle→online — тот же узел, статус в данных и aria", async () => {
    const app = await loadDispatchApp();
    try {
      send(app, "READY", readyWithMembersGuild());
      await selectGuild(app, GUILD_MEMBERS);
      send(app, "GUILD_MEMBER_LIST_UPDATE", LIST_SYNC_FRAME.d);
      const order = oracleOrder([], [LIST_SYNC_FRAME.d.ops[0]]);
      const userId = /** @type {string} */ (order[0]);
      const panel = byId(app, "member-panel-list");
      const chatList = byId(app, "chat-list");
      const chatScroll = chatList.scrollTop;
      const focus = app.document.activeElement;
      const screen = app.document.body.getAttribute("data-active-screen");
      const row = /** @type {ShimElement} */ (
        all(app, "#member-panel-list .member[data-user-id]").find(
          (node) => node.getAttribute("data-user-id") === userId,
        )
      );
      assert.ok(row, "нет строки для presence");

      /** @param {string} status @param {number} ts */
      const presence = (status, ts) => {
        const d = cloneFrame("010-PRESENCE_UPDATE.json").d;
        d.guild_id = GUILD_MEMBERS;
        d.user.id = userId;
        d.status = status;
        d.client_status = { desktop: status };
        d.processed_at_timestamp = String(ts);
        return d;
      };

      send(app, "PRESENCE_UPDATE", presence("online", 1_000));
      assert.equal(row.getAttribute("data-status"), "online");
      send(app, "PRESENCE_UPDATE", presence("idle", 1_001));
      assert.equal(row.getAttribute("data-status"), "idle", "data-status не idle");
      assert.equal(row.querySelector(".status-dot")?.getAttribute("data-status"), "idle", ".status-dot не обновлён");
      assert.match(row.getAttribute("aria-label") ?? "", /неактивен/, "статус не попал в доступное имя");
      assert.equal(
        all(app, "#member-panel-list .member[data-user-id]").find((node) => node.getAttribute("data-user-id") === userId),
        row,
        "presence пересоздал строку",
      );

      send(app, "PRESENCE_UPDATE", presence("online", 1_002));
      assert.equal(row.getAttribute("data-status"), "online");
      assert.equal(panel.scrollTop, 0, "presence сдвинул скролл панели");
      assert.equal(chatList.scrollTop, chatScroll, "presence сдвинул скролл чата");
      assert.equal(app.document.activeElement, focus, "presence перевёл фокус");
      assert.equal(app.document.body.getAttribute("data-active-screen"), screen, "presence переключил экран");
    } finally {
      app.dispose();
    }
  });

  it("C5–C7: реакции — ADD чужой/себя, REMOVE до нуля удаляет пилюлю", async () => {
    const app = await loadDispatchApp();
    try {
      send(app, "READY", readyWithMembersGuild());
      await openMainChannel(app);
      const row = chatRow(app, MESSAGE_ID);

      send(app, "MESSAGE_REACTION_ADD", reactionFrame("ADD", { userId: "777" }));
      let pill = pillOf(row, "🔥");
      assert.equal(pill.getAttribute("aria-pressed"), "false", "чужая реакция стала своей");
      assert.equal(pill.getAttribute("aria-label"), "Реакция 🔥, 1");
      assert.equal(pill.querySelector(".reaction-count")?.textContent, "1");
      assert.equal(row.querySelector(".text")?.getAttribute("data-stack"), "true", ".text без data-stack");
      assert.ok(row.querySelector(".text .reactions-row"), "нет .reactions-row в .text");

      send(app, "MESSAGE_REACTION_ADD", reactionFrame("ADD", { userId: SELF_ID }));
      pill = pillOf(row, "🔥");
      assert.equal(pill.getAttribute("aria-pressed"), "true", "своя реакция не aria-pressed");
      assert.equal(pill.getAttribute("data-me"), "true");
      assert.equal(pill.querySelector(".reaction-count")?.textContent, "2");

      send(app, "MESSAGE_REACTION_REMOVE", reactionFrame("REMOVE", { userId: "777" }));
      pill = pillOf(row, "🔥");
      assert.equal(pill.querySelector(".reaction-count")?.textContent, "1");
      assert.equal(pill.getAttribute("aria-pressed"), "true", "снятие чужой реакции сняло мою");

      send(app, "MESSAGE_REACTION_REMOVE", reactionFrame("REMOVE", { userId: SELF_ID }));
      assert.equal(row.querySelectorAll(".reaction").length, 0, "пилюля с count=0 осталась призраком");
      assert.equal(
        row.querySelectorAll('.reaction[aria-pressed="true"]').length,
        0,
        "aria-pressed=true остался после удаления",
      );
    } finally {
      app.dispose();
    }
  });

  it("C5b: live-реакция прибавляется к базовому счётчику messages_page", async () => {
    const app = await loadDispatchApp({ reactions: [{ emoji: "🔥", count: 2, me: false }] });
    try {
      send(app, "READY", readyWithMembersGuild());
      await openMainChannel(app);
      const row = chatRow(app, MESSAGE_ID);
      assert.equal(pillOf(row, "🔥").querySelector(".reaction-count")?.textContent, "2", "базовая реакция не отрисована");

      send(app, "MESSAGE_REACTION_ADD", reactionFrame("ADD", { userId: "777" }));
      assert.equal(
        pillOf(row, "🔥").querySelector(".reaction-count")?.textContent,
        "3",
        "live ADD не прибавился к базе (второй источник истины)",
      );
      send(app, "MESSAGE_REACTION_REMOVE", reactionFrame("REMOVE", { userId: "777" }));
      assert.equal(pillOf(row, "🔥").querySelector(".reaction-count")?.textContent, "2", "REMOVE не вернул базу");
    } finally {
      app.dispose();
    }
  });

  it("C8: реакция чужого канала/незагруженного сообщения — DOM не тронут", async () => {
    const app = await loadDispatchApp();
    try {
      send(app, "READY", readyWithMembersGuild());
      await openMainChannel(app);
      const list = byId(app, "chat-list");
      const children = [...list.children];
      const text = first(app, "#chat-list .row .text").textContent;

      send(app, "MESSAGE_REACTION_ADD", reactionFrame("ADD", { channelId: "999" }));
      assert.equal(
        appApi(app).dispatch.stats.notOpen,
        1,
        "событие чужого канала не помечено not-open",
      );
      send(app, "MESSAGE_REACTION_ADD", reactionFrame("ADD", { messageId: "999" }));
      assert.equal(
        appApi(app).dispatch.stats.notLoaded,
        1,
        "событие незагруженного сообщения не помечено not-loaded",
      );
      assert.deepEqual([...list.children], children, "#chat-list пересобран чужой реакцией");
      assert.equal(first(app, "#chat-list .row .text").textContent, text, "текст строки изменился");
      assert.equal(app.callsOf("messages_page").length, 1, "реакция вызвала reload истории");
    } finally {
      app.dispose();
    }
  });

  it("C9: секреты READY не попадают ни в DOM, ни в снапшот", async () => {
    const app = await loadDispatchApp();
    try {
      send(app, "READY", readyWithMembersGuild());
      const dump = domDump(app.document);
      for (const key of ["session_id", "qos_token", "resume_gateway_url", "auth_session_id_hash", "analytics_token"]) {
        const value = READY_FRAME.d[key];
        assert.equal(typeof value, "string", `оракул ${key} изменился`);
        assert.equal(dump.includes(value), false, `значение ${key} попало в DOM`);
      }
      const snapshot = JSON.stringify(app.snapshot.payload ?? {});
      for (const key of ["session_id", "qos_token", "resume_gateway_url"]) {
        assert.equal(snapshot.includes(READY_FRAME.d[key]), false, `значение ${key} попало в снапшот`);
      }
      assert.ok(appApi(app).dispatch.stats.redactedKeys >= 5, "redactedKeys не увидел секреты");
    } finally {
      app.dispose();
    }
  });

  it("C10: N1/N2 — неизвестный kind и malformed не меняют DOM и не шумят", async () => {
    const app = await loadDispatchApp();
    try {
      const before = domDump(app.document);
      send(app, "TYPING_START", { channel_id: CHANNEL });
      send(app, "MESSAGE_CREATE", { channel_id: CHANNEL });
      send(app, "X", {});
      send(app, "PRESENCE_UPDATE", null);
      assert.equal(domDump(app.document), before, "N1/N2 изменили DOM");
      assert.equal(app.consoleErrors.length, 0, "dispatch пишет в console.error");
      assert.equal(appApi(app).dispatch.stats.ignored, 3, "ignored не 3");
      assert.equal(appApi(app).dispatch.stats.malformed, 1, "malformed не 1");

      // Подписка/канал живы после мусора.
      const result = send(app, "READY", readyWithMembersGuild());
      assert.equal(result.changes.length, 1, "после мусора READY не применился");
    } finally {
      app.dispose();
    }
  });

  it("C11: burst 100 PRESENCE — 100 применений, строки не пересозданы", async () => {
    const app = await loadDispatchApp();
    try {
      send(app, "READY", readyWithMembersGuild());
      await selectGuild(app, GUILD_MEMBERS);
      send(app, "GUILD_MEMBER_LIST_UPDATE", LIST_SYNC_FRAME.d);
      const order = oracleOrder([], [LIST_SYNC_FRAME.d.ops[0]]);
      const userId = /** @type {string} */ (order[0]);
      const rowsBefore = all(app, "#member-panel-list .member[data-user-id]");
      const node = /** @type {ShimElement} */ (
        rowsBefore.find((row) => row.getAttribute("data-user-id") === userId)
      );
      const statuses = ["online", "idle", "dnd", "offline"];
      const appliedBefore = appApi(app).dispatch.stats.applied;

      for (let index = 0; index < 100; index += 1) {
        const d = cloneFrame("010-PRESENCE_UPDATE.json").d;
        d.guild_id = GUILD_MEMBERS;
        d.user.id = userId;
        d.status = /** @type {string} */ (statuses[index % statuses.length]);
        d.processed_at_timestamp = String(index + 1);
        send(app, "PRESENCE_UPDATE", d);
      }
      const rowsAfter = all(app, "#member-panel-list .member[data-user-id]");
      assert.equal(rowsAfter.length, rowsBefore.length, "burst пересобрал список");
      assert.equal(
        rowsAfter.find((row) => row.getAttribute("data-user-id") === userId),
        node,
        "burst пересоздал строку",
      );
      assert.equal(node.getAttribute("data-status"), "offline", "последний статус не победил (99 → offline)");
      assert.equal(
        appApi(app).dispatch.stats.applied - appliedBefore,
        100,
        "применено не 100 presence-событий",
      );
      assert.equal(app.consoleErrors.length, 0, "burst дал console.error");
    } finally {
      app.dispose();
    }
  });

  it("C12: N11 — событие на экране settings обновляет DOM в фоне", async () => {
    const app = await loadDispatchApp();
    try {
      send(app, "READY", readyWithMembersGuild());
      await selectGuild(app, GUILD_MEMBERS);
      send(app, "GUILD_MEMBER_LIST_UPDATE", LIST_SYNC_FRAME.d);
      const order = oracleOrder([], [LIST_SYNC_FRAME.d.ops[0]]);
      const userId = /** @type {string} */ (order[0]);
      const settings = all(app, ".rail-btn").find((node) => node.getAttribute("data-screen") === "settings");
      assert.ok(settings, "нет кнопки settings");
      settings.click();
      assert.equal(app.document.body.getAttribute("data-active-screen"), "settings");

      const d = cloneFrame("010-PRESENCE_UPDATE.json").d;
      d.guild_id = GUILD_MEMBERS;
      d.user.id = userId;
      d.status = "idle";
      d.processed_at_timestamp = "5";
      send(app, "PRESENCE_UPDATE", d);

      const row = all(app, "#member-panel-list .member[data-user-id]").find(
        (node) => node.getAttribute("data-user-id") === userId,
      );
      assert.ok(row, "строка пропала, пока был открыт settings");
      assert.equal(row.getAttribute("data-status"), "idle", "фоновое обновление не применилось");
      assert.equal(app.document.body.getAttribute("data-active-screen"), "settings", "событие переключило экран");
    } finally {
      app.dispose();
    }
  });

  it("C13: N12 — без transport source=absent, handle работает (тестовый путь)", async () => {
    // Без invokeHandler: вход не выполнен, но bridge собран на boot; в шиме нет
    // `window.__TAURI__.event` → честный absent, а не молчаливое ожидание.
    const app = await loadApp();
    try {
      assert.equal(
        app.document.body.getAttribute("data-dispatch-source"),
        "absent",
        "body не сообщает об отсутствии транспорта",
      );
      assert.equal(
        byId(app, "net-status").getAttribute("data-dispatch-source"),
        "absent",
        "#net-status не сообщает об отсутствии транспорта",
      );
      const result = appApi(app).dispatch.handle(envelope("READY", readyWithMembersGuild()));
      assert.equal(result.changes.length, 1, "handle() без transport не работает");
      assert.equal(appApi(app).dispatch.source, "absent");
      assert.equal(app.consoleErrors.length, 0, "отсутствие транспорта дало console.error");
    } finally {
      app.dispose();
    }
  });

  it("C14: снапшот содержит dispatch без payload/секретов; живые счётчики в stats", async () => {
    const app = await loadDispatchApp();
    try {
      const payload = app.snapshot.payload;
      assert.ok(payload, "report_ui_snapshot не опубликован");
      assert.ok(payload.dispatch, "в снапшоте нет dispatch");
      assert.equal(typeof payload.dispatch.source, "string", "source не строка");
      assert.ok("received" in payload.dispatch, "нет счётчика received");
      assert.ok("malformed" in payload.dispatch, "нет счётчика malformed");
      assert.equal("kind" in payload.dispatch, false, "в снапшот попал kind payload");
      assert.equal("data" in payload.dispatch, false, "в снапшот попал data payload");

      send(app, "READY", readyWithMembersGuild());
      send(app, "PRESENCE_UPDATE", PRESENCE_FRAME.d);
      send(app, "TYPING_START", {});
      const stats = appApi(app).dispatch.stats;
      assert.equal(stats.received, 3, "живой received не читается из dispatch.stats");
      assert.equal(stats.applied, 2, "живой applied не читается");
      assert.equal(stats.ignored, 1, "живой ignored не читается");
      assert.equal(JSON.stringify(stats).includes(READY_FRAME.d.session_id), false, "stats утёк секретом");
    } finally {
      app.dispose();
    }
  });

  it("C15: пустой READY очищает рейл (полный снапшот), без reload-IPC", async () => {
    const app = await loadDispatchApp();
    try {
      send(app, "READY", readyWithMembersGuild());
      assert.ok(
        all(app, "#guild-rail .guild-item[data-guild-id]").length > 0,
        "подготовка: рейл пуст после непустого READY",
      );
      const before = {
        ready: app.callsOf("ready").length,
        channels: app.callsOf("channels").length,
        messages: app.callsOf("messages_page").length,
      };

      const result = send(app, "READY", { ...readyWithMembersGuild(), guilds: [] });
      assert.equal(result.changes.length, 1, "пустой READY не применён как полный снапшот");
      assert.equal(
        all(app, "#guild-rail .guild-item[data-guild-id]").length,
        0,
        "устаревшие плитки серверов остались в рейле",
      );
      assert.equal(first(app, "#guild-rail .guild-empty").textContent, "Нет серверов", "нет пустого состояния рейла");
      assert.ok(first(app, "#guild-rail .guild-add"), "плитка «Добавить сервер» пропала");
      assert.equal(appApi(app).dispatch.store.order.length, 0, "store.order не очищен");
      assert.deepEqual(
        {
          ready: app.callsOf("ready").length,
          channels: app.callsOf("channels").length,
          messages: app.callsOf("messages_page").length,
        },
        before,
        "пустой READY вызвал reload-IPC",
      );
    } finally {
      app.dispose();
    }
  });

  it("C15b: первый пустой READY после boot очищает рейл от ready() IPC", async () => {
    const app = await loadDispatchApp();
    try {
      assert.ok(
        all(app, "#guild-rail .guild-item[data-guild-id]").length > 0,
        "подготовка: ready() IPC не наполнил рейл",
      );
      const result = send(app, "READY", { ...readyWithMembersGuild(), guilds: [] });
      assert.equal(result.changes.length, 1, "первый пустой READY проглочен как duplicate");
      assert.equal(
        all(app, "#guild-rail .guild-item[data-guild-id]").length,
        0,
        "плитки, показанные ready() IPC, остались в рейле",
      );
      assert.equal(first(app, "#guild-rail .guild-empty").textContent, "Нет серверов", "нет пустого состояния");

      const repeat = send(app, "READY", { ...readyWithMembersGuild(), guilds: [] });
      assert.deepEqual(repeat.changes, [], "повтор пустого READY пересобрал рейл");
      assert.equal(repeat.counters.duplicate, 1, "повтор пустого READY не идемпотентен");
    } finally {
      app.dispose();
    }
  });

  it("C16: ready() self ID — string точно, unsafe integer отвергнут (без String())", async () => {
    const unsafe = 384299970632876032;
    const snowflake = "384299970632876032";
    assert.notEqual(String(unsafe), snowflake, "оракул не подтверждает потерю точности");

    const rejected = await loadDispatchApp({ readyUserId: unsafe });
    try {
      assert.equal(
        appApi(rejected).dispatch.store.selfUserId,
        "",
        "unsafe integer превращён в строку вслепую",
      );
      // Искажённый id не должен дать ложный `me` у реакций.
      const reaction = send(
        rejected,
        "MESSAGE_REACTION_ADD",
        reactionFrame("ADD", { userId: snowflake }),
      );
      assert.equal(
        /** @type {any} */ (reaction.changes[0]).me,
        false,
        "me выставлен по искажённому self ID",
      );
    } finally {
      rejected.dispose();
    }

    const accepted = await loadDispatchApp({ readyUserId: snowflake });
    try {
      assert.equal(
        appApi(accepted).dispatch.store.selfUserId,
        snowflake,
        "string snowflake потерян на границе ready()",
      );
    } finally {
      accepted.dispose();
    }
  });
});
