// @ts-check
/**
 * TASK-113 / N1-07 (RW-40): UI-модуль личных сообщений, групповых DM и друзей
 * (`#home-rail-btn` в `#guild-rail`, список DM/Group DM в `#channel-tree`,
 * вкладки друзей `Online / All / Pending / Blocked / Add Friend` в `#friends-view`,
 * модалка создания Group DM `#group-dm-dialog` до 10 участников и диалог
 * подтверждения незнакомца `#stranger-confirm-dialog` по коду `80013`).
 *
 * Нормативные источники:
 * - `waves/RW-40/DM-FRIENDS-RE.md` (§0–§12);
 * - `waves/RW-40/DESIGN/DM-FRIENDS-SPEC.md` (§1–§6);
 * - `fixtures/gateway/by-type/002-READY.json` (17 `private_channels`, 60 `relationships`, 146 `users`).
 */

/** Канонический идентификатор домашнего режима («Домой / Личные сообщения и Друзья»). */
export const HOME_GUILD_ID = "@me";

/**
 * Вставляет кнопку Home в `#guild-rail` на место `.brand` через штатный DOM API
 * (`insertBefore` + `remove`). Не читает `children.indexOf`/`children.splice`
 * (`HTMLCollection` их не имеет) и не пишет в getter-only `parentNode`
 * (`DM-FRIENDS-SPEC.md §0.2 п.3`: один код-путь в браузере и DOM-шиме).
 * @param {any} rail
 * @param {any} button
 * @param {any} [existingBrand]
 */
export function insertHomeRailButton(rail, button, existingBrand = null) {
  if (existingBrand) {
    rail.insertBefore(button, existingBrand);
    existingBrand.remove();
    return;
  }
  rail.append(button);
}

/** Типы приватных каналов (`[D] @11855584`, `@11856265`). */
export const PRIVATE_CHANNEL_TYPES = Object.freeze({
  DM: 1,
  GROUP_DM: 3,
});

/** Множество допустимых типов приватных каналов (`{1, 3}`). */
export const PRIVATE_CHANNEL_TYPE_SET = Object.freeze(new Set([1, 3]));

/** Битовые флаги канала (`[D] @3250152`). */
export const CHANNEL_FLAGS = Object.freeze({
  PINNED: 2,
  IS_SPAM: 32,
});

/** Битовые флаги получателя (`[D] decoded/d38b37e4d52b4495_0 @243430`). */
export const RECIPIENT_FLAGS = Object.freeze({
  DISMISSED_IN_GAME_MESSAGE_NUX: 1,
  DISMISSED_CURRENT_CHAT_WALLPAPER: 2,
});

/** Типы связей `RelationshipType` (`[D] @258005`, enum `eA$`). */
export const RELATIONSHIP_TYPES = Object.freeze({
  NONE: 0,
  FRIEND: 1,
  BLOCKED: 2,
  PENDING_INCOMING: 3,
  PENDING_OUTGOING: 4,
  IMPLICIT: 5,
  SUGGESTION: 6,
  INCOMING_REQUEST: 3,
  OUTGOING_REQUEST: 4,
});

/** Фильтры очистки входящих заявок (`[D] @7346275`). */
export const RELATIONSHIP_FILTERS = Object.freeze({
  SPAM: 1,
  IGNORED: 2,
});

/** Упорядоченный список вкладок панели `#friends-view` (`DM-FRIENDS-SPEC.md §2.3, §4.1`). */
export const FRIENDS_TABS = Object.freeze([
  "online",
  "all",
  "pending",
  "blocked",
  "add_friend",
]);

/** Внутренние константы секций экрана друзей (`[D] @243700`). */
export const FRIENDS_SECTIONS = Object.freeze({
  ONLINE: "ONLINE",
  ALL: "ALL",
  PENDING: "PENDING",
  PENDING_IGNORED: "PENDING_IGNORED",
  SPAM: "SPAM",
  SUGGESTIONS: "SUGGESTIONS",
  ADD_FRIEND: "ADD_FRIEND",
  BLOCKED: "BLOCKED",
});

/** Максимальное число выбираемых друзей в модалке Group DM (9 друзей + `@me` = 10 участников). */
export const MAX_GROUP_DM_RECIPIENTS = 9;

/** Максимальное общее число участников групповой DM включая `@me`. */
export const MAX_GROUP_DM_PARTICIPANTS = 10;

/** Коды ошибок REST для ЛС и друзей (`[D] @321000..332000`, `DM-FRIENDS-SPEC.md §1.4`). */
export const DM_FRIENDS_ERROR_CODES = Object.freeze({
  UNKNOWN_USER: 10013,
  QUARANTINED: 20026,
  TOO_MANY_FRIENDS: 30002,
  TOO_MANY_RECIPIENTS: 30004,
  TOO_MANY_BLOCKED_USERS: 30059,
  TOO_MANY_PENDING_OUTGOING: 30078,
  EMAIL_VERIFICATION_REQUIRED: 40002,
  RATE_LIMIT_DM_OPEN: 40003,
  USER_BANNED: 40007,
  USER_QUARANTINED: 40068,
  INVALID_ACTION_DM: 50003,
  INVALID_NOTE: 50015,
  INVALID_FORM_BODY: 50035,
  INVALID_CANNOT_FRIEND_SELF: 50096,
  RELATIONSHIP_INCOMING_BLOCKED: 80001,
  RELATIONSHIP_INVALUD_USER_BOT: 80002,
  RELATIONSHIP_INVALID_SELF: 80003,
  RELATIONSHIP_INVALID_DISCORD_TAG: 80004,
  RELATIONSHIP_ALREADY_FRIENDS: 80007,
  RELATIONSHIP_INVALID_NO_CONFIRMATION: 80013,
  USER_DM_UPLOAD_ATTACHMENT_LIMITED_ACCESS: 340005,
  USER_GDM_UPLOAD_ATTACHMENT_LIMITED_ACCESS: 340006,
  USER_FRIEND_REQUEST_LIMITED_ACCESS: 340007,
});

/** Каталог эндпоинтов DM и друзей (`[D] @258630, @259640, @260280, @266725, @270359, @273990, @310495`). */
export const DM_FRIENDS_ENDPOINTS = Object.freeze({
  USER_CHANNELS: "/users/@me/channels",
  USER_CHANNELS_BULK_LEAVE: "/users/@me/channels/bulk-leave",
  USER_GROUP_DM_SHELL: "/users/@me/channels/group-dm-shell",
  /** @param {string} userId */
  DM_CHANNEL: (userId) => `/users/@me/dms/${userId}`,
  /** @param {string} [userId] */
  USER_RELATIONSHIPS: (userId = "@me") => `/users/${userId}/relationships`,
  /** @param {string} userId */
  USER_RELATIONSHIP: (userId) => `/users/@me/relationships/${userId}`,
  /** @param {string} userId */
  IGNORE_USER: (userId) => `/users/@me/relationships/${userId}/ignore`,
  /** @param {string} userId */
  NOTE: (userId) => `/users/@me/notes/${userId}`,
  /** @param {string} channelId */
  CHANNEL: (channelId) => `/channels/${channelId}`,
  /** @param {string} channelId @param {string} userId */
  CHANNEL_RECIPIENT: (channelId, userId) => `/channels/${channelId}/recipients/${userId}`,
  /** @param {string} channelId */
  CHANNEL_RECIPIENT_ME: (channelId) => `/channels/${channelId}/recipients/@me`,
  CHANNEL_RECIPIENT_REJECT_BATCH: () => "/channels/recipients/@me/batch-reject",
});

/** Русские сообщения ошибок по коду `body.code` (`DM-FRIENDS-SPEC.md §1.4`). */
const ERROR_MESSAGES_BY_CODE = Object.freeze({
  [DM_FRIENDS_ERROR_CODES.QUARANTINED]: "Аккаунт ограничен (карантин). Действие недоступно.",
  [DM_FRIENDS_ERROR_CODES.USER_QUARANTINED]: "Аккаунт ограничен (карантин). Действие недоступно.",
  [DM_FRIENDS_ERROR_CODES.TOO_MANY_FRIENDS]: "Достигнут максимальный лимит друзей",
  [DM_FRIENDS_ERROR_CODES.TOO_MANY_RECIPIENTS]: "В этой группе уже максимальное число участников",
  [DM_FRIENDS_ERROR_CODES.TOO_MANY_BLOCKED_USERS]: "Достигнут лимит заблокированных пользователей",
  [DM_FRIENDS_ERROR_CODES.TOO_MANY_PENDING_OUTGOING]: "Слишком много ожидающих исходящих заявок",
  [DM_FRIENDS_ERROR_CODES.EMAIL_VERIFICATION_REQUIRED]:
    "Для отправки заявок необходимо подтвердить адрес электронной почты",
  [DM_FRIENDS_ERROR_CODES.RATE_LIMIT_DM_OPEN]: "Слишком частое открытие новых личных сообщений",
  [DM_FRIENDS_ERROR_CODES.USER_BANNED]: "Аккаунт заблокирован",
  [DM_FRIENDS_ERROR_CODES.UNKNOWN_USER]: "Пользователь не найден",
  [DM_FRIENDS_ERROR_CODES.INVALID_ACTION_DM]: "Невозможно выполнить это действие в личных сообщениях",
  [DM_FRIENDS_ERROR_CODES.INVALID_NOTE]: "Некорректная длина или формат заметки",
  [DM_FRIENDS_ERROR_CODES.INVALID_FORM_BODY]: "Некорректные данные формы",
  [DM_FRIENDS_ERROR_CODES.INVALID_CANNOT_FRIEND_SELF]: "Вы не можете добавить в друзья самого себя",
  [DM_FRIENDS_ERROR_CODES.RELATIONSHIP_INVALID_SELF]: "Вы не можете добавить в друзья самого себя",
  [DM_FRIENDS_ERROR_CODES.RELATIONSHIP_INCOMING_BLOCKED]: "Пользователь не принимает заявки в друзья",
  [DM_FRIENDS_ERROR_CODES.RELATIONSHIP_INVALUD_USER_BOT]: "Ботов нельзя добавлять в друзья",
  [DM_FRIENDS_ERROR_CODES.RELATIONSHIP_INVALID_DISCORD_TAG]:
    "Не удалось найти пользователя с таким именем. Проверьте регистр и написание",
  [DM_FRIENDS_ERROR_CODES.RELATIONSHIP_ALREADY_FRIENDS]: "Вы уже дружите с этим пользователем",
  [DM_FRIENDS_ERROR_CODES.USER_DM_UPLOAD_ATTACHMENT_LIMITED_ACCESS]: "Ограничение доступа к вложениям в ЛС",
  [DM_FRIENDS_ERROR_CODES.USER_GDM_UPLOAD_ATTACHMENT_LIMITED_ACCESS]: "Ограничение доступа к вложениям в группе",
  [DM_FRIENDS_ERROR_CODES.USER_FRIEND_REQUEST_LIMITED_ACCESS]: "Ограничение отправки заявок в друзья",
});

/** Подписи пустого состояния для каждой вкладки `#friends-view` (`DM-FRIENDS-SPEC.md §4.1`). */
const EMPTY_TAB_TEXT = Object.freeze({
  online: "Сейчас никого из друзей нет в сети",
  all: "У вас пока нет друзей. Добавьте кого-нибудь!",
  pending: "Нет ожидающих запросов в друзья",
  blocked: "У вас нет заблокированных пользователей",
  add_friend: "",
});

/** Заголовки секций для вкладок списка друзей (`DM-FRIENDS-SPEC.md §2.3, §4.1`). */
const SECTION_HEADING_PREFIX = Object.freeze({
  online: "В СЕТИ",
  all: "ВСЕ ДРУЗЬЯ",
  pending: "ОЖИДАНИЕ",
  blocked: "ЗАБЛОКИРОВАННЫЕ",
  add_friend: "ДОБАВИТЬ В ДРУЗЬЯ",
});

const RU_COLLATOR = new Intl.Collator("ru", { sensitivity: "base", numeric: true });

/**
 * @typedef {{
 *   id: string,
 *   username: string,
 *   global_name: string | null,
 *   display_name: string | null,
 *   discriminator: string,
 *   avatar: string | null,
 *   bot: boolean,
 * }} NormalizedUser
 *
 * @typedef {{
 *   url: string | null,
 *   avatarHash: string | null,
 *   initials: string,
 *   hue: number,
 * }} ResolvedAvatar
 *
 * @typedef {{
 *   id: string,
 *   type: number,
 *   guild_id: null,
 *   name: string,
 *   rawName: string,
 *   icon: string | null,
 *   owner_id: string | null,
 *   nicks: Record<string, string>,
 *   recipient_ids: string[],
 *   rawRecipients: NormalizedUser[],
 *   last_message_id: string | null,
 *   lastMessageBigInt: bigint,
 *   last_pin_timestamp: string | null,
 *   flags: number,
 *   pinned: boolean,
 *   is_spam: boolean,
 *   is_message_request: boolean,
 *   is_message_request_timestamp: string | null,
 *   recipient_flags: number,
 *   safety_warnings: any[] | null,
 *   blocked_user_warning_dismissed: boolean,
 *   participantCount: number,
 *   subtext: string,
 *   avatar: ResolvedAvatar,
 *   unread: boolean,
 *   mention_count: number,
 * }} HydratedPrivateChannel
 *
 * @typedef {{
 *   id: string,
 *   user_id: string,
 *   type: number,
 *   nickname: string | null,
 *   since: string | null,
 *   is_spam_request: boolean,
 *   is_stranger_request: boolean,
 *   user_ignored: boolean,
 *   note: string | null,
 *   origin_application_id: string | null,
 *   user: NormalizedUser | null,
 *   displayName: string,
 *   userTag: string,
 *   avatar: ResolvedAvatar,
 *   status: string,
 * }} NormalizedRelationship
 */

/**
 * Строгий разбор булевых значений и ETF-строк `"true"` / `"false"` / `"nil"`.
 * @param {unknown} value
 * @param {boolean} [fallback]
 * @returns {boolean}
 */
export function parseEtfBool(value, fallback = false) {
  if (value === true || value === "true") return true;
  if (value === false || value === "false" || value === "nil" || value === null || value === undefined) {
    return false;
  }
  return fallback;
}

/**
 * Нормализует строковое поле с учётом ETF `"nil"`.
 * @param {unknown} value
 * @returns {string | null}
 */
export function parseEtfString(value) {
  if (value === null || value === undefined || value === "nil") return null;
  const str = String(value);
  return str === "nil" ? null : str;
}

/**
 * Безопасно преобразует snowflake (`last_message_id`, `id`) в `BigInt`.
 * `"nil"`, `null`, `undefined`, `""` и нечисловые строки возвращают `0n`.
 * @param {unknown} value
 * @returns {bigint}
 */
export function parseSnowflakeBigInt(value) {
  if (value === null || value === undefined || value === "nil" || value === "") return 0n;
  if (typeof value === "bigint") return value;
  if (typeof value === "number") {
    return Number.isFinite(value) && value >= 0 ? BigInt(Math.trunc(value)) : 0n;
  }
  const str = String(value).trim();
  if (!str || str === "nil" || !/^\d+$/.test(str)) return 0n;
  try {
    return BigInt(str);
  } catch {
    return 0n;
  }
}

/**
 * Вычисляет детерминированный оттенок `--avatar-hue` (`0..359`) из строки идентификатора.
 * @param {string} seed
 * @returns {number}
 */
export function computeAvatarHue(seed) {
  const text = String(seed || "0");
  let hash = 0;
  for (let i = 0; i < text.length; i += 1) {
    hash = (hash * 31 + text.charCodeAt(i)) >>> 0;
  }
  return hash % 360;
}

/**
 * Вычисляет 1–2 заглавных инициала из отображаемого имени.
 * @param {string} displayName
 * @returns {string}
 */
export function computeInitials(displayName) {
  const cleaned = String(displayName ?? "").trim();
  if (!cleaned) return "?";
  const parts = cleaned.split(/\s+/).filter(Boolean);
  if (parts.length >= 2) {
    const first = Array.from(parts[0] ?? "")[0] ?? "";
    const second = Array.from(parts[1] ?? "")[0] ?? "";
    return (first + second).toUpperCase();
  }
  return Array.from(cleaned).slice(0, 2).join("").toUpperCase();
}

/**
 * Нормализует объект пользователя из `READY.users` или вложенного `user`.
 * @param {any} raw
 * @returns {NormalizedUser | null}
 */
export function normalizeUser(raw) {
  if (!raw || typeof raw !== "object" || raw === "nil") return null;
  const id = parseEtfString(raw.id);
  if (!id) return null;
  const username = parseEtfString(raw.username) ?? id;
  const globalName = parseEtfString(raw.global_name);
  const displayName = parseEtfString(raw.display_name);
  const discRaw = parseEtfString(raw.discriminator);
  const discriminator = discRaw && discRaw !== "" ? discRaw : "0";
  const avatar = parseEtfString(raw.avatar);
  const bot = parseEtfBool(raw.bot, false);
  return {
    id,
    username,
    global_name: globalName && globalName.length > 0 ? globalName : null,
    display_name: displayName && displayName.length > 0 ? displayName : null,
    discriminator,
    avatar: avatar && avatar.length > 0 ? avatar : null,
    bot,
  };
}

/**
 * Строит карту `Map<string, NormalizedUser>` из массива `READY.d.users` (146 записей в `002-READY.json`)
 * или существующего `Map` / словаря.
 * @param {any} usersInput
 * @returns {Map<string, NormalizedUser>}
 */
export function buildUserMap(usersInput) {
  /** @type {Map<string, NormalizedUser>} */
  const map = new Map();
  if (!usersInput || usersInput === "nil") return map;
  if (usersInput instanceof Map) {
    for (const [k, v] of usersInput.entries()) {
      const norm = normalizeUser(v) ?? normalizeUser({ ...(v || {}), id: k });
      if (norm) map.set(norm.id, norm);
    }
    return map;
  }
  if (Array.isArray(usersInput)) {
    for (const item of usersInput) {
      const norm = normalizeUser(item);
      if (norm) map.set(norm.id, norm);
    }
    return map;
  }
  if (typeof usersInput === "object") {
    for (const [k, v] of Object.entries(usersInput)) {
      const norm = normalizeUser(v) ?? normalizeUser({ ...(v || {}), id: k });
      if (norm) map.set(norm.id, norm);
    }
  }
  return map;
}

/**
 * Разрешает отображаемое имя пользователя с приоритетом:
 * `nickname || global_name || display_name || username || id` (`DM-FRIENDS-SPEC.md §2.2`).
 * @param {any} user
 * @param {string | null} [nickname]
 * @returns {string}
 */
export function resolveUserDisplayName(user, nickname = null) {
  const cleanNick = parseEtfString(nickname);
  if (cleanNick && cleanNick.trim().length > 0) return cleanNick.trim();
  if (!user || typeof user !== "object") return "";
  const globalName = parseEtfString(user.global_name);
  if (globalName && globalName.trim().length > 0) return globalName;
  const displayName = parseEtfString(user.display_name);
  if (displayName && displayName.trim().length > 0) return displayName;
  const username = parseEtfString(user.username);
  if (username && username.trim().length > 0) return username;
  return parseEtfString(user.id) ?? "";
}

