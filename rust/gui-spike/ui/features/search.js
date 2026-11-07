// @ts-check
/**
 * Поиск по загруженному окну данных (chat/member search).
 *
 * Честная граница: в JS-heap живёт ≤500 строк (ADR-1), полный каталог — за
 * серверным поиском (Phase 3). Поэтому ищем только по загруженным страницам и
 * показываем `найдено N из M загруженных`, а не выдаём это за поиск по 100 000.
 */
import { UIError, reportError } from "./errors.js";

/**
 * @typedef {import("../model.js").PageStore} PageStore
 * @typedef {import("./virtual-list.js").VirtualList} VirtualList
 */

/* =============================================================================
 * STUB-14 (M-13): разбор и нормализация фильтров поиска.
 *
 * Оракул — waves/RW-40/RE-DEEP-REA.md §2.3–2.4 (таблица `queryKey` из
 * `SearchFetcher`/`SearchStore`): `author_id`, `mentions`, `has`, `channel_id`,
 * `link_hostname`, `attachment_extension`, `attachment_filename`, `pinned`,
 * `author_type`, `min_id`/`max_id`, `content`; сортировка — `sort_by`/
 * `sort_order`, страница — 25 (`limit`), `offset` — номер пропущенной записи.
 * Здесь только чистая нормализация UI → REST-параметры, без сети и DOM.
 * ========================================================================== */

/** §2.4: размер страницы поиска (константа `T_y`/`iG` в дампе — 25). */
export const SEARCH_PAGE_SIZE = 25;

/** M-13 (§1.1 [A]): потолок пагинации (400 страниц по 25 = 10 000 записей, последний offset = 9975). */
export const MAX_SEARCH_OFFSET = 9975;
export const MAX_SEARCH_PAGES = 400;

/** M-13 (§1.1 [D]): лимит автоматических повторов при HTTP 202 (5 повторов, всего ≤6 запросов). */
export const SEARCH_MAX_INDEXING_ATTEMPTS = 5;
/** M-13 (§1.1 [D]): дефолтная задержка при отсутствии/0/NaN заголовка `Retry-After` (5000 мс). */
export const SEARCH_DEFAULT_RETRY_AFTER_MS = 5000;

/** M-13 (§4.1 [D]): бит `AGE_GATED_SPACES = 1` в `regional_feature_config.age_gated_features`. */
export const AGE_GATED_SPACES = 1;
/** M-13 (§4.1 [D]): статусы взрослого пользователя (`VERIFIED_ADULT=3`, `INFERRED_ADULT=4`). */
export const ADULT_VERIFICATION_STATUSES = Object.freeze([3, 4]);
/** M-13 (§4.1 [D]): ключ хранилища согласия NSFW гильдии. */
export const GUILD_NSFW_AGREE_STORAGE_KEY = "GuildNSFWAgreeStore";

/** §2.3: подтверждённые серверные значения `has`. */
/** @type {readonly string[]} */
export const SEARCH_HAS_VALUES = Object.freeze([
  "link",
  "embed",
  "poll",
  "snapshot",
  "file",
  "video",
  "image",
  "sound",
  "sticker",
]);

/**
 * §2.3: подписи «Содержит» (`rows.js` HAS_OPTIONS) и русские строки каталога
 * клиента → токены сервера. Уже серверный токен проходит без изменений.
 * @type {Record<string, string>}
 */
export const SEARCH_HAS_ALIASES = Object.freeze({
  "audio": "sound",
  "forward": "snapshot",
  "ссылка": "link",
  "ссылку": "link",
  "картинка": "image",
  "изображение": "image",
  "файл": "file",
  "звук": "sound",
  "аудио": "sound",
  "видео": "video",
  "опрос": "poll",
  "переслать": "snapshot",
  "снапшот": "snapshot",
  "приглашение": "embed",
  "вложение": "embed",
  "стикер": "sticker",
  "эмодзи": "sticker",
});

