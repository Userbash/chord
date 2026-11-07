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

/* =============================================================================
 * TASK-618 (G2 Frontend): Поисковый попап с выпадающими подсказками токенов,
 * подсветкой активных токенов в строке ввода, рендерингом списка результатов
 * с подсветкой поискового запроса, прыжком к сообщению (onJumpToMessage)
 * и клавиатурной навигацией (ArrowUp, ArrowDown, Enter, Escape).
 *
 * RE-оракул:
 * - /home/sanya/re-deep/frontend-d03.md §1.1–§1.3 (searchResultsWrap_a98f3b,
 *   searchResultGroup_a7e67f, id="search-result-<id>");
 * - docs/re/frontend-d03.md §1.1–§1.3;
 * - waves/RW-40/DESIGN/SEARCH-202-NSFW-SPEC.md §5 (грамматика токенов и чипов).
 * ========================================================================== */

/**
 * @typedef {{
 *   token: "from:" | "mentions:" | "has:" | "in:" | "before:" | "after:" | "pinned:",
 *   key: "from" | "mentions" | "has" | "in" | "before" | "after" | "pinned",
 *   labelRu: string,
 *   hintRu: string,
 *   example: string,
 * }} SearchTokenSuggestionItem
 */

/**
 * Выпадающие подсказки токенов поискового попапа (`from:`, `mentions:`, `has:`,
 * `in:`, `before:`, `after:`, `pinned:`).
 * @type {ReadonlyArray<Readonly<SearchTokenSuggestionItem>>}
 */
export const SEARCH_POPOVER_TOKEN_SUGGESTIONS = Object.freeze([
  Object.freeze({
    token: /** @type {const} */ ("from:"),
    key: /** @type {const} */ ("from"),
    labelRu: "от: пользователь",
    hintRu: "Сообщения от выбранного пользователя",
    example: "from:alex",
  }),
  Object.freeze({
    token: /** @type {const} */ ("mentions:"),
    key: /** @type {const} */ ("mentions"),
    labelRu: "упоминания: пользователь",
    hintRu: "Сообщения с упоминанием пользователя",
    example: "mentions:bob",
  }),
  Object.freeze({
    token: /** @type {const} */ ("has:"),
    key: /** @type {const} */ ("has"),
    labelRu: "содержит: ссылка, вложение или файл",
    hintRu: "link, embed, file, image, video, sound, sticker, poll",
    example: "has:image",
  }),
  Object.freeze({
    token: /** @type {const} */ ("in:"),
    key: /** @type {const} */ ("in"),
    labelRu: "в: канал",
    hintRu: "Сообщения в выбранном канале",
    example: "in:новости",
  }),
  Object.freeze({
    token: /** @type {const} */ ("before:"),
    key: /** @type {const} */ ("before"),
    labelRu: "до: ГГГГ-ММ-ДД",
    hintRu: "Сообщения, отправленные до даты",
    example: "before:2026-10-20",
  }),
  Object.freeze({
    token: /** @type {const} */ ("after:"),
    key: /** @type {const} */ ("after"),
    labelRu: "после: ГГГГ-ММ-ДД",
    hintRu: "Сообщения, отправленные после даты",
    example: "after:2026-01-01",
  }),
  Object.freeze({
    token: /** @type {const} */ ("pinned:"),
    key: /** @type {const} */ ("pinned"),
    labelRu: "закреплено: true или false",
    hintRu: "Только закреплённые сообщения",
    example: "pinned:true",
  }),
]);

/**
 * Возвращает отфильтрованный список выпадающих подсказок токенов по текущему вводу.
 * Если строка пуста или оканчивается пробелом — возвращает все 7 подсказок.
 * Иначе сопоставляет последний незавершённый фрагмент с префиксом токена или русской подписью.
 *
 * @param {string | undefined} rawQuery
 * @returns {ReadonlyArray<Readonly<SearchTokenSuggestionItem>>}
 */
