// @ts-check
/**
 * L-02-UI (RW-40): чистые проекции dispatch-кадра в состояние UI.
 *
 * Контракт — `waves/RW-40/PM/l02-ui/PLAN.md` §2–§6 и `CONTRACTS/dispatch.md` §3:
 * конверт ровно `{kind, data}` (поля `s`/`seq` в нём нет), поэтому
 * идемпотентность строится по бизнес-ключам, а не по номеру последовательности.
 *
 * Здесь нет DOM, IPC и глобалов: только нормализация payload, store и
 * `Change[]` — единственный шов model↔view. Идентификаторы остаются строками
 * (снежинки бывают i64/отрицательными — `Number()` теряет точность).
 */
import { STATUS_LABELS } from "./members.js";

/** Tauri-событие dispatch (`net::DISPATCH_EVENT`). */
export const DISPATCH_EVENT = "gateway://dispatch";

/** Единственные имена, которые L-02 форвардит (`net::FORWARDED_DISPATCHES`). */
export const FORWARDED_KINDS = Object.freeze([
  "READY",
  "GUILD_MEMBERS_CHUNK",
  "GUILD_MEMBER_LIST_UPDATE",
  "PRESENCE_UPDATE",
  "MESSAGE_REACTION_ADD",
  "MESSAGE_REACTION_REMOVE",
]);

/** Статусы присутствия из `[L]`-кадров; всё остальное — `unknown` (не offline). */
export const KNOWN_STATUSES = Object.freeze(["online", "idle", "dnd", "offline"]);

/** Подпись неизвестного статуса (MEM-18: статус в доступном имени). */
const UNKNOWN_STATUS_LABEL = "неизвестно";
const FORWARDED = new Set(FORWARDED_KINDS);
/** Bounded-набор ключей чанков (PLAN §3.2). */
const CHUNK_LIMIT = 512;
/** Сколько уровней рекурсии смотрим в поиске секретов (payload — JSON-дерево). */
const SECRET_SCAN_DEPTH = 32;
/** Ключи из политики `net.rs::redact_secrets` + `*_token`. */
const SECRET_KEYS = new Set([
  "session_id",
  "resume_gateway_url",
  "auth_session_id_hash",
  "client_secret",
  "password",
  "token",
  "access_token",
  "refresh_token",
]);

/* ---------- нормализация payload (PLAN §2) ---------- */

/** @param {unknown} value @returns {boolean} */
export function nilish(value) {
  return value === null || value === undefined || value === "nil";
}

/**
 * Строка: trim; `nilish` → ""; конечное число → String. Прочее (массивы,
 * объекты, NaN) — "" (не бросаем и не выдумываем значение).
 * @param {unknown} value
 * @returns {string}
 */
export function asString(value) {
  if (typeof value === "string") {
    const text = value.trim();
    return text === "nil" ? "" : text;
  }
  if (nilish(value)) return "";
  if (typeof value === "number" && Number.isFinite(value)) return String(value);
  return "";
}

/**
 * Идентификатор-снежинка без потери точности. Строка принимается как есть;
 * число — только `Number.isSafeInteger` (тогда `String()` точен); unsafe integer
 * отвергается пустой строкой: точность уже потеряна при разборе JSON, и
 * `String(value)` дал бы чужой id (P2 REVIEW-L02-UI).
 * @param {unknown} value
 * @returns {string}
 */
export function asSnowflake(value) {
  if (typeof value === "string") return asString(value);
  if (typeof value === "number" && Number.isSafeInteger(value)) return String(value);
  return "";
}

/**
 * Конечное число или строка `^[+-]?\d+(\.\d+)?$`; иначе `null`.
 * @param {unknown} value
 * @returns {number | null}
 */
export function asNumber(value) {
  if (typeof value === "number") return Number.isFinite(value) ? value : null;
  // Регулярка контракта якорная: пробелы/хвосты — не число (nil тоже не число).
  if (typeof value === "string" && /^[+-]?\d+(\.\d+)?$/.test(value)) {
    return Number(value);
  }
  return null;
}

/** @param {unknown} value @returns {boolean} */
export function asBool(value) {
  return value === true || value === 1 || value === "1" || value === "true";
}