/**
 * Формирует Discord Tag пользователя: для Pomelo (`discriminator === "0"`) без `#`,
 * для классического 4-значного дискриминатора — `username#1234` (`[D] getUserTag`, `DM-FRIENDS-RE.md §4.2`).
 * @param {any} user
 * @returns {string}
 */
export function resolveUserTag(user) {
  if (!user || typeof user !== "object") return "";
  const username = parseEtfString(user.username) ?? parseEtfString(user.id) ?? "";
  const disc = parseEtfString(user.discriminator);
  if (!disc || disc === "0") return username;
  return `${username}#${disc}`;
}

/**
 * Разрешает данные аватара пользователя (CDN URL для хеша аватара, инициалы и оттенок `--avatar-hue`).
 * @param {any} user
 * @param {string} [displayNameFallback]
 * @returns {ResolvedAvatar}
 */
export function resolveUserAvatar(user, displayNameFallback = "") {
  const norm = normalizeUser(user);
  const label = displayNameFallback || resolveUserDisplayName(norm) || norm?.id || "?";
  const initials = computeInitials(label);
  const hue = computeAvatarHue(norm?.id ?? label);
  if (!norm || !norm.avatar) {
    return { url: null, avatarHash: null, initials, hue };
  }
  const ext = norm.avatar.startsWith("a_") ? "gif" : "png";
  return {
    url: `https://cdn.discordapp.com/avatars/${norm.id}/${norm.avatar}.${ext}`,
    avatarHash: norm.avatar,
    initials,
    hue,
  };
}

/**
 * Склоняет подпись числа участников для `GROUP_DM` (`type === 3`):
 * `1 участник`, `2..4 участника`, `5..20 участников` (`DM-FRIENDS-SPEC.md §2.2`).
 * @param {number} count
 * @returns {string}
 */
export function formatParticipantsSubtext(count) {
  const n = Math.max(0, Math.trunc(Number(count) || 0));
  const mod100 = n % 100;
  const mod10 = n % 10;
  if (mod100 >= 11 && mod100 <= 14) return `${n} участников`;
  if (mod10 === 1) return `${n} участник`;
  if (mod10 >= 2 && mod10 <= 4) return `${n} участника`;
  return `${n} участников`;
}

/**
 * Нормализует карту локальных ников `nicks` в групповой DM (`userId -> nick`).
 * @param {any} rawNicks
 * @returns {Record<string, string>}
 */
function normalizeChannelNicks(rawNicks) {
  /** @type {Record<string, string>} */
  const map = {};
  if (!rawNicks || rawNicks === "nil" || typeof rawNicks !== "object") return map;
  if (Array.isArray(rawNicks)) {
    for (const entry of rawNicks) {
      if (entry && typeof entry === "object") {
        const uid = parseEtfString(entry.id ?? entry.user_id);
        const nick = parseEtfString(entry.nick ?? entry.nickname);
        if (uid && nick) map[uid] = nick;
      }
    }
    return map;
  }
  for (const [k, v] of Object.entries(rawNicks)) {
    const nick = parseEtfString(v);
    if (k && nick) map[String(k)] = nick;
  }
  return map;
}

/**
 * Гидрирует приватный канал (`type: 1 | 3`) из `READY.private_channels` (`recipient_ids`)
 * или из REST / `CHANNEL_CREATE` (`recipients: User[]`) (`[D] @4920048, @8899327`, `DM-FRIENDS-SPEC.md §1.1, §2.2`).
 * @param {any} rawChannel
 * @param {Map<string, NormalizedUser> | Record<string, any> | any[]} [usersInput]
 * @param {Map<string, any> | Record<string, any> | any[]} [relationshipsInput]
 * @returns {HydratedPrivateChannel}
 */
export function hydratePrivateChannel(rawChannel, usersInput, relationshipsInput) {
  const ch = rawChannel && typeof rawChannel === "object" ? rawChannel : {};
  const usersById = usersInput instanceof Map ? usersInput : buildUserMap(usersInput);

  /** @type {Map<string, string>} */
  const friendNickById = new Map();
  if (relationshipsInput instanceof Map) {
    for (const [k, rel] of relationshipsInput.entries()) {
      const nick = parseEtfString(rel?.nickname);
      if (nick) friendNickById.set(String(rel?.user_id ?? k), nick);
    }
  } else if (Array.isArray(relationshipsInput)) {
    for (const rel of relationshipsInput) {
      const nick = parseEtfString(rel?.nickname);
      const uid = parseEtfString(rel?.user_id ?? rel?.id);
      if (uid && nick) friendNickById.set(uid, nick);
    }
  }

  const id = parseEtfString(ch.id) ?? "";
  const rawType = Number(ch.type);
  // `[D] @8899327`: тип не подменяется, если сервер прислал не {1,3}; по умолчанию — DM.
  const type = Number.isFinite(rawType) ? rawType : PRIVATE_CHANNEL_TYPES.DM;

  /** @type {string[]} */
  const recipientIds = [];
  /** @type {NormalizedUser[]} */
  const rawRecipients = [];

  if (Array.isArray(ch.recipients) && ch.recipients.length > 0) {
    for (const item of ch.recipients) {
      if (item && typeof item === "object") {
        const normUser = normalizeUser(item);
        if (normUser) {
          usersById.set(normUser.id, normUser);
          recipientIds.push(normUser.id);
          rawRecipients.push(normUser);
        }
      } else {
        const rid = parseEtfString(item);
        if (rid) {
          recipientIds.push(rid);
          const known = usersById.get(rid) ?? {
            id: rid,
            username: rid,
            global_name: null,
            display_name: null,
            discriminator: "0",
            avatar: null,
            bot: false,
          };
          rawRecipients.push(known);
        }
      }
    }
  } else if (Array.isArray(ch.recipient_ids)) {
    for (const rawId of ch.recipient_ids) {
      const rid = parseEtfString(rawId);
      if (!rid) continue;
      recipientIds.push(rid);
      const known = usersById.get(rid) ?? {
        id: rid,
        username: rid,
        global_name: null,
        display_name: null,
        discriminator: "0",
        avatar: null,
        bot: false,
      };
      rawRecipients.push(known);
    }
  }

  // `eD.sortRecipients(recipients, channel.id)` (§2.2 п.3): порядок получателей не
  // зависит от порядка прихода в REST/READY/CHANNEL_CREATE. Точный хеш бандла
  // (`hash(id) ^ hash(channel.id)`) в RE не восстановлен — детерминированный
  // порядок задаётся BigInt(id) по возрастанию; для одного канала он стабилен.
  const order = recipientIds.map((rid, index) => ({ rid, index }));
  order.sort((a, b) => {
    const bigA = parseSnowflakeBigInt(a.rid);
    const bigB = parseSnowflakeBigInt(b.rid);
    if (bigA !== bigB) return bigA < bigB ? -1 : 1;
    return a.rid < b.rid ? -1 : a.rid > b.rid ? 1 : a.index - b.index;
  });
  const sortedIds = order.map((entry) => recipientIds[entry.index] ?? "");
  /** @type {NormalizedUser[]} */
  const sortedRecipients = [];
  for (const entry of order) {
    const recipient = rawRecipients[entry.index];
    if (recipient) sortedRecipients.push(recipient);
  }
  recipientIds.splice(0, recipientIds.length, ...sortedIds);
  rawRecipients.splice(0, rawRecipients.length, ...sortedRecipients);

  const nicks = normalizeChannelNicks(ch.nicks);
  const rawName = parseEtfString(ch.name) ?? "";
  const flags = Number.isFinite(Number(ch.flags ?? ch.flags_)) ? Number(ch.flags ?? ch.flags_) : 0;
  const pinned = (flags & CHANNEL_FLAGS.PINNED) !== 0;
  const isSpam =
    parseEtfBool(ch.is_spam ?? ch.isSpam, false) || (flags & CHANNEL_FLAGS.IS_SPAM) !== 0;
  const isMessageRequest = parseEtfBool(ch.is_message_request ?? ch.isMessageRequest, false);
  const isMessageRequestTimestamp = parseEtfString(
    ch.is_message_request_timestamp ?? ch.isMessageRequestTimestamp,
  );
  const lastMessageId = parseEtfString(ch.last_message_id ?? ch.lastMessageId);
  const lastMessageBigInt = parseSnowflakeBigInt(lastMessageId);
  const lastPinTimestamp = parseEtfString(ch.last_pin_timestamp ?? ch.lastPinTimestamp);
  const icon = parseEtfString(ch.icon);
  const ownerId = parseEtfString(ch.owner_id ?? ch.ownerId);
  const recipientFlags = Number.isFinite(Number(ch.recipient_flags ?? ch.recipientFlags))
    ? Number(ch.recipient_flags ?? ch.recipientFlags)
    : 0;
  const safetyWarnings = Array.isArray(ch.safety_warnings ?? ch.safetyWarnings)
    ? ch.safety_warnings ?? ch.safetyWarnings
    : null;
  const blockedUserWarningDismissed = parseEtfBool(
    ch.blocked_user_warning_dismissed ?? ch.blockedUserWarningDismissed,
    false,
  );

  let name = "";
  let participantCount = 2;
  let subtext = "";
  /** @type {ResolvedAvatar} */
  let avatar;

  if (type === PRIVATE_CHANNEL_TYPES.GROUP_DM) {
    participantCount = recipientIds.length + 1;
    subtext = formatParticipantsSubtext(participantCount);
    if (rawName.trim().length > 0) {
      name = rawName.trim();
    } else if (rawRecipients.length > 0) {
      name = rawRecipients
        .map((u) => nicks[u.id] || friendNickById.get(u.id) || resolveUserDisplayName(u))
        .filter(Boolean)
        .join(", ");
    } else {
      name = "Групповой чат";
    }
    avatar = {
      url: icon ? `https://cdn.discordapp.com/channel-icons/${id}/${icon}.png` : null,
      avatarHash: icon,
      initials: computeInitials(name),
      hue: computeAvatarHue(id || name),
    };
  } else {
    const firstRecipient = rawRecipients[0] ?? (recipientIds[0] ? usersById.get(recipientIds[0]) : null);
    const firstId = recipientIds[0] ?? "";
    const friendNick = firstId ? (friendNickById.get(firstId) ?? nicks[firstId] ?? null) : null;
    name = resolveUserDisplayName(firstRecipient, friendNick) || firstId || id;
    participantCount = 2;
    subtext = parseEtfString(ch.subtext) ?? "";
    avatar = resolveUserAvatar(firstRecipient, name);
  }

  return {
    id,
    type,
    guild_id: null,
    name,
    rawName,
    icon,
    owner_id: ownerId,
    nicks,
    recipient_ids: recipientIds,
    rawRecipients,
    last_message_id: lastMessageId,
    lastMessageBigInt,
    last_pin_timestamp: lastPinTimestamp,
    flags,
    pinned,
    is_spam: isSpam,
    is_message_request: isMessageRequest,
    is_message_request_timestamp: isMessageRequestTimestamp,
    recipient_flags: recipientFlags,
    safety_warnings: safetyWarnings,
    blocked_user_warning_dismissed: blockedUserWarningDismissed,
    participantCount,
    subtext,
    avatar,
    unread: parseEtfBool(ch.unread, false),
    mention_count: Math.max(0, Math.trunc(Number(ch.mention_count ?? ch.mentionCount ?? 0) || 0)),
  };
}

/**
 * Сортирует приватные каналы:
 * 1) сначала закреплённые (`(flags & 2) !== 0`, `ChannelFlags.PINNED = 2`);
 * 2) внутри группы — по убыванию `BigInt(last_message_id ?? 0)` (`parseSnowflakeBigInt`);
 * 3) при равенстве `last_message_id` — по убыванию `BigInt(id ?? 0)` (`DM-FRIENDS-SPEC.md §2.2`).
 * @param {any[]} channels
 * @returns {any[]}
 */
export function sortPrivateChannels(channels) {
  if (!Array.isArray(channels)) return [];
  return [...channels].sort((a, b) => {
    const pinA = ((Number(a?.flags ?? 0) & CHANNEL_FLAGS.PINNED) !== 0) || Boolean(a?.pinned);
    const pinB = ((Number(b?.flags ?? 0) & CHANNEL_FLAGS.PINNED) !== 0) || Boolean(b?.pinned);
    if (pinA !== pinB) return pinB ? 1 : -1;

    const lastA =
      typeof a?.lastMessageBigInt === "bigint"
        ? a.lastMessageBigInt
        : parseSnowflakeBigInt(a?.last_message_id ?? a?.lastMessageId);
    const lastB =
      typeof b?.lastMessageBigInt === "bigint"
        ? b.lastMessageBigInt
        : parseSnowflakeBigInt(b?.last_message_id ?? b?.lastMessageId);
    if (lastB !== lastA) return lastB > lastA ? 1 : -1;

    const idA = parseSnowflakeBigInt(a?.id);
    const idB = parseSnowflakeBigInt(b?.id);
    if (idB !== idA) return idB > idA ? 1 : -1;
    return 0;
  });
}

/**
 * Определяет эффективный статус присутствия пользователя (`online`, `idle`, `dnd`, `streaming`, `offline`).
 * @param {any} presence
 * @returns {string}
 */
export function resolvePresenceStatus(presence) {
  if (!presence || typeof presence !== "object" || presence === "nil") return "offline";
  const rawStatus = String(presence.status ?? "offline").toLowerCase();
  if (rawStatus === "streaming") return "streaming";
  if (
    Array.isArray(presence.activities) &&
    presence.activities.some((/** @type {any} */ a) => Number(a?.type) === 1)
  ) {
    if (rawStatus !== "offline" && rawStatus !== "invisible") return "streaming";
  }
  if (rawStatus === "online" || rawStatus === "idle" || rawStatus === "dnd") return rawStatus;
  return "offline";
}

/**
 * Проверяет правило `isIgnored`: пользователь считается игнорируемым, если `type !== 2` (`!BLOCKED`)
 * и `user_ignored === true` (`[D] @9319095`, `DM-FRIENDS-SPEC.md §1.3.4`).
 * @param {any} relationship
 * @returns {boolean}
 */
export function isRelationshipIgnored(relationship) {
  if (!relationship || typeof relationship !== "object") return false;
  const type = Number(relationship.type ?? 0);
  if (type === RELATIONSHIP_TYPES.BLOCKED) return false;
  return parseEtfBool(relationship.user_ignored ?? relationship.userIgnored, false);
}

/**
 * Нормализует запись `Relationship` из `READY.relationships` (`user_id`) или из REST / `RELATIONSHIP_*` (`user`).
 * @param {any} rawRel
 * @param {Map<string, NormalizedUser> | Record<string, any> | any[]} [usersInput]
 * @param {Record<string, any> | Map<string, any>} [presencesInput]
 * @returns {NormalizedRelationship}
 */
export function normalizeRelationship(rawRel, usersInput, presencesInput) {
  const r = rawRel && typeof rawRel === "object" ? rawRel : {};
  const usersById = usersInput instanceof Map ? usersInput : buildUserMap(usersInput);
  const inlineUser = normalizeUser(r.user);
  if (inlineUser) usersById.set(inlineUser.id, inlineUser);

  const id = parseEtfString(r.id ?? r.user_id ?? inlineUser?.id) ?? "";
  const userId = parseEtfString(r.user_id ?? inlineUser?.id ?? r.id) ?? id;
  const type = Number.isFinite(Number(r.type)) ? Number(r.type) : RELATIONSHIP_TYPES.NONE;
  const nickname = parseEtfString(r.nickname);
  const since = parseEtfString(r.since);
  const isSpamRequest = parseEtfBool(r.is_spam_request ?? r.isSpamRequest, false);
  const isStrangerRequest = parseEtfBool(r.is_stranger_request ?? r.isStrangerRequest, false);
  const userIgnored = parseEtfBool(r.user_ignored ?? r.userIgnored, false);
  const note = parseEtfString(r.note);
  const originApplicationId = parseEtfString(r.origin_application_id ?? r.originApplicationId);

  const user = inlineUser ?? usersById.get(userId) ?? null;
  const displayName = resolveUserDisplayName(user, nickname) || userId;
  const userTag = user ? resolveUserTag(user) : userId;
  const avatar = resolveUserAvatar(user, displayName);

  let rawPresence = null;
  if (presencesInput instanceof Map) {
    rawPresence = presencesInput.get(userId);
  } else if (presencesInput && typeof presencesInput === "object") {
    rawPresence = /** @type {Record<string, any>} */ (presencesInput)[userId];
  }
  const status = resolvePresenceStatus(rawPresence);

  return {
    id,
    user_id: userId,
    type,
    nickname: nickname && nickname.length > 0 ? nickname : null,
    since,
    is_spam_request: isSpamRequest,
    is_stranger_request: isStrangerRequest,
    user_ignored: userIgnored,
    note,
    origin_application_id: originApplicationId,
    user,
    displayName,
    userTag,
    avatar,
    status,
  };
}

/**
 * Классифицирует коллекцию `relationships` и считает счётчики `RelationshipStore`
 * (`[D] @9318383`, `DM-FRIENDS-SPEC.md §1.3.4`):
 * `pendingCount = Math.max(incomingCount - spamCount - pendingIgnoredCount, 0)`.
 * @param {any[]} relationships
 * @param {Map<string, NormalizedUser> | Record<string, any> | any[]} [usersInput]
 * @param {Record<string, any> | Map<string, any>} [presencesInput]
 */
export function classifyRelationships(relationships, usersInput, presencesInput) {
  const usersById = usersInput instanceof Map ? usersInput : buildUserMap(usersInput);
  const list = Array.isArray(relationships)
    ? relationships.map((r) => normalizeRelationship(r, usersById, presencesInput))
    : [];

  let friendCount = 0;
  let blockedCount = 0;
  let incomingCount = 0;
  let outgoingCount = 0;
  let spamCount = 0;
  let pendingIgnoredCount = 0;
  let onlineFriendCount = 0;

  for (const rel of list) {
    if (rel.type === RELATIONSHIP_TYPES.FRIEND) {
      friendCount += 1;
      if (
        !isRelationshipIgnored(rel) &&
        (rel.status === "online" || rel.status === "idle" || rel.status === "dnd" || rel.status === "streaming")
      ) {
        onlineFriendCount += 1;
      }
    } else if (rel.type === RELATIONSHIP_TYPES.BLOCKED) {
      blockedCount += 1;
    } else if (rel.type === RELATIONSHIP_TYPES.PENDING_INCOMING) {
      incomingCount += 1;
      if (rel.is_spam_request) spamCount += 1;
      if (rel.user_ignored) pendingIgnoredCount += 1;
    } else if (rel.type === RELATIONSHIP_TYPES.PENDING_OUTGOING) {
      outgoingCount += 1;
    }
  }

  const pendingCount = Math.max(incomingCount - spamCount - pendingIgnoredCount, 0);

  return {
    friendCount,
    onlineFriendCount,
    blockedCount,
    incomingCount,
    outgoingCount,
    spamCount,
    pendingIgnoredCount,
    pendingCount,
    normalized: list,
  };
}