/**
 * §2.3: `author_type` — подписи UI → значения сервера. «Любой» — без фильтра.
 * @type {Record<string, string>}
 */
export const SEARCH_AUTHOR_TYPES = Object.freeze({
  "Человек": "user",
  "Бот": "bot",
  "Вебхук": "webhook",
});

/** @type {Record<string, string>} */
const AUTHOR_TYPE_ALIASES = Object.freeze({
  "user": "user",
  "bot": "bot",
  "webhook": "webhook",
  "человек": "user",
  "бот": "bot",
  "вебхук": "webhook",
});

/** Discord epoch (2015-01-01T00:00:00.000Z) для snowflake из timestamp. */
const DISCORD_EPOCH_MS = 1420070400000n;

/**
 * `fromTimestamp` из дампа: `(ms − epoch) << 22`.
 * @param {number} timestampMs
 * @returns {string}
 */
export function snowflakeFromTimestamp(timestampMs) {
  if (!Number.isFinite(timestampMs)) return "";
  return ((BigInt(Math.trunc(timestampMs)) - DISCORD_EPOCH_MS) << 22n).toString();
}

/**
 * Календарная дата `ГГГГ-ММ-ДД`. Одной регулярки и `Date.parse` мало:
 * `Date.parse("2026-02-31T00:00:00.000Z")` нормализует несуществующий день в
 * март, поэтому UTC-компоненты разобранной даты сверяются с исходными числами.
 * @param {string} value
 * @returns {boolean}
 */
export function isCalendarDate(value) {
  if (!/^\d{4}-\d{2}-\d{2}$/.test(value)) return false;
  const year = Number(value.slice(0, 4));
  const month = Number(value.slice(5, 7));
  const day = Number(value.slice(8, 10));
  const parsed = new Date(Date.parse(`${value}T00:00:00.000Z`));
  if (Number.isNaN(parsed.getTime())) return false;
  return parsed.getUTCFullYear() === year
    && parsed.getUTCMonth() + 1 === month
    && parsed.getUTCDate() === day;
}

/**
 * §2.3 / §5.2: дата `on:`/`before:`/`after:` → `min_id`/`max_id`.
 * Поддерживает строковый аргумент `searchDateBounds("2026-10-20")` (обратная
 * совместимость) и объект `{ date?: string, on?: string, before?: string, after?: string }`.
 * @param {string | { date?: string, on?: string, before?: string, after?: string } | null | undefined} value
 * @returns {{ min_id?: string, max_id?: string } | null}
 */
export function searchDateBounds(value) {
  if (typeof value === "string") {
    if (!isCalendarDate(value)) return null;
    const start = Date.parse(`${value}T00:00:00.000Z`);
    const end = Date.parse(`${value}T23:59:59.999Z`);
    if (!Number.isFinite(start) || !Number.isFinite(end)) return null;
    return { min_id: snowflakeFromTimestamp(start), max_id: snowflakeFromTimestamp(end) };
  }
  if (value === null || value === undefined || typeof value !== "object") return null;
  /** @type {{ min_id?: string, max_id?: string }} */
  const result = {};
  const exactDay = typeof value.date === "string" && value.date !== ""
    ? value.date
    : typeof value.on === "string" && value.on !== ""
      ? value.on
      : "";
  if (exactDay !== "") {
    if (isCalendarDate(exactDay)) {
      const start = Date.parse(`${exactDay}T00:00:00.000Z`);
      const end = Date.parse(`${exactDay}T23:59:59.999Z`);
      result.min_id = snowflakeFromTimestamp(start);
      result.max_id = snowflakeFromTimestamp(end);
    }
  }
  if (typeof value.after === "string" && value.after !== "") {
    if (isCalendarDate(value.after)) {
      const afterEnd = Date.parse(`${value.after}T23:59:59.999Z`);
      result.min_id = snowflakeFromTimestamp(afterEnd);
    }
  }
  if (typeof value.before === "string" && value.before !== "") {
    if (isCalendarDate(value.before)) {
      const beforeStart = Date.parse(`${value.before}T00:00:00.000Z`);
      result.max_id = snowflakeFromTimestamp(beforeStart);
    }
  }
  return Object.keys(result).length > 0 ? result : null;
}