/** @param {unknown} value @returns {unknown[]} */
export function asArray(value) {
  return Array.isArray(value) ? value : [];
}

/** Непустой объект (не массив) → он же; иначе `{}`. @param {unknown} value */
export function asRecord(value) {
  if (value === null || typeof value !== "object" || Array.isArray(value)) return {};
  const record = /** @type {Record<string, unknown>} */ (value);
  return Object.keys(record).length === 0 ? {} : record;
}

/* ---------- типы ---------- */

/** @typedef {{ id: string, name: string, memberCount: number | null }} GuildSnapshot */
/** @typedef {{
 *   userId: string,
 *   name: string,
 *   nick: string,
 *   bot: boolean,
 *   roles: string[],
 *   status: string,
 *   statusLabel: string,
 *   activity: string,
 * }} LiveMember */
/** @typedef {{
 *   status: string,
 *   statusLabel: string,
 *   clientStatus: Record<string, string>,
 *   activity: string,
 *   ts: number | null,
 * }} LivePresence */
/** @typedef {{
 *   listId: string,
 *   memberCount: number | null,
 *   onlineCount: number | null,
 *   order: string[],
 *   items: Map<string, LiveMember>,
 *   range: { start: number, end: number } | null,
 *   opsApplied: number,
 *   complete: boolean,
 * }} MemberList */
/** @typedef {{
 *   guilds: Map<string, GuildSnapshot>,
 *   order: string[],
 *   selfUserId: string,
 *   onlineCountKnown: boolean,
 *   source: string,
 *   members: Map<string, Map<string, LiveMember>>,
 *   chunks: string[],
 *   lists: Map<string, MemberList>,
 *   presences: Map<string, LivePresence>,
 *   reactions: Map<string, { users: Set<string>, anon: number }>,
 * }} DispatchStore */
/** @typedef {{ userId: string, name: string, status: string, statusLabel: string, activity: string }} MemberRow */
/** @typedef {{ type: "guilds", guilds: GuildSnapshot[], selfUserId: string }} GuildsChange */
/** @typedef {{
 *   type: "members",
 *   guildId: string,
 *   order: string[],
 *   rows: MemberRow[],
 *   listId: string,
 *   onlineCount?: number,
 *   memberCount?: number,
 * }} MembersChange */
/** @typedef {{
 *   type: "presence",
 *   guildId: string,
 *   userId: string,
 *   status: string,
 *   statusLabel: string,
 *   activity: string,
 * }} PresenceChange */
/** @typedef {{
 *   type: "reaction",
 *   channelId: string,
 *   messageId: string,
 *   emoji: string,
 *   count: number,
 *   me: boolean,
 *   removed: boolean,
 * }} ReactionChange */
/** @typedef {GuildsChange | MembersChange | PresenceChange | ReactionChange} DispatchChange */
/** @typedef {{
 *   applied: number,
 *   malformed: number,
 *   ignored: number,
 *   duplicate: number,
 *   stale: number,
 *   outOfRange: number,
 *   skipped: number,
 *   degraded: number,
 *   unknownStatus: number,
 *   notLoaded: number,
 *   notOpen: number,
 *   redactedKeys: number,
 * }} ModelCounters */

/** @returns {ModelCounters} */
function emptyCounters() {
  return {
    applied: 0,
    malformed: 0,
    ignored: 0,
    duplicate: 0,
    stale: 0,
    outOfRange: 0,
    skipped: 0,
    degraded: 0,
    unknownStatus: 0,
    notLoaded: 0,
    notOpen: 0,
    redactedKeys: 0,
  };
}

/* ---------- конверт ---------- */

/**
 * Конверт dispatch: `{kind, data}` (+ необязательный `seq`, если ядро его
 * когда-нибудь добавит — UI использует как дополнительный ключ, не ломаясь).
 * @param {unknown} value
 * @returns {{ ok: true, kind: string, data: Record<string, unknown>, seq?: number } | { ok: false, reason: string }}
 */