/**
 * Фильтрует и сортирует список связей для выбранной вкладки `#friends-view`
 * (`online | all | pending | blocked | add_friend`, `DM-FRIENDS-SPEC.md §4.1`).
 * @param {any[]} relationships
 * @param {string} tab
 * @param {{
 *   usersById?: Map<string, NormalizedUser> | Record<string, any> | any[],
 *   presencesById?: Record<string, any> | Map<string, any>,
 *   searchQuery?: string,
 *   showSpamAndIgnored?: boolean,
 * }} [options]
 * @returns {NormalizedRelationship[]}
 */
export function filterRelationshipsByTab(relationships, tab, options = {}) {
  const activeTab = String(tab || "online");
  if (activeTab === "add_friend") return [];

  const usersById =
    options.usersById instanceof Map ? options.usersById : buildUserMap(options.usersById);
  const normalized = Array.isArray(relationships)
    ? relationships.map((r) => normalizeRelationship(r, usersById, options.presencesById))
    : [];

  const query = String(options.searchQuery ?? "").trim().toLowerCase();
  const showSpam = Boolean(options.showSpamAndIgnored);

  const filtered = normalized.filter((rel) => {
    if (query) {
      const hay = `${rel.displayName} ${rel.userTag} ${rel.user?.username ?? ""}`.toLowerCase();
      if (!hay.includes(query)) return false;
    }

    if (activeTab === "online") {
      return (
        rel.type === RELATIONSHIP_TYPES.FRIEND &&
        !isRelationshipIgnored(rel) &&
        (rel.status === "online" ||
          rel.status === "idle" ||
          rel.status === "dnd" ||
          rel.status === "streaming")
      );
    }
    if (activeTab === "all") {
      return rel.type === RELATIONSHIP_TYPES.FRIEND;
    }
    if (activeTab === "pending") {
      if (showSpam) {
        return (
          rel.type === RELATIONSHIP_TYPES.PENDING_INCOMING &&
          (rel.is_spam_request || rel.user_ignored)
        );
      }
      return (
        (rel.type === RELATIONSHIP_TYPES.PENDING_INCOMING &&
          !rel.is_spam_request &&
          !rel.user_ignored) ||
        rel.type === RELATIONSHIP_TYPES.PENDING_OUTGOING
      );
    }
    if (activeTab === "blocked") {
      return rel.type === RELATIONSHIP_TYPES.BLOCKED;
    }
    return false;
  });

  if (activeTab === "pending") {
    return filtered.sort((a, b) => {
      if (a.type !== b.type) {
        return a.type === RELATIONSHIP_TYPES.PENDING_INCOMING ? -1 : 1;
      }
      const timeA = a.since ? Date.parse(a.since) || 0 : 0;
      const timeB = b.since ? Date.parse(b.since) || 0 : 0;
      if (timeB !== timeA) return timeB - timeA;
      return RU_COLLATOR.compare(a.user_id, b.user_id);
    });
  }

  return filtered.sort((a, b) => {
    const cmp = RU_COLLATOR.compare(a.displayName, b.displayName);
    if (cmp !== 0) return cmp;
    return RU_COLLATOR.compare(a.user_id, b.user_id);
  });
}

/**
 * Разбирает и валидирует ввод формы «Добавить в друзья» (`[D] @697487`, `DM-FRIENDS-SPEC.md §2.3`):
 * - Pomelo (`"username"` без `#`): `parseInt(undefined) === NaN` -> JSON `discriminator: null`;
 * - Классический тег (`"username#1234"`): `discriminator: 1234` (число!).
 * @param {string} rawInput
 * @param {string} [note]
 * @param {Record<string, any>} [captchaPayload]
 * @returns {{
 *   ok: true,
 *   payload: { username: string, discriminator: number | null, note?: string, [key: string]: any }
 * } | {
 *   ok: false,
 *   error: string
 * }}
 */
export function parseFriendRequestInput(rawInput, note, captchaPayload) {
  const trimmed = String(rawInput ?? "").trim();
  if (!trimmed) {
    return { ok: false, error: "Введите имя пользователя Discord." };
  }
  const parts = trimmed.split("#");
  if (parts.length > 2) {
    return {
      ok: false,
      error: "Некорректный тег Discord. Используйте формат username или username#1234.",
    };
  }
  const username = (parts[0] ?? "").trim();
  const rawDisc = parts[1];
  if (!username) {
    return { ok: false, error: "Имя пользователя не может быть пустым." };
  }

  /** @type {number | null} */
  let discriminator = null;
  if (rawDisc !== undefined) {
    if (!/^\d{4}$/.test(rawDisc)) {
      return {
        ok: false,
        error: "Дискриминатор после # должен состоять из 4 цифр (например, username#1234).",
      };
    }
    const parsed = Number.parseInt(rawDisc, 10);
    discriminator = Number.isNaN(parsed) ? null : parsed;
  } else {
    // [D] @697487: parseInt(undefined) -> NaN -> при JSON.stringify даёт null
    const nanDisc = Number.parseInt(/** @type {any} */ (undefined), 10);
    discriminator = JSON.parse(JSON.stringify(nanDisc));
  }

  /** @type {{ username: string, discriminator: number | null, note?: string, [key: string]: any }} */
  const payload = {
    username,
    discriminator,
    ...(captchaPayload && typeof captchaPayload === "object" ? captchaPayload : {}),
  };
  if (note !== undefined && note !== "") {
    payload.note = note;
  }
  return { ok: true, payload };
}

/**
 * Нормализует вход получателей `_getRecipients` для `POST /users/@me/channels` (`[D] @569410`).
 * Итоговое тело всегда содержит массив `recipients` и никогда не содержит `recipient_id`.
 * @param {string | string[] | null | undefined} recipientIds
 * @returns {{ recipients: string[] }}
 */
export function normalizeRecipientsPayload(recipientIds) {
  if (recipientIds === null || recipientIds === undefined) {
    return { recipients: [] };
  }
  const list = Array.isArray(recipientIds) ? recipientIds : [recipientIds];
  const recipients = list
    .map((id) => parseEtfString(id))
    .filter(/** @returns {id is string} */ (id) => Boolean(id && id.trim().length > 0));
  return { recipients };
}

/**
 * Проверяет ответ `GET /users/@me/notes/{user_id}` (`[D] decoded/9f6dccfb41eb44b6_0 @493`).
 * Если `note_user_id !== expectedUserId`, выбрасывает `Error("Invalid response from server")`.
 * @param {any} responseBody
 * @param {string} expectedUserId
 * @returns {{ note_user_id: string, note: string }}
 */
export function validateNoteResponse(responseBody, expectedUserId) {
  if (!responseBody || typeof responseBody !== "object") {
    throw new Error("Invalid response from server");
  }
  const actualId = String(responseBody.note_user_id ?? "");
  if (actualId !== String(expectedUserId)) {
    throw new Error("Invalid response from server");
  }
  return {
    note_user_id: actualId,
    note: String(responseBody.note ?? ""),
  };
}

/**
 * Применяет Gateway dispatch-события ЛС и друзей к состоянию стора (`[D] DM-FRIENDS-RE.md §6.4, §7.1–§7.4`,
 * `DM-FRIENDS-SPEC.md §1.5`, `T-N107-DISPATCH-01`, `T-N107-READY-02`).
 * Поддерживаемые события:
 * - `READY` / `CONNECTION_OPEN` (полная замена `private_channels` и `relationships`);
 * - `CHANNEL_CREATE` (идемпотентный upsert по `id` для `type: 1 | 3` + мерж `recipients` в `users`);
 * - `CHANNEL_DELETE` (удаление приватного канала по `id`);
 * - `CHANNEL_RECIPIENT_ADD` / `CHANNEL_RECIPIENT_REMOVE` (обновление `recipient_ids` и `nicks`);
 * - `RELATIONSHIP_ADD` / `RELATIONSHIP_UPDATE` (при `4 -> 1` выставляет `friendRequestAcceptedUser`) / `RELATIONSHIP_REMOVE`.
 * @param {{
 *   privateChannels?: any[],
 *   relationships?: any[],
 *   users?: any[] | Map<string, NormalizedUser>,
 *   friendRequestAcceptedUser?: NormalizedUser | null,
 * }} currentState
 * @param {{ type: string, d?: any, channel?: any, relationship?: any, [key: string]: any }} event
 */
export function applyDmFriendsDispatch(currentState, event) {
  const usersById = buildUserMap(currentState?.users);
  let rawChannels = Array.isArray(currentState?.privateChannels)
    ? [...currentState.privateChannels]
    : [];
  let rawRelationships = Array.isArray(currentState?.relationships)
    ? [...currentState.relationships]
    : [];
  /** @type {NormalizedUser | null} */
  let friendRequestAcceptedUser = currentState?.friendRequestAcceptedUser ?? null;

  const type = String(event?.type ?? "");
  const payload = event?.d ?? event;

  if (type === "READY" || type === "CONNECTION_OPEN") {
    usersById.clear();
    const nextUsers = payload?.users ?? [];
    for (const u of Array.isArray(nextUsers) ? nextUsers : []) {
      const norm = normalizeUser(u);
      if (norm) usersById.set(norm.id, norm);
    }
    rawRelationships = Array.isArray(payload?.relationships)
      ? payload.relationships.map((/** @type {any} */ r) => normalizeRelationship(r, usersById))
      : [];
    const incomingChannels =
      payload?.private_channels ?? payload?.initialPrivateChannels ?? [];
    rawChannels = Array.isArray(incomingChannels)
      ? incomingChannels.map((/** @type {any} */ c) =>
          hydratePrivateChannel(c, usersById, rawRelationships),
        )
      : [];
    friendRequestAcceptedUser = null;
  } else if (type === "CHANNEL_CREATE") {
    const chPayload = payload?.channel ?? payload;
    const chType = Number(chPayload?.type ?? PRIVATE_CHANNEL_TYPES.DM);
    if (PRIVATE_CHANNEL_TYPE_SET.has(chType)) {
      const hydrated = hydratePrivateChannel(chPayload, usersById, rawRelationships);
      if (hydrated.id) {
        const idx = rawChannels.findIndex((c) => String(c?.id) === hydrated.id);
        if (idx >= 0) {
          rawChannels[idx] = hydrated;
        } else {
          rawChannels.push(hydrated);
        }
      }
    }
  } else if (type === "CHANNEL_DELETE") {
    const chPayload = payload?.channel ?? payload;
    const delId = parseEtfString(chPayload?.id);
    if (delId) {
      rawChannels = rawChannels.filter((c) => String(c?.id) !== delId);
    }
  } else if (type === "CHANNEL_RECIPIENT_ADD") {
    const channelId = parseEtfString(payload?.channel_id ?? payload?.channelId);
    const addedUser = normalizeUser(payload?.user);
    const nick = parseEtfString(payload?.nick);
    if (addedUser) usersById.set(addedUser.id, addedUser);
    if (channelId && addedUser) {
      rawChannels = rawChannels.map((c) => {
        if (String(c?.id) !== channelId) return c;
        const hydrated = hydratePrivateChannel(c, usersById, rawRelationships);
        const nextIds = hydrated.recipient_ids.includes(addedUser.id)
          ? hydrated.recipient_ids
          : [...hydrated.recipient_ids, addedUser.id];
        const nextNicks = { ...hydrated.nicks };
        if (nick) nextNicks[addedUser.id] = nick;
        return hydratePrivateChannel(
          {
            ...hydrated,
            recipient_ids: nextIds,
            nicks: nextNicks,
          },
          usersById,
          rawRelationships,
        );
      });
    }
  } else if (type === "CHANNEL_RECIPIENT_REMOVE") {
    const channelId = parseEtfString(payload?.channel_id ?? payload?.channelId);
    const removedUserId = parseEtfString(payload?.user?.id ?? payload?.user_id);
    if (channelId && removedUserId) {
      rawChannels = rawChannels.map((c) => {
        if (String(c?.id) !== channelId) return c;
        const hydrated = hydratePrivateChannel(c, usersById, rawRelationships);
        const nextIds = hydrated.recipient_ids.filter((id) => id !== removedUserId);
        const nextNicks = { ...hydrated.nicks };
        delete nextNicks[removedUserId];
        return hydratePrivateChannel(
          {
            ...hydrated,
            recipient_ids: nextIds,
            nicks: nextNicks,
          },
          usersById,
          rawRelationships,
        );
      });
    }
  } else if (type === "RELATIONSHIP_ADD" || type === "RELATIONSHIP_UPDATE") {
    const relPayload = payload?.relationship ?? payload;
    const normRel = normalizeRelationship(relPayload, usersById);
    if (normRel.id) {
      const existingIdx = rawRelationships.findIndex(
        (r) => String(r?.id ?? r?.user_id) === normRel.id,
      );
      const prevType = existingIdx >= 0 ? Number(rawRelationships[existingIdx]?.type ?? 0) : 0;
      if (
        type === "RELATIONSHIP_UPDATE" &&
        prevType === RELATIONSHIP_TYPES.PENDING_OUTGOING &&
        normRel.type === RELATIONSHIP_TYPES.FRIEND
      ) {
        friendRequestAcceptedUser = normRel.user;
      }
      if (existingIdx >= 0) {
        rawRelationships[existingIdx] = normRel;
      } else {
        rawRelationships.push(normRel);
      }
    }
  } else if (type === "RELATIONSHIP_REMOVE") {
    const relPayload = payload?.relationship ?? payload;
    const remId = parseEtfString(relPayload?.id ?? relPayload?.user_id);
    if (remId) {
      rawRelationships = rawRelationships.filter(
        (r) => String(r?.id ?? r?.user_id) !== remId,
      );
    }
  }

  return {
    privateChannels: sortPrivateChannels(
      rawChannels.map((c) => hydratePrivateChannel(c, usersById, rawRelationships)),
    ),
    relationships: rawRelationships.map((r) => normalizeRelationship(r, usersById)),
    users: Array.from(usersById.values()),
    usersById,
    friendRequestAcceptedUser,
  };
}

/**
 * Извлекает код ошибки Discord (`body.code` или `code`) из исключения транспорта.
 * @param {unknown} err
 * @returns {number}
 */
function extractDiscordErrorCode(err) {
  if (!err || typeof err !== "object") return 0;
  const bag = /** @type {Record<string, any>} */ (err);
  if (bag.body && typeof bag.body === "object" && typeof bag.body.code === "number") {
    return bag.body.code;
  }
  if (typeof bag.code === "number") return bag.code;
  return 0;
}

/**
 * Извлекает задержку rate limit из `retry_after_ms` (мс) или `retry_after` (секунды).
 * У ответа `429` Discord поля `body.code` нет (`DM-FRIENDS-SPEC.md §1.4`).
 * @param {unknown} err
 * @returns {number}
 */
export function extractRateLimitRetryAfterMs(err) {
  if (!err || typeof err !== "object") return 0;
  const bag = /** @type {Record<string, any>} */ (err);
  const payload =
    bag.body && typeof bag.body === "object" ? /** @type {Record<string, any>} */ (bag.body) : bag;
  const directMs = Number(payload.retry_after_ms);
  if (Number.isFinite(directMs) && directMs > 0) return Math.round(directMs);
  const seconds = Number(payload.retry_after);
  if (Number.isFinite(seconds) && seconds > 0) return Math.round(seconds * 1000);
  return 0;
}

/**
 * @param {unknown} err
 * @returns {boolean}
 */
function isRateLimitError(err) {
  const bag = /** @type {Record<string, any>} */ (err ?? {});
  return Number(bag.status) === 429 || extractRateLimitRetryAfterMs(err) > 0;
}

/**
 * @param {number} retryAfterMs
 * @returns {string}
 */
function rateLimitText(retryAfterMs) {
  const seconds = Math.max(1, Math.ceil(retryAfterMs / 1000));
  return `Слишком много запросов. Повторите через ${seconds} с.`;
}

/**
 * Создаёт UI-контроллер личных сообщений, групповых DM и друзей (`DM-FRIENDS-SPEC.md §2–§5`).
 * @param {{
 *   document?: any,
 *   request?: (req: { method: string, url: string, body?: any, query?: any }) => Promise<any>,
 *   invoke?: (cmd: string, args?: any) => Promise<any>,
 *   privateChannels?: any[],
 *   relationships?: any[],
 *   users?: any[],
 *   presences?: Record<string, any>,
 *   guildId?: string,
 *   onSelectChannel?: (channelId: string, channel: HydratedPrivateChannel) => void,
 *   onSelectGuild?: (guildId: string) => void,
 *   timers?: { setInterval: (fn: () => void, ms: number) => any, clearInterval: (handle: any) => void },
 * }} [options]
 */
