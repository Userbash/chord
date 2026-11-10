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

/**
 * Все имена, которые бэкенд форвардит в UI: 21 каноническое имя ядра
 * (`DispatchKind::ALL`, порядок как в `as_str`), 8 расширенных событий
 * `ExtendedDispatchKind` (`MESSAGE_DELETE`, `MESSAGE_DELETE_BULK`,
 * `CHANNEL_CREATE/UPDATE/DELETE`, `THREAD_CREATE/UPDATE/DELETE`) и
 * `MESSAGE_REACTION_REMOVE` (в ядре raw, форвардится по имени провода —
 * `CONTRACTS/dispatch.md` §5). Порядок и состав — как в
 * `net::FORWARDED_DISPATCHES` (task-156): список UI обязан быть его
 * надмножеством (F-GW-03), иначе живые удаления, каналы и треды отбрасываются
 * фильтром модели. Проекций у расширенных событий пока нет — они принимаются
 * как `skipped`, без изменений состояния.
 */
export const FORWARDED_KINDS = Object.freeze([
  "CHANNEL_INFO",
  "GUILD_MEMBERS_CHUNK",
  "GUILD_MEMBER_LIST_UPDATE",
  "MESSAGE_ACK",
  "MESSAGE_CREATE",
  "MESSAGE_POLL_VOTE_ADD",
  "MESSAGE_REACTION_ADD",
  "MESSAGE_UPDATE",
  "PRESENCE_UPDATE",
  "READY",
  "READY_SUPPLEMENTAL",
  "RESUMED",
  "SESSIONS_REPLACE",
  "SOUNDBOARD_SOUNDS",
  "THREAD_LIST_SYNC",
  "TYPING_START",
  "USER_SETTINGS_PROTO_UPDATE",
  "VOICE_CHANNEL_START_TIME_UPDATE",
  "VOICE_CHANNEL_STATUS_UPDATE",
  "VOICE_SERVER_UPDATE",
  "VOICE_STATE_UPDATE",
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
 *   id: string,
 *   channelId: string,
 *   guildId: string,
 *   authorId: string,
 *   authorName: string,
 *   authorAvatar: string | null,
 *   authorBot: boolean,
 *   content: string,
 *   timestamp: string,
 *   editedTimestamp: string | null,
 *   edited: boolean,
 *   pinned: boolean,
 *   mentionEveryone: boolean,
 *   mentions: string[],
 *   embedCount: number,
 *   attachmentCount: number,
 *   attachments: Record<string, unknown>[],
 *   embeds: Record<string, unknown>[],
 * }} LiveMessage */
/** @typedef {{
 *   channelId: string,
 *   lastMessageId: string,
 *   unread: boolean,
 *   mentionCount: number,
 *   version: number | null,
 *   lastViewed: number | null,
 *   flags: number | null,
 * }} LiveReadState */
/** @typedef {{
 *   channelId: string,
 *   userId: string,
 *   guildId: string,
 *   timestamp: number,
 * }} LiveTyping */
/** @typedef {{
 *   guildId: string,
 *   userId: string,
 *   channelId: string | null,
 *   selfMute: boolean,
 *   selfDeaf: boolean,
 *   selfStream: boolean,
 *   selfVideo: boolean,
 *   mute: boolean,
 *   deaf: boolean,
 *   suppress: boolean,
 * }} LiveVoiceState */
/** @typedef {{
 *   guildId: string,
 *   endpoint: string | null,
 * }} LiveVoiceServer */