export function normalizeEnvelope(value) {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    return { ok: false, reason: "not-object" };
  }
  const record = /** @type {Record<string, unknown>} */ (value);
  if (!("kind" in record)) return { ok: false, reason: "kind" };
  const kind = asString(record.kind);
  // Пустая строка — «неизвестный kind» (N1: ignored), а не битый конверт.
  if (kind === "" && typeof record.kind !== "string") return { ok: false, reason: "kind" };
  const data = record.data;
  if (data === null || typeof data !== "object" || Array.isArray(data)) {
    return { ok: false, reason: "data" };
  }
  /** @type {{ ok: true, kind: string, data: Record<string, unknown>, seq?: number }} */
  const result = { ok: true, kind, data: /** @type {Record<string, unknown>} */ (data) };
  const seq = asNumber(record.seq);
  return seq === null ? result : { ...result, seq };
}

/* ---------- store ---------- */

/** @returns {DispatchStore} */
export function createDispatchStore() {
  return {
    guilds: new Map(),
    order: [],
    selfUserId: "",
    onlineCountKnown: false,
    source: "",
    members: new Map(),
    chunks: [],
    lists: new Map(),
    presences: new Map(),
    reactions: new Map(),
  };
}

/**
 * Авторитетный `selfUserId` — из `ready()` IPC; READY dispatch заполняет поле
 * только если оно пустое (PLAN §3.1: фикстура принадлежит другому аккаунту).
 * @param {DispatchStore} store
 * @param {unknown} value
 */
export function setDispatchSelfUserId(store, value) {
  const id = asSnowflake(value);
  if (id !== "") store.selfUserId = id;
}

/* ---------- внутренние хелперы ---------- */

/**
 * @param {string} status
 * @returns {string}
 */
function statusLabelOf(status) {
  if (KNOWN_STATUSES.includes(status)) return STATUS_LABELS[status] ?? status;
  return UNKNOWN_STATUS_LABEL;
}

/**
 * @param {unknown} status
 * @param {boolean} countUnknown
 * @param {ModelCounters} counters
 * @returns {{ status: string, statusLabel: string }}
 */
function normalizeStatus(status, countUnknown, counters) {
  const raw = asString(status);
  if (KNOWN_STATUSES.includes(raw)) {
    return { status: raw, statusLabel: STATUS_LABELS[raw] ?? raw };
  }
  if (countUnknown) counters.unknownStatus += 1;
  return { status: "unknown", statusLabel: UNKNOWN_STATUS_LABEL };
}

/**
 * @param {unknown} value
 * @returns {string}
 */
function activityName(value) {
  const first = asRecord(asArray(value)[0]);
  return asString(first.name);
}

/**
 * @param {unknown} value
 * @returns {{ start: number, end: number } | null}
 */
function normalizeRange(value) {
  if (value === null || typeof value !== "object" || Array.isArray(value)) return null;
  const record = /** @type {Record<string, unknown>} */ (value);
  const start = asNumber(record.start);
  const end = asNumber(record.end);
  if (start === null || end === null || !Number.isInteger(start) || !Number.isInteger(end)) return null;
  return { start, end };
}

/**
 * @param {unknown} value
 * @returns {string}
 */
function emojiOf(value) {
  if (typeof value === "string") return asString(value);
  const record = asRecord(value);
  return asString(record.name) || asString(record.id);
}

/**
 * Участник из `member`-объекта CHUNK/LIST_UPDATE. `null` — без `user.id`
 * (элемент пропускается, остальные применяются).
 * @param {unknown} value
 * @param {ModelCounters} counters
 * @returns {LiveMember | null}
 */
function asLiveMember(value, counters) {
  const record = asRecord(value);
  const user = asRecord(record.user);
  const userId = asString(user.id);
  if (userId === "") return null;
  const presence = asRecord(record.presence);
  const status = normalizeStatus(presence.status, false, counters);
  return {
    userId,
    // Порядок фиксирован контрактом: global_name → nick → username → id.
    name: asString(user.global_name) || asString(record.nick) || asString(user.username) || userId,
    nick: asString(record.nick),
    bot: asBool(user.bot),
    roles: asArray(record.roles).map((role) => asString(role)).filter((role) => role !== ""),
    status: status.status,
    statusLabel: status.statusLabel,
    activity: activityName(presence.activities),
  };
}

/**
 * @param {DispatchStore} store
 * @param {string} guildId
 * @param {LiveMember} member
 */
