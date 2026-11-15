// @ts-check
/// <reference lib="dom" />
/**
 * TAURI-MD-01 (RW-49): модульный парсер и рендерер упоминаний Discord (`<@id>`, `<@!id>`,
 * `<#id>`, `<@&id>`, `@everyone`, `@here`), выделенный из `markdown.js`.
 *
 * Контракт:
 * - Максимальная длина snowflake ID в упоминании: `MAX_MENTION_ID = 32` цифры;
 * - Роль (`<@&id>`) проверяется раньше пользователя (`<@id>`/`<@!id>`), чтобы `<@&789>`
 *   не распозналось как некорректное упоминание пользователя;
 * - `@everyone` и `@here` распознаются только целым словом (с учётом Unicode-границы слова:
 *   `почта@everyone.com`, `user@here.org`, `@everyoneX` остаются обычным текстом);
 * - Неизвестный идентификатор получает безопасный русский фоллбэк:
 *   `@пользователь`, `#канал`, `@роль`;
 * - Безопасность (P0): имена из `context` (пользователи/каналы/роли) считаются недоверенными
 *   данными из сети и подставляются в DOM исключительно через `textContent`.
 */

/** @typedef {{ type: "user_mention", id: string, label: string }} UserMentionToken */
/** @typedef {{ type: "channel_mention", id: string, label: string }} ChannelMentionToken */
/** @typedef {{ type: "role_mention", id: string, label: string }} RoleMentionToken */
/** @typedef {{ type: "everyone" | "here", label: string }} BroadcastToken */
/** @typedef {UserMentionToken | ChannelMentionToken | RoleMentionToken} EntityMentionToken */
/** @typedef {EntityMentionToken | BroadcastToken} MentionToken */

/**
 * Справочники имён для разрешения упоминаний. Значения — произвольные строки из Discord,
 * поэтому считаются недоверенными (вставляются только как текст).
 * @typedef {{
 *   users?: Record<string, string> | Map<string, string>,
 *   channels?: Record<string, string> | Map<string, string>,
 *   roles?: Record<string, string> | Map<string, string>,
 * }} MentionContext
 */

/**
 * Нормализованный контекст упоминаний с плоскими словарями `id -> name`.
 * @typedef {{
 *   users: Record<string, string>,
 *   channels: Record<string, string>,
 *   roles: Record<string, string>,
 * }} NormalizedMentionContext
 */

/** Максимальная длина id упоминания (Discord snowflake — 64 бита, до 32 десятичных цифр с запасом). */
export const MAX_MENTION_ID = 32;

/**
 * Дефолтные русские подписи для неизвестных сущностей.
 * @type {Readonly<{ USER: string, CHANNEL: string, ROLE: string }>}
 */
export const DEFAULT_MENTION_FALLBACKS = Object.freeze({
  USER: "@пользователь",
  CHANNEL: "#канал",
  ROLE: "@роль",
});

/**
 * Регулярное выражение угловых упоминаний Discord в начале среза:
 * 1) `<@&role_id>` — роль (проверяется первой),
 * 2) `<@user_id>` или `<@!user_id>` — пользователь,
 * 3) `<#channel_id>` — канал.
 */
export const MENTION_RE = new RegExp(
  `^(?:<@&(\\d{1,${MAX_MENTION_ID}})>|<@!?(\\d{1,${MAX_MENTION_ID}})>|<#(\\d{1,${MAX_MENTION_ID}})>)`,
);

/** Регулярное выражение широковещательных упоминаний `@everyone` и `@here` в начале среза. */
export const BROADCAST_RE = /^@(everyone|here)\b/;