/** @typedef {{
 *   id: string,
 *   guildId: string,
 *   parentId: string,
 *   ownerId: string,
 *   name: string,
 *   type: number | null,
 *   messageCount: number | null,
 *   memberCount: number | null,
 *   lastMessageId: string | null,
 *   archived: boolean,
 *   locked: boolean,
 * }} LiveThread */
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
 *   messages: Map<string, Map<string, LiveMessage>>,
 *   messageCounts: Map<string, number>,
 *   readStates: Map<string, LiveReadState>,
 *   typing: Map<string, Map<string, LiveTyping>>,
 *   voiceStates: Map<string, LiveVoiceState>,
 *   voiceServers: Map<string, LiveVoiceServer>,
 *   threads: Map<string, Map<string, LiveThread>>,
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
/** @typedef {{
 *   type: "message_create",
 *   channelId: string,
 *   messageId: string,
 *   message: LiveMessage,
 *   messageCount: number,
 *   updated: boolean,
 * }} MessageCreateChange */
/** @typedef {{
 *   type: "message_update",
 *   channelId: string,
 *   messageId: string,
 *   content: string,
 *   editedTimestamp: string | null,
 *   edited: boolean,
 *   pinned: boolean,
 *   embedCount: number,
 *   message: LiveMessage,
 * }} MessageUpdateChange */
/** @typedef {{
 *   type: "message_ack",
 *   channelId: string,
 *   messageId: string,
 *   unread: boolean,
 *   mentionCount: number,
 *   version: number | null,
 *   lastViewed: number | null,
 * }} MessageAckChange */
/** @typedef {{
 *   type: "typing",
 *   channelId: string,
 *   userId: string,
 *   guildId: string,
 *   timestamp: number,
 * }} TypingChange */
/** @typedef {{
 *   type: "voice_state",
 *   guildId: string,
 *   userId: string,
 *   channelId: string | null,
 *   selfMute: boolean,
 *   selfDeaf: boolean,
 *   selfStream: boolean,
 *   selfVideo: boolean,
 *   mute: boolean,
 *   deaf: boolean,
 *   suppress: boolean,
 * }} VoiceStateChange */
/** @typedef {{
 *   type: "voice_server",
 *   guildId: string,
 *   endpoint: string | null,
 * }} VoiceServerChange */
/** @typedef {{
 *   type: "threads",
 *   guildId: string,
 *   threads: LiveThread[],
 * }} ThreadsChange */
/** @typedef {{
 *   type: "message_delete",
 *   channelId: string,
 *   messageId: string,
 *   guildId?: string,
 * }} MessageDeleteChange */
/** @typedef {GuildsChange | MembersChange | PresenceChange | ReactionChange | MessageCreateChange | MessageUpdateChange | MessageDeleteChange | MessageAckChange | TypingChange | VoiceStateChange | VoiceServerChange | ThreadsChange} DispatchChange */
/**
 * Счётчики модели. `ignored` — `kind` вне `FORWARDED_KINDS` или no-op проекции
 * (например, `REMOVE` реакции без предшествующего `ADD`); `skipped` — событие
 * принято, но не применено (нет проекции в UI или элемент payload отброшен);
 * `malformed` — конверт/данные не разобраны.
 * @typedef {{
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
  // `d` — произвольное JSON-значение: у `SESSIONS_REPLACE` это массив
  // (`docs/re/gateway-schema.md`), поэтому массив — валидный конверт, а не
  // битый `data`. Проекции сами требуют объект и считают malformed.
  if (data === null || typeof data !== "object") {
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
    messages: new Map(),
    messageCounts: new Map(),
    readStates: new Map(),
    typing: new Map(),
    voiceStates: new Map(),
    voiceServers: new Map(),
    threads: new Map(),
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

/**
 * MESSAGE_CREATE — добавление/обновление сообщения в канале, инкремент счётчика
 * канала, сброс активного тайпинга автора и учёт unread/mentions.
 * @param {DispatchStore} store
 * @param {Record<string, unknown>} data
 * @param {ModelCounters} [counters]
 * @returns {DispatchChange[]}
 */