function rememberMember(store, guildId, member) {
  let members = store.members.get(guildId);
  if (!members) {
    members = new Map();
    store.members.set(guildId, members);
  }
  members.set(member.userId, member);
}

/**
 * Строка окна участников: имя — из CHUNK/LIST_UPDATE, статус — presence
 * (LWW), при отсутствии обоих — `offline` только если данных нет вовсе.
 * Presence имя не выдумывает (R8).
 * @param {DispatchStore} store
 * @param {string} guildId
 * @param {string} userId
 * @returns {MemberRow}
 */
function rowOf(store, guildId, userId) {
  const member = store.members.get(guildId)?.get(userId)
    ?? store.lists.get(guildId)?.items.get(userId)
    ?? null;
  const presence = store.presences.get(`${guildId}:${userId}`) ?? null;
  const status = presence?.status ?? member?.status ?? "offline";
  const statusLabel = presence?.statusLabel ?? member?.statusLabel ?? statusLabelOf(status);
  const activity = presence?.activity || member?.activity || "";
  return {
    userId,
    name: member?.name || userId,
    status,
    statusLabel,
    activity,
  };
}

/**
 * @param {DispatchStore} store
 * @param {string} guildId
 * @param {string[]} order
 * @param {MemberList | undefined} list
 * @returns {MembersChange}
 */
function membersChange(store, guildId, order, list) {
  /** @type {MembersChange} */
  const change = {
    type: "members",
    guildId,
    order: [...order],
    rows: order.map((userId) => rowOf(store, guildId, userId)),
    listId: list?.listId ?? "",
  };
  if (list && list.onlineCount !== null) change.onlineCount = list.onlineCount;
  if (list && list.memberCount !== null) change.memberCount = list.memberCount;
  return change;
}

/**
 * Подпись окна: изменение содержимого строк тоже считается изменением.
 * @param {MemberList} list
 * @returns {string}
 */
function listSignature(list) {
  return JSON.stringify({
    order: list.order,
    memberCount: list.memberCount,
    onlineCount: list.onlineCount,
    range: list.range,
    items: list.order.map((userId) => list.items.get(userId) ?? null),
  });
}

/**
 * @param {unknown} value
 * @param {number} depth
 * @returns {number}
 */
function countSecretKeys(value, depth = 0) {
  if (depth > SECRET_SCAN_DEPTH || value === null || typeof value !== "object") return 0;
  let count = 0;
  if (Array.isArray(value)) {
    for (const item of value) count += countSecretKeys(item, depth + 1);
    return count;
  }
  for (const [key, item] of Object.entries(value)) {
    const name = key.toLowerCase();
    if (SECRET_KEYS.has(name) || name.endsWith("_token")) count += 1;
    count += countSecretKeys(item, depth + 1);
  }
  return count;
}

/* ---------- проекции ---------- */

/**
 * READY — полный снапшот гильдий. Поля `channels`/`roles`/`merged_members`
 * игнорируются: проекция каналов принадлежит C-02.
 * @param {DispatchStore} store
 * @param {Record<string, unknown>} data
 * @param {ModelCounters} [counters]
 * @returns {DispatchChange[]}
 */
export function projectReady(store, data, counters = emptyCounters()) {
  const raw = data.guilds;
  if (!Array.isArray(raw)) {
    counters.malformed += 1;
    return [];
  }
  /** @type {GuildSnapshot[]} */
  const guilds = [];
  for (const value of raw) {
    const record = asRecord(value);
    const id = asString(record.id);
    const name = asString(asRecord(record.properties).name);
    if (id === "" || name === "") {
      counters.skipped += 1;
      continue;
    }
    guilds.push({ id, name, memberCount: asNumber(record.member_count) });
  }
  const order = guilds.map((guild) => guild.id);
  const fallbackSelf = asSnowflake(asRecord(data.user).id);
  const nextSelf = store.selfUserId === "" ? fallbackSelf : store.selfUserId;
  // `source` — маркер, что READY-снапшот уже доходил до view. Без него первый
  // пустой READY (store ещё пуст, а рейл наполнен `ready()` IPC) считался бы
  // duplicate и не очистил бы DOM (P2 REVIEW-L02-UI).
  const same = store.source === "dispatch"
    && nextSelf === store.selfUserId
    && order.length === store.order.length
    && order.every((id, index) => {
      const previous = store.guilds.get(id);
      const next = guilds[index];
      return id === store.order[index]
        && previous !== undefined
        && next !== undefined
        && previous.name === next.name
        && previous.memberCount === next.memberCount;
    });
  // Состояние применяется всегда (полный снапшот), но повтор не перерисовывает UI.
  store.guilds = new Map(guilds.map((guild) => [guild.id, guild]));
  store.order = order;
  store.selfUserId = nextSelf;
  // Данные READY (в том числе пустой снапшот) пришли из dispatch — без reload.
  store.source = "dispatch";
  if (same) {
    counters.duplicate += 1;
    return [];
  }
  // Пустой READY — тоже полный снапшот: view обязан очистить отсутствующие
  // плитки (P2 REVIEW-L02-UI), иначе серверы прошлого аккаунта остаются в DOM.
  return [{ type: "guilds", guilds, selfUserId: nextSelf }];
}