/**
 * Значения поля фильтра: через запятую и/или пробел (`42, 77`).
 * @param {string | undefined} value
 * @returns {string[]}
 */
export function splitQueryValues(value) {
  if (typeof value !== "string") return [];
  return value
    .split(/[\s,]+/)
    .map((item) => item.trim())
    .filter((item) => item !== "");
}

/**
 * §2.3 / §5.1: `has` — подписи UI и токены сервера (включая отрицание `-file`)
 * → массив подтверждённых значений. Неизвестное значение отбрасывается.
 * @param {string | undefined} value
 * @returns {string[]}
 */
export function normalizeHasFilter(value) {
  /** @type {string[]} */
  const tokens = [];
  for (const item of splitQueryValues(value)) {
    const lower = item.toLowerCase();
    const negated = lower.startsWith("-");
    const rawKey = negated ? lower.slice(1) : lower;
    const mapped = SEARCH_HAS_ALIASES[rawKey] ?? rawKey;
    if (!SEARCH_HAS_VALUES.includes(mapped)) continue;
    const token = negated ? `-${mapped}` : mapped;
    if (!tokens.includes(token)) tokens.push(token);
  }
  return tokens;
}

/**
 * @typedef {{
 *   key: "from" | "in" | "has" | "before" | "after" | "on" | "pinned" | "mentions" | "authorType",
 *   prefix: string,
 *   value: string,
 *   rawToken: string,
 *   negated: boolean,
 * }} SearchFilterChip
 */

/**
 * §5.1–§5.2: разбор инлайн-токенов фильтрации в строке поиска (`from:`, `от:`,
 * `in:`, `в:`, `has:`, `-has:`, `имеет:`, `before:`, `до:`, `after:`, `после:`,
 * `on:`, `during:`, `день:`, `pinned:`, `закреплённые:`, `mentions:`,
 * `с упоминанием:`, `authorType:`, `-authorType:`, `ТипАвтора:`).
 * @param {string | undefined} rawQuery
 * @returns {{
 *   freeText: string,
 *   chips: SearchFilterChip[],
 *   tokenFilters: {
 *     from: string,
 *     in: string,
 *     has: string,
 *     mentions: string,
 *     date: string,
 *     before: string,
 *     after: string,
 *     author: string,
 *     authorTypes: string[],
 *     pinned: boolean | null,
 *   },
 * }}
 */