export function projectMessageCreate(store, data, counters = emptyCounters()) {
  if (Array.isArray(data)) {
    counters.malformed += 1;
    return [];
  }
  const channelId = asString(data.channel_id) || asString(data.channelId);
  const messageId = asString(data.id) || asString(data.message_id) || asString(data.messageId);
  if (channelId === "" || messageId === "") {
    counters.malformed += 1;
    return [];
  }
  const guildId = asString(data.guild_id) || asString(data.guildId);
  const author = asRecord(data.author);
  const member = asRecord(data.member);
  const authorId = asString(author.id) || asString(data.author_id) || asString(data.authorId);
  const authorName = asString(author.global_name)
    || asString(member.nick)
    || asString(author.username)
    || authorId;
  const authorAvatar = asString(author.avatar) || asString(member.avatar) || null;
  const authorBot = asBool(author.bot);
  const content = typeof data.content === "string"
    ? (data.content === "nil" ? "" : data.content)
    : asString(data.content);
  const timestamp = asString(data.timestamp);
  const rawEdited = asString(data.edited_timestamp) || asString(data.editedTimestamp);
  const editedTimestamp = rawEdited !== "" ? rawEdited : null;
  const edited = editedTimestamp !== null;
  const pinned = asBool(data.pinned);
  const mentionEveryone = asBool(data.mention_everyone ?? data.mentionEveryone);
  const mentions = asArray(data.mentions)
    .map((item) => asString(asRecord(item).id) || asString(item))
    .filter((id) => id !== "");
  const embeds = asArray(data.embeds)
    .filter((e) => e !== null && typeof e === "object" && !Array.isArray(e))
    .map((e) => /** @type {Record<string, unknown>} */ (e));
  const attachments = asArray(data.attachments)
    .filter((a) => a !== null && typeof a === "object" && !Array.isArray(a))
    .map((a) => /** @type {Record<string, unknown>} */ (a));
  const embedCount = embeds.length;
  const attachmentCount = attachments.length;

  /** @type {LiveMessage} */
  const message = {
    id: messageId,
    channelId,
    guildId,
    authorId,
    authorName,
    authorAvatar,
    authorBot,
    content,
    timestamp,
    editedTimestamp,
    edited,
    pinned,
    mentionEveryone,
    mentions,
    embedCount,
    attachmentCount,
    attachments,
    embeds,
  };

  let channelMessages = store.messages.get(channelId);
  if (!channelMessages) {
    channelMessages = new Map();
    store.messages.set(channelId, channelMessages);
  }
  const previous = channelMessages.get(messageId);
  if (previous && JSON.stringify(previous) === JSON.stringify(message)) {
    counters.duplicate += 1;
    return [];
  }
  const updated = previous !== undefined;
  channelMessages.set(messageId, message);
  const previousCount = store.messageCounts.get(channelId) ?? 0;
  const messageCount = updated ? previousCount : previousCount + 1;
  store.messageCounts.set(channelId, messageCount);

  if (authorId !== "") {
    store.typing.get(channelId)?.delete(authorId);
  }
  if (!updated && (store.selfUserId === "" || authorId !== store.selfUserId)) {
    const prevRead = store.readStates.get(channelId);
    const mentionedSelf = mentionEveryone || (store.selfUserId !== "" && mentions.includes(store.selfUserId));
    store.readStates.set(channelId, {
      channelId,
      lastMessageId: prevRead?.lastMessageId ?? "",
      unread: true,
      mentionCount: (prevRead?.mentionCount ?? 0) + (mentionedSelf ? 1 : 0),
      version: prevRead?.version ?? null,
      lastViewed: prevRead?.lastViewed ?? null,
      flags: prevRead?.flags ?? null,
    });
  }

  return [{
    type: "message_create",
    channelId,
    messageId,
    message,
    messageCount,
    updated,
  }];
}

/**
 * MESSAGE_UPDATE — обновление контента, `edited_timestamp` и вложений сообщения
 * без увеличения счётчика сообщений канала.
 * @param {DispatchStore} store
 * @param {Record<string, unknown>} data
 * @param {ModelCounters} [counters]
 * @returns {DispatchChange[]}
 */