/**
 * GUILD_MEMBERS_CHUNK — участники окна. Ключ идемпотентности —
 * `guildId:chunk_index/chunk_count`.
 * @param {DispatchStore} store
 * @param {Record<string, unknown>} data
 * @param {ModelCounters} [counters]
 * @returns {DispatchChange[]}
 */
export function projectMembersChunk(store, data, counters = emptyCounters()) {
  const guildId = asString(data.guild_id);
  const members = data.members;
  const chunkIndex = asNumber(data.chunk_index);
  const chunkCount = asNumber(data.chunk_count);
  if (guildId === "" || !Array.isArray(members) || chunkIndex === null || chunkCount === null || chunkCount <= 0) {
    counters.malformed += 1;
    return [];
  }
  const key = `${guildId}:${chunkIndex}/${chunkCount}`;
  if (store.chunks.includes(key)) {
    counters.duplicate += 1;
    return [];
  }
  store.chunks.push(key);
  if (store.chunks.length > CHUNK_LIMIT) store.chunks.splice(0, store.chunks.length - CHUNK_LIMIT);

  /** @type {string[]} */
  const incoming = [];
  for (const value of members) {
    const member = asLiveMember(value, counters);
    if (!member) {
      counters.skipped += 1;
      continue;
    }
    rememberMember(store, guildId, member);
    incoming.push(member.userId);
  }
  if (incoming.length === 0) return [];
  const list = store.lists.get(guildId);
  // Окно LIST_UPDATE авторитетно; без списка показываем ровно пришедший чанк.
  const order = list ? [...list.order] : [...new Set(incoming)];
  return [membersChange(store, guildId, order, list)];
}

/**
 * SYNC: `order` — только `member`-элементы (группы в порядке окна не входят).
 * @param {DispatchStore} store
 * @param {MemberList} list
 * @param {string} guildId
 * @param {Record<string, unknown>} op
 */
function applySync(store, list, guildId, op) {
  /** @type {string[]} */
  const order = [];
  /** @type {Map<string, LiveMember>} */
  const items = new Map();
  for (const value of asArray(op.items)) {
    const record = asRecord(value);
    if (Object.keys(asRecord(record.group)).length > 0) continue;
    const member = asLiveMember(record.member, emptyCounters());
    if (!member || items.has(member.userId)) continue;
    rememberMember(store, guildId, member);
    items.set(member.userId, member);
    order.push(member.userId);
  }
  list.order = order;
  list.items = items;
  list.range = normalizeRange(op.range);
  list.complete = true;
}

/**
 * GUILD_MEMBER_LIST_UPDATE — плоское окно. Кадр применяется целиком и в
 * порядке провода; неизвестный `op` делает кадр `malformed` без частичных правок.
 * @param {DispatchStore} store
 * @param {Record<string, unknown>} data
 * @param {ModelCounters} [counters]
 * @returns {DispatchChange[]}
 */