export function parseSearchQueryTokens(rawQuery) {
  const input = typeof rawQuery === "string" ? rawQuery : "";
  /** @type {SearchFilterChip[]} */
  const chips = [];
  /** @type {string[]} */
  const fromList = [];
  /** @type {string[]} */
  const inList = [];
  /** @type {string[]} */
  const hasList = [];
  /** @type {string[]} */
  const mentionsList = [];
  /** @type {string[]} */
  const authorTypes = [];
  let dateVal = "";
  let beforeVal = "";
  let afterVal = "";
  /** @type {boolean | null} */
  let pinnedVal = null;
  let authorSelect = "Любой";

  const tokenRegex = /(^|\s)(-?(?:с\s+упоминанием|закреплённые|authorType|author_type|ТипАвтора|mentions|before|during|pinned|after|имеет|from|день|has|от|in|в|до|после|on)):(?:"([^"]+)"|(\S+))/giu;
  /** @type {Array<[number, number]>} */
  const matchedRanges = [];

  for (const match of input.matchAll(tokenRegex)) {
    const leadingSpace = match[1] ?? "";
    const rawOp = (match[2] ?? "").trim();
    const rawVal = (match[3] ?? match[4] ?? "").trim();
    const tokenStart = (match.index ?? 0) + leadingSpace.length;
    const tokenEnd = (match.index ?? 0) + match[0].length;
    const rawToken = input.slice(tokenStart, tokenEnd);

    const opNegated = rawOp.startsWith("-");
    const opName = (opNegated ? rawOp.slice(1) : rawOp).replace(/\s+/g, " ").toLowerCase();

    if (opName === "from" || opName === "от") {
      // [D] @7556693: -from: не снимает минус, уходит в content.
      if (opNegated || rawVal === "") continue;
      fromList.push(rawVal);
      chips.push({ key: "from", prefix: "from:", value: rawVal, rawToken, negated: false });
      matchedRanges.push([tokenStart, tokenEnd]);
    } else if (opName === "in" || opName === "в") {
      if (opNegated) continue;
      const cleanChannel = rawVal.replace(/^#+/, "");
      if (cleanChannel === "") continue;
      inList.push(cleanChannel);
      chips.push({ key: "in", prefix: "in:", value: cleanChannel, rawToken, negated: false });
      matchedRanges.push([tokenStart, tokenEnd]);
    } else if (opName === "has" || opName === "имеет") {
      const valNegated = rawVal.startsWith("-");
      const negated = opNegated || valNegated;
      const cleanVal = (valNegated ? rawVal.slice(1) : rawVal).toLowerCase();
      const canonical = SEARCH_HAS_ALIASES[cleanVal] ?? cleanVal;
      if (!SEARCH_HAS_VALUES.includes(canonical)) continue;
      const storedToken = negated ? `-${canonical}` : canonical;
      if (!hasList.includes(storedToken)) hasList.push(storedToken);
      chips.push({
        key: "has",
        prefix: negated ? "-has:" : "has:",
        value: canonical,
        rawToken,
        negated,
      });
      matchedRanges.push([tokenStart, tokenEnd]);
    } else if (opName === "before" || opName === "до") {
      if (opNegated || !isCalendarDate(rawVal)) continue;
      beforeVal = rawVal;
      chips.push({ key: "before", prefix: "before:", value: rawVal, rawToken, negated: false });
      matchedRanges.push([tokenStart, tokenEnd]);
    } else if (opName === "after" || opName === "после") {
      if (opNegated || !isCalendarDate(rawVal)) continue;
      afterVal = rawVal;
      chips.push({ key: "after", prefix: "after:", value: rawVal, rawToken, negated: false });
      matchedRanges.push([tokenStart, tokenEnd]);
    } else if (opName === "on" || opName === "during" || opName === "день") {
      if (opNegated || !isCalendarDate(rawVal)) continue;
      dateVal = rawVal;
      chips.push({ key: "on", prefix: "on:", value: rawVal, rawToken, negated: false });
      matchedRanges.push([tokenStart, tokenEnd]);
    } else if (opName === "pinned" || opName === "закреплённые") {
      if (opNegated) continue;
      const lowerVal = rawVal.toLowerCase();
      if (lowerVal !== "true" && lowerVal !== "false" && lowerVal !== "да" && lowerVal !== "нет") continue;
      const isTrue = lowerVal === "true" || lowerVal === "да";
      pinnedVal = isTrue;
      chips.push({
        key: "pinned",
        prefix: "pinned:",
        value: isTrue ? "true" : "false",
        rawToken,
        negated: false,
      });
      matchedRanges.push([tokenStart, tokenEnd]);
    } else if (opName === "mentions" || opName === "с упоминанием") {
      if (opNegated || rawVal === "") continue;
      mentionsList.push(rawVal);
      chips.push({ key: "mentions", prefix: "mentions:", value: rawVal, rawToken, negated: false });
      matchedRanges.push([tokenStart, tokenEnd]);
    } else if (opName === "authortype" || opName === "author_type" || opName === "типавтора") {
      const valNegated = rawVal.startsWith("-");
      const negated = opNegated || valNegated;
      const cleanVal = (valNegated ? rawVal.slice(1) : rawVal).toLowerCase();
      const canonical = AUTHOR_TYPE_ALIASES[cleanVal];
      if (!canonical) continue;
      const stored = negated ? `-${canonical}` : canonical;
      if (!authorTypes.includes(stored)) authorTypes.push(stored);
      if (!negated) {
        if (canonical === "user") authorSelect = "Человек";
        else if (canonical === "bot") authorSelect = "Бот";
        else if (canonical === "webhook") authorSelect = "Вебхук";
      }
      chips.push({
        key: "authorType",
        prefix: negated ? "-authorType:" : "authorType:",
        value: canonical,
        rawToken,
        negated,
      });
      matchedRanges.push([tokenStart, tokenEnd]);
    }
  }

  let freeText = "";
  let cursor = 0;
  for (const [start, end] of matchedRanges) {
    freeText += input.slice(cursor, start);
    cursor = end;
  }
  freeText += input.slice(cursor);
  freeText = freeText.replace(/\s+/g, " ").trim();

  return {
    freeText,
    chips,
    tokenFilters: {
      from: fromList.join(" "),
      in: inList.join(" "),
      has: hasList.join(" "),
      mentions: mentionsList.join(" "),
      date: dateVal,
      before: beforeVal,
      after: afterVal,
      author: authorSelect,
      authorTypes,
      pinned: pinnedVal,
    },
  };
}

