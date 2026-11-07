// @ts-check
/// <reference lib="dom" />
/**
 * M-08 (RW-40): безопасный markdown и упоминания сообщений Discord.
 *
 * Контракт (заморожен тестами `tests/unit/gui-spike-model.test.mjs`, describe «M-08»):
 *   parseMessageMarkdown(text, context) -> Token[]      — чистый разбор, без DOM;
 *   renderMessageMarkdown(container, text, context)     — сборка DOM.
 *
 * Безопасность (P0):
 * - сырой HTML никогда не разбирается: текст сообщения попадает в DOM только через
 *   `textContent`/`createTextNode`, разметка не собирается строками и не пишется
 *   в разбор HTML-парсером;
 * - ссылкой становится только абсолютный http(s)-URL (проверка через `new URL()`),
 *   поэтому `javascript:`, `data:`, `vbscript:`, `file:` и `//host` остаются текстом;
 * - у ссылки всегда `rel="noopener noreferrer"` и `target="_blank"`;
 * - имена из `context` (пользователи/каналы/роли) — тоже недоверенные данные:
 *   подставляются только как текст;
 * - внутри инлайн-кода и блоков кода markdown и упоминания не разбираются.
 *
 * Упоминания: `<@123>`/`<@!123>` → `@имя` + `data-user-id`, `<#456>` → `#имя` +
 * `data-channel-id`, `<@&789>` → `@имя` + `data-role-id`, `@everyone`/`@here` —
 * целым словом. Неизвестный id → `@пользователь` / `#канал` / `@роль`.
 */

/** @typedef {{ type: "text", value: string }} TextToken */
/** @typedef {{ type: "code", value: string }} CodeToken */
/** @typedef {{ type: "code_block", value: string, language: string }} CodeBlockToken */
/** @typedef {{ type: "bold" | "italic" | "strikethrough" | "spoiler", children: InlineToken[] }} MarkToken */
/** @typedef {{ type: "quote", children: InlineToken[] }} QuoteToken */
/** @typedef {{ type: "link", href: string, children: InlineToken[] }} LinkToken */
/** @typedef {{ type: "user_mention", id: string, label: string }} UserMentionToken */
/** @typedef {{ type: "channel_mention", id: string, label: string }} ChannelMentionToken */
/** @typedef {{ type: "role_mention", id: string, label: string }} RoleMentionToken */
/** @typedef {{ type: "everyone" | "here", label: string }} BroadcastToken */
/**
 * @typedef {TextToken | CodeToken | MarkToken | LinkToken | UserMentionToken | ChannelMentionToken | RoleMentionToken | BroadcastToken} InlineToken
 */
/** @typedef {InlineToken | CodeBlockToken | QuoteToken} Token */

/**
 * Справочники имён для упоминаний. Значения — произвольные строки из Discord,
 * поэтому считаются недоверенными (только текст).
 * @typedef {{
 *   users?: Record<string, string> | Map<string, string>,
 *   channels?: Record<string, string> | Map<string, string>,
 *   roles?: Record<string, string> | Map<string, string>,
 * }} MentionContext
 */

/** @typedef {{ users: Record<string, string>, channels: Record<string, string>, roles: Record<string, string> }} NormalizedContext */

/** Только эти схемы допустимы в `href` (M-08, P0). */
const SAFE_PROTOCOLS = Object.freeze(["http:", "https:"]);

/** Предел вложенности инлайн-форматирования: защита от патологического ввода. */
const MAX_INLINE_DEPTH = 8;

/** Максимальная длина id упоминания (Discord snowflake — 64 бита). */
const MAX_MENTION_ID = 32;

/** Роль раньше пользователя: `<@&789>` не должно распознаться как `<@…>`. */
const MENTION_RE = new RegExp(
  `^(?:<@&(\\d{1,${MAX_MENTION_ID}})>|<@!?(\\d{1,${MAX_MENTION_ID}})>|<#(\\d{1,${MAX_MENTION_ID}})>)`,
);