export function projectMemberListUpdate(store, data, counters = emptyCounters()) {
  const guildId = asString(data.guild_id);
  const ops = data.ops;
  if (guildId === "" || !Array.isArray(ops)) {
    counters.malformed += 1;
    return [];
  }
  // Валидация до мутаций: частичное применение запрещено (порядок окна).
  for (const value of ops) {
    const type = asString(asRecord(value).op).toUpperCase();
    if (type !== "SYNC" && type !== "INSERT" && type !== "UPDATE" && type !== "DELETE") {
      counters.malformed += 1;
      return [];
    }
    if (type === "SYNC" && !Array.isArray(asRecord(value).items)) {
      counters.malformed += 1;
      return [];
    }
  }

  let list = store.lists.get(guildId);
  if (!list) {
    list = {
      listId: asString(data.id),
      memberCount: null,
      onlineCount: null,
      order: [],
      items: new Map(),
      range: null,
      opsApplied: 0,
      complete: false,
    };
    store.lists.set(guildId, list);
  }
  const listId = asString(data.id);
  if (listId !== "") list.listId = listId;
  const memberCount = asNumber(data.member_count);
  if (memberCount !== null) list.memberCount = memberCount;
  const onlineCount = asNumber(data.online_count);
  if (onlineCount !== null) {
    list.onlineCount = onlineCount;
    // Точный счётчик из окна: READY больше не перебивает его member_count.
    store.onlineCountKnown = true;
  }

  const before = listSignature(list);
  let duplicateOp = false;
  for (const value of ops) {
    const op = asRecord(value);
    const type = asString(op.op).toUpperCase();
    if (type === "SYNC") {
      applySync(store, list, guildId, op);
      list.opsApplied += 1;
      continue;
    }
    const index = asNumber(op.index);
    if (index === null || !Number.isInteger(index) || index < 0) {
      counters.outOfRange += 1;
      continue;
    }
    if (type === "DELETE") {
      if (index >= list.order.length) {
        counters.outOfRange += 1;
        continue;
      }
      const removed = list.order.splice(index, 1)[0];
      if (removed !== undefined && !list.order.includes(removed)) list.items.delete(removed);
      list.opsApplied += 1;
      continue;
    }
    const item = asRecord(op.item);
    // Групповой элемент — снимок заголовков, не строка участника (PLAN §3.3).
    if (Object.keys(asRecord(item.group)).length > 0) {
      counters.skipped += 1;
      continue;
    }
    const member = asLiveMember(item.member, counters);
    if (!member) {
      counters.skipped += 1;
      continue;
    }
    rememberMember(store, guildId, member);
    if (type === "INSERT") {
      if (list.order.includes(member.userId)) {
        // Повтор userId трактуется как UPDATE без дубля.
        duplicateOp = true;
        list.opsApplied += 1;
        continue;
      }
      list.order.splice(Math.min(index, list.order.length), 0, member.userId);
      list.items.set(member.userId, member);
      list.opsApplied += 1;
      continue;
    }
    // UPDATE
    if (index >= list.order.length) {
      counters.outOfRange += 1;
      continue;
    }
    const previousId = list.order[index];
    if (previousId !== undefined && previousId !== member.userId && !list.order.includes(member.userId)) {
      list.items.delete(previousId);
    }
    list.order[index] = member.userId;
    list.items.set(member.userId, member);
    list.opsApplied += 1;
  }

  if (listSignature(list) === before) {
    if (duplicateOp) counters.duplicate += 1;
    return [];
  }
  if (duplicateOp) counters.duplicate += 1;
  return [membersChange(store, guildId, list.order, list)];
}

/**
 * PRESENCE_UPDATE — LWW по `processed_at_timestamp` (может быть отрицательным).
 * @param {DispatchStore} store
 * @param {Record<string, unknown>} data
 * @param {ModelCounters} [counters]
 * @returns {DispatchChange[]}
 */
export function projectPresence(store, data, counters = emptyCounters()) {
  const guildId = asString(data.guild_id);
  const userId = asString(asRecord(data.user).id) || asString(data.user_id);
  if (guildId === "" || userId === "") {
    counters.malformed += 1;
    return [];
  }
  const status = normalizeStatus(data.status, true, counters);
  const key = `${guildId}:${userId}`;
  const ts = asNumber(data.processed_at_timestamp);
  const stored = store.presences.get(key);
  if (stored && stored.ts !== null && ts !== null && ts < stored.ts) {
    counters.stale += 1;
    return [];
  }
  /** @type {Record<string, string>} */
  const clientStatus = {};
  const rawClient = asRecord(data.client_status);
  for (const [name, value] of Object.entries(rawClient)) {
    const text = asString(value);
    if (text !== "") clientStatus[name] = text;
  }
  const activity = activityName(data.activities);
  store.presences.set(key, {
    status: status.status,
    statusLabel: status.statusLabel,
    clientStatus,
    activity,
    ts,
  });
  const member = store.members.get(guildId)?.get(userId);
  if (member) {
    member.status = status.status;
    member.statusLabel = status.statusLabel;
    if (activity !== "") member.activity = activity;
  }
  return [{
    type: "presence",
    guildId,
    userId,
    status: status.status,
    statusLabel: status.statusLabel,
    activity,
  }];
}