export function projectMessageUpdate(store, data, counters = emptyCounters()) {
  if (Array.isArray(data)) {
    counters.malformed += 1;
    return [];
  }
  const channelId = asString(data.channel_id) || asString(data.channelId);
  const messageId = asString(data.id) || asString(data.message_id) || asString(data.messageId);
  if (channelId === "" || messageId === "") {
    counters.malformed += 1;
    return [];
  }
  let channelMessages = store.messages.get(channelId);
  if (!channelMessages) {
    channelMessages = new Map();
    store.messages.set(channelId, channelMessages);
  }
  const previous = channelMessages.get(messageId);
  const guildId = asString(data.guild_id) || asString(data.guildId) || previous?.guildId || "";
  const author = asRecord(data.author);
  const member = asRecord(data.member);
  const authorId = asString(author.id) || asString(data.author_id) || previous?.authorId || "";
  const authorName = asString(author.global_name)
    || asString(member.nick)
    || asString(author.username)
    || previous?.authorName
    || authorId;
  const authorAvatar = asString(author.avatar)
    || asString(member.avatar)
    || previous?.authorAvatar
    || null;
  const authorBot = "bot" in author ? asBool(author.bot) : (previous?.authorBot ?? false);
  const content = ("content" in data && !nilish(data.content))
    ? (typeof data.content === "string" ? data.content : asString(data.content))
    : (previous?.content ?? "");
  const timestamp = asString(data.timestamp) || previous?.timestamp || "";
  const rawEdited = asString(data.edited_timestamp) || asString(data.editedTimestamp);
  const editedTimestamp = rawEdited !== "" ? rawEdited : (previous?.editedTimestamp ?? null);
  const edited = editedTimestamp !== null || (previous?.edited ?? false);
  const pinned = ("pinned" in data && !nilish(data.pinned))
    ? asBool(data.pinned)
    : (previous?.pinned ?? false);
  const mentionEveryone = ("mention_everyone" in data || "mentionEveryone" in data)
    ? asBool(data.mention_everyone ?? data.mentionEveryone)
    : (previous?.mentionEveryone ?? false);
  const mentions = Array.isArray(data.mentions)
    ? data.mentions.map((item) => asString(asRecord(item).id) || asString(item)).filter((id) => id !== "")
    : (previous?.mentions ?? []);
  const embeds = Array.isArray(data.embeds)
    ? data.embeds
      .filter((e) => e !== null && typeof e === "object" && !Array.isArray(e))
      .map((e) => /** @type {Record<string, unknown>} */ (e))
    : (previous?.embeds ?? []);
  const attachments = Array.isArray(data.attachments)
    ? data.attachments
      .filter((a) => a !== null && typeof a === "object" && !Array.isArray(a))
      .map((a) => /** @type {Record<string, unknown>} */ (a))
    : (previous?.attachments ?? []);
  const embedCount = Array.isArray(data.embeds) ? data.embeds.length : (previous?.embedCount ?? 0);
  const attachmentCount = Array.isArray(data.attachments)
    ? data.attachments.length
    : (previous?.attachmentCount ?? 0);

  /** @type {LiveMessage} */
  const next = {
    id: messageId,
    channelId,
    guildId,
    authorId,
    authorName,
    authorAvatar,
    authorBot,
    content,
    timestamp,
    editedTimestamp,
    edited,
    pinned,
    mentionEveryone,
    mentions,
    embedCount,
    attachmentCount,
    attachments,
    embeds,
  };
  if (previous && JSON.stringify(previous) === JSON.stringify(next)) {
    counters.duplicate += 1;
    return [];
  }
  channelMessages.set(messageId, next);
  return [{
    type: "message_update",
    channelId,
    messageId,
    content: next.content,
    editedTimestamp: next.editedTimestamp,
    edited: next.edited,
    pinned: next.pinned,
    embedCount: next.embedCount,
    message: next,
  }];
}