const BROADCAST_RE = /^@(everyone|here)\b/;

/** Открывающая строка блока кода: ``` или ```js. */
const FENCE_RE = /^\s*```([A-Za-z0-9_+-]*)\s*$/;

/** Закрывающая строка блока кода. */
const CLOSING_FENCE_RE = /^\s*```\s*$/;

/** Строка цитаты: `> текст`. */
const QUOTE_RE = /^\s*>\s?(.*)$/;

/** @type {ReadonlyArray<readonly [string, "bold" | "italic" | "strikethrough" | "spoiler"]>} */
const MARKERS = [
  ["**", "bold"],
  ["~~", "strikethrough"],
  ["||", "spoiler"],
  ["*", "italic"],
];

/**
 * @param {Record<string, string> | Map<string, string> | undefined} source
 * @returns {Record<string, string>}
 */
function toRecord(source) {
  if (!source) return {};
  if (source instanceof Map) {
    /** @type {Record<string, string>} */
    const record = {};
    for (const [key, value] of source) {
      if (typeof value === "string") record[key] = value;
    }
    return record;
  }
  return source;
}

/**
 * Безопасный `href`: только абсолютный http(s). Всё прочее — `null`.
 * @param {string} value
 * @returns {string | null}
 */
function safeHref(value) {
  const raw = value.trim();
  if (raw === "") return null;
  /** @type {URL} */
  let url;
  try {
    url = new URL(raw);
  } catch {
    // Относительный/битый адрес (`//host`, `example.com`) не навигируется:
    // в спайке без сети ссылка должна быть только явной и абсолютной.
    return null;
  }
  return SAFE_PROTOCOLS.includes(url.protocol) ? url.href : null;
}

/**
 * @param {Record<string, string>} source
 * @param {string} id
 * @param {string} prefix
 * @param {string} fallback
 * @returns {string}
 */
function mentionLabel(source, id, prefix, fallback) {
  const name = source[id];
  return typeof name === "string" && name !== "" ? `${prefix}${name}` : fallback;
}

/**
 * Инлайн-код: обратные кавычки одинаковой длины открывают и закрывают фрагмент.
 * @param {string} text
 * @param {number} start
 * @returns {{ value: string, end: number } | null}
 */
function readCodeSpan(text, start) {
  let length = 0;
  while (text.charAt(start + length) === "`") length += 1;
  const contentStart = start + length;
  let cursor = contentStart;
  while (cursor < text.length) {
    if (text.charAt(cursor) !== "`") {
      cursor += 1;
      continue;
    }
    let run = 0;
    while (text.charAt(cursor + run) === "`") run += 1;
    if (run === length) return { value: text.slice(contentStart, cursor), end: cursor + run };
    cursor += run;
  }
  return null;
}

/**
 * @param {string} text
 * @param {number} start
 * @param {NormalizedContext} context
 * @returns {{ token: InlineToken, end: number } | null}
 */
function readMention(text, start, context) {
  const match = MENTION_RE.exec(text.slice(start));
  if (!match) return null;
  const end = start + match[0].length;
  const roleId = match[1];
  const userId = match[2];
  const channelId = match[3];
  if (roleId) {
    return {
      token: { type: "role_mention", id: roleId, label: mentionLabel(context.roles, roleId, "@", "@роль") },
      end,
    };
  }
  if (userId) {
    return {
      token: {
        type: "user_mention",
        id: userId,
        label: mentionLabel(context.users, userId, "@", "@пользователь"),
      },
      end,
    };
  }
  if (channelId) {
    return {
      token: {
        type: "channel_mention",
        id: channelId,
        label: mentionLabel(context.channels, channelId, "#", "#канал"),
      },
      end,
    };
  }
  return null;
}

/**
 * `@everyone`/`@here` — только целым словом: `почта@everyone.com` и `@everyoneX`
 * остаются текстом.
 * @param {string} text
 * @param {number} start
 * @returns {{ token: BroadcastToken, end: number } | null}
 */