/** Символы, которые не могут стоять непосредственно перед `@everyone` / `@here`. */
const WORD_PRECEDING_RE = /[\p{L}\p{N}_@#]/u;

/**
 * Преобразует словарь или `Map<string, string>` в плоский объект `Record<string, string>`,
 * отбрасывая нестроковые значения.
 * @param {Record<string, string> | Map<string, string> | null | undefined} source
 * @returns {Record<string, string>}
 */
export function toMentionRecord(source) {
  if (!source || typeof source !== "object") return {};
  if (source instanceof Map) {
    /** @type {Record<string, string>} */
    const record = {};
    for (const [key, value] of source) {
      if (typeof key === "string" && typeof value === "string") {
        record[key] = value;
      }
    }
    return record;
  }
  /** @type {Record<string, string>} */
  const out = {};
  for (const [key, value] of Object.entries(source)) {
    if (typeof value === "string") {
      out[key] = value;
    }
  }
  return out;
}

/**
 * Нормализует пользовательский `MentionContext` в структуру со словарями `users`, `channels`, `roles`.
 * @param {MentionContext | null | undefined} [context]
 * @returns {NormalizedMentionContext}
 */
export function normalizeMentionContext(context) {
  return {
    users: toMentionRecord(context?.users),
    channels: toMentionRecord(context?.channels),
    roles: toMentionRecord(context?.roles),
  };
}

/**
 * Формирует видимую метку упоминания (`@имя`, `#канал`, `@роль`) или возвращает `fallback`,
 * если имя отсутствует либо пустое.
 * @param {Record<string, string> | null | undefined} source
 * @param {string} id
 * @param {string} prefix
 * @param {string} fallback
 * @returns {string}
 */
export function formatMentionLabel(source, id, prefix, fallback) {
  const name = source && typeof source === "object" ? source[id] : undefined;
  return typeof name === "string" && name !== "" ? `${prefix}${name}` : fallback;
}

/**
 * Пытается прочитать угловое упоминание (`<@id>`, `<@!id>`, `<#id>`, `<@&id>`)
 * с позиции `start` в строке `text`.
 *
 * @param {string} text
 * @param {number} start
 * @param {MentionContext | NormalizedMentionContext} [context]
 * @returns {{ token: EntityMentionToken, end: number } | null}
 */
export function readMention(text, start, context) {
  if (typeof text !== "string" || start < 0 || start >= text.length) return null;
  if (text.charAt(start) !== "<") return null;

  const match = MENTION_RE.exec(text.slice(start));
  if (!match) return null;

  const users = context?.users instanceof Map ? toMentionRecord(context.users) : (context?.users ?? {});
  const channels = context?.channels instanceof Map ? toMentionRecord(context.channels) : (context?.channels ?? {});
  const roles = context?.roles instanceof Map ? toMentionRecord(context.roles) : (context?.roles ?? {});

  const end = start + match[0].length;
  const roleId = match[1];
  const userId = match[2];
  const channelId = match[3];

  if (roleId) {
    return {
      token: {
        type: "role_mention",
        id: roleId,
        label: formatMentionLabel(roles, roleId, "@", DEFAULT_MENTION_FALLBACKS.ROLE),
      },
      end,
    };
  }
  if (userId) {
    return {
      token: {
        type: "user_mention",
        id: userId,
        label: formatMentionLabel(users, userId, "@", DEFAULT_MENTION_FALLBACKS.USER),
      },
      end,
    };
  }
  if (channelId) {
    return {
      token: {
        type: "channel_mention",
        id: channelId,
        label: formatMentionLabel(channels, channelId, "#", DEFAULT_MENTION_FALLBACKS.CHANNEL),
      },
      end,
    };
  }
  return null;
}

/**
 * Пытается прочитать широковещательное упоминание (`@everyone` или `@here`)
 * с позиции `start` в строке `text`.
 *
 * Учитывает Unicode-границу слова перед `@`: `почта@everyone.com` и `@everyone123`
 * не являются упоминаниями и возвращают `null`.
 *
 * @param {string} text
 * @param {number} start
 * @returns {{ token: BroadcastToken, end: number } | null}
 */
export function readBroadcast(text, start) {
  if (typeof text !== "string" || start < 0 || start >= text.length) return null;
  if (text.charAt(start) !== "@") return null;
  if (start > 0 && WORD_PRECEDING_RE.test(text.charAt(start - 1))) return null;

  const match = BROADCAST_RE.exec(text.slice(start));
  if (!match) return null;

  const name = match[1];
  if (name !== "everyone" && name !== "here") return null;

  return {
    token: { type: name, label: `@${name}` },
    end: start + match[0].length,
  };
}

/**
 * Пытается прочитать любое упоминание Discord (`<@id>`, `<@!id>`, `<#id>`, `<@&id>`,
 * `@everyone`, `@here`) с позиции `start` в строке `text`.
 *
 * @param {string} text
 * @param {number} start
 * @param {MentionContext | NormalizedMentionContext} [context]
 * @returns {{ token: MentionToken, end: number } | null}
 */
export function readAnyMention(text, start, context) {
  if (typeof text !== "string" || start < 0 || start >= text.length) return null;
  const ch = text.charAt(start);
  if (ch === "<") return readMention(text, start, context);
  if (ch === "@") return readBroadcast(text, start);
  return null;
}

/**
 * Извлекает все упоминания из строки `text` (в порядке следования),
 * пропуская содержимое инлайн-кода (`` `...` ``) и блоков кода (` ```...``` `).
 *
 * @param {string} text
 * @param {MentionContext} [context]
 * @returns {MentionToken[]}
 */
export function extractMentions(text, context = {}) {
  if (typeof text !== "string") {
    throw new TypeError("extractMentions: text должен быть строкой");
  }
  const normalized = normalizeMentionContext(context);
  /** @type {MentionToken[]} */
  const result = [];
  let index = 0;

  while (index < text.length) {
    const ch = text.charAt(index);

    // Пропускаем обратные кавычки (инлайн-код и fenced-блоки), чтобы не извлекать упоминания из кода
    if (ch === "`") {
      let fenceLen = 0;
      while (text.charAt(index + fenceLen) === "`") fenceLen += 1;
      const contentStart = index + fenceLen;
      let cursor = contentStart;
      let closed = false;
      while (cursor < text.length) {
        if (text.charAt(cursor) !== "`") {
          cursor += 1;
          continue;
        }
        let run = 0;
        while (text.charAt(cursor + run) === "`") run += 1;
        if (run === fenceLen) {
          index = cursor + run;
          closed = true;
          break;
        }
        cursor += run;
      }
      if (closed) continue;
    }

    const found = readAnyMention(text, index, normalized);
    if (found) {
      result.push(found.token);
      index = found.end;
      continue;
    }

    index += 1;
  }

  return result;
}

/**
 * Создаёт безопасный DOM-элемент упоминания (`span.mention`) с использованием `textContent`.
 *
 * - Для `user_mention`: `<span class="mention mention--user" data-user-id="...">@имя</span>`
 * - Для `channel_mention`: `<span class="mention mention--channel" data-channel-id="...">#канал</span>`
 * - Для `role_mention`: `<span class="mention mention--role" data-role-id="...">@роль</span>`
 * - Для `everyone` / `here`: `<span class="mention mention--everyone" data-mention="everyone">@everyone</span>`
 *
 * @param {Document} doc
 * @param {MentionToken} token
 * @returns {HTMLElement}
 */
export function createMentionElement(doc, token) {
  if (!doc || typeof doc.createElement !== "function") {
    throw new TypeError("createMentionElement: нужен объект Document с методом createElement");
  }
  switch (token.type) {
    case "user_mention": {
      const node = doc.createElement("span");
      node.className = "mention mention--user";
      node.setAttribute("data-user-id", token.id);
      node.textContent = token.label;
      return node;
    }
    case "channel_mention": {
      const node = doc.createElement("span");
      node.className = "mention mention--channel";
      node.setAttribute("data-channel-id", token.id);
      node.textContent = token.label;
      return node;
    }
    case "role_mention": {
      const node = doc.createElement("span");
      node.className = "mention mention--role";
      node.setAttribute("data-role-id", token.id);
      node.textContent = token.label;
      return node;
    }
    case "everyone":
    case "here": {
      const node = doc.createElement("span");
      node.className = `mention mention--${token.type}`;
      node.setAttribute("data-mention", token.type);
      node.textContent = token.label;
      return node;
    }
  }
}