export function getSearchTokenSuggestions(rawQuery) {
  if (typeof rawQuery !== "string" || rawQuery.length === 0 || /\s$/.test(rawQuery)) {
    return SEARCH_POPOVER_TOKEN_SUGGESTIONS;
  }
  const parts = rawQuery.split(/\s+/);
  const trailing = (parts[parts.length - 1] ?? "").toLowerCase();
  if (trailing.length === 0) {
    return SEARCH_POPOVER_TOKEN_SUGGESTIONS;
  }
  return SEARCH_POPOVER_TOKEN_SUGGESTIONS.filter(
    (item) => item.token.startsWith(trailing) || item.labelRu.toLowerCase().startsWith(trailing),
  );
}

/**
 * @typedef {{
 *   type: "text" | "token",
 *   text: string,
 *   key?: string,
 *   prefix?: string,
 *   value?: string,
 *   negated?: boolean,
 * }} SearchInputHighlightSegment
 */

/**
 * Разбивает поисковую строку на сегменты свободного текста и активных токенов
 * (`from:`, `mentions:`, `has:`, `in:`, `before:`, `after:`, `pinned:` и их алиасы)
 * для визуальной подсветки токенов в строке ввода с сохранением исходного текста.
 *
 * @param {string | undefined} rawQuery
 * @returns {SearchInputHighlightSegment[]}
 */
export function tokenizeSearchInputForHighlight(rawQuery) {
  const input = typeof rawQuery === "string" ? rawQuery : "";
  if (input.length === 0) return [];

  const highlightTokenRegex =
    /(^|\s)(-?(?:с\s+упоминанием|закреплённые|authorType|author_type|ТипАвтора|mentions|before|during|pinned|after|имеет|from|день|has|от|in|в|до|после|on)):(?:"([^"]+)"|(\S+))/giu;

  /** @type {SearchInputHighlightSegment[]} */
  const segments = [];
  let cursor = 0;

  for (const match of input.matchAll(highlightTokenRegex)) {
    const leadingSpace = match[1] ?? "";
    const rawOp = (match[2] ?? "").trim();
    const rawVal = (match[3] ?? match[4] ?? "").trim();
    const tokenStart = (match.index ?? 0) + leadingSpace.length;
    const tokenEnd = (match.index ?? 0) + match[0].length;

    if (rawVal === "") continue;

    const opNegated = rawOp.startsWith("-");
    const opLower = (opNegated ? rawOp.slice(1) : rawOp).replace(/\s+/g, " ").toLowerCase();

    let canonicalKey = "";
    let canonicalPrefix = "";
    let cleanValue = rawVal;

    if (opLower === "from" || opLower === "от") {
      if (opNegated) continue;
      canonicalKey = "from";
      canonicalPrefix = "from:";
    } else if (opLower === "mentions" || opLower === "с упоминанием") {
      if (opNegated) continue;
      canonicalKey = "mentions";
      canonicalPrefix = "mentions:";
    } else if (opLower === "has" || opLower === "имеет") {
      canonicalKey = "has";
      canonicalPrefix = opNegated ? "-has:" : "has:";
    } else if (opLower === "in" || opLower === "в") {
      if (opNegated) continue;
      cleanValue = rawVal.replace(/^#+/, "");
      if (cleanValue === "") continue;
      canonicalKey = "in";
      canonicalPrefix = "in:";
    } else if (opLower === "before" || opLower === "до") {
      if (opNegated) continue;
      canonicalKey = "before";
      canonicalPrefix = "before:";
    } else if (opLower === "after" || opLower === "после") {
      if (opNegated) continue;
      canonicalKey = "after";
      canonicalPrefix = "after:";
    } else if (opLower === "pinned" || opLower === "закреплённые") {
      if (opNegated) continue;
      canonicalKey = "pinned";
      canonicalPrefix = "pinned:";
    } else if (opLower === "on" || opLower === "during" || opLower === "день") {
      if (opNegated) continue;
      canonicalKey = "on";
      canonicalPrefix = "on:";
    } else if (opLower === "authortype" || opLower === "author_type" || opLower === "типавтора") {
      canonicalKey = "authorType";
      canonicalPrefix = opNegated ? "-authorType:" : "authorType:";
    } else {
      continue;
    }

    if (tokenStart > cursor) {
      segments.push({
        type: "text",
        text: input.slice(cursor, tokenStart),
      });
    }

    segments.push({
      type: "token",
      text: input.slice(tokenStart, tokenEnd),
      key: canonicalKey,
      prefix: canonicalPrefix,
      value: cleanValue,
      negated: opNegated,
    });
    cursor = tokenEnd;
  }

  if (cursor < input.length) {
    segments.push({
      type: "text",
      text: input.slice(cursor),
    });
  }

  return segments;
}