export function createDmFriendsController(options = {}) {
  const doc = options.document ?? /** @type {any} */ (globalThis).document;
  const usersById = buildUserMap(options.users ?? []);
  /** @type {Record<string, any>} */
  let presencesById = { ...(options.presences ?? {}) };

  /** @type {NormalizedRelationship[]} */
  let relationships = (options.relationships ?? []).map((r) =>
    normalizeRelationship(r, usersById, presencesById),
  );
  /** @type {HydratedPrivateChannel[]} */
  let privateChannels = sortPrivateChannels(
    (options.privateChannels ?? [])
      .filter((c) => {
        const type = c?.type;
        return type === null || type === undefined || PRIVATE_CHANNEL_TYPE_SET.has(Number(type));
      })
      .map((c) => hydratePrivateChannel(c, usersById, relationships)),
  );

  let currentGuildId = String(options.guildId ?? "696738350706458685");
  /** @type {"friends" | "requests" | "dm"} */
  let homeSubview = "friends";
  let activeDmChannelId = "";
  let activeFriendsTab = "online";
  let friendsSearchQuery = "";
  let showPendingSpamAndIgnored = false;

  // Состояние модалки Group DM
  /** @type {Set<string>} */
  const selectedGroupDmUserIds = new Set();
  let groupDmSearchQuery = "";
  let groupDmActiveOptionIndex = 0;
  /** @type {any} */
  let groupDmOpener = null;

  // Состояние диалога подтверждения незнакомца (код 80013)
  let pendingStrangerUserId = "";
  /** @type {any} */
  let strangerDialogOpener = null;

  // Состояние контекстного меню друга
  let activeContextFriendId = "";
  /** @type {any} */
  let friendMenuOpener = null;

  /**
   * @param {string} tag
   * @param {string} [className]
   * @param {string} [text]
   * @returns {any}
   */
  const make = (tag, className = "", text = "") => {
    const el = doc.createElement(tag);
    if (className) el.className = className;
    if (text) el.textContent = text;
    return el;
  };

  /**
   * Отправляет REST-запрос через переданный адаптер `options.request`.
   *
   * P0-2 (task-134): `invoke`-fallback на `discord_rest_request` убран — такой
   * команды нет в `generate_handler!` (`src/main.rs`), и в рантайме она падала
   * бы с «unknown command». Без адаптера контроллер отказывает явно, чтобы
   * вызывающий код получил ошибку через `reportActionError`, а не тихий успех.
   * @param {{ method: string, url: string, body?: any, query?: any }} req
   * @returns {Promise<any>}
   */
  const sendRest = async (req) => {
    if (typeof options.request === "function") {
      return options.request(req);
    }
    throw new Error(
      `createDmFriendsController: не передан REST-адаптер request; ` +
        `${req.method} ${req.url} не выполнен`,
    );
  };

  /**
   * Озвучивает статусное сообщение в `#server-status` (`role="status"`).
   * @param {string} text
   */
  const announceStatus = (text) => {
    const statusEl = doc?.getElementById("server-status");
    if (statusEl) statusEl.textContent = text;
  };

  const timers = options.timers ?? {
    setInterval: (fn, ms) => globalThis.setInterval(fn, ms),
    clearInterval: (handle) => globalThis.clearInterval(handle),
  };

  /** @type {any} */
  let rateLimitTimer = null;

  /**
   * Останавливает активный посекундный отсчёт 429.
   */
  function clearRateLimitCountdown() {
    if (rateLimitTimer !== null) {
      timers.clearInterval(rateLimitTimer);
      rateLimitTimer = null;
    }
  }

  /**
   * Показывает сообщение 429 и ведёт посекундный отсчёт `retry_after_ms`
   * (`DM-FRIENDS-SPEC.md §1.4`, `errorUxConfig === 0`).
   * @param {number} retryAfterMs
   * @param {any} feedbackEl
   * @param {() => void} [onDone]
   */
  function startRateLimitCountdown(retryAfterMs, feedbackEl, onDone = () => {}) {
    clearRateLimitCountdown();
    let remaining = Math.max(1, Math.ceil(retryAfterMs / 1000));
    const render = () => {
      if (!feedbackEl) return;
      feedbackEl.textContent =
        remaining > 0 ? rateLimitText(remaining * 1000) : "Можно повторить попытку.";
    };
    render();
    if (remaining <= 0) {
      onDone();
      return;
    }
    const handle = timers.setInterval(() => {
      remaining -= 1;
      render();
      if (remaining <= 0) {
        clearRateLimitCountdown();
        onDone();
      }
    }, 1000);
    if (handle && typeof handle.unref === "function") handle.unref();
    rateLimitTimer = handle;
  }

  /**
   * Формирует результат операции при `429`.
   * @param {unknown} err
   * @param {any} [feedbackEl]
   * @param {() => void} [onDone]
   */
  function applyRateLimit(err, feedbackEl = null, onDone = () => {}) {
    const retryAfterMs = Math.max(1000, extractRateLimitRetryAfterMs(err));
    startRateLimitCountdown(retryAfterMs, feedbackEl, onDone);
    return { ok: false, code: 0, rateLimited: true, retryAfterMs, error: rateLimitText(retryAfterMs) };
  }

  /**
   * Единый путь ошибок действий с друзьями: код → текст, сообщение в `#server-status`.
   * @param {unknown} err
   * @param {string} fallback
   */
  function reportActionError(err, fallback) {
    if (isRateLimitError(err)) {
      const retryAfterMs = Math.max(1000, extractRateLimitRetryAfterMs(err));
      announceStatus(rateLimitText(retryAfterMs));
      return { ok: false, code: 0, rateLimited: true, retryAfterMs, error: rateLimitText(retryAfterMs) };
    }
    const code = extractDiscordErrorCode(err);
    const message =
      /** @type {Record<number, string>} */ (ERROR_MESSAGES_BY_CODE)[code] ?? fallback;
    announceStatus(message);
    return { ok: false, code, error: message };
  }

  /**
   * Фокусируемый элемент внутри контейнера (для focus trap диалогов).
   * @param {any} container
   * @returns {any[]}
   */
  function getFocusable(container) {
    if (!container) return [];
    const nodes = Array.from(container.querySelectorAll("button, input, select, textarea, [tabindex]"));
    return nodes.filter((node) => {
      if (node.disabled) return false;
      if (node.getAttribute("disabled") !== null) return false;
      if (node.getAttribute("hidden") !== null) return false;
      if (node.getAttribute("tabindex") === "-1") return false;
      return true;
    });
  }

  /**
   * Узел лежит внутри контейнера.
   * @param {any} container
   * @param {any} node
   * @returns {boolean}
   */
  function isInside(container, node) {
    let current = node;
    while (current) {
      if (current === container) return true;
      current = current.parentNode;
    }
    return false;
  }

  /**
   * Циклический `Tab`/`Shift+Tab` внутри диалога (`DM-FRIENDS-SPEC.md §5.1, §5.2 п.5`).
   * @param {any} container
   * @param {any} ev
   * @returns {boolean}
   */
  function trapTabKey(container, ev) {
    if (ev.key !== "Tab") return false;
    const focusables = getFocusable(container);
    if (focusables.length === 0) return false;
    const first = focusables[0];
    const last = focusables[focusables.length - 1];
    const active = doc?.activeElement;
    const outside = !isInside(container, active);
    if (ev.shiftKey) {
      if (active === first || outside) {
        ev.preventDefault();
        last?.focus();
        return true;
      }
    } else if (active === last || outside) {
      ev.preventDefault();
      first?.focus();
      return true;
    }
    return false;
  }

  /* ========================================================================
   * 1. Монтирование кнопки #home-rail-btn в #guild-rail (DM-FRIENDS-SPEC §2.1)
   * ======================================================================== */
  const rail = doc?.getElementById("guild-rail");
  /** @type {any} */
  let homeRailBtn = doc?.getElementById("home-rail-btn");
  /** @type {any} */
  let homeRailBadge = null;

  if (rail && !homeRailBtn) {
    const existingBrand = rail.querySelector(".brand");
    homeRailBtn = make("button", "brand home-rail-btn");
    homeRailBtn.id = "home-rail-btn";
    homeRailBtn.setAttribute("type", "button");
    homeRailBtn.setAttribute("data-action", "nav-home");
    homeRailBtn.setAttribute("data-guild-id", HOME_GUILD_ID);
    if (homeRailBtn.dataset) homeRailBtn.dataset.guild = HOME_GUILD_ID;
    homeRailBtn.setAttribute("title", "Личные сообщения и Друзья");
    homeRailBtn.setAttribute("aria-label", "Личные сообщения и Друзья");
    homeRailBtn.setAttribute("aria-current", "false");
    homeRailBtn.setAttribute("tabindex", "-1");

    const monogram = make("span", "brand-monogram", "DL");
    monogram.setAttribute("aria-hidden", "true");
    homeRailBadge = make("span", "guild-mention-badge", "0");
    homeRailBadge.setAttribute("aria-hidden", "true");
    homeRailBadge.setAttribute("hidden", "");
    homeRailBtn.append(monogram, homeRailBadge);

    if (existingBrand && existingBrand.parentNode === rail) {
      insertHomeRailButton(rail, homeRailBtn, existingBrand);
    } else {
      insertHomeRailButton(rail, homeRailBtn, null);
    }
  } else if (homeRailBtn) {
    homeRailBtn.setAttribute("data-guild-id", HOME_GUILD_ID);
    homeRailBtn.setAttribute("data-action", "nav-home");
    homeRailBtn.removeAttribute("aria-hidden");
    homeRailBadge = homeRailBtn.querySelector(".guild-mention-badge");
    if (!homeRailBadge) {
      homeRailBadge = make("span", "guild-mention-badge", "0");
      homeRailBadge.setAttribute("aria-hidden", "true");
      homeRailBadge.setAttribute("hidden", "");
      homeRailBtn.append(homeRailBadge);
    }
  }

  // Сохраняем исходное содержимое шапки и дерева гильдии для чистого возврата (закрытие P1.4)
  const tree = doc?.getElementById("channel-tree");
  const treeHeader = tree?.querySelector(".tree-header") ?? null;
  const treeScroll = tree?.querySelector(".tree-scroll") ?? null;
  /** @type {any[]} */
  const savedGuildHeaderNodes = treeHeader ? [...treeHeader.children] : [];
  /** @type {any[]} */
  const savedGuildScrollNodes = treeScroll ? [...treeScroll.children] : [];

  /* ========================================================================
   * 2. Монтирование панели #friends-view в .content (DM-FRIENDS-SPEC §2.3)
   * ======================================================================== */
  const content = doc?.querySelector(".content") ?? doc?.body;
  const screenChat = doc?.getElementById("screen-chat");
  const chatHeader = doc?.getElementById("chat-header");

  const friendsView = doc?.getElementById("friends-view") ?? make("section", "friends-view");
  friendsView.id = "friends-view";
  friendsView.className = "friends-view";
  friendsView.setAttribute("role", "region");
  friendsView.setAttribute("aria-label", "Друзья");
  friendsView.setAttribute("data-active-tab", activeFriendsTab);
  friendsView.setAttribute("data-state", "ready");
  friendsView.setAttribute("hidden", "");
  if (content && !friendsView.parentNode) {
    content.append(friendsView);
  }
  friendsView.replaceChildren();

  const friendsHeader = make("header", "friends-header");
  const headerBrand = make("div", "friends-header-brand");
  const headerIcon = make("span", "friends-header-icon", "👥");
  headerIcon.setAttribute("aria-hidden", "true");
  const headerTitle = make("h1", "friends-header-title", "Друзья");
  headerBrand.append(headerIcon, headerTitle);

  const headerDivider = make("div", "friends-header-divider");
  headerDivider.setAttribute("role", "separator");
  headerDivider.setAttribute("aria-hidden", "true");

  const tablist = make("div", "friends-tablist");
  tablist.id = "friends-tablist";
  tablist.setAttribute("role", "tablist");
  tablist.setAttribute("aria-orientation", "horizontal");
  tablist.setAttribute("aria-label", "Фильтры списка друзей");

  const TAB_DEFS = [
    { id: "friends-tab-online", tab: "online", label: "В сети", controls: "friends-tabpanel", addClass: "" },
    { id: "friends-tab-all", tab: "all", label: "Все", controls: "friends-tabpanel", addClass: "" },
    { id: "friends-tab-pending", tab: "pending", label: "Ожидание", controls: "friends-tabpanel", addClass: "" },
    { id: "friends-tab-blocked", tab: "blocked", label: "Заблокированные", controls: "friends-tabpanel", addClass: "" },
    {
      id: "friends-tab-add",
      tab: "add_friend",
      label: "Добавить в друзья",
      controls: "friends-add-panel",
      addClass: " friends-tab--add",
    },
  ];

  /** @type {any[]} */
  const tabButtons = [];
  /** @type {any} */
  let pendingTabBadge = null;

  for (const def of TAB_DEFS) {
    const btn = make("button", `friends-tab${def.addClass}`);
    btn.id = def.id;
    btn.setAttribute("type", "button");
    btn.setAttribute("role", "tab");
    btn.setAttribute("data-tab", def.tab);
    const isSel = def.tab === activeFriendsTab;
    btn.setAttribute("aria-selected", isSel ? "true" : "false");
    btn.setAttribute("aria-controls", def.controls);
    btn.setAttribute("tabindex", isSel ? "0" : "-1");

    if (def.tab === "pending") {
      const labelSpan = make("span", "", def.label);
      pendingTabBadge = make("span", "mention-badge", "0");
      pendingTabBadge.id = "friends-pending-badge";
      pendingTabBadge.setAttribute("aria-hidden", "true");
      pendingTabBadge.setAttribute("hidden", "");
      btn.append(labelSpan, pendingTabBadge);
    } else {
      btn.textContent = def.label;
    }

    btn.addEventListener("click", () => {
      selectFriendsTab(def.tab, false);
    });

    btn.addEventListener("keydown", (/** @type {any} */ ev) => {
      const idx = tabButtons.indexOf(btn);
      if (idx < 0) return;
      if (ev.key === "ArrowRight") {
        ev.preventDefault();
        const nextTab = TAB_DEFS[(idx + 1) % TAB_DEFS.length];
        if (nextTab) selectFriendsTab(nextTab.tab, true);
      } else if (ev.key === "ArrowLeft") {
        ev.preventDefault();
        const prevTab = TAB_DEFS[(idx - 1 + TAB_DEFS.length) % TAB_DEFS.length];
        if (prevTab) selectFriendsTab(prevTab.tab, true);
      } else if (ev.key === "Home") {
        ev.preventDefault();
        const firstTab = TAB_DEFS[0];
        if (firstTab) selectFriendsTab(firstTab.tab, true);
      } else if (ev.key === "End") {
        ev.preventDefault();
        const lastTab = TAB_DEFS[TAB_DEFS.length - 1];
        if (lastTab) selectFriendsTab(lastTab.tab, true);
      } else if (ev.key === "Enter" || ev.key === " ") {
        ev.preventDefault();
        selectFriendsTab(def.tab, true);
      }
    });

    tablist.append(btn);
    tabButtons.push(btn);
  }

  const headerActions = make("div", "friends-header-actions");
  const headerGroupDmBtn = make("button", "icon-btn", "💬");
  headerGroupDmBtn.setAttribute("type", "button");
  headerGroupDmBtn.setAttribute("data-action", "open-create-group-dm");
  headerGroupDmBtn.setAttribute("title", "Новый групповой чат");
  headerGroupDmBtn.setAttribute("aria-label", "Новый групповой чат");
  headerGroupDmBtn.setAttribute("aria-haspopup", "dialog");
  headerGroupDmBtn.addEventListener("click", () => openGroupDmModal(headerGroupDmBtn));
  headerActions.append(headerGroupDmBtn);

  friendsHeader.append(headerBrand, headerDivider, tablist, headerActions);

  // Рабочая область #friends-view
  const friendsBody = make("div", "friends-body");

  // 2A. Панель списков #friends-tabpanel
  const friendsTabpanel = make("div", "friends-tabpanel");
  friendsTabpanel.id = "friends-tabpanel";
  friendsTabpanel.setAttribute("role", "tabpanel");
  friendsTabpanel.setAttribute("aria-labelledby", "friends-tab-online");

  const friendsToolbar = make("div", "friends-toolbar");
  const searchField = make("search", "search-field friends-search-field");
  searchField.setAttribute("role", "search");
  const friendsSearchInput = make("input", "search-input");
  friendsSearchInput.id = "friends-search-input";
  friendsSearchInput.setAttribute("type", "search");
  friendsSearchInput.setAttribute("placeholder", "Поиск");
  friendsSearchInput.setAttribute("aria-label", "Поиск по списку друзей");
  friendsSearchInput.setAttribute("autocomplete", "off");
  friendsSearchInput.addEventListener("input", () => {
    friendsSearchQuery = String(friendsSearchInput.value ?? "");
    renderFriendsPanel();
  });
  searchField.append(friendsSearchInput);

  const pendingControls = make("div", "friends-pending-controls");
  pendingControls.id = "friends-pending-controls";
  pendingControls.setAttribute("hidden", "");

  const toggleSpamBtn = make("button", "friends-subfilter-btn");
  toggleSpamBtn.id = "friends-toggle-spam";
  toggleSpamBtn.setAttribute("type", "button");
  toggleSpamBtn.setAttribute("data-action", "toggle-pending-spam");
  toggleSpamBtn.setAttribute("aria-pressed", "false");
  const spamCountSpan = make("span", "", "0");
  spamCountSpan.id = "friends-spam-count";
  toggleSpamBtn.append(
    make("span", "", "Спам и игнор ("),
    spamCountSpan,
    make("span", "", ")"),
  );
  toggleSpamBtn.addEventListener("click", () => {
    showPendingSpamAndIgnored = !showPendingSpamAndIgnored;
    toggleSpamBtn.setAttribute("aria-pressed", showPendingSpamAndIgnored ? "true" : "false");
    renderFriendsPanel();
  });

  const clearPendingBtn = make(
    "button",
    "friends-subfilter-btn friends-subfilter-btn--danger",
    "Очистить входящие",
  );
  clearPendingBtn.id = "friends-clear-pending-btn";
  clearPendingBtn.setAttribute("type", "button");
  clearPendingBtn.setAttribute("data-action", "clear-pending-requests");
  clearPendingBtn.addEventListener("click", () => {
    void clearPendingRequests(showPendingSpamAndIgnored);
  });

  pendingControls.append(toggleSpamBtn, clearPendingBtn);
  friendsToolbar.append(searchField, pendingControls);

  const sectionHeading = make("h2", "friends-section-heading");
  sectionHeading.id = "friends-section-heading";
  sectionHeading.setAttribute("aria-live", "polite");
  const sectionHeadingLabel = make("span", "", "В СЕТИ — ");
  const sectionCountSpan = make("span", "", "0");
  sectionCountSpan.id = "friends-section-count";
  sectionHeading.append(sectionHeadingLabel, sectionCountSpan);

  const friendsList = make("ul", "friends-list");
  friendsList.id = "friends-list";
  friendsList.setAttribute("role", "list");
  friendsList.setAttribute("aria-labelledby", "friends-section-heading");

  const emptyState = make("div", "friends-empty-state");
  emptyState.id = "friends-empty-state";
  emptyState.setAttribute("role", "status");
  emptyState.setAttribute("hidden", "");
  const emptyText = make("p", "friends-empty-text", EMPTY_TAB_TEXT.online);
  emptyText.id = "friends-empty-text";
  emptyState.append(emptyText);

  friendsTabpanel.append(friendsToolbar, sectionHeading, friendsList, emptyState);

  // 2B. Панель «Добавить в друзья» #friends-add-panel (DM-FRIENDS-SPEC §2.3)
  const friendsAddPanel = make("div", "friends-add-panel");
  friendsAddPanel.id = "friends-add-panel";
  friendsAddPanel.setAttribute("role", "tabpanel");
  friendsAddPanel.setAttribute("aria-labelledby", "friends-tab-add");
  friendsAddPanel.setAttribute("hidden", "");

  const addTitle = make("h2", "friends-add-title", "ДОБАВИТЬ В ДРУЗЬЯ");
  const addDesc = make(
    "p",
    "friends-add-desc",
    "Вы можете добавить друзей по их имени пользователя Discord (например, username или username#1234).",
  );
  addDesc.id = "friends-add-desc";

  const addForm = make("form", "add-friend-form");
  addForm.id = "add-friend-form";
  addForm.setAttribute("novalidate", "");

  const addInputWrap = make("div", "add-friend-input-wrap");
  addInputWrap.setAttribute("data-state", "idle");

  const addInput = make("input", "add-friend-input");
  addInput.id = "add-friend-input";
  addInput.setAttribute("type", "text");
  addInput.setAttribute("placeholder", "Вы можете добавить друзей по их имени пользователя Discord.");
  addInput.setAttribute("aria-label", "Имя пользователя Discord");
  addInput.setAttribute("aria-describedby", "friends-add-desc add-friend-feedback");
  addInput.setAttribute("autocomplete", "off");
  addInput.setAttribute("required", "");

  const addSubmitBtn = make("button", "add-friend-submit", "Отправить запрос дружбы");
  addSubmitBtn.id = "add-friend-submit";
  addSubmitBtn.setAttribute("type", "submit");
  addSubmitBtn.disabled = true;
  addSubmitBtn.setAttribute("disabled", "");
  addSubmitBtn.setAttribute("aria-disabled", "true");

  addInput.addEventListener("input", () => {
    const hasText = String(addInput.value ?? "").trim().length > 0;
    addSubmitBtn.disabled = !hasText;
    if (hasText) {
      addSubmitBtn.removeAttribute("disabled");
      addSubmitBtn.setAttribute("aria-disabled", "false");
    } else {
      addSubmitBtn.setAttribute("disabled", "");
      addSubmitBtn.setAttribute("aria-disabled", "true");
    }
    addInputWrap.setAttribute("data-state", "idle");
  });

  addInputWrap.append(addInput, addSubmitBtn);

  const addFeedback = make("p", "add-friend-feedback");
  addFeedback.id = "add-friend-feedback";
  addFeedback.setAttribute("role", "status");
  addFeedback.setAttribute("aria-live", "polite");
  addFeedback.setAttribute("hidden", "");

  addForm.addEventListener("submit", (/** @type {any} */ ev) => {
    ev.preventDefault();
    void submitAddFriendForm();
  });

  addForm.append(addInputWrap, addFeedback);
  friendsAddPanel.append(addTitle, addDesc, addForm);

  friendsBody.append(friendsTabpanel, friendsAddPanel);
  friendsView.append(friendsHeader, friendsBody);

  /* ========================================================================
   * 3. Модальное окно создания групповой DM #group-dm-dialog (SPEC §2.4)
   * ======================================================================== */
  const groupDmDialog = doc?.getElementById("group-dm-dialog") ?? make("div", "dialog-backdrop");
  groupDmDialog.id = "group-dm-dialog";
  groupDmDialog.className = "dialog-backdrop";
  groupDmDialog.setAttribute("role", "presentation");
  groupDmDialog.setAttribute("hidden", "");
  if (doc?.body && !groupDmDialog.parentNode) doc.body.append(groupDmDialog);
  groupDmDialog.replaceChildren();

  const groupDmModal = make("div", "group-dm-modal");
  groupDmModal.setAttribute("role", "dialog");
  groupDmModal.setAttribute("aria-modal", "true");
  groupDmModal.setAttribute("aria-labelledby", "group-dm-title");
  groupDmModal.setAttribute("aria-describedby", "group-dm-subtitle");
  groupDmModal.setAttribute("data-state", "idle");

  const gdmHeader = make("header", "group-dm-header");
  const gdmTitleWrap = make("div", "group-dm-title-wrap");
  const gdmTitle = make("h2", "group-dm-title", "Выберите друзей");
  gdmTitle.id = "group-dm-title";
  const gdmSubtitle = make("p", "group-dm-subtitle");
  gdmSubtitle.id = "group-dm-subtitle";
  gdmSubtitle.setAttribute("aria-live", "polite");
  const gdmRemainingSpan = make("span", "", String(MAX_GROUP_DM_RECIPIENTS));
  gdmRemainingSpan.id = "group-dm-remaining";
  gdmSubtitle.append(
    make("span", "", "Вы можете добавить ещё "),
    gdmRemainingSpan,
    make("span", "", " друзей."),
  );
  gdmTitleWrap.append(gdmTitle, gdmSubtitle);

  const gdmCloseBtn = make("button", "icon-btn", "✕");
  gdmCloseBtn.setAttribute("type", "button");
  gdmCloseBtn.setAttribute("data-action", "close-group-dm-modal");
  gdmCloseBtn.setAttribute("aria-label", "Закрыть окно создания чата");
  gdmCloseBtn.addEventListener("click", () => closeGroupDmModal(true));
  gdmHeader.append(gdmTitleWrap, gdmCloseBtn);

  const gdmTagbox = make("div", "group-dm-tagbox");
  gdmTagbox.setAttribute("role", "group");
  gdmTagbox.setAttribute("aria-label", "Выбранные получатели и фильтр");

  const gdmChipsList = make("ul", "group-dm-chips");
  gdmChipsList.id = "group-dm-chips";
  gdmChipsList.setAttribute("role", "list");
  gdmChipsList.setAttribute("aria-label", "Выбранные друзья");

  const gdmSearchInput = make("input", "group-dm-search");
  gdmSearchInput.id = "group-dm-search";
  gdmSearchInput.setAttribute("type", "text");
  gdmSearchInput.setAttribute("role", "combobox");
  gdmSearchInput.setAttribute("aria-expanded", "true");
  gdmSearchInput.setAttribute("aria-autocomplete", "list");
  gdmSearchInput.setAttribute("aria-controls", "group-dm-friend-list");
  gdmSearchInput.setAttribute("aria-label", "Введите имя пользователя друга");
  gdmSearchInput.setAttribute("placeholder", "Введите имя пользователя друга");
  gdmSearchInput.setAttribute("autocomplete", "off");
  gdmSearchInput.addEventListener("input", () => {
    groupDmSearchQuery = String(gdmSearchInput.value ?? "");
    groupDmActiveOptionIndex = 0;
    renderGroupDmModal();
  });
  gdmSearchInput.addEventListener("keydown", (/** @type {any} */ ev) => {
    const opts = Array.from(gdmFriendList.querySelectorAll('[role="option"]'));
    if (ev.key === "ArrowDown") {
      ev.preventDefault();
      if (opts.length > 0) {
        groupDmActiveOptionIndex = (groupDmActiveOptionIndex + 1) % opts.length;
        const activeOpt = opts[groupDmActiveOptionIndex];
        if (activeOpt?.id) gdmSearchInput.setAttribute("aria-activedescendant", activeOpt.id);
      }
    } else if (ev.key === "ArrowUp") {
      ev.preventDefault();
      if (opts.length > 0) {
        groupDmActiveOptionIndex = (groupDmActiveOptionIndex - 1 + opts.length) % opts.length;
        const activeOpt = opts[groupDmActiveOptionIndex];
        if (activeOpt?.id) gdmSearchInput.setAttribute("aria-activedescendant", activeOpt.id);
      }
    } else if (ev.key === "Enter") {
      ev.preventDefault();
      const activeOpt = opts[groupDmActiveOptionIndex];
      const uid = activeOpt?.getAttribute("data-user-id");
      if (uid) toggleGroupDmRecipient(uid);
    } else if (ev.key === "Backspace" && !gdmSearchInput.value && selectedGroupDmUserIds.size > 0) {
      const allSelected = Array.from(selectedGroupDmUserIds);
      const lastId = allSelected[allSelected.length - 1];
      if (lastId) toggleGroupDmRecipient(lastId);
    }
  });
  gdmTagbox.append(gdmChipsList, gdmSearchInput);

  const gdmFriendList = make("div", "group-dm-friend-list");
  gdmFriendList.id = "group-dm-friend-list";
  gdmFriendList.setAttribute("role", "listbox");
  gdmFriendList.setAttribute("aria-multiselectable", "true");
  gdmFriendList.setAttribute("aria-labelledby", "group-dm-title");

  const gdmError = make("p", "group-dm-error");
  gdmError.id = "group-dm-error";
  gdmError.setAttribute("role", "alert");
  gdmError.setAttribute("hidden", "");

  const gdmFooter = make("footer", "group-dm-footer");
  const gdmCancelBtn = make("button", "group-dm-btn group-dm-btn--secondary", "Отмена");
  gdmCancelBtn.setAttribute("type", "button");
  gdmCancelBtn.setAttribute("data-action", "close-group-dm-modal");
  gdmCancelBtn.addEventListener("click", () => closeGroupDmModal(true));

  const gdmSubmitBtn = make("button", "group-dm-btn group-dm-btn--primary", "Создать групповой чат");
  gdmSubmitBtn.id = "group-dm-submit";
  gdmSubmitBtn.setAttribute("type", "button");
  gdmSubmitBtn.setAttribute("data-action", "submit-group-dm");
  gdmSubmitBtn.addEventListener("click", () => {
    void submitGroupDmModal();
  });
  gdmFooter.append(gdmCancelBtn, gdmSubmitBtn);

  groupDmModal.append(gdmHeader, gdmTagbox, gdmFriendList, gdmError, gdmFooter);
  groupDmDialog.append(groupDmModal);

  groupDmDialog.addEventListener("keydown", (/** @type {any} */ ev) => {
    if (trapTabKey(groupDmModal, ev)) return;
    if (ev.key === "Escape") {
      ev.preventDefault();
      ev.stopPropagation();
      closeGroupDmModal(true);
    }
  });

  /* ========================================================================
   * 4. Диалог подтверждения незнакомца #stranger-confirm-dialog (код 80013, SPEC §2.5)
   * ======================================================================== */
  const strangerDialog = doc?.getElementById("stranger-confirm-dialog") ?? make("div", "dialog-backdrop");
  strangerDialog.id = "stranger-confirm-dialog";
  strangerDialog.className = "dialog-backdrop";
  strangerDialog.setAttribute("role", "presentation");
  strangerDialog.setAttribute("hidden", "");
  if (doc?.body && !strangerDialog.parentNode) doc.body.append(strangerDialog);
  strangerDialog.replaceChildren();

  const strangerModal = make("div", "group-dm-modal stranger-confirm-modal");
  strangerModal.setAttribute("role", "alertdialog");
  strangerModal.setAttribute("aria-modal", "true");
  strangerModal.setAttribute("aria-labelledby", "stranger-confirm-title");
  strangerModal.setAttribute("aria-describedby", "stranger-confirm-desc");

  const strangerTitle = make("h2", "group-dm-title", "Принять запрос в друзья?");
  strangerTitle.id = "stranger-confirm-title";
  const strangerDesc = make(
    "p",
    "group-dm-subtitle",
    "У вас нет общих серверов или друзей с этим пользователем. Вы уверены, что хотите принять запрос?",
  );
  strangerDesc.id = "stranger-confirm-desc";

  const strangerFooter = make("footer", "group-dm-footer");
  const strangerCancelBtn = make("button", "group-dm-btn group-dm-btn--secondary", "Отмена");
  strangerCancelBtn.setAttribute("type", "button");
  strangerCancelBtn.setAttribute("data-action", "cancel-stranger-confirm");
  strangerCancelBtn.addEventListener("click", () => closeStrangerDialog(true));

  const strangerAcceptBtn = make("button", "group-dm-btn group-dm-btn--primary", "Принять запрос");
  strangerAcceptBtn.setAttribute("type", "button");
  strangerAcceptBtn.setAttribute("data-action", "accept-stranger-confirm");
  strangerAcceptBtn.addEventListener("click", () => {
    void confirmStrangerRequest();
  });
  strangerFooter.append(strangerCancelBtn, strangerAcceptBtn);
  strangerModal.append(strangerTitle, strangerDesc, strangerFooter);
  strangerDialog.append(strangerModal);

  strangerDialog.addEventListener("keydown", (/** @type {any} */ ev) => {
    if (trapTabKey(strangerModal, ev)) return;
    if (ev.key === "Escape") {
      ev.preventDefault();
      ev.stopPropagation();
      closeStrangerDialog(true);
    }
  });

  /* ========================================================================
   * 5. Контекстное меню друга #friend-context-menu (SPEC §2.5)
   * ======================================================================== */
  const friendMenu = doc?.getElementById("friend-context-menu") ?? make("div", "menu friend-context-menu");
  friendMenu.id = "friend-context-menu";
  friendMenu.className = "menu friend-context-menu";
  friendMenu.setAttribute("role", "menu");
  friendMenu.setAttribute("aria-label", "Действия с пользователем");
  friendMenu.setAttribute("hidden", "");
  if (doc?.body && !friendMenu.parentNode) doc.body.append(friendMenu);
  friendMenu.replaceChildren();

  const MENU_ACTIONS = [
    { action: "friend-edit-nick", label: "Изменить никнейм друга", danger: false },
    { action: "friend-edit-note", label: "Изменить заметку", danger: false },
    { action: "friend-toggle-ignore", label: "Игнорировать", danger: false },
    { action: "friend-remove", label: "Удалить из друзей", danger: true },
    { action: "friend-block", label: "Заблокировать", danger: true },
  ];

  for (let i = 0; i < MENU_ACTIONS.length; i += 1) {
    const item = MENU_ACTIONS[i];
    if (!item) continue;
    if (i === 3) {
      const sep = make("div", "rail-sep");
      sep.setAttribute("role", "separator");
      sep.setAttribute("aria-hidden", "true");
      friendMenu.append(sep);
    }
    const btn = make("button", item.danger ? "menu-item menu-item--danger" : "menu-item", item.label);
    btn.setAttribute("type", "button");
    btn.setAttribute("role", "menuitem");
    btn.setAttribute("data-action", item.action);
    btn.setAttribute("tabindex", i === 0 ? "0" : "-1");
    btn.addEventListener("click", () => {
      if (!activeContextFriendId) return;
      const uid = activeContextFriendId;
      closeFriendContextMenu(true);
      if (item.action === "friend-remove") {
        void removeRelationship(uid);
      } else if (item.action === "friend-block") {
        void blockUser(uid);
      } else if (item.action === "friend-toggle-ignore") {
        void ignoreUser(uid);
      }
    });
    friendMenu.append(btn);
  }

  friendMenu.addEventListener("keydown", (/** @type {any} */ ev) => {
    const items = Array.from(friendMenu.querySelectorAll('[role="menuitem"]'));
    const curIdx = items.indexOf(doc?.activeElement);
    if (ev.key === "Escape") {
      ev.preventDefault();
      ev.stopPropagation();
      closeFriendContextMenu(true);
    } else if (ev.key === "ArrowDown" && items.length > 0) {
      ev.preventDefault();
      const nextIdx = (curIdx + 1) % items.length;
      items.forEach((el, idx) => el.setAttribute("tabindex", idx === nextIdx ? "0" : "-1"));
      items[nextIdx]?.focus();
    } else if (ev.key === "ArrowUp" && items.length > 0) {
      ev.preventDefault();
      const prevIdx = (curIdx - 1 + items.length) % items.length;
      items.forEach((el, idx) => el.setAttribute("tabindex", idx === prevIdx ? "0" : "-1"));
      items[prevIdx]?.focus();
    } else if (ev.key === "Tab") {
      // §5.2 п.4: Tab закрывает меню, фокус уходит дальше по документу.
      closeFriendContextMenu(true);
    }
  });

  /* ========================================================================
   * 6. Отрисовка бейджей, дерева ЛС и панели друзей
   * ======================================================================== */

  function updateBadges() {
    const stats = classifyRelationships(relationships, usersById, presencesById);
    const dmMentions = privateChannels.reduce((sum, ch) => sum + (ch.mention_count || 0), 0);
    const totalHomeBadge = stats.pendingCount + dmMentions;

    if (homeRailBtn && homeRailBadge) {
      homeRailBtn.setAttribute("data-pending-count", String(stats.pendingCount));
      homeRailBtn.setAttribute("data-unread", totalHomeBadge > 0 ? "true" : "false");
      homeRailBadge.textContent = String(totalHomeBadge);
      if (totalHomeBadge > 0) {
        homeRailBadge.removeAttribute("hidden");
        homeRailBtn.setAttribute(
          "aria-label",
          `Личные сообщения и Друзья, ${totalHomeBadge} уведомлений`,
        );
      } else {
        homeRailBadge.setAttribute("hidden", "");
        homeRailBtn.setAttribute("aria-label", "Личные сообщения и Друзья");
      }
    }

    if (pendingTabBadge) {
      pendingTabBadge.textContent = String(stats.pendingCount);
      if (stats.pendingCount > 0) {
        pendingTabBadge.removeAttribute("hidden");
      } else {
        pendingTabBadge.setAttribute("hidden", "");
      }
    }

    const navFriendsBadge = doc?.getElementById("dm-nav-friends-badge");
    if (navFriendsBadge) {
      navFriendsBadge.textContent = String(stats.pendingCount);
      if (stats.pendingCount > 0) navFriendsBadge.removeAttribute("hidden");
      else navFriendsBadge.setAttribute("hidden", "");
    }
  }

  function renderDmTree() {
    if (!tree || currentGuildId !== HOME_GUILD_ID) return;

    tree.setAttribute("data-nav-mode", "home");
    tree.setAttribute("aria-label", "Личные сообщения");
    tree.setAttribute("data-state", "ready");
    tree.setAttribute("aria-busy", "false");

    if (treeHeader) {
      treeHeader.className = "tree-header dm-tree-header";
      treeHeader.replaceChildren();
      const quickSearchBtn = make("button", "dm-search-trigger");
      quickSearchBtn.id = "dm-quick-search-btn";
      quickSearchBtn.setAttribute("type", "button");
      quickSearchBtn.setAttribute("data-action", "open-quick-switcher");
      quickSearchBtn.setAttribute("aria-label", "Найти или начать беседу");
      quickSearchBtn.setAttribute("aria-haspopup", "dialog");
      const searchIcon = make("span", "dm-search-trigger-icon", "🔍");
      searchIcon.setAttribute("aria-hidden", "true");
      const searchText = make("span", "dm-search-trigger-text", "Найти или начать беседу");
      quickSearchBtn.append(searchIcon, searchText);
      treeHeader.append(quickSearchBtn);
    }

    if (!treeScroll) return;
    treeScroll.className = "tree-scroll dm-tree-scroll";
    treeScroll.setAttribute("aria-label", "Навигация личных сообщений");
    treeScroll.replaceChildren();

    const stats = classifyRelationships(relationships, usersById, presencesById);
    const requestChannels = privateChannels.filter((c) => c.is_message_request);
    const regularChannels = sortPrivateChannels(
      privateChannels.filter((c) => !c.is_message_request),
    );
    const displayedChannels = homeSubview === "requests" ? requestChannels : regularChannels;

    // 1. Верхние навигационные пункты Home (#dm-nav-friends, #dm-nav-requests)
    const navList = make("ul", "dm-nav-list");
    navList.setAttribute("role", "list");

    const friendsLi = make("li");
    const isFriendsSelected = homeSubview === "friends";
    const navFriendsBtn = make(
      "button",
      `channel-item dm-nav-item${isFriendsSelected ? " is-selected" : ""}`,
    );
    navFriendsBtn.id = "dm-nav-friends";
    navFriendsBtn.setAttribute("type", "button");
    navFriendsBtn.setAttribute("data-action", "open-friends-view");
    if (isFriendsSelected) navFriendsBtn.setAttribute("aria-current", "page");
    navFriendsBtn.setAttribute("tabindex", isFriendsSelected ? "0" : "-1");

    const friendsIcon = make("span", "channel-icon", "👥");
    friendsIcon.setAttribute("aria-hidden", "true");
    const friendsName = make("span", "channel-name", "Друзья");
    const friendsBadge = make("span", "mention-badge", String(stats.pendingCount));
    friendsBadge.id = "dm-nav-friends-badge";
    friendsBadge.setAttribute("aria-hidden", "true");
    if (stats.pendingCount <= 0) friendsBadge.setAttribute("hidden", "");
    navFriendsBtn.append(friendsIcon, friendsName, friendsBadge);
    navFriendsBtn.addEventListener("click", () => {
      openFriendsSubview();
    });
    friendsLi.append(navFriendsBtn);
    navList.append(friendsLi);

    const requestsLi = make("li");
    requestsLi.id = "dm-nav-requests-item";
    if (requestChannels.length === 0) requestsLi.setAttribute("hidden", "");
    const isRequestsSelected = homeSubview === "requests";
    const navRequestsBtn = make(
      "button",
      `channel-item dm-nav-item${isRequestsSelected ? " is-selected" : ""}`,
    );
    navRequestsBtn.id = "dm-nav-requests";
    navRequestsBtn.setAttribute("type", "button");
    navRequestsBtn.setAttribute("data-action", "open-message-requests");
    if (isRequestsSelected) navRequestsBtn.setAttribute("aria-current", "page");
    navRequestsBtn.setAttribute("tabindex", isRequestsSelected ? "0" : "-1");
    const reqIcon = make("span", "channel-icon", "📨");
    reqIcon.setAttribute("aria-hidden", "true");
    const reqName = make("span", "channel-name", "Запросы переписки");
    const reqBadge = make("span", "mention-badge", String(requestChannels.length));
    reqBadge.id = "dm-nav-requests-badge";
    reqBadge.setAttribute("aria-hidden", "true");
    if (requestChannels.length <= 0) reqBadge.setAttribute("hidden", "");
    navRequestsBtn.append(reqIcon, reqName, reqBadge);
    navRequestsBtn.addEventListener("click", () => {
      homeSubview = "requests";
      renderDmTree();
    });
    requestsLi.append(navRequestsBtn);
    navList.append(requestsLi);

    // 2. Секция «ЛИЧНЫЕ СООБЩЕНИЯ»
    const dmGroup = make("section", "channel-group dm-channel-group");
    dmGroup.setAttribute("aria-labelledby", "dm-group-heading");

    const dmGroupHeader = make("div", "dm-group-header");
    const dmGroupHeading = make("h3", "channel-group-title dm-group-title", "ЛИЧНЫЕ СООБЩЕНИЯ");
    dmGroupHeading.id = "dm-group-heading";

    const createGroupBtn = make(
      "button",
      "icon-btn dm-create-btn",
      typeof globalThis.Image === "function" ? "+" : "＋",
    );
    createGroupBtn.id = "create-group-dm-btn";
    createGroupBtn.setAttribute("type", "button");
    createGroupBtn.setAttribute("data-action", "open-create-group-dm");
    createGroupBtn.setAttribute("title", "Создать групповой чат");
    createGroupBtn.setAttribute("aria-label", "Создать личное сообщение или групповой чат");
    createGroupBtn.setAttribute("aria-haspopup", "dialog");
    createGroupBtn.setAttribute("aria-expanded", "false");
    createGroupBtn.addEventListener("click", () => openGroupDmModal(createGroupBtn));
    dmGroupHeader.append(dmGroupHeading, createGroupBtn);

    const dmList = make("ul", "channel-list dm-channel-list");
    dmList.id = "dm-channel-list";
    dmList.setAttribute("role", "list");

    /** @type {any[]} */
    const rovingItems = [navFriendsBtn];
    if (requestChannels.length > 0) rovingItems.push(navRequestsBtn);

    for (const ch of displayedChannels) {
      const row = make("li", "dm-channel-row");
      row.setAttribute("data-channel-id", ch.id);
      row.setAttribute("data-channel-type", String(ch.type));
      if (ch.pinned) row.setAttribute("data-pinned", "true");
      if (ch.owner_id) row.setAttribute("data-owner-id", ch.owner_id);

      const isCurrentChannel = homeSubview === "dm" && activeDmChannelId === ch.id;
      const itemClasses = [
        "channel-item",
        "dm-channel-item",
        ch.unread ? "is-unread" : "",
        ch.mention_count > 0 ? "has-mention" : "",
      ]
        .filter(Boolean)
        .join(" ");

      const itemBtn = make("button", itemClasses);
      itemBtn.setAttribute("type", "button");
      itemBtn.setAttribute("data-channel-id", ch.id);
      if (ch.type === PRIVATE_CHANNEL_TYPES.DM && ch.recipient_ids[0]) {
        itemBtn.setAttribute("data-recipient-id", ch.recipient_ids[0]);
      }
      itemBtn.setAttribute("data-openable", "true");
      itemBtn.setAttribute("data-unread", ch.unread ? "true" : "false");
      if (ch.mention_count > 0) {
        itemBtn.setAttribute("data-mention-count", String(ch.mention_count));
      }
      if (isCurrentChannel) {
        itemBtn.setAttribute("aria-current", "page");
      }
      itemBtn.setAttribute("tabindex", isCurrentChannel ? "0" : "-1");

      const ariaParts = [
        ch.type === PRIVATE_CHANNEL_TYPES.GROUP_DM
          ? `Групповой чат: ${ch.name}, ${ch.subtext}`
          : `Личное сообщение: ${ch.name}`,
      ];
      if (ch.pinned) ariaParts.push("закреплён");
      if (ch.unread) ariaParts.push("непрочитано");
      if (ch.mention_count > 0) ariaParts.push(`${ch.mention_count} упоминания`);
      itemBtn.setAttribute("aria-label", ariaParts.join(", "));

      const avatarSpan = make(
        "span",
        ch.type === PRIVATE_CHANNEL_TYPES.GROUP_DM
          ? "avatar avatar--sm dm-avatar dm-avatar--group"
          : "avatar avatar--sm dm-avatar",
        ch.avatar.initials,
      );
      avatarSpan.style.setProperty("--avatar-hue", String(ch.avatar.hue));
      avatarSpan.setAttribute("aria-hidden", "true");
      if (
        ch.avatar?.url &&
        typeof globalThis.Image === "function" &&
        !(/** @type {any} */ (globalThis).__SPIKE_AUTH_DISABLED__)
      ) {
        const avatarUrl = ch.avatar.url;
        const img = new globalThis.Image();
        img.onload = () => {
          avatarSpan.style.backgroundImage = `url("${avatarUrl}")`;
          avatarSpan.style.backgroundSize = "cover";
          avatarSpan.style.backgroundPosition = "center";
          avatarSpan.style.color = "transparent";
        };
        img.src = avatarUrl;
      }

      if (ch.type === PRIVATE_CHANNEL_TYPES.DM && ch.recipient_ids[0]) {
        const recStatus = resolvePresenceStatus(presencesById[ch.recipient_ids[0]]);
        const dot = make("span", "status-dot");
        dot.setAttribute("data-status", recStatus);
        avatarSpan.append(dot);
      }

      const metaSpan = make("span", "dm-item-meta");
      const nameSpan = make("span", "channel-name dm-item-name", ch.name);
      metaSpan.append(nameSpan);
      if (ch.subtext) {
        const subtextSpan = make("span", "dm-item-subtext", ch.subtext);
        metaSpan.append(subtextSpan);
      }

      itemBtn.append(avatarSpan, metaSpan);

      if (ch.pinned) {
        const pinSpan = make("span", "dm-pin-indicator", "📌");
        pinSpan.setAttribute("title", "Закреплённый чат");
        pinSpan.setAttribute("aria-hidden", "true");
        itemBtn.append(pinSpan);
      }
      if (ch.mention_count > 0) {
        const badgeSpan = make("span", "mention-badge", String(ch.mention_count));
        badgeSpan.setAttribute("aria-hidden", "true");
        itemBtn.append(badgeSpan);
      }

      const closeBtn = make("button", "icon-btn dm-close-btn", "✕");
      closeBtn.setAttribute("type", "button");
      closeBtn.setAttribute(
        "data-action",
        ch.type === PRIVATE_CHANNEL_TYPES.GROUP_DM ? "leave-group-dm" : "close-dm",
      );
      closeBtn.setAttribute("data-channel-id", ch.id);
      closeBtn.setAttribute(
        "aria-label",
        ch.type === PRIVATE_CHANNEL_TYPES.GROUP_DM
          ? `Покинуть группу «${ch.name}»`
          : `Закрыть переписку с ${ch.name}`,
      );
      closeBtn.setAttribute("tabindex", "-1");

      itemBtn.addEventListener("click", () => {
        selectDmChannel(ch.id);
      });

      itemBtn.addEventListener("keydown", (/** @type {any} */ ev) => {
        if (ev.key === "ArrowRight" || ev.key === "Delete") {
          ev.preventDefault();
          itemBtn.setAttribute("tabindex", "-1");
          closeBtn.setAttribute("tabindex", "0");
          closeBtn.focus();
        }
      });

      closeBtn.addEventListener("keydown", (/** @type {any} */ ev) => {
        if (ev.key === "ArrowLeft") {
          ev.preventDefault();
          closeBtn.setAttribute("tabindex", "-1");
          itemBtn.setAttribute("tabindex", "0");
          itemBtn.focus();
        } else if (ev.key === "Enter" || ev.key === " ") {
          ev.preventDefault();
          void closePrivateChannel(ch.id);
        }
      });

      closeBtn.addEventListener("click", () => {
        void closePrivateChannel(ch.id);
      });

      row.append(itemBtn, closeBtn);
      dmList.append(row);
      rovingItems.push(itemBtn);
    }

    // Roving tabindex ArrowDown / ArrowUp / Home / End по пунктам дерева Home (SPEC §5.2 п.2)
    for (const item of rovingItems) {
      item.addEventListener("keydown", (/** @type {any} */ ev) => {
        const curIdx = rovingItems.indexOf(item);
        if (curIdx < 0) return;
        let nextIdx = -1;
        if (ev.key === "ArrowDown") {
          nextIdx = Math.min(rovingItems.length - 1, curIdx + 1);
        } else if (ev.key === "ArrowUp") {
          nextIdx = Math.max(0, curIdx - 1);
        } else if (ev.key === "Home") {
          nextIdx = 0;
        } else if (ev.key === "End") {
          nextIdx = rovingItems.length - 1;
        }
        if (nextIdx >= 0 && nextIdx !== curIdx) {
          ev.preventDefault();
          rovingItems.forEach((btn, idx) =>
            btn.setAttribute("tabindex", idx === nextIdx ? "0" : "-1"),
          );
          rovingItems[nextIdx]?.focus();
        }
      });
    }

    dmGroup.append(dmGroupHeader, dmList);
    treeScroll.append(navList, dmGroup);
  }

  function renderFriendsPanel() {
    friendsView.setAttribute("data-active-tab", activeFriendsTab);

    for (const btn of tabButtons) {
      const tab = btn.getAttribute("data-tab");
      const isSel = tab === activeFriendsTab;
      btn.setAttribute("aria-selected", isSel ? "true" : "false");
      btn.setAttribute("tabindex", isSel ? "0" : "-1");
      if (isSel && tab !== "add_friend") {
        friendsTabpanel.setAttribute("aria-labelledby", btn.id);
      }
    }

    if (activeFriendsTab === "add_friend") {
      friendsTabpanel.setAttribute("hidden", "");
      friendsAddPanel.removeAttribute("hidden");
      return;
    }

    friendsAddPanel.setAttribute("hidden", "");
    friendsTabpanel.removeAttribute("hidden");

    const stats = classifyRelationships(relationships, usersById, presencesById);
    const spamAndIgnoredTotal = stats.spamCount + stats.pendingIgnoredCount;
    spamCountSpan.textContent = String(spamAndIgnoredTotal);
    if (spamAndIgnoredTotal === 0 && showPendingSpamAndIgnored && stats.pendingCount > 0) {
      showPendingSpamAndIgnored = false;
      toggleSpamBtn.setAttribute("aria-pressed", "false");
    }

    if (activeFriendsTab === "pending") {
      pendingControls.removeAttribute("hidden");
    } else {
      pendingControls.setAttribute("hidden", "");
    }

    const filtered = filterRelationshipsByTab(relationships, activeFriendsTab, {
      usersById,
      presencesById,
      searchQuery: friendsSearchQuery,
      showSpamAndIgnored: showPendingSpamAndIgnored,
    });

    const prefix =
      /** @type {Record<string, string>} */ (SECTION_HEADING_PREFIX)[activeFriendsTab] ?? "ДРУЗЬЯ";
    sectionHeadingLabel.textContent = `${prefix} — `;
    sectionCountSpan.textContent = String(filtered.length);

    friendsList.replaceChildren();

    if (filtered.length === 0) {
      emptyText.textContent =
        /** @type {Record<string, string>} */ (EMPTY_TAB_TEXT)[activeFriendsTab] ??
        EMPTY_TAB_TEXT.online;
      emptyState.removeAttribute("hidden");
      return;
    }

    emptyState.setAttribute("hidden", "");

    /** @type {any[]} */
    const rowMainButtons = [];

    for (const rel of filtered) {
      const li = make("li", "friend-row");
      li.setAttribute("data-user-id", rel.user_id);
      li.setAttribute("data-relationship-type", String(rel.type));
      li.setAttribute("data-status", rel.status);
      if (rel.is_stranger_request) {
        li.setAttribute("data-stranger", "true");
      }

      const rowMain = make("div", "friend-row-main");
      rowMain.setAttribute("data-user-id", rel.user_id);

      const avatarSpan = make("span", "avatar avatar--md", rel.avatar.initials);
      avatarSpan.style.setProperty("--avatar-hue", String(rel.avatar.hue));
      avatarSpan.setAttribute("aria-hidden", "true");
      if (
        rel.avatar?.url &&
        typeof globalThis.Image === "function" &&
        !(/** @type {any} */ (globalThis).__SPIKE_AUTH_DISABLED__)
      ) {
        const avatarUrl = rel.avatar.url;
        const img = new globalThis.Image();
        img.onload = () => {
          avatarSpan.style.backgroundImage = `url("${avatarUrl}")`;
          avatarSpan.style.backgroundSize = "cover";
          avatarSpan.style.backgroundPosition = "center";
          avatarSpan.style.color = "transparent";
        };
        img.src = avatarUrl;
      }
      if (rel.type === RELATIONSHIP_TYPES.FRIEND) {
        const dot = make("span", "status-dot");
        dot.setAttribute("data-status", rel.status);
        avatarSpan.append(dot);
      }

      const metaDiv = make("div", "friend-meta");
      const nameLine = make("div", "friend-name-line");
      const dispSpan = make("span", "friend-display-name", rel.displayName);
      const tagSpan = make("span", "friend-username-tag", rel.userTag);
      nameLine.append(dispSpan, tagSpan);

      let statusLabel = "В сети";
      if (rel.type === RELATIONSHIP_TYPES.PENDING_INCOMING) {
        statusLabel = "Входящий запрос в друзья";
      } else if (rel.type === RELATIONSHIP_TYPES.PENDING_OUTGOING) {
        statusLabel = "Исходящий запрос в друзья";
      } else if (rel.type === RELATIONSHIP_TYPES.BLOCKED) {
        statusLabel = "Заблокирован";
      } else if (rel.status === "idle") {
        statusLabel = "Не активен";
      } else if (rel.status === "dnd") {
        statusLabel = "Не беспокоить";
      } else if (rel.status === "streaming") {
        statusLabel = "Стримит";
      } else if (rel.status === "offline") {
        statusLabel = "Не в сети";
      }
      const statusSpan = make("span", "friend-status-text", statusLabel);
      metaDiv.append(nameLine, statusSpan);
      rowMain.append(avatarSpan, metaDiv);

      const actionsDiv = make("div", "friend-actions");
      actionsDiv.setAttribute("role", "group");
      actionsDiv.setAttribute("aria-label", `Действия с ${rel.displayName}`);

      if (rel.type === RELATIONSHIP_TYPES.FRIEND) {
        rowMain.setAttribute("data-action", "open-dm");
        rowMain.setAttribute("role", "button");
        rowMain.setAttribute("tabindex", "0");
        rowMain.setAttribute(
          "aria-label",
          `${rel.displayName}, ${statusLabel}. Нажмите, чтобы открыть переписку`,
        );
        rowMain.addEventListener("click", () => {
          void openPrivateChannel([rel.user_id]);
        });
        rowMain.addEventListener("keydown", (/** @type {any} */ ev) => {
          if (ev.key === "Enter" || ev.key === " ") {
            ev.preventDefault();
            void openPrivateChannel([rel.user_id]);
          } else if (ev.key === "ContextMenu" || (ev.shiftKey && ev.key === "F10")) {
            ev.preventDefault();
            openFriendContextMenu(rel.user_id, rowMain);
          }
        });
        rowMainButtons.push(rowMain);

        const dmBtn = make("button", "icon-btn friend-action-btn", "💬");
        dmBtn.setAttribute("type", "button");
        dmBtn.setAttribute("data-action", "open-dm");
        dmBtn.setAttribute("data-user-id", rel.user_id);
        dmBtn.setAttribute("title", "Написать сообщение");
        dmBtn.setAttribute("aria-label", `Написать сообщение ${rel.displayName}`);
        dmBtn.addEventListener("click", () => {
          void openPrivateChannel([rel.user_id]);
        });

        const moreBtn = make("button", "icon-btn friend-action-btn", "⋮");
        moreBtn.setAttribute("type", "button");
        moreBtn.setAttribute("data-action", "friend-more-menu");
        moreBtn.setAttribute("data-user-id", rel.user_id);
        moreBtn.setAttribute("title", "Ещё");
        moreBtn.setAttribute("aria-label", `Ещё действия для ${rel.displayName}`);
        moreBtn.setAttribute("aria-haspopup", "menu");
        moreBtn.setAttribute("aria-expanded", "false");
        moreBtn.addEventListener("click", () => {
          openFriendContextMenu(rel.user_id, moreBtn);
        });

        actionsDiv.append(dmBtn, moreBtn);
      } else if (rel.type === RELATIONSHIP_TYPES.PENDING_INCOMING) {
        const acceptBtn = make(
          "button",
          "icon-btn friend-action-btn friend-action-btn--accept",
          "✓",
        );
        acceptBtn.setAttribute("type", "button");
        acceptBtn.setAttribute("data-action", "accept-friend");
        acceptBtn.setAttribute("data-user-id", rel.user_id);
        acceptBtn.setAttribute("title", "Принять");
        acceptBtn.setAttribute("aria-label", `Принять запрос в друзья от ${rel.displayName}`);
        acceptBtn.addEventListener("click", () => {
          void acceptFriendRequest(rel.user_id, acceptBtn);
        });

        const declineBtn = make(
          "button",
          "icon-btn friend-action-btn friend-action-btn--danger",
          "✕",
        );
        declineBtn.setAttribute("type", "button");
        declineBtn.setAttribute("data-action", "decline-friend");
        declineBtn.setAttribute("data-user-id", rel.user_id);
        declineBtn.setAttribute("title", "Игнорировать / Отклонить");
        declineBtn.setAttribute("aria-label", `Отклонить запрос в друзья от ${rel.displayName}`);
        declineBtn.addEventListener("click", () => {
          void removeRelationship(rel.user_id);
        });

        actionsDiv.append(acceptBtn, declineBtn);
      } else if (rel.type === RELATIONSHIP_TYPES.PENDING_OUTGOING) {
        const cancelBtn = make(
          "button",
          "icon-btn friend-action-btn friend-action-btn--danger",
          "✕",
        );
        cancelBtn.setAttribute("type", "button");
        cancelBtn.setAttribute("data-action", "cancel-friend-request");
        cancelBtn.setAttribute("data-user-id", rel.user_id);
        cancelBtn.setAttribute("title", "Отменить");
        cancelBtn.setAttribute("aria-label", `Отменить запрос в друзья к ${rel.displayName}`);
        cancelBtn.addEventListener("click", () => {
          void removeRelationship(rel.user_id);
        });
        actionsDiv.append(cancelBtn);
      } else if (rel.type === RELATIONSHIP_TYPES.BLOCKED) {
        const unblockBtn = make(
          "button",
          "icon-btn friend-action-btn friend-action-btn--danger",
          "🔓",
        );
        unblockBtn.setAttribute("type", "button");
        unblockBtn.setAttribute("data-action", "unblock-user");
        unblockBtn.setAttribute("data-user-id", rel.user_id);
        unblockBtn.setAttribute("title", "Разблокировать");
        unblockBtn.setAttribute("aria-label", `Разблокировать ${rel.displayName}`);
        unblockBtn.addEventListener("click", () => {
          void removeRelationship(rel.user_id);
        });
        actionsDiv.append(unblockBtn);
      }

      li.append(rowMain, actionsDiv);
      friendsList.append(li);
    }

    for (const mainEl of rowMainButtons) {
      mainEl.addEventListener("keydown", (/** @type {any} */ ev) => {
        const idx = rowMainButtons.indexOf(mainEl);
        if (idx < 0) return;
        if (ev.key === "ArrowDown" && idx + 1 < rowMainButtons.length) {
          ev.preventDefault();
          rowMainButtons[idx + 1]?.focus();
        } else if (ev.key === "ArrowUp" && idx - 1 >= 0) {
          ev.preventDefault();
          rowMainButtons[idx - 1]?.focus();
        }
      });
    }
  }

  /* ========================================================================
   * 7. Навигация между Home (@me) и серверами (DM-FRIENDS-SPEC §2.1, P1.1, P1.3, P1.4)
   * ======================================================================== */

  function openFriendsSubview() {
    homeSubview = "friends";
    activeDmChannelId = "";
    if (content) content.setAttribute("data-home-subview", "friends");
    friendsView.removeAttribute("hidden");
    const allScreens = content
      ? Array.from(content.querySelectorAll(".screen"))
      : screenChat
        ? [screenChat]
        : [];
    for (const scr of allScreens) {
      scr.classList.remove("active");
      scr.setAttribute("aria-hidden", "true");
    }
    if (screenChat) {
      screenChat.classList.remove("active");
      screenChat.setAttribute("aria-hidden", "true");
    }
    if (chatHeader) {
      chatHeader.setAttribute("hidden", "");
    }
    renderDmTree();
    renderFriendsPanel();
  }

  /**
   * Синхронизирует `aria-current`, `tabindex` и `.is-selected` в уже отрисованном дереве ЛС
   * без пересоздания узлов (сохраняет фокус и ссылки на кнопки).
   */
  function syncDmTreeSelectionInPlace() {
    if (!treeScroll) return false;
    const dmList = treeScroll.querySelector("#dm-channel-list");
    if (!dmList) return false;

    const navFriendsBtn = treeScroll.querySelector("#dm-nav-friends");
    if (navFriendsBtn) {
      const isFriends = homeSubview === "friends";
      if (isFriends) {
        navFriendsBtn.classList.add("is-selected");
        navFriendsBtn.setAttribute("aria-current", "page");
        navFriendsBtn.setAttribute("tabindex", "0");
      } else {
        navFriendsBtn.classList.remove("is-selected");
        navFriendsBtn.removeAttribute("aria-current");
        navFriendsBtn.setAttribute("tabindex", "-1");
      }
    }

    const navRequestsBtn = treeScroll.querySelector("#dm-nav-requests");
    if (navRequestsBtn) {
      const isReq = homeSubview === "requests";
      if (isReq) {
        navRequestsBtn.classList.add("is-selected");
        navRequestsBtn.setAttribute("aria-current", "page");
        navRequestsBtn.setAttribute("tabindex", "0");
      } else {
        navRequestsBtn.classList.remove("is-selected");
        navRequestsBtn.removeAttribute("aria-current");
        navRequestsBtn.setAttribute("tabindex", "-1");
      }
    }

    const channelBtns = Array.from(dmList.querySelectorAll(".dm-channel-item"));
    let foundActive = false;
    for (const btn of channelBtns) {
      const cid = btn.getAttribute("data-channel-id");
      const isCurrent = homeSubview === "dm" && cid === activeDmChannelId;
      if (isCurrent) {
        foundActive = true;
        btn.classList.add("is-selected");
        btn.setAttribute("aria-current", "page");
        btn.setAttribute("tabindex", "0");
      } else {
        btn.classList.remove("is-selected");
        btn.removeAttribute("aria-current");
        btn.setAttribute("tabindex", "-1");
      }
    }
    return homeSubview !== "dm" || foundActive;
  }

  /**
   * @param {string} channelId
   */
  function selectDmChannel(channelId) {
    const ch = privateChannels.find((c) => c.id === String(channelId));
    if (!ch) return;
    currentGuildId = HOME_GUILD_ID;
    homeSubview = "dm";
    activeDmChannelId = ch.id;
    if (content) content.setAttribute("data-home-subview", "dm");
    friendsView.setAttribute("hidden", "");
    if (chatHeader) {
      chatHeader.removeAttribute("hidden");
      const titleText = chatHeader.querySelector(".chat-title-text");
      if (titleText) titleText.textContent = ch.name;
    }
    const allScreens = content ? Array.from(content.querySelectorAll(".screen")) : [];
    for (const scr of allScreens) {
      if (scr !== screenChat) {
        scr.classList.remove("active");
        scr.setAttribute("aria-hidden", "true");
      }
    }
    if (screenChat) {
      screenChat.classList.add("active");
      screenChat.setAttribute("aria-hidden", "false");
    }
    if (!syncDmTreeSelectionInPlace()) {
      renderDmTree();
    }
    if (typeof options.onSelectChannel === "function") {
      options.onSelectChannel(ch.id, ch);
    }
  }

  function selectHome() {
    currentGuildId = HOME_GUILD_ID;
    if (rail) {
      const guildTiles = Array.from(rail.querySelectorAll(".guild-item"));
      for (const tile of guildTiles) {
        tile.classList.remove("is-selected");
        tile.setAttribute("aria-current", "false");
        tile.setAttribute("tabindex", "-1");
      }
    }
    if (homeRailBtn) {
      homeRailBtn.classList.add("is-selected");
      homeRailBtn.setAttribute("aria-current", "page");
      homeRailBtn.setAttribute("tabindex", "0");
    }
    updateBadges();
    openFriendsSubview();
  }

  /**
   * @param {string} guildId
   */
  function selectGuild(guildId) {
    const targetId = String(guildId || "");
    if (!targetId || targetId === HOME_GUILD_ID) {
      selectHome();
      return;
    }
    currentGuildId = targetId;
    activeDmChannelId = "";

    if (homeRailBtn) {
      homeRailBtn.classList.remove("is-selected");
      homeRailBtn.setAttribute("aria-current", "false");
      homeRailBtn.setAttribute("tabindex", "-1");
    }

    if (rail) {
      const guildTiles = Array.from(rail.querySelectorAll(".guild-item"));
      for (const tile of guildTiles) {
        const gid = tile.getAttribute("data-guild-id") ?? tile.dataset?.guild ?? "";
        const isSel = gid === targetId;
        if (isSel) tile.classList.add("is-selected");
        else tile.classList.remove("is-selected");
        tile.setAttribute("aria-current", isSel ? "true" : "false");
        tile.setAttribute("tabindex", isSel ? "0" : "-1");
      }
    }

    // Восстанавливаем исходную шапку .tree-header и .tree-scroll гильдии (закрытие P1.4)
    if (tree) {
      tree.setAttribute("data-nav-mode", "guild");
      tree.setAttribute("aria-label", "Каналы сервера");
    }
    if (treeHeader) {
      treeHeader.className = "tree-header";
      treeHeader.replaceChildren(...savedGuildHeaderNodes);
    }
    if (treeScroll) {
      treeScroll.className = "tree-scroll";
      treeScroll.removeAttribute("aria-label");
      treeScroll.replaceChildren(...savedGuildScrollNodes);
    }

    if (content) content.removeAttribute("data-home-subview");
    friendsView.setAttribute("hidden", "");
    if (chatHeader) chatHeader.removeAttribute("hidden");
    if (screenChat) {
      screenChat.classList.add("active");
      screenChat.setAttribute("aria-hidden", "false");
    }

    if (typeof options.onSelectGuild === "function") {
      options.onSelectGuild(targetId);
    }
  }

  if (homeRailBtn) {
    homeRailBtn.addEventListener("click", () => {
      selectHome();
    });
    homeRailBtn.addEventListener("keydown", (/** @type {any} */ ev) => {
      if (ev.key === "Enter" || ev.key === " ") {
        ev.preventDefault();
        selectHome();
      } else if (ev.key === "ArrowDown" && rail) {
        const firstGuild = rail.querySelector(".guild-item");
        if (firstGuild) {
          ev.preventDefault();
          firstGuild.focus();
        }
      }
    });
  }

  if (rail) {
    const guildTiles = Array.from(rail.querySelectorAll(".guild-item"));
    for (const tile of guildTiles) {
      tile.addEventListener("click", () => {
        const gid = tile.getAttribute("data-guild-id") ?? tile.dataset?.guild ?? "";
        if (gid) selectGuild(gid);
      });
    }
  }

  /**
   * Переключает активную вкладку `#friends-view` (`online | all | pending | blocked | add_friend`).
   * @param {string} tab
   * @param {boolean} [focusTab]
   */
  function selectFriendsTab(tab, focusTab = false) {
    const next = FRIENDS_TABS.includes(/** @type {any} */ (tab)) ? tab : "online";
    if (activeFriendsTab !== next && showPendingSpamAndIgnored) {
      showPendingSpamAndIgnored = false;
      toggleSpamBtn.setAttribute("aria-pressed", "false");
    }
    activeFriendsTab = next;
    renderFriendsPanel();
    if (focusTab) {
      const btn = tabButtons.find((b) => b.getAttribute("data-tab") === next);
      btn?.focus();
    }
  }

  /* ========================================================================
   * 8. Сетевые операции с друзьями, заявками, заметками и кодом 80013
   * ======================================================================== */

  async function submitAddFriendForm() {
    const rawValue = String(addInput.value ?? "");
    const parsed = parseFriendRequestInput(rawValue);
    if (!parsed.ok) {
      addInputWrap.setAttribute("data-state", "error");
      addFeedback.setAttribute("data-kind", "error");
      addFeedback.textContent = parsed.error;
      addFeedback.removeAttribute("hidden");
      return { ok: false, error: parsed.error };
    }

    addSubmitBtn.disabled = true;
    addSubmitBtn.setAttribute("disabled", "");
    addSubmitBtn.setAttribute("aria-disabled", "true");

    try {
      const res = await sendRest({
        method: "POST",
        url: DM_FRIENDS_ENDPOINTS.USER_RELATIONSHIPS(),
        body: parsed.payload,
      });
      if (res && typeof res === "object") {
        if (Array.isArray(res.relationships)) {
          syncLiveState({
            relationships: res.relationships,
            privateChannels: res.privateChannels,
            users: res.users,
            presences: res.presences,
          });
        } else if (res.body && typeof res.body === "object" && (res.body.id || res.body.user_id)) {
          const norm = normalizeRelationship(res.body, usersById, presencesById);
          relationships = [...relationships.filter((r) => r.user_id !== norm.user_id), norm];
          updateBadges();
        }
      }
      addInputWrap.setAttribute("data-state", "success");
      addFeedback.setAttribute("data-kind", "success");
      addFeedback.textContent = `Запрос дружбы отправлен пользователю ${parsed.payload.username}.`;
      addFeedback.removeAttribute("hidden");
      addInput.value = "";
      addSubmitBtn.disabled = false;
      addSubmitBtn.removeAttribute("disabled");
      addSubmitBtn.setAttribute("aria-disabled", "false");
      announceStatus(`Запрос дружбы отправлен пользователю ${parsed.payload.username}`);
      return { ok: true };
    } catch (err) {
      addInputWrap.setAttribute("data-state", "error");
      addFeedback.setAttribute("data-kind", "error");
      addFeedback.removeAttribute("hidden");
      if (isRateLimitError(err)) {
        return applyRateLimit(err, addFeedback, () => {
          addSubmitBtn.disabled = false;
          addSubmitBtn.removeAttribute("disabled");
          addSubmitBtn.setAttribute("aria-disabled", "false");
        });
      }
      const code = extractDiscordErrorCode(err);
      const msg =
        /** @type {Record<number, string>} */ (ERROR_MESSAGES_BY_CODE)[code] ??
        "Не удалось отправить запрос дружбы. Проверьте имя пользователя.";
      addFeedback.textContent = msg;
      addSubmitBtn.disabled = false;
      addSubmitBtn.removeAttribute("disabled");
      addSubmitBtn.setAttribute("aria-disabled", "false");
      return { ok: false, code, error: msg };
    }
  }

  /**
   * Принимает входящую заявку в друзья (`PUT /users/@me/relationships/{user_id}`).
   * При ответе сервера `80013` (`RELATIONSHIP_INVALID_NO_CONFIRMATION`) открывает `#stranger-confirm-dialog` (`[D] @6673300`).
   * @param {string} userId
   * @param {any} [openerEl]
   * @param {boolean} [confirmStranger]
   */
  async function acceptFriendRequest(userId, openerEl = null, confirmStranger = false) {
    const uid = String(userId);
    const row = friendsList.querySelector(`.friend-row[data-user-id="${uid}"]`);
    if (row) row.setAttribute("data-state", "pending");

    try {
      const res = await sendRest({
        method: "PUT",
        url: DM_FRIENDS_ENDPOINTS.USER_RELATIONSHIP(uid),
        body: {
          type: RELATIONSHIP_TYPES.FRIEND,
          confirm_stranger_request: Boolean(confirmStranger),
        },
      });
      relationships = relationships.map((r) =>
        r.user_id === uid
          ? { ...r, type: RELATIONSHIP_TYPES.FRIEND, is_stranger_request: false }
          : r,
      );
      if (res && typeof res === "object" && Array.isArray(res.privateChannels)) {
        syncLiveState({
          relationships: Array.isArray(res.relationships) ? res.relationships : undefined,
          privateChannels: res.privateChannels,
          users: Array.isArray(res.users) ? res.users : undefined,
          presences: res.presences,
        });
      }
      updateBadges();
      renderFriendsPanel();
      renderDmTree();
      announceStatus("Запрос в друзья принят");
      return { ok: true };
    } catch (err) {
      const code = extractDiscordErrorCode(err);
      if (code === DM_FRIENDS_ERROR_CODES.RELATIONSHIP_INVALID_NO_CONFIRMATION) {
        relationships = relationships.map((r) =>
          r.user_id === uid ? { ...r, is_stranger_request: true } : r,
        );
        renderFriendsPanel();
        openStrangerDialog(uid, openerEl);
        return { ok: false, code, needsConfirmation: true };
      }
      if (isRateLimitError(err)) {
        const retryAfterMs = Math.max(1000, extractRateLimitRetryAfterMs(err));
        if (row) row.setAttribute("data-state", "error");
        announceStatus(rateLimitText(retryAfterMs));
        return { ok: false, code: 0, rateLimited: true, retryAfterMs, error: rateLimitText(retryAfterMs) };
      }
      if (row) row.setAttribute("data-state", "error");
      const message =
        /** @type {Record<number, string>} */ (ERROR_MESSAGES_BY_CODE)[code] ??
        "Не удалось принять запрос в друзья";
      if (row && Boolean(/** @type {any} */ (globalThis).__TAURI_INTERNALS__)) {
        const stEl = row.querySelector(".friend-status-text");
        if (stEl) {
          stEl.textContent = message;
          /** @type {HTMLElement} */ (stEl).style.color = "#f23f43";
        }
      }
      announceStatus(message);
      return { ok: false, code, error: message };
    }
  }

  /**
   * @param {string} userId
   * @param {any} [openerEl]
   */
  function openStrangerDialog(userId, openerEl = null) {
    pendingStrangerUserId = String(userId);
    strangerDialogOpener = openerEl ?? doc?.activeElement ?? null;
    strangerDialog.removeAttribute("hidden");
    strangerCancelBtn.focus();
  }

  /**
   * @param {boolean} [restoreFocus]
   */
  function closeStrangerDialog(restoreFocus = true) {
    strangerDialog.setAttribute("hidden", "");
    const opener = strangerDialogOpener;
    strangerDialogOpener = null;
    if (restoreFocus && opener && typeof opener.focus === "function") {
      opener.focus();
    }
  }

  async function confirmStrangerRequest() {
    if (!pendingStrangerUserId) return { ok: false };
    const uid = pendingStrangerUserId;
    pendingStrangerUserId = "";
    closeStrangerDialog(true);
    return acceptFriendRequest(uid, null, true);
  }

  /**
   * Удаляет связь (`DELETE /users/@me/relationships/{user_id}`: удаление друга, отмена/отклонение заявки, разблокировка).
   * @param {string} userId
   */
  async function removeRelationship(userId) {
    const uid = String(userId);
    try {
      await sendRest({
        method: "DELETE",
        url: DM_FRIENDS_ENDPOINTS.USER_RELATIONSHIP(uid),
      });
    } catch (err) {
      return reportActionError(err, "Не удалось удалить связь с пользователем");
    }
    relationships = relationships.filter((r) => r.user_id !== uid);
    updateBadges();
    renderFriendsPanel();
    renderDmTree();
    announceStatus("Связь с пользователем удалена");
    return { ok: true };
  }

  /**
   * Блокирует пользователя (`PUT /users/@me/relationships/{user_id}` с `{ type: 2 }`).
   * @param {string} userId
   */
  async function blockUser(userId) {
    const uid = String(userId);
    try {
      await sendRest({
        method: "PUT",
        url: DM_FRIENDS_ENDPOINTS.USER_RELATIONSHIP(uid),
        body: { type: RELATIONSHIP_TYPES.BLOCKED },
      });
    } catch (err) {
      return reportActionError(err, "Не удалось заблокировать пользователя");
    }
    const existing = relationships.find((r) => r.user_id === uid);
    if (existing) {
      relationships = relationships.map((r) =>
        r.user_id === uid ? { ...r, type: RELATIONSHIP_TYPES.BLOCKED } : r,
      );
    } else {
      relationships.push(
        normalizeRelationship({ id: uid, user_id: uid, type: RELATIONSHIP_TYPES.BLOCKED }, usersById, presencesById),
      );
    }
    updateBadges();
    renderFriendsPanel();
    announceStatus("Пользователь заблокирован");
    return { ok: true };
  }

  /**
   * Игнорирует пользователя (`PUT /users/@me/relationships/{user_id}/ignore`, `[D] @699907`).
   * @param {string} userId
   */
  async function ignoreUser(userId) {
    const uid = String(userId);
    try {
      await sendRest({
        method: "PUT",
        url: DM_FRIENDS_ENDPOINTS.IGNORE_USER(uid),
      });
    } catch (err) {
      return reportActionError(err, "Не удалось игнорировать пользователя");
    }
    relationships = relationships.map((r) =>
      r.user_id === uid ? { ...r, user_ignored: true } : r,
    );
    updateBadges();
    renderFriendsPanel();
    announceStatus("Пользователь игнорируется");
    return { ok: true };
  }

  /**
   * Массово очищает входящие заявки (`DELETE /users/@me/relationships?relationship_type=3`, `[D] @699318, @699593`).
   * @param {boolean} [spamAndIgnoredOnly]
   */
  async function clearPendingRequests(spamAndIgnoredOnly = false) {
    /** @type {{ method: string, url: string, query: { relationship_type: number }, body?: { filters: number[] } }} */
    const req = {
      method: "DELETE",
      url: DM_FRIENDS_ENDPOINTS.USER_RELATIONSHIPS(),
      query: { relationship_type: RELATIONSHIP_TYPES.PENDING_INCOMING },
    };
    if (spamAndIgnoredOnly) {
      req.body = { filters: [RELATIONSHIP_FILTERS.SPAM, RELATIONSHIP_FILTERS.IGNORED] };
    }
    try {
      await sendRest(req);
    } catch (err) {
      return reportActionError(err, "Не удалось очистить входящие заявки");
    }
    relationships = relationships.filter((r) => {
      if (r.type !== RELATIONSHIP_TYPES.PENDING_INCOMING) return true;
      if (spamAndIgnoredOnly) return !(r.is_spam_request || r.user_ignored);
      return false;
    });
    updateBadges();
    renderFriendsPanel();
    renderDmTree();
    announceStatus("Входящие заявки очищены");
    return { ok: true };
  }

  /**
   * Читает заметку о пользователе (`GET /users/@me/notes/{user_id}`) с обязательной проверкой `note_user_id === userId`.
   * @param {string} userId
   */
  async function fetchUserNote(userId) {
    const uid = String(userId);
    /** @type {any} */
    let res;
    try {
      res = await sendRest({
        method: "GET",
        url: DM_FRIENDS_ENDPOINTS.NOTE(uid),
      });
    } catch (err) {
      return reportActionError(err, "Не удалось загрузить заметку");
    }
    let validated;
    try {
      validated = validateNoteResponse(res?.body ?? res, uid);
    } catch (err) {
      return reportActionError(err, "Некорректный ответ сервера заметки");
    }
    relationships = relationships.map((r) =>
      r.user_id === uid ? { ...r, note: validated.note } : r,
    );
    return { ok: true, ...validated };
  }

  /**
   * @param {string} userId
   * @param {any} openerEl
   */
  function openFriendContextMenu(userId, openerEl) {
    activeContextFriendId = String(userId);
    friendMenuOpener = openerEl ?? doc?.activeElement ?? null;
    if (friendMenuOpener) friendMenuOpener.setAttribute("aria-expanded", "true");
    friendMenu.removeAttribute("hidden");
    const items = Array.from(friendMenu.querySelectorAll('[role="menuitem"]'));
    items.forEach((el, idx) => el.setAttribute("tabindex", idx === 0 ? "0" : "-1"));
    items[0]?.focus();
  }

  /**
   * @param {boolean} [restoreFocus]
   */
  function closeFriendContextMenu(restoreFocus = true) {
    friendMenu.setAttribute("hidden", "");
    const opener = friendMenuOpener;
    friendMenuOpener = null;
    activeContextFriendId = "";
    if (opener) {
      opener.setAttribute("aria-expanded", "false");
      if (restoreFocus && typeof opener.focus === "function") opener.focus();
    }
  }

  /* ========================================================================
   * 9. Создание DM / Group DM, кэш одиночных DM и модалка #group-dm-dialog
   * ======================================================================== */

  /**
   * Открывает существующую 1-на-1 DM из кэша или создаёт приватный канал через
   * `POST /users/@me/channels` с телом `{"recipients": [...]}` (`[D] @567585`, `T-N107-DM-POST-01`).
   * @param {string | string[]} recipientIdsInput
   * @returns {Promise<string>}
   */
  async function openPrivateChannel(recipientIdsInput) {
    const { recipients } = normalizeRecipientsPayload(recipientIdsInput);
    if (recipients.length === 1) {
      const targetUserId = recipients[0];
      const cached = privateChannels.find(
        (c) =>
          c.type === PRIVATE_CHANNEL_TYPES.DM &&
          c.recipient_ids.length === 1 &&
          c.recipient_ids[0] === targetUserId,
      );
      if (cached) {
        selectHome();
        selectDmChannel(cached.id);
        return cached.id;
      }
    }

    const res = await sendRest({
      method: "POST",
      url: DM_FRIENDS_ENDPOINTS.USER_CHANNELS,
      body: { recipients },
    });
    const createdRaw = res?.body ?? res;
    if (createdRaw && typeof createdRaw === "object" && Array.isArray(createdRaw.recipients)) {
      for (const recUser of createdRaw.recipients) {
        const nu = normalizeUser(recUser);
        if (nu) usersById.set(nu.id, nu);
      }
    }
    const hydrated = hydratePrivateChannel(createdRaw, usersById, relationships);
    if (hydrated.id) {
      const idx = privateChannels.findIndex((c) => c.id === hydrated.id);
      if (idx >= 0) privateChannels[idx] = hydrated;
      else privateChannels.push(hydrated);
      privateChannels = sortPrivateChannels(privateChannels);
      selectHome();
      selectDmChannel(hydrated.id);
    }
    return hydrated.id;
  }

  /**
   * Создаёт пустую группу через `POST /users/@me/channels/group-dm-shell` (`{"recipient_id": userId}`, `[D] @568215`).
   * @param {string} recipientId
   * @returns {Promise<string>}
   */
  async function createGroupDmShell(recipientId) {
    const uid = String(recipientId);
    const res = await sendRest({
      method: "POST",
      url: DM_FRIENDS_ENDPOINTS.USER_GROUP_DM_SHELL,
      body: { recipient_id: uid },
    });
    const body = res?.body ?? res;
    const hydrated = hydratePrivateChannel(body, usersById, relationships);
    if (hydrated.id) {
      const idx = privateChannels.findIndex((c) => c.id === hydrated.id);
      if (idx >= 0) privateChannels[idx] = hydrated;
      else privateChannels.push(hydrated);
      privateChannels = sortPrivateChannels(privateChannels);
      selectDmChannel(hydrated.id);
    }
    return hydrated.id;
  }

  /**
   * Добавляет получателя в группу (`PUT /channels/{channelId}/recipients/{userId}`, `[D] @570912`).
   * Статус `201` переключает активный канал на `res.body.id`, иначе (`200/204`) возвращает прежний `channelId`.
   * @param {string} channelId
   * @param {string} userId
   * @returns {Promise<string>}
   */
  async function addRecipientToGroup(channelId, userId) {
    const cid = String(channelId);
    const uid = String(userId);
    const res = await sendRest({
      method: "PUT",
      url: DM_FRIENDS_ENDPOINTS.CHANNEL_RECIPIENT(cid, uid),
    });
    if (res?.status === 201 && res?.body?.id) {
      const hydrated = hydratePrivateChannel(res.body, usersById, relationships);
      const idx = privateChannels.findIndex((c) => c.id === hydrated.id);
      if (idx >= 0) privateChannels[idx] = hydrated;
      else privateChannels.push(hydrated);
      privateChannels = sortPrivateChannels(privateChannels);
      selectDmChannel(hydrated.id);
      return hydrated.id;
    }
    return cid;
  }

  /**
   * Переводит фокус на канал в дереве ЛС, иначе на `#dm-nav-friends` (`DM-FRIENDS-SPEC.md §5.2 п.2`).
   * @param {string} channelId
   */
  function focusDmTreeItem(channelId) {
    const list = doc?.getElementById("dm-channel-list");
    const target = channelId
      ? list?.querySelector(`[data-channel-id="${channelId}"]`) ?? null
      : null;
    const node = target ?? doc?.getElementById("dm-nav-friends");
    if (node && typeof node.focus === "function") node.focus();
  }

  /**
   * Закрывает ЛС (`DELETE /channels/{id}?silent=false` с оптимистичным `CHANNEL_DELETE`, `[D] @569942`).
   * При отказе DELETE канал возвращается в дерево и в `#server-status` выводится причина.
   * @param {string} channelId
   * @param {boolean} [silent]
   */
  async function closePrivateChannel(channelId, silent = false) {
    const cid = String(channelId);
    const index = privateChannels.findIndex((c) => c.id === cid);
    const removed = index >= 0 ? privateChannels[index] ?? null : null;
    const neighborId =
      index >= 0 ? privateChannels[index + 1]?.id ?? privateChannels[index - 1]?.id ?? "" : "";
    const wasActive = activeDmChannelId === cid;
    const activeNode = doc?.activeElement;
    const hadFocusInRow = Boolean(
      activeNode && String(activeNode.getAttribute?.("data-channel-id") ?? "") === cid,
    );

    privateChannels = privateChannels.filter((c) => c.id !== cid);
    if (wasActive) {
      openFriendsSubview();
    } else {
      renderDmTree();
    }
    if (hadFocusInRow) focusDmTreeItem(neighborId);

    try {
      await sendRest({
        method: "DELETE",
        url: DM_FRIENDS_ENDPOINTS.CHANNEL(cid),
        query: { silent: Boolean(silent) },
      });
    } catch (err) {
      if (removed) {
        privateChannels.push(removed);
        privateChannels = sortPrivateChannels(privateChannels);
        if (wasActive) selectDmChannel(cid);
        else renderDmTree();
      }
      return reportActionError(err, "Не удалось закрыть переписку");
    }
    return { ok: true };
  }

  function renderGroupDmModal() {
    const remaining = Math.max(0, MAX_GROUP_DM_RECIPIENTS - selectedGroupDmUserIds.size);
    gdmRemainingSpan.textContent = String(remaining);
    gdmSubmitBtn.textContent =
      selectedGroupDmUserIds.size === 1 ? "Создать ЛС" : "Создать групповой чат";
    gdmSubmitBtn.disabled = selectedGroupDmUserIds.size === 0;

    // 1. Чипы выбранных друзей
    gdmChipsList.replaceChildren();
    for (const uid of selectedGroupDmUserIds) {
      const u = usersById.get(uid);
      const rel = relationships.find((r) => r.user_id === uid);
      const name = resolveUserDisplayName(u, rel?.nickname ?? null) || uid;
      const chip = make("li", "group-dm-chip");
      chip.setAttribute("data-user-id", uid);
      const chipName = make("span", "group-dm-chip-name", name);
      const chipRemove = make("button", "group-dm-chip-remove", "✕");
      chipRemove.setAttribute("type", "button");
      chipRemove.setAttribute("data-action", "remove-group-dm-recipient");
      chipRemove.setAttribute("data-user-id", uid);
      chipRemove.setAttribute("aria-label", `Убрать ${name} из списка получателей`);
      chipRemove.addEventListener("click", () => {
        toggleGroupDmRecipient(uid);
      });
      chip.append(chipName, chipRemove);
      gdmChipsList.append(chip);
    }

    // 2. Список друзей (type === 1) для выбора
    const friends = filterRelationshipsByTab(relationships, "all", {
      usersById,
      presencesById,
      searchQuery: groupDmSearchQuery,
    });

    const limitReached = selectedGroupDmUserIds.size >= MAX_GROUP_DM_RECIPIENTS;
    const existingOpts = Array.from(gdmFriendList.querySelectorAll('[role="option"]'));
    const sameOrder =
      existingOpts.length === friends.length &&
      existingOpts.every((el, idx) => el.getAttribute("data-user-id") === friends[idx]?.user_id);

    if (sameOrder && existingOpts.length > 0) {
      for (const opt of existingOpts) {
        const uid = opt.getAttribute("data-user-id") ?? "";
        const isSelected = selectedGroupDmUserIds.has(uid);
        const isDisabled = !isSelected && limitReached;
        opt.setAttribute("aria-selected", isSelected ? "true" : "false");
        if (isDisabled) opt.setAttribute("aria-disabled", "true");
        else opt.removeAttribute("aria-disabled");
      }
      return;
    }

    gdmFriendList.replaceChildren();

    for (let i = 0; i < friends.length; i += 1) {
      const friend = friends[i];
      if (!friend) continue;
      const isSelected = selectedGroupDmUserIds.has(friend.user_id);
      const isDisabled = !isSelected && limitReached;

      const opt = make("div", "group-dm-friend-option");
      opt.id = `group-dm-opt-${friend.user_id}`;
      opt.setAttribute("role", "option");
      opt.setAttribute("data-user-id", friend.user_id);
      opt.setAttribute("aria-selected", isSelected ? "true" : "false");
      if (isDisabled) {
        opt.setAttribute("aria-disabled", "true");
      }

      const avatarSpan = make("span", "avatar avatar--sm", friend.avatar.initials);
      avatarSpan.style.setProperty("--avatar-hue", String(friend.avatar.hue));
      avatarSpan.setAttribute("aria-hidden", "true");
      const dot = make("span", "status-dot");
      dot.setAttribute("data-status", friend.status);
      avatarSpan.append(dot);

      const metaSpan = make("span", "group-dm-friend-meta");
      metaSpan.append(
        make("span", "group-dm-friend-name", friend.displayName),
        make("span", "group-dm-friend-tag", friend.userTag),
      );

      const checkboxSpan = make("span", "group-dm-checkbox", "✓");
      checkboxSpan.setAttribute("aria-hidden", "true");

      opt.append(avatarSpan, metaSpan, checkboxSpan);
      opt.addEventListener("click", () => {
        toggleGroupDmRecipient(friend.user_id);
      });
      gdmFriendList.append(opt);
    }
  }

  /**
   * @param {string} userId
   */
  function toggleGroupDmRecipient(userId) {
    const uid = String(userId);
    if (selectedGroupDmUserIds.has(uid)) {
      selectedGroupDmUserIds.delete(uid);
    } else {
      if (selectedGroupDmUserIds.size >= MAX_GROUP_DM_RECIPIENTS) {
        return;
      }
      selectedGroupDmUserIds.add(uid);
    }
    gdmError.setAttribute("hidden", "");
    renderGroupDmModal();
  }

  /**
   * @param {any} [openerEl]
   */
  function openGroupDmModal(openerEl = null) {
    groupDmOpener = openerEl ?? doc?.activeElement ?? null;
    if (groupDmOpener) groupDmOpener.setAttribute("aria-expanded", "true");
    selectedGroupDmUserIds.clear();
    groupDmSearchQuery = "";
    gdmSearchInput.value = "";
    gdmError.setAttribute("hidden", "");
    gdmError.textContent = "";
    groupDmModal.setAttribute("data-state", "idle");
    groupDmDialog.removeAttribute("hidden");
    renderGroupDmModal();
    gdmSearchInput.focus();
  }

  /**
   * @param {boolean} [restoreFocus]
   */
  function closeGroupDmModal(restoreFocus = true) {
    groupDmDialog.setAttribute("hidden", "");
    groupDmModal.setAttribute("data-state", "idle");
    const opener = groupDmOpener;
    groupDmOpener = null;
    if (opener) {
      opener.setAttribute("aria-expanded", "false");
      if (restoreFocus && typeof opener.focus === "function") opener.focus();
    }
  }

  async function submitGroupDmModal() {
    const selected = Array.from(selectedGroupDmUserIds);
    if (selected.length === 0) return { ok: false };
    groupDmModal.setAttribute("data-state", "pending");
    gdmError.setAttribute("hidden", "");

    try {
      const channelId = await openPrivateChannel(selected);
      closeGroupDmModal(false);
      return { ok: true, channelId };
    } catch (err) {
      groupDmModal.setAttribute("data-state", "error");
      gdmError.removeAttribute("hidden");
      if (isRateLimitError(err)) {
        return applyRateLimit(err, gdmError);
      }
      const code = extractDiscordErrorCode(err);
      const msg =
        /** @type {Record<number, string>} */ (ERROR_MESSAGES_BY_CODE)[code] ??
        "Не удалось создать групповой чат.";
      gdmError.textContent = msg;
      return { ok: false, code, error: msg };
    }
  }

  /**
   * Синхронизирует живое состояние друзей, заявок, ЛС, пользователей и статусов на лету,
   * не сбрасывая текущую открытую вкладку или переписку.
   * @param {{
   *   relationships?: any[],
   *   privateChannels?: any[],
   *   users?: any[],
   *   presences?: any,
   * }} [nextState]
   */
  function syncLiveState(nextState = {}) {
    if (Array.isArray(nextState.users)) {
      for (const rawUser of nextState.users) {
        const nu = normalizeUser(rawUser);
        if (nu) usersById.set(nu.id, nu);
      }
    }
    if (nextState.presences && typeof nextState.presences === "object") {
      if (Array.isArray(nextState.presences)) {
        for (const p of nextState.presences) {
          const uid = parseEtfString(p?.user?.id ?? p?.user_id);
          if (uid) presencesById[uid] = p;
        }
      } else {
        presencesById = { ...presencesById, ...nextState.presences };
      }
    }
    if (Array.isArray(nextState.relationships)) {
      if (nextState.relationships.length > 0 || relationships.length === 0 || /** @type {any} */ (nextState).forceWipe === true) {
        relationships = nextState.relationships.map((r) =>
          normalizeRelationship(r, usersById, presencesById),
        );
      }
    }
    if (Array.isArray(nextState.privateChannels)) {
      if (nextState.privateChannels.length > 0 || privateChannels.length === 0 || /** @type {any} */ (nextState).forceWipe === true) {
        for (const rawCh of nextState.privateChannels) {
          if (Array.isArray(rawCh?.recipients)) {
            for (const rec of rawCh.recipients) {
              const nu = normalizeUser(rec);
              if (nu) usersById.set(nu.id, nu);
            }
          }
        }
        privateChannels = sortPrivateChannels(
          nextState.privateChannels
            .filter((c) => {
              const type = c?.type;
              return type === null || type === undefined || PRIVATE_CHANNEL_TYPE_SET.has(Number(type));
            })
            .map((c) => hydratePrivateChannel(c, usersById, relationships)),
        );
      }
    }
    updateBadges();
    if (currentGuildId === HOME_GUILD_ID) {
      renderDmTree();
      if (homeSubview === "friends" && activeFriendsTab !== "add_friend") {
        renderFriendsPanel();
      }
    }
  }

  /**
   * Применяет одиночное Gateway-событие (`RELATIONSHIP_*`, `CHANNEL_*`, `READY`) к контроллеру.
   * @param {{ type?: string, t?: string, d?: any, data?: any }} event
   */
  function handleGatewayEvent(event) {
    if (!event || typeof event !== "object") return;
    const next = applyDmFriendsDispatch(
      {
        privateChannels,
        relationships,
        users: usersById,
      },
      /** @type {any} */ (event),
    );
    relationships = next.relationships;
    privateChannels = next.privateChannels;
    updateBadges();
    if (currentGuildId === HOME_GUILD_ID) {
      renderDmTree();
      if (homeSubview === "friends" && activeFriendsTab !== "add_friend") {
        renderFriendsPanel();
      }
    }
  }

  updateBadges();

  return {
    getGuildId: () => currentGuildId,
    getActiveFriendsTab: () => activeFriendsTab,
    getPrivateChannels: () => [...privateChannels],
    getRelationships: () => [...relationships],
    syncLiveState,
    handleGatewayEvent,
    selectHome,
    selectGuild,
    selectDmChannel,
    selectFriendsTab,
    submitAddFriendForm,
    acceptFriendRequest,
    confirmStrangerRequest,
    removeRelationship,
    blockUser,
    ignoreUser,
    clearPendingRequests,
    fetchUserNote,
    openPrivateChannel,
    createGroupDmShell,
    addRecipientToGroup,
    closePrivateChannel,
    openGroupDmModal,
    closeGroupDmModal,
    toggleGroupDmRecipient,
    submitGroupDmModal,
    /** Сброс всего состояния при смене аккаунта / logout. */
    reset() {
      privateChannels = [];
      relationships = [];
      currentGuildId = HOME_GUILD_ID;
      homeSubview = "friends";
      activeFriendsTab = "online";
      if (treeScroll) treeScroll.replaceChildren();
      if (treeHeader) treeHeader.replaceChildren();
      if (friendsList) friendsList.replaceChildren();
      if (friendsView) friendsView.replaceChildren();
    },
  };
}