/**
 * MESSAGE_DELETE — удаление сообщения по snowflake ID (M-08 / TASK-762).
 * @param {DispatchStore} store
 * @param {Record<string, unknown>} data
 * @param {ModelCounters} [counters]
 * @returns {DispatchChange[]}
 */
export function projectMessageDelete(store, data, counters = emptyCounters()) {
  if (Array.isArray(data) || !data || typeof data !== "object") {
    counters.malformed += 1;
    return [];
  }
  const id = asString(data.id);
  const channelId = asString(data.channel_id) || asString(data.channelId);
  if (!id || !channelId) {
    counters.malformed += 1;
    return [];
  }
  counters.applied += 1;
  return [
    {
      type: "message_delete",
      channelId,
      messageId: id,
      guildId: asString(data.guild_id) || asString(data.guildId) || undefined,
    },
  ];
}

/**
 * MESSAGE_DELETE_BULK — пачка удалений сообщений в канале (M-08 / TASK-762).
 * @param {DispatchStore} store
 * @param {Record<string, unknown>} data
 * @param {ModelCounters} [counters]
 * @returns {DispatchChange[]}
 */
export function projectMessageDeleteBulk(store, data, counters = emptyCounters()) {
  if (Array.isArray(data) || !data || typeof data !== "object") {
    counters.malformed += 1;
    return [];
  }
  const channelId = asString(data.channel_id) || asString(data.channelId);
  const rawIds = Array.isArray(data.ids) ? data.ids : [];
  const ids = rawIds.map((item) => asString(item)).filter((s) => s.length > 0);
  if (ids.length === 0 || !channelId) {
    counters.malformed += 1;
    return [];
  }
  counters.applied += ids.length;
  const guildId = asString(data.guild_id) || asString(data.guildId) || undefined;
  return ids.map((id) => ({
    type: "message_delete",
    channelId,
    messageId: id,
    guildId,
  }));
}

/**
 * MESSAGE_ACK — сброс `unread` и `mentionCount` для `channel_id` до `message_id`.
 * @param {DispatchStore} store
 * @param {Record<string, unknown>} data
 * @param {ModelCounters} [counters]
 * @returns {DispatchChange[]}
 */
export function projectMessageAck(store, data, counters = emptyCounters()) {
  if (Array.isArray(data)) {
    counters.malformed += 1;
    return [];
  }
  const channelId = asString(data.channel_id) || asString(data.channelId);
  const messageId = asString(data.message_id) || asString(data.messageId);
  if (channelId === "" || messageId === "") {
    counters.malformed += 1;
    return [];
  }
  const manual = asBool(data.manual);
  const rawMentions = asNumber(data.mention_count ?? data.mentionCount);
  const mentionCount = manual && rawMentions !== null ? Math.max(0, Math.trunc(rawMentions)) : 0;
  const previous = store.readStates.get(channelId);
  const version = asNumber(data.version) ?? previous?.version ?? null;
  const lastViewed = asNumber(data.last_viewed ?? data.lastViewed) ?? previous?.lastViewed ?? null;
  const flags = asNumber(data.flags) ?? previous?.flags ?? null;

  /** @type {LiveReadState} */
  const next = {
    channelId,
    lastMessageId: messageId,
    unread: false,
    mentionCount,
    version,
    lastViewed,
    flags,
  };
  if (previous && JSON.stringify(previous) === JSON.stringify(next)) {
    counters.duplicate += 1;
    return [];
  }
  store.readStates.set(channelId, next);
  return [{
    type: "message_ack",
    channelId,
    messageId,
    unread: false,
    mentionCount,
    version,
    lastViewed,
  }];
}