/**
 * @param {any} obj
 * @returns {boolean}
 */
function isUserLikeObject(obj) {
  if (!obj || typeof obj !== "object" || Array.isArray(obj)) return false;
  return (
    "age_verification_status" in obj
    || "ageVerificationStatus" in obj
    || "nsfw_allowed" in obj
    || "nsfwAllowed" in obj
    || "username" in obj
  );
}

/**
 * M-13 (§4.1 [D]): проверка ограничения `AGE_GATED_SPACES = 1` и статуса возраста
 * `age_verification_status ∈ {3, 4}` (`VERIFIED_ADULT=3`, `INFERRED_ADULT=4`).
 *
 * Точная формула §4.1: `Number(regional_feature_config.age_gated_features ?? 0)`.
 * Другие источники маски (user/guild) не подставляются: иначе отсутствие
 * регионального конфига включало бы гейт по одному `age_verification_status`.
 * @param {any} user
 * @param {any} [regionalFeatureConfig]
 * @returns {boolean}
 */
export function isAgeGatedSpacesRestricted(user, regionalFeatureConfig) {
  const mask = Number(regionalFeatureConfig?.age_gated_features ?? 0);
  const isSpaceAgeGated = (mask & AGE_GATED_SPACES) !== 0;
  if (!isSpaceAgeGated) return false;
  const status = Number(user?.age_verification_status ?? user?.ageVerificationStatus ?? 0);
  return !ADULT_VERIFICATION_STATUSES.includes(status);
}

/**
 * M-13 (§4.1 [D]): `GuildNSFWAgreeStore.didAgree(guild)`.
 * Поддерживает сигнатуры `didAgreeNsfwGuild(guild, user, agreedStore, regionalFeatureConfig)`
 * и `didAgreeNsfwGuild(guildId, storedAgreement, user, regionalFeatureConfig)`.
 * @param {any} guildOrId
 * @param {any} [arg2]
 * @param {any} [arg3]
 * @param {any} [arg4]
 * @returns {boolean}
 */
export function didAgreeNsfwGuild(guildOrId, arg2, arg3, arg4) {
  const guildObj = guildOrId !== null && typeof guildOrId === "object" ? guildOrId : null;
  const guildId = guildObj
    ? String(guildObj.id ?? guildObj.guild_id ?? guildObj.guildId ?? "")
    : guildOrId === null || guildOrId === undefined
      ? ""
      : String(guildOrId);
  if (guildId === "") return false;

  const userFirst = isUserLikeObject(arg2);
  const user = userFirst ? arg2 : arg3;
  const agreedStore = userFirst ? arg3 : arg2;
  const regionalFeatureConfig = arg4;

  if (isAgeGatedSpacesRestricted(user, regionalFeatureConfig)) {
    return false;
  }

  if (!agreedStore) return false;
  if (agreedStore instanceof Map) return agreedStore.get(guildId) === true;
  if (agreedStore instanceof Set) return agreedStore.has(guildId);
  if (Array.isArray(agreedStore)) return agreedStore.includes(guildId);
  if (typeof agreedStore === "object") {
    if (agreedStore[guildId] === true) return true;
    if (agreedStore.guilds && typeof agreedStore.guilds === "object" && agreedStore.guilds[guildId] === true) {
      return true;
    }
  }
  return false;
}