/**
 * Экранирует спецсимволы регулярного выражения в пользовательской подстроке.
 * @param {string} value
 * @returns {string}
 */
function escapeRegExp(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

/**
 * Разбивает текст сообщения на фрагменты `{ text, match }` для подсветки поискового запроса.
 * Инлайн-токены фильтрации (`from:`, `in:` и др.) исключаются из подсветки текста сообщения.
 *
 * @param {string | undefined} content
 * @param {string | undefined} rawQuery
 * @returns {Array<{ text: string, match: boolean }>}
 */
export function splitTextBySearchQuery(content, rawQuery) {
  const source = typeof content === "string" ? content : "";
  if (source.length === 0) return [];

  const parsed = parseSearchQueryTokens(rawQuery);
  const freeText = parsed.freeText.trim();
  if (freeText.length === 0) {
    return [{ text: source, match: false }];
  }

  const terms = uniqueStrings(
    freeText
      .split(/\s+/)
      .map((t) => t.trim())
      .filter((t) => t.length > 0),
  ).sort((a, b) => b.length - a.length);

  if (terms.length === 0) {
    return [{ text: source, match: false }];
  }

  const pattern = new RegExp(`(${terms.map(escapeRegExp).join("|")})`, "giu");
  /** @type {Array<{ text: string, match: boolean }>} */
  const parts = [];
  let lastIndex = 0;

  for (const m of source.matchAll(pattern)) {
    const idx = m.index ?? 0;
    const matchedText = m[0] ?? "";
    if (matchedText.length === 0) continue;
    if (idx > lastIndex) {
      parts.push({ text: source.slice(lastIndex, idx), match: false });
    }
    parts.push({ text: matchedText, match: true });
    lastIndex = idx + matchedText.length;
  }

  if (lastIndex < source.length) {
    parts.push({ text: source.slice(lastIndex), match: false });
  }

  return parts.length > 0 ? parts : [{ text: source, match: false }];
}

/**
 * Возвращает фабрику DOM-элементов из контейнера или глобального документа.
 * @param {any} containerEl
 * @returns {{ createElement: (tag: string) => any }}
 */
function resolvePopoverDocument(containerEl) {
  if (
    containerEl &&
    containerEl.ownerDocument &&
    typeof containerEl.ownerDocument.createElement === "function"
  ) {
    return containerEl.ownerDocument;
  }
  if (containerEl && typeof containerEl.createElement === "function") {
    return containerEl;
  }
  const globalDoc = /** @type {any} */ (globalThis).document;
  if (globalDoc && typeof globalDoc.createElement === "function") {
    return globalDoc;
  }
  throw new TypeError("containerEl must provide ownerDocument.createElement");
}

/**
 * Заменяет дочерние узлы элемента без использования `innerHTML`.
 * @param {any} targetEl
 * @param {any[]} nodes
 */
function replacePopoverChildren(targetEl, nodes) {
  if (typeof targetEl.replaceChildren === "function") {
    targetEl.replaceChildren(...nodes);
    return;
  }
  if (Array.isArray(targetEl.children)) {
    for (const child of targetEl.children) {
      if (child && typeof child === "object") {
        child.parentNode = null;
      }
    }
    targetEl.children.length = 0;
  }
  for (const node of nodes) {
    targetEl.appendChild(node);
  }
}

/**
 * @typedef {{
 *   id: string,
 *   channelId: string,
 *   channelName?: string,
 *   author?: string,
 *   content?: string,
 *   timestamp?: string,
 * }} SearchPopoverResultItem
 */

/**
 * @typedef {{
 *   initialQuery?: string,
 *   onSearch?: (payload: { query: string, freeText: string, parsedQuery: Record<string, unknown> }) => void,
 *   onJumpToMessage?: (messageId: string, channelId: string, item: SearchPopoverResultItem) => void,
 *   onClose?: () => void,
 * }} SearchPopoverOptions
 */

/**
 * Создаёт UI-компонент поискового попапа с выпадающими подсказками токенов,
 * подсветкой активных токенов в строке ввода, списком результатов с подсветкой
 * поискового запроса, прыжком к сообщению (`onJumpToMessage`) и клавиатурной
 * навигацией (`ArrowUp`, `ArrowDown`, `Enter`, `Escape`).
 *
 * @param {any} containerEl
 * @param {SearchPopoverOptions} [options]
 */
export function createSearchPopover(containerEl, options = {}) {
  const doc = resolvePopoverDocument(containerEl);

  let isOpen = true;
  let query = typeof options.initialQuery === "string" ? options.initialQuery : "";
  /** @type {SearchPopoverResultItem[]} */
  let results = [];
  let totalResults = 0;
  let activeResultIndex = -1;
  let activeSuggestionIndex = 0;

  const root = doc.createElement("section");
  root.setAttribute("class", "search-popover");
  root.setAttribute("data-search-popover", "");
  root.setAttribute("role", "search");
  root.setAttribute("aria-label", "Поиск сообщений");
  root.setAttribute("data-open", "true");
  root.setAttribute("aria-hidden", "false");

  const barEl = doc.createElement("div");
  barEl.setAttribute("class", "search-popover-bar");
  barEl.setAttribute("data-search-popover-bar", "");

  const input = doc.createElement("input");
  input.setAttribute("type", "search");
  input.setAttribute("class", "search-popover-input search-input");
  input.setAttribute("data-search-popover-input", "");
  input.setAttribute("placeholder", "Поиск");
  input.setAttribute("aria-label", "Поиск сообщений");
  input.setAttribute("autocomplete", "off");
  input.value = query;

  const highlightBar = doc.createElement("div");
  highlightBar.setAttribute("class", "search-popover-input-highlight");
  highlightBar.setAttribute("data-search-input-highlight", "");
  highlightBar.setAttribute("aria-hidden", "true");

  barEl.appendChild(input);
  barEl.appendChild(highlightBar);

  const suggestionsList = doc.createElement("div");
  suggestionsList.setAttribute("class", "search-popover-suggestions");
  suggestionsList.setAttribute("role", "listbox");
  suggestionsList.setAttribute("aria-label", "Подсказки фильтров поиска");
  suggestionsList.setAttribute("data-search-token-suggestions", "");

  const headerEl = doc.createElement("div");
  headerEl.setAttribute("class", "search-popover-header");
  headerEl.setAttribute("data-search-results-header", "");
  headerEl.setAttribute("aria-live", "polite");

  const resultsList = doc.createElement("div");
  resultsList.setAttribute("class", "search-popover-results");
  resultsList.setAttribute("role", "listbox");
  resultsList.setAttribute("aria-label", "Результаты поиска");
  resultsList.setAttribute("data-search-popover-results", "");

  root.appendChild(barEl);
  root.appendChild(suggestionsList);
  root.appendChild(headerEl);
  root.appendChild(resultsList);
  replacePopoverChildren(containerEl, [root]);

  function renderInputHighlight() {
    const segments = tokenizeSearchInputForHighlight(query);
    /** @type {any[]} */
    const nodes = [];
    for (const seg of segments) {
      if (seg.type === "token") {
        const mark = doc.createElement("mark");
        mark.setAttribute("class", "search-input-token");
        mark.setAttribute("data-active-token", String(seg.key ?? ""));
        mark.setAttribute("data-token-prefix", String(seg.prefix ?? ""));
        mark.setAttribute("data-token-value", String(seg.value ?? ""));

        const prefixSpan = doc.createElement("span");
        prefixSpan.setAttribute("class", "search-chip-prefix");
        prefixSpan.textContent = String(seg.prefix ?? "");

        const valueSpan = doc.createElement("span");
        valueSpan.setAttribute("class", "search-chip-value");
        valueSpan.textContent = String(seg.value ?? "");

        mark.appendChild(prefixSpan);
        mark.appendChild(valueSpan);
        nodes.push(mark);
      } else if (seg.text.trim().length > 0) {
        const textSpan = doc.createElement("span");
        textSpan.setAttribute("class", "search-input-free-text");
        textSpan.textContent = seg.text;
        nodes.push(textSpan);
      }
    }
    replacePopoverChildren(highlightBar, nodes);
  }

  function renderSuggestions() {
    const items = getSearchTokenSuggestions(query);
    if (activeSuggestionIndex >= items.length) {
      activeSuggestionIndex = 0;
    }
    /** @type {any[]} */
    const nodes = [];
    items.forEach((item, idx) => {
      const btn = doc.createElement("button");
      btn.setAttribute("type", "button");
      btn.setAttribute("role", "option");
      btn.setAttribute("class", "search-popover-suggestion-item");
      btn.setAttribute("data-token-suggestion", item.token);
      btn.setAttribute("aria-selected", idx === activeSuggestionIndex ? "true" : "false");

      const tokenSpan = doc.createElement("span");
      tokenSpan.setAttribute("class", "search-suggestion-token");
      tokenSpan.textContent = item.token;

      const labelSpan = doc.createElement("span");
      labelSpan.setAttribute("class", "search-suggestion-label");
      labelSpan.textContent = item.labelRu;

      btn.appendChild(tokenSpan);
      btn.appendChild(labelSpan);
      btn.addEventListener("click", () => {
        applyTokenSuggestion(item.token);
      });
      nodes.push(btn);
    });
    replacePopoverChildren(suggestionsList, nodes);
  }

  function renderResults() {
    if (totalResults > 0) {
      headerEl.textContent = `Найдено результатов: ${totalResults}`;
    } else {
      headerEl.textContent = "Нет результатов";
    }
    headerEl.setAttribute("data-total-results", String(totalResults));

    /** @type {any[]} */
    const nodes = [];
    results.forEach((item, idx) => {
      const msgId = String(item.id ?? "");
      const channelId = String(item.channelId ?? "");
      const isSelected = idx === activeResultIndex;

      const card = doc.createElement("article");
      card.setAttribute("id", `search-result-${msgId}`);
      card.setAttribute("role", "option");
      card.setAttribute(
        "class",
        isSelected ? "search-popover-result-item is-active" : "search-popover-result-item",
      );
      card.setAttribute("data-search-result-id", msgId);
      card.setAttribute("data-channel-id", channelId);
      card.setAttribute("aria-selected", isSelected ? "true" : "false");

      const head = doc.createElement("div");
      head.setAttribute("class", "search-popover-result-head");

      const authorEl = doc.createElement("span");
      authorEl.setAttribute("class", "search-popover-result-author");
      authorEl.setAttribute("data-result-author", "");
      authorEl.textContent = typeof item.author === "string" ? item.author : "";
      head.appendChild(authorEl);

      if (typeof item.channelName === "string" && item.channelName.length > 0) {
        const channelEl = doc.createElement("span");
        channelEl.setAttribute("class", "search-popover-result-channel");
        channelEl.setAttribute("data-result-channel", "");
        channelEl.textContent = `#${item.channelName.replace(/^#+/, "")}`;
        head.appendChild(channelEl);
      }

      if (typeof item.timestamp === "string" && item.timestamp.length > 0) {
        const timeEl = doc.createElement("span");
        timeEl.setAttribute("class", "search-popover-result-timestamp");
        timeEl.setAttribute("data-result-timestamp", "");
        timeEl.textContent = item.timestamp;
        head.appendChild(timeEl);
      }

      const jumpBtn = doc.createElement("button");
      jumpBtn.setAttribute("type", "button");
      jumpBtn.setAttribute("class", "search-popover-jump-btn");
      jumpBtn.setAttribute("data-action", "jump-to-message");
      jumpBtn.setAttribute("data-jump-message-id", msgId);
      jumpBtn.setAttribute("data-jump-channel-id", channelId);
      jumpBtn.textContent = "Перейти";
      jumpBtn.addEventListener("click", () => {
        triggerJump(item);
      });
      head.appendChild(jumpBtn);

      const contentEl = doc.createElement("p");
      contentEl.setAttribute("class", "search-popover-result-content");
      contentEl.setAttribute("data-result-content", "");

      const textParts = splitTextBySearchQuery(item.content ?? "", query);
      /** @type {any[]} */
      const contentNodes = [];
      for (const part of textParts) {
        if (part.match) {
          const mark = doc.createElement("mark");
          mark.setAttribute("class", "search-highlight");
          mark.setAttribute("data-search-highlight", "");
          mark.textContent = part.text;
          contentNodes.push(mark);
        } else {
          const span = doc.createElement("span");
          span.textContent = part.text;
          contentNodes.push(span);
        }
      }
      replacePopoverChildren(contentEl, contentNodes);

      card.appendChild(head);
      card.appendChild(contentEl);
      nodes.push(card);
    });

    replacePopoverChildren(resultsList, nodes);

    const activeItem =
      activeResultIndex >= 0 && activeResultIndex < results.length
        ? results[activeResultIndex]
        : null;
    if (activeItem) {
      input.setAttribute("aria-activedescendant", `search-result-${String(activeItem.id ?? "")}`);
    } else {
      input.removeAttribute("aria-activedescendant");
    }
  }

  /**
   * @param {SearchPopoverResultItem} item
   */
  function triggerJump(item) {
    if (typeof options.onJumpToMessage === "function") {
      options.onJumpToMessage(String(item.id ?? ""), String(item.channelId ?? ""), item);
    }
  }

  function emitSearch() {
    const parsedTokens = parseSearchQueryTokens(query);
    const parsedQuery = buildSearchQuery({}, { content: query });
    if (typeof options.onSearch === "function") {
      options.onSearch({
        query,
        freeText: parsedTokens.freeText,
        parsedQuery,
      });
    }
  }

  /**
   * Подставляет выбранный токен подсказки в конец строки поиска (заменяя вводимый префикс).
   * @param {string} token
   */
  function applyTokenSuggestion(token) {
    const cleanToken = String(token ?? "").trim();
    if (cleanToken === "") return;

    if (query.length === 0 || /\s$/.test(query)) {
      setQuery(`${query}${cleanToken}`);
      return;
    }

    const lastSpaceIdx = query.search(/\S+$/);
    const prefixBefore = lastSpaceIdx > 0 ? query.slice(0, lastSpaceIdx) : "";
    setQuery(`${prefixBefore}${cleanToken}`);
  }

  /**
   * Устанавливает поисковый запрос, обновляет подсветку токенов, подсказки и подсветку результатов.
   * @param {string} nextQuery
   */
  function setQuery(nextQuery) {
    query = typeof nextQuery === "string" ? nextQuery : "";
    input.value = query;
    activeResultIndex = -1;
    if (!isOpen) {
      open();
    }
    renderInputHighlight();
    renderSuggestions();
    renderResults();
    emitSearch();
  }

  /**
   * Устанавливает список результатов поиска и перерисовывает выдачу.
   * @param {any} payload
   */
  function setResults(payload) {
    if (Array.isArray(payload)) {
      results = payload.map((/** @type {SearchPopoverResultItem} */ item) => ({ ...item }));
      totalResults = results.length;
    } else if (payload && typeof payload === "object") {
      /** @type {SearchPopoverResultItem[]} */
      const list = Array.isArray(payload.results)
        ? payload.results
        : Array.isArray(payload.hits)
          ? payload.hits
          : [];
      results = list.map((/** @type {SearchPopoverResultItem} */ item) => ({ ...item }));
      const rawTotal = payload.totalResults ?? payload.totalCount;
      totalResults =
        typeof rawTotal === "number" && Number.isFinite(rawTotal) && rawTotal >= 0
          ? Math.trunc(rawTotal)
          : results.length;
    } else {
      results = [];
      totalResults = 0;
    }
    activeResultIndex = -1;
    renderResults();
  }

  function open() {
    isOpen = true;
    root.setAttribute("data-open", "true");
    root.setAttribute("aria-hidden", "false");
  }

  function close() {
    isOpen = false;
    activeResultIndex = -1;
    root.setAttribute("data-open", "false");
    root.setAttribute("aria-hidden", "true");
    renderResults();
    if (typeof options.onClose === "function") {
      options.onClose();
    }
  }

  /**
   * Обрабатывает навигацию по результатам стрелками (`ArrowDown`, `ArrowUp`),
   * переход к выбранному сообщению по `Enter` и закрытие попапа по `Escape`.
   * @param {any} event
   */
  function handleKeyDown(event) {
    const key = event?.key;
    if (key === "Escape") {
      event?.preventDefault?.();
      close();
      return;
    }

    if (key === "ArrowDown") {
      event?.preventDefault?.();
      if (results.length > 0) {
        activeResultIndex =
          activeResultIndex < 0 ? 0 : (activeResultIndex + 1) % results.length;
        renderResults();
      } else {
        const suggestions = getSearchTokenSuggestions(query);
        if (suggestions.length > 0) {
          activeSuggestionIndex = (activeSuggestionIndex + 1) % suggestions.length;
          renderSuggestions();
        }
      }
      return;
    }

    if (key === "ArrowUp") {
      event?.preventDefault?.();
      if (results.length > 0) {
        activeResultIndex =
          activeResultIndex <= 0 ? results.length - 1 : activeResultIndex - 1;
        renderResults();
      } else {
        const suggestions = getSearchTokenSuggestions(query);
        if (suggestions.length > 0) {
          activeSuggestionIndex =
            activeSuggestionIndex <= 0 ? suggestions.length - 1 : activeSuggestionIndex - 1;
          renderSuggestions();
        }
      }
      return;
    }

    if (key === "Enter") {
      if (activeResultIndex >= 0 && activeResultIndex < results.length) {
        event?.preventDefault?.();
        const activeHit = results[activeResultIndex];
        if (activeHit) {
          triggerJump(activeHit);
        }
      } else if (results.length === 0) {
        const suggestions = getSearchTokenSuggestions(query);
        const chosen = suggestions[activeSuggestionIndex];
        if (chosen && query.trim().length > 0 && !query.endsWith(":")) {
          event?.preventDefault?.();
          applyTokenSuggestion(chosen.token);
        }
      }
    }
  }

  input.addEventListener("input", (/** @type {any} */ event) => {
    const nextVal = event?.target?.value ?? input.value ?? "";
    setQuery(String(nextVal));
  });

  input.addEventListener("keydown", (/** @type {any} */ event) => {
    handleKeyDown(event);
  });

  renderInputHighlight();
  renderSuggestions();
  renderResults();

  return {
    root,
    input,
    setQuery,
    setResults,
    applyTokenSuggestion,
    handleKeyDown,
    open,
    close,
    getState() {
      const parsed = parseSearchQueryTokens(query);
      return {
        isOpen,
        query,
        freeText: parsed.freeText,
        activeTokens: tokenizeSearchInputForHighlight(query).filter((s) => s.type === "token"),
        suggestions: [...getSearchTokenSuggestions(query)],
        results: results.map((r) => ({ ...r })),
        totalResults,
        activeResultIndex,
        activeSuggestionIndex,
      };
    },
  };
}

export const createSearchPopoverController = createSearchPopover;