/**
 * TYPING_START — запись активного тайпинга `user_id` в `channel_id` с `timestamp`.
 * @param {DispatchStore} store
 * @param {Record<string, unknown>} data
 * @param {ModelCounters} [counters]
 * @returns {DispatchChange[]}
 */
export function projectTypingStart(store, data, counters = emptyCounters()) {
  if (Array.isArray(data)) {
    counters.malformed += 1;
    return [];
  }
  const channelId = asString(data.channel_id) || asString(data.channelId);
  const userId = asString(data.user_id)
    || asString(data.userId)
    || asString(asRecord(asRecord(data.member).user).id);
  const timestamp = asNumber(data.timestamp);
  if (channelId === "" || userId === "" || timestamp === null) {
    counters.malformed += 1;
    return [];
  }
  const guildId = asString(data.guild_id) || asString(data.guildId);
  let channelTyping = store.typing.get(channelId);
  if (!channelTyping) {
    channelTyping = new Map();
    store.typing.set(channelId, channelTyping);
  }
  const previous = channelTyping.get(userId);
  if (previous && timestamp < previous.timestamp) {
    counters.stale += 1;
    return [];
  }
  if (previous && timestamp === previous.timestamp && guildId === previous.guildId) {
    counters.duplicate += 1;
    return [];
  }
  /** @type {LiveTyping} */
  const entry = { channelId, userId, guildId, timestamp };
  channelTyping.set(userId, entry);
  return [{
    type: "typing",
    channelId,
    userId,
    guildId,
    timestamp,
  }];
}

/**
 * VOICE_STATE_UPDATE — обновление `channel_id`/`self_mute`/`self_deaf`/`self_stream`
 * участника голоса без сохранения `session_id`.
 * @param {DispatchStore} store
 * @param {Record<string, unknown>} data
 * @param {ModelCounters} [counters]
 * @returns {DispatchChange[]}
 */
export function projectVoiceState(store, data, counters = emptyCounters()) {
  if (Array.isArray(data)) {
    counters.malformed += 1;
    return [];
  }
  const userId = asString(data.user_id)
    || asString(data.userId)
    || asString(asRecord(asRecord(data.member).user).id);
  const guildId = asString(data.guild_id) || asString(data.guildId);
  const rawChannel = asString(data.channel_id) || asString(data.channelId);
  const channelId = rawChannel !== "" ? rawChannel : null;
  if (userId === "" || (guildId === "" && channelId === null && !("channel_id" in data) && !("channelId" in data))) {
    counters.malformed += 1;
    return [];
  }
  /** @type {LiveVoiceState} */
  const next = {
    guildId,
    userId,
    channelId,
    selfMute: asBool(data.self_mute ?? data.selfMute),
    selfDeaf: asBool(data.self_deaf ?? data.selfDeaf),
    selfStream: asBool(data.self_stream ?? data.selfStream),
    selfVideo: asBool(data.self_video ?? data.selfVideo),
    mute: asBool(data.mute),
    deaf: asBool(data.deaf),
    suppress: asBool(data.suppress),
  };
  const key = `${guildId}:${userId}`;
  const previous = store.voiceStates.get(key);
  if (previous && JSON.stringify(previous) === JSON.stringify(next)) {
    counters.duplicate += 1;
    return [];
  }
  store.voiceStates.set(key, next);
  return [{
    type: "voice_state",
    ...next,
  }];
}

/**
 * VOICE_SERVER_UPDATE — обновление RTC-endpoint гильдии без сохранения `token`.
 * @param {DispatchStore} store
 * @param {Record<string, unknown>} data
 * @param {ModelCounters} [counters]
 * @returns {DispatchChange[]}
 */