/**
 * M-13 (§4.1 [D] `4d2a64cb5df5fc4e_0@7550064`): добавляет `include_nsfw` в `query`
 * ТОЛЬКО когда `didAgreeNsfwGuild(...) === true`; при `!didAgree` ключ `include_nsfw`
 * не добавляется в запрос вообще.
 * @param {Record<string, unknown>} queryObj
 * @param {any} guildOrId
 * @param {any} [arg2]
 * @param {any} [arg3]
 * @param {any} [arg4]
 * @returns {Record<string, unknown>}
 */
export function applyIncludeNsfw(queryObj, guildOrId, arg2, arg3, arg4) {
  const userFirst = isUserLikeObject(arg2);
  const currentUser = userFirst ? arg2 : arg3;
  if (didAgreeNsfwGuild(guildOrId, arg2, arg3, arg4)) {
    const allowed = currentUser?.nsfwAllowed ?? currentUser?.nsfw_allowed;
    queryObj.include_nsfw = allowed === undefined || allowed === null ? true : Boolean(allowed);
  } else {
    delete queryObj.include_nsfw;
  }
  return queryObj;
}

/**
 * M-13 (§3.2 [D] `82a5af57ee11dcd5_0@212`): вычисление задержки ретрая HTTP 202.
 * Тело ответа 202 (`response.body.retry_after`, `response.retry_after`) полностью
 * игнорируется; читается только заголовок `Retry-After` (целые секунды через `parseInt`)
 * либо предвычисленное Rust-слоем поле Tauri IPC конверта `response.retry_after_ms`.
 * @param {any} response
 * @returns {number}
 */
export function computeIndexingRetryDelayMs(response) {
  const headers = response?.headers;
  const rawHeader = typeof headers?.get === "function"
    ? (headers.get("retry-after") ?? headers.get("Retry-After"))
    : (headers?.["retry-after"] ?? headers?.["Retry-After"]);
  if (rawHeader !== undefined && rawHeader !== null) {
    const parsedSec = parseInt(String(rawHeader), 10);
    if (!Number.isNaN(parsedSec) && parsedSec > 0) return parsedSec * 1000;
  }
  const ipcMs = Number(response?.retry_after_ms ?? response?.retryAfterMs);
  if (Number.isFinite(ipcMs) && ipcMs > 0) {
    return Math.trunc(ipcMs);
  }
  return SEARCH_DEFAULT_RETRY_AFTER_MS;
}

/**
 * M-13 (§1.2 `4Y3O+O` [D]): строка прогресса глубокой архивной индексации для HTTP 200 OK.
 * @param {number} documentsIndexed
 * @returns {string}
 */
export function formatHistoricalIndexedStatus(documentsIndexed) {
  const count = Math.max(0, Math.trunc(Number(documentsIndexed) || 0));
  let prefix = "";
  if (count === 0) {
    prefix = "Результатов поиска нет";
  } else {
    const mod10 = count % 10;
    const mod100 = count % 100;
    if (mod10 === 1 && mod100 !== 11) {
      prefix = `Найден ${count} результат`;
    } else if (mod10 >= 2 && mod10 <= 4 && (mod100 < 12 || mod100 > 14)) {
      prefix = `Найдено ${count} результата`;
    } else {
      prefix = `Найдено ${count} результатов`;
    }
  }
  return `${prefix}. Мы всё ещё индексируем старые сообщения.`;
}