function readBroadcast(text, start) {
  // Граница слова с Unicode: «почта@everyone.com» не содержит упоминания,
  // хотя кириллическая «а» не входит в ASCII-класс \w.
  if (start > 0 && /[\p{L}\p{N}_@#]/u.test(text.charAt(start - 1))) return null;
  const match = BROADCAST_RE.exec(text.slice(start));
  if (!match) return null;
  const name = match[1];
  if (name !== "everyone" && name !== "here") return null;
  return { token: { type: name, label: `@${name}` }, end: start + match[0].length };
}

/**
 * `[подпись](адрес)`: ссылкой становится только проверенный http(s)-адрес.
 * Небезопасная ссылка остаётся исходной markdown-строкой (текстом), а не `<a>`.
 * @param {string} text
 * @param {number} start
 * @param {NormalizedContext} context
 * @param {number} depth
 * @returns {{ token: InlineToken, end: number } | null}
 */
function readLink(text, start, context, depth) {
  const labelEnd = text.indexOf("]", start + 1);
  if (labelEnd < 0) return null;
  if (text.charAt(labelEnd + 1) !== "(") return null;
  const hrefEnd = text.indexOf(")", labelEnd + 2);
  if (hrefEnd < 0) return null;
  const label = text.slice(start + 1, labelEnd);
  const source = text.slice(start, hrefEnd + 1);
  const href = safeHref(text.slice(labelEnd + 2, hrefEnd));
  if (label === "" || href === null) {
    return { token: { type: "text", value: source }, end: hrefEnd + 1 };
  }
  return {
    token: { type: "link", href, children: parseInline(label, context, depth + 1) },
    end: hrefEnd + 1,
  };
}

/**
 * Парные маркеры `**`, `~~`, `||`, `*`. Незакрытый маркер остаётся текстом.
 * @param {string} text
 * @param {number} start
 * @param {NormalizedContext} context
 * @param {number} depth
 * @returns {{ token: MarkToken, end: number } | null}
 */
function readMark(text, start, context, depth) {
  for (const [marker, type] of MARKERS) {
    if (!text.startsWith(marker, start)) continue;
    // Одиночная «*» не должна съедать начало «**».
    if (marker === "*" && text.startsWith("**", start)) continue;
    const contentStart = start + marker.length;
    let searchFrom = contentStart;
    for (;;) {
      const close = text.indexOf(marker, searchFrom);
      if (close < 0) break;
      if (marker === "*" && (text.charAt(close - 1) === "*" || text.charAt(close + 1) === "*")) {
        searchFrom = close + 1;
        continue;
      }
      // Пустое содержимое (`****`, `~~~~`) — литерал, а не формат.
      if (close === contentStart) {
        searchFrom = close + 1;
        continue;
      }
      return {
        token: { type, children: parseInline(text.slice(contentStart, close), context, depth + 1) },
        end: close + marker.length,
      };
    }
  }
  return null;
}

/**
 * Инлайн-разбор: код и упоминания имеют приоритет над форматированием, поэтому
 * внутри кода markdown не действует.
 * @param {string} text
 * @param {NormalizedContext} context
 * @param {number} depth
 * @returns {InlineToken[]}
 */
function parseInline(text, context, depth) {
  /** @type {InlineToken[]} */
  const tokens = [];
  let buffer = "";
  let index = 0;
  const flush = () => {
    if (buffer === "") return;
    tokens.push({ type: "text", value: buffer });
    buffer = "";
  };
  while (index < text.length) {
    const char = text.charAt(index);
    if (char === "`") {
      const span = readCodeSpan(text, index);
      if (span) {
        flush();
        tokens.push({ type: "code", value: span.value });
        index = span.end;
        continue;
      }
    }
    if (char === "<") {
      const mention = readMention(text, index, context);
      if (mention) {
        flush();
        tokens.push(mention.token);
        index = mention.end;
        continue;
      }
    }
    if (char === "@") {
      const broadcast = readBroadcast(text, index);
      if (broadcast) {
        flush();
        tokens.push(broadcast.token);
        index = broadcast.end;
        continue;
      }
    }
    if (char === "[") {
      const link = readLink(text, index, context, depth);
      if (link) {
        flush();
        tokens.push(link.token);
        index = link.end;
        continue;
      }
    }
    if (depth < MAX_INLINE_DEPTH && "*~|".includes(char)) {
      const mark = readMark(text, index, context, depth);
      if (mark) {
        flush();
        tokens.push(mark.token);
        index = mark.end;
        continue;
      }
    }
    buffer += char;
    index += 1;
  }
  flush();
  return tokens;
}

/**
 * Блочный разбор: блоки кода и цитаты, остальное — инлайн-поток с переносами.
 * @param {string} text
 * @param {NormalizedContext} context
 * @returns {Token[]}
 */
function parseBlocks(text, context) {
  const lines = text.split(/\r?\n/);
  /** @type {Token[]} */
  const tokens = [];
  /** @type {string[]} */
  let paragraph = [];
  const flushParagraph = () => {
    if (paragraph.length === 0) return;
    const value = paragraph.join("\n");
    paragraph = [];
    for (const token of parseInline(value, context, 0)) tokens.push(token);
  };
  let index = 0;
  while (index < lines.length) {
    const line = lines[index] ?? "";
    const fence = FENCE_RE.exec(line);
    if (fence) {
      flushParagraph();
      const language = fence[1] ?? "";
      /** @type {string[]} */
      const code = [];
      index += 1;
      while (index < lines.length) {
        const next = lines[index] ?? "";
        if (CLOSING_FENCE_RE.test(next)) {
          index += 1;
          break;
        }
        code.push(next);
        index += 1;
      }
      tokens.push({ type: "code_block", value: code.join("\n"), language });
      continue;
    }
    if (QUOTE_RE.test(line)) {
      flushParagraph();
      /** @type {string[]} */
      const quoted = [];
      while (index < lines.length) {
        const quote = QUOTE_RE.exec(lines[index] ?? "");
        if (!quote) break;
        quoted.push(quote[1] ?? "");
        index += 1;
      }
      tokens.push({ type: "quote", children: parseInline(quoted.join("\n"), context, 0) });
      continue;
    }
    if (line.trim() === "") {
      flushParagraph();
      index += 1;
      continue;
    }
    paragraph.push(line);
    index += 1;
  }
  flushParagraph();
  return tokens;
}

/**
 * Разбирает текст сообщения в токены. DOM не нужен.
 * @param {string} text
 * @param {MentionContext} [context]
 * @returns {Token[]}
 */
export function parseMessageMarkdown(text, context = {}) {
  if (typeof text !== "string") {
    throw new TypeError("parseMessageMarkdown: text должен быть строкой");
  }
  return parseBlocks(text, {
    users: toRecord(context?.users),
    channels: toRecord(context?.channels),
    roles: toRecord(context?.roles),
  });
}

/**
 * @param {Document} doc
 * @param {string} tagName
 * @param {string} className
 * @returns {HTMLElement}
 */
function element(doc, tagName, className) {
  const node = doc.createElement(tagName);
  node.className = className;
  return node;
}

/**
 * @param {Document} doc
 * @param {"user" | "channel" | "role"} kind
 * @param {string} id
 * @param {string} label
 * @returns {HTMLElement}
 */
function mentionNode(doc, kind, id, label) {
  const node = element(doc, "span", `mention mention--${kind}`);
  node.setAttribute(`data-${kind}-id`, id);
  node.textContent = label;
  return node;
}

/**
 * Спойлер скрыт до активации. Раскрытие — по клику и по Enter/Space
 * (элемент с `tabindex="0"`, поэтому доступен с клавиатуры).
 * @param {HTMLElement} node
 */
function configureSpoiler(node) {
  node.setAttribute("data-spoiler", "hidden");
  node.setAttribute("tabindex", "0");
  node.setAttribute("role", "button");
  node.setAttribute("aria-expanded", "false");
  node.addEventListener("click", () => revealSpoiler(node));
  node.addEventListener("keydown", (event) => {
    if (event.key === "Enter" || event.key === " " || event.key === "Spacebar") {
      event.preventDefault();
      revealSpoiler(node);
    }
  });
}

/**
 * @param {HTMLElement} node
 */
function revealSpoiler(node) {
  if (node.getAttribute("data-spoiler") !== "hidden") return;
  node.setAttribute("data-spoiler", "revealed");
  node.setAttribute("aria-expanded", "true");
}

/**
 * @param {Document} doc
 * @param {InlineToken[]} children
 * @param {HTMLElement} parent
 */
function renderChildren(doc, children, parent) {
  for (const child of children) renderToken(doc, child, parent);
}

/**
 * @param {Document} doc
 * @param {Token} token
 * @param {HTMLElement} parent
 */
function renderToken(doc, token, parent) {
  switch (token.type) {
    case "text":
      parent.append(doc.createTextNode(token.value));
      return;
    case "code": {
      const code = element(doc, "code", "md-code");
      code.textContent = token.value;
      parent.append(code);
      return;
    }
    case "code_block": {
      const pre = element(doc, "pre", "md-code-block");
      pre.setAttribute("data-language", token.language);
      const code = element(doc, "code", "md-code");
      code.textContent = token.value;
      pre.append(code);
      parent.append(pre);
      return;
    }
    case "bold": {
      const node = element(doc, "strong", "md-bold");
      renderChildren(doc, token.children, node);
      parent.append(node);
      return;
    }
    case "italic": {
      const node = element(doc, "em", "md-italic");
      renderChildren(doc, token.children, node);
      parent.append(node);
      return;
    }
    case "strikethrough": {
      const node = element(doc, "s", "md-strike");
      renderChildren(doc, token.children, node);
      parent.append(node);
      return;
    }
    case "spoiler": {
      const node = element(doc, "span", "md-spoiler");
      configureSpoiler(node);
      renderChildren(doc, token.children, node);
      parent.append(node);
      return;
    }
    case "quote": {
      const node = element(doc, "blockquote", "md-quote");
      renderChildren(doc, token.children, node);
      parent.append(node);
      return;
    }
    case "link": {
      const node = element(doc, "a", "md-link");
      // href уже проверен safeHref: только http(s), никаких javascript:/data:.
      node.setAttribute("href", token.href);
      node.setAttribute("target", "_blank");
      node.setAttribute("rel", "noopener noreferrer");
      renderChildren(doc, token.children, node);
      parent.append(node);
      return;
    }
    case "user_mention":
      parent.append(mentionNode(doc, "user", token.id, token.label));
      return;
    case "channel_mention":
      parent.append(mentionNode(doc, "channel", token.id, token.label));
      return;
    case "role_mention":
      parent.append(mentionNode(doc, "role", token.id, token.label));
      return;
    case "everyone":
    case "here": {
      const node = element(doc, "span", `mention mention--${token.type}`);
      node.setAttribute("data-mention", token.type);
      node.textContent = token.label;
      parent.append(node);
      return;
    }
  }
}

/**
 * Собирает DOM сообщения в `container`. Текст и данные `context` попадают только
 * через `textContent`/`createTextNode` — HTML-инъекция невозможна.
 * @param {HTMLElement} container
 * @param {string} text
 * @param {MentionContext} [context]
 * @returns {HTMLElement}
 */
export function renderMessageMarkdown(container, text, context = {}) {
  const doc = container?.ownerDocument;
  if (!container || typeof container.append !== "function" || !doc) {
    throw new TypeError("renderMessageMarkdown: нужен DOM-контейнер с ownerDocument");
  }
  for (const token of parseMessageMarkdown(text, context)) renderToken(doc, token, container);
  return container;
}