export function projectVoiceServer(store, data, counters = emptyCounters()) {
  if (Array.isArray(data)) {
    counters.malformed += 1;
    return [];
  }
  const guildId = asString(data.guild_id) || asString(data.guildId);
  if (guildId === "" || !("endpoint" in data)) {
    counters.malformed += 1;
    return [];
  }
  const rawEndpoint = asString(data.endpoint);
  const endpoint = rawEndpoint !== "" ? rawEndpoint : null;
  const previous = store.voiceServers.get(guildId);
  if (previous && previous.endpoint === endpoint) {
    counters.duplicate += 1;
    return [];
  }
  store.voiceServers.set(guildId, { guildId, endpoint });
  return [{
    type: "voice_server",
    guildId,
    endpoint,
  }];
}

/**
 * THREAD_LIST_SYNC — синхронизация списка тредов гильдии.
 * @param {DispatchStore} store
 * @param {Record<string, unknown>} data
 * @param {ModelCounters} [counters]
 * @returns {DispatchChange[]}
 */
export function projectThreadListSync(store, data, counters = emptyCounters()) {
  if (Array.isArray(data)) {
    counters.malformed += 1;
    return [];
  }
  const guildId = asString(data.guild_id) || asString(data.guildId);
  const validThreads = "threads" in data && (Array.isArray(data.threads) || nilish(data.threads));
  if (guildId === "" || !validThreads) {
    counters.malformed += 1;
    return [];
  }
  /** @type {LiveThread[]} */
  const threads = [];
  for (const value of asArray(data.threads)) {
    const record = asRecord(value);
    const id = asString(record.id);
    const name = asString(record.name);
    if (id === "" || name === "") {
      counters.skipped += 1;
      continue;
    }
    const meta = asRecord(record.thread_metadata ?? record.threadMetadata);
    const lastMsg = asString(record.last_message_id ?? record.lastMessageId);
    threads.push({
      id,
      guildId: asString(record.guild_id ?? record.guildId) || guildId,
      parentId: asString(record.parent_id ?? record.parentId),
      ownerId: asString(record.owner_id ?? record.ownerId),
      name,
      type: asNumber(record.type),
      messageCount: asNumber(record.message_count ?? record.messageCount),
      memberCount: asNumber(record.member_count ?? record.memberCount),
      lastMessageId: lastMsg !== "" ? lastMsg : null,
      archived: asBool(meta.archived),
      locked: asBool(meta.locked),
    });
  }
  const previousMap = store.threads.get(guildId);
  if (previousMap) {
    const previousList = [...previousMap.values()];
    if (JSON.stringify(previousList) === JSON.stringify(threads)) {
      counters.duplicate += 1;
      return [];
    }
  }
  store.threads.set(guildId, new Map(threads.map((thread) => [thread.id, thread])));
  return [{
    type: "threads",
    guildId,
    threads,
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
    case "MESSAGE_REACTION_ADD":
    case "MESSAGE_REACTION_REMOVE":
      changes = projectReaction(store, kind, data, counters);
      break;
    case "MESSAGE_CREATE":
      changes = projectMessageCreate(store, data, counters);
      break;
    case "MESSAGE_UPDATE":
      changes = projectMessageUpdate(store, data, counters);
      break;
    case "MESSAGE_ACK":
      changes = projectMessageAck(store, data, counters);
      break;
    case "TYPING_START":
      changes = projectTypingStart(store, data, counters);
      break;
    case "VOICE_STATE_UPDATE":
      changes = projectVoiceState(store, data, counters);
      break;
    case "VOICE_SERVER_UPDATE":
      changes = projectVoiceServer(store, data, counters);
      break;
    case "THREAD_LIST_SYNC":
      changes = projectThreadListSync(store, data, counters);
      break;
    default:
      // Каноническое событие дошло до модели, но проекции в UI пока нет
      // (CHANNEL_INFO, READY_SUPPLEMENTAL, RESUMED, SESSIONS_REPLACE, …):
      // считаем skipped, чтобы приём был виден в dispatch.stats, а не молчал.
      counters.skipped += 1;
      break;
  }
  counters.applied = changes.length > 0 ? 1 : 0;
  return { ok: true, kind, changes, counters };
}