/**
 * M-13 (§1.2 `E+2azY` [D]): предупреждение при достижении потолка пагинации (400 стр.).
 * @param {number} [maxPages]
 * @returns {string}
 */
export function formatMaxPagesWarning(maxPages = MAX_SEARCH_PAGES) {
  return `Мы можем предоставить не более ${maxPages} стр. результатов. Попробуйте сузить поиск.`;
}

/**
 * Состояние фильтров оверлея (R-10 + M-13 `before`/`after`/`authorTypes`).
 * @typedef {{
 *   from?: string, in?: string, has?: string, mentions?: string, date?: string,
 *   before?: string, after?: string, author?: string, authorTypes?: string[],
 *   pinned?: boolean,
 * }} SearchFilterState
 */

/**
 * @param {string[]} items
 * @returns {string[]}
 */
function uniqueStrings(items) {
  /** @type {string[]} */
  const out = [];
  for (const item of items) {
    if (item !== "" && !out.includes(item)) out.push(item);
  }
  return out;
}

/**
 * §2.3–2.4: REST query-string подтверждённого поиска. Пустые фильтры не
 * попадают в результат; `limit`/`sort_by`/`sort_order`/`offset` есть всегда.
 * @param {SearchFilterState} filters
 * @param {{
 *   content?: string,
 *   sortBy?: string,
 *   sortOrder?: string,
 *   offset?: number,
 *   attempts?: number,
 *   guild?: any,
 *   guildId?: string,
 *   user?: any,
 *   agreedStore?: any,
 *   regionalFeatureConfig?: any,
 * }} [options]
 * @returns {Record<string, unknown>}
 */