/**
 * MESSAGE_REACTION_ADD/REMOVE — счётчик окна; store держит только userId-сетку
 * (второй источник истины для реакций — `chat.channelWindows.items[].reactions`).
 * @param {DispatchStore} store
 * @param {string} kind
 * @param {Record<string, unknown>} data
 * @param {ModelCounters} [counters]
 * @returns {DispatchChange[]}
 */
export function projectReaction(store, kind, data, counters = emptyCounters()) {
  const channelId = asString(data.channel_id) || asString(data.channelId);
  const messageId = asString(data.message_id) || asString(data.messageId);
  const userId = asString(data.user_id) || asString(data.userId);
  const emoji = emojiOf(data.emoji);
  if (channelId === "" || messageId === "" || emoji === "") {
    counters.malformed += 1;
    return [];
  }
  const key = `${channelId}:${messageId}:${emoji}`;
  const state = store.reactions.get(key) ?? { users: new Set(), anon: 0 };
  const add = kind === "MESSAGE_REACTION_ADD";
  if (userId === "") {
    // Без user_id событие применяется как «кто-то»: счётчик важнее атрибуции.
    counters.degraded += 1;
    if (add) state.anon += 1;
    else if (state.anon > 0) state.anon -= 1;
    else {
      counters.ignored += 1;
      return [];
    }
  } else if (add) {
    if (state.users.has(userId)) {
      counters.duplicate += 1;
      return [];
    }
    state.users.add(userId);
  } else {
    if (!state.users.has(userId)) {
      // REMOVE без ADD — no-op, счётчик не уходит в минус.
      counters.ignored += 1;
      return [];
    }
    state.users.delete(userId);
  }
  const count = state.users.size + state.anon;
  if (count <= 0) store.reactions.delete(key);
  else store.reactions.set(key, state);
  const me = userId !== "" && store.selfUserId !== "" && userId === store.selfUserId;
  return [{
    type: "reaction",
    channelId,
    messageId,
    emoji,
    count: Math.max(0, count),
    me,
    removed: !add,
  }];
}

/* ---------- вход модели ---------- */

/**
 * @param {DispatchStore} store
 * @param {unknown} value
 * @param {ModelCounters} [counters]
 * @returns {{ ok: boolean, kind: string, changes: DispatchChange[], counters: ModelCounters }}
 */
export function applyDispatch(store, value, counters = emptyCounters()) {
  const normalized = normalizeEnvelope(value);
  if (!normalized.ok) {
    counters.malformed += 1;
    return { ok: false, kind: "", changes: [], counters };
  }
  const { kind, data } = normalized;
  const secrets = countSecretKeys(data);
  if (secrets > 0) counters.redactedKeys += secrets;

  /** @type {DispatchChange[]} */
  let changes = [];
  if (!FORWARDED.has(kind)) {
    // N1: не форвардим и не шумим — ignored, без throw и console.
    counters.ignored += 1;
    return { ok: true, kind, changes, counters };
  }
  switch (kind) {
    case "READY":
      changes = projectReady(store, data, counters);
      break;
    case "GUILD_MEMBERS_CHUNK":
      changes = projectMembersChunk(store, data, counters);
      break;
    case "GUILD_MEMBER_LIST_UPDATE":
      changes = projectMemberListUpdate(store, data, counters);
      break;
    case "PRESENCE_UPDATE":
      changes = projectPresence(store, data, counters);
      break;
    default:
      changes = projectReaction(store, kind, data, counters);
      break;
  }
  counters.applied = changes.length > 0 ? 1 : 0;
  return { ok: true, kind, changes, counters };
}