export function buildSearchQuery(filters = {}, options = {}) {
  /** @type {Record<string, unknown>} */
  const query = {};
  const rawContent = String(options.content ?? "").trim();
  const parsedTokens = parseSearchQueryTokens(rawContent);
  const content = parsedTokens.chips.length > 0 ? parsedTokens.freeText : rawContent;
  if (content !== "") query.content = content;

  const authors = uniqueStrings([
    ...splitQueryValues(filters.from),
    ...splitQueryValues(parsedTokens.tokenFilters.from),
  ]);
  if (authors.length > 0) query.author_id = authors;

  const mentions = uniqueStrings([
    ...splitQueryValues(filters.mentions),
    ...splitQueryValues(parsedTokens.tokenFilters.mentions),
  ]);
  if (mentions.length > 0) query.mentions = mentions;

  const has = uniqueStrings([
    ...normalizeHasFilter(filters.has),
    ...normalizeHasFilter(parsedTokens.tokenFilters.has),
  ]);
  if (has.length > 0) query.has = has;

  const channels = uniqueStrings([
    ...splitQueryValues(filters.in).map((ch) => ch.replace(/^#+/, "")),
    ...splitQueryValues(parsedTokens.tokenFilters.in).map((ch) => ch.replace(/^#+/, "")),
  ]);
  if (channels.length > 0) query.channel_id = channels;

  // `pinned:false` — это фильтр «без закреплённых», а выключенный тумблер —
  // отсутствие фильтра, поэтому false от тумблера не отправляется.
  if (filters.pinned === true || parsedTokens.tokenFilters.pinned === true) {
    query.pinned = true;
  } else if (parsedTokens.tokenFilters.pinned === false) {
    query.pinned = false;
  }

  const inlineAuthorTypes = uniqueStrings([
    ...(Array.isArray(filters.authorTypes) ? filters.authorTypes : []),
    ...parsedTokens.tokenFilters.authorTypes,
  ]);
  const authorType = SEARCH_AUTHOR_TYPES[String(filters.author ?? "")];
  if (inlineAuthorTypes.length > 0 && (inlineAuthorTypes.some((t) => t.startsWith("-")) || inlineAuthorTypes.length > 1)) {
    query.author_type = inlineAuthorTypes;
  } else if (authorType !== undefined) {
    query.author_type = authorType;
  } else if (inlineAuthorTypes.length === 1) {
    query.author_type = inlineAuthorTypes[0];
  }

  const bounds = searchDateBounds({
    date: filters.date || parsedTokens.tokenFilters.date || undefined,
    before: filters.before || parsedTokens.tokenFilters.before || undefined,
    after: filters.after || parsedTokens.tokenFilters.after || undefined,
  });
  if (bounds !== null) Object.assign(query, bounds);

  const guildTarget = options.guild ?? options.guildId;
  if (guildTarget !== undefined && guildTarget !== null && guildTarget !== "") {
    applyIncludeNsfw(
      query,
      guildTarget,
      options.user,
      options.agreedStore,
      options.regionalFeatureConfig,
    );
  } else if (/** @type {any} */ (options).includeNsfw === true) {
    query.include_nsfw = true;
  }

  query.limit = SEARCH_PAGE_SIZE;
  query.sort_by = options.sortBy ?? "timestamp";
  query.sort_order = options.sortOrder ?? "desc";
  query.offset = Number.isInteger(options.offset) ? options.offset : 0;
  if (Number.isInteger(options.attempts) && Number(options.attempts) > 0) {
    query.attempts = Number(options.attempts);
  }
  return query;
}

/**
 * Подпись счётчика: «найдено N из M загруженных» (W39h: честная граница окна).
 * @param {number} matches
 * @param {number} loadedRows
 * @returns {string}
 */
export function formatFound(matches, loadedRows) {
  return `найдено ${matches} из ${loadedRows} загруженных`;
}

/**
 * @param {PageStore} store
 * @param {number} pageSize
 * @param {(row: any, index: number) => boolean} predicate
 * @returns {{ first: number | null, matches: number, loadedRows: number }}
 */
export function scanLoadedRows(store, pageSize, predicate) {
  /** @type {number | null} */
  let first = null;
  let matches = 0;
  let loadedRows = 0;
  for (const [pageIndex, page] of store.pages) {
    for (let offset = 0; offset < page.rows.length; offset++) {
      const index = pageIndex * pageSize + offset;
      loadedRows += 1;
      if (!predicate(page.rows[offset], index)) continue;
      matches += 1;
      if (first === null) first = index;
    }
  }
  return { first, matches, loadedRows };
}

/**
 * Подписка на `input` списка: прыжок к первому совпадению + счётчик.
 * @param {{
 *   input: HTMLInputElement,
 *   list: VirtualList,
 *   store: PageStore,
 *   pageSize: number,
 *   countLabel: HTMLElement | null,
 *   totalLabel: string,
 *   predicate: (row: any, index: number, query: string) => boolean,
 *   onResult?: (found: { first: number | null, matches: number, loadedRows: number }) => void,
 *   onError?: (error: UIError) => void,
 * }} options
 */
export function bindListSearch(options) {
  const { input, list, store, pageSize, countLabel, totalLabel, predicate } = options;
  const onError = options.onError ?? (() => {});
  input.addEventListener("input", () => {
    try {
      const query = input.value.trim().toLowerCase();
      for (const row of list.rows.values()) row.classList.remove("is-selected");
      if (query === "") {
        if (countLabel) countLabel.textContent = totalLabel;
        options.onResult?.({ first: null, matches: 0, loadedRows: 0 });
        return;
      }
      const found = scanLoadedRows(store, pageSize, (row, index) => predicate(row, index, query));
      // Счётчик виден и озвучивается (aria-live="polite" у счётчиков).
      if (countLabel) countLabel.textContent = formatFound(found.matches, found.loadedRows);
      options.onResult?.(found);
      if (found.first === null) return;
      list.scrollToIndex(found.first);
      const target = list.rows.get(found.first) ?? null;
      if (target) target.classList.add("is-selected");
    } catch (error) {
      onError(reportError("ACTION_FAILED", error));
    }
  });
}
