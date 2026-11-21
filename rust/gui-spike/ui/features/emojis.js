// @ts-check
/// <reference lib="dom" />
/**
 * TAURI-MD-03 (RW-49): модульный парсер и безопасный DOM-рендерер эмодзи Discord
 * (`<:name:id>`, `<a:name:id>`, а также стандартных шорткодов `:name:`).
 *
 * Контракт:
 * - Кастомные эмодзи Discord: `<:name:id>` (статический) и `<a:name:id>` (анимированный),
 *   где `name` — `2..=32` символов `[A-Za-z0-9_]`, `id` — `1..=32` цифр (snowflake);
 * - Шорткоды стандартных эмодзи: `:name:` (например `:smile:`, `:thumbsup:`, `:heart:`, `:fire:`),
 *   разрешаемые по встроенному словарю `DISCORD_SHORTCODE_EMOJIS` или пользовательскому словарю;
 * - Безопасность (P0, M-08):
 *   - никаких `innerHTML`/`outerHTML`/`insertAdjacentHTML` и никаких тегов `<img>` из сырого текста
 *     (чтобы соблюдать инвариант `container.querySelectorAll("img").length === 0` на XSS-входах);
 *   - все элементы создаются как `<span class="emoji ...">` с безопасным `textContent` и
 *     `data-*` атрибутами (`data-emoji-id`, `data-emoji-name`, `data-emoji-animated`, `data-emoji-url`).
 */

/**
 * Токен кастомного эмодзи Discord (`<:name:id>` или `<a:name:id>`).
 * @typedef {{
 *   type: "custom_emoji",
 *   id: string,
 *   name: string,
 *   animated: boolean,
 *   url: string,
 *   label: string,
 * }} CustomEmojiToken
 */

/**
 * Токен стандартного Unicode-эмодзи из шорткода (`:name:`).
 * @typedef {{
 *   type: "unicode_emoji",
 *   name: string,
 *   unicode: string,
 *   label: string,
 * }} UnicodeEmojiToken
 */

/** @typedef {CustomEmojiToken | UnicodeEmojiToken} EmojiToken */

/** Максимальная длина snowflake ID кастомного эмодзи. */
export const MAX_EMOJI_ID = 32;

/** Минимальная длина имени эмодзи в Discord (`2` символа). */
export const MIN_EMOJI_NAME = 2;

/** Максимальная длина имени эмодзи в Discord (`32` символа). */
export const MAX_EMOJI_NAME = 32;

/** Базовый CDN URL эмодзи Discord. */
export const DISCORD_EMOJI_CDN_BASE = "https://cdn.discordapp.com/emojis";

/**
 * Регулярное выражение кастомного эмодзи Discord в начале среза:
 * `<:name:id>` или `<a:name:id>`.
 */
export const CUSTOM_EMOJI_RE = new RegExp(
  `^<(a?):([A-Za-z0-9_]{${MIN_EMOJI_NAME},${MAX_EMOJI_NAME}}):(\\d{1,${MAX_EMOJI_ID}})>`,
);

/**
 * Регулярное выражение шорткода `:name:` в начале среза.
 */
export const SHORTCODE_EMOJI_RE = new RegExp(
  `^:([A-Za-z0-9_+-]{${MIN_EMOJI_NAME},${MAX_EMOJI_NAME}}):`,
);

/**
 * Замороженный словарь стандартных шорткодов эмодзи Discord (`:name:` -> Unicode).
 * @type {Readonly<Record<string, string>>}
 */
export const DISCORD_SHORTCODE_EMOJIS = Object.freeze({
  smile: "😄",
  grinning: "😀",
  joy: "😂",
  rofl: "🤣",
  wink: "😉",
  blush: "😊",
  heart_eyes: "😍",
  kissing_heart: "😘",
  thinking: "🤔",
  neutral_face: "😐",
  expressionless: "😑",
  unamused: "😒",
  sweat_smile: "😅",
  sob: "😭",
  cry: "😢",
  angry: "😠",
  rage: "😡",
  skull: "💀",
  sunglasses: "😎",
  partying_face: "🥳",
  pleading_face: "🥺",
  eyes: "👀",
  wave: "👋",
  clap: "👏",
  pray: "🙏",
  muscle: "💪",
  thumbsup: "👍",
  "+1": "👍",
  thumbsdown: "👎",
  "-1": "👎",
  ok_hand: "👌",
  v: "✌️",
  Salute: "🫡",
  salute: "🫡",
  heart: "❤️",
  broken_heart: "💔",
  sparkles: "✨",
  star: "⭐",
  fire: "🔥",
  100: "💯",
  tada: "🎉",
  rocket: "🚀",
  warning: "⚠️",
  white_check_mark: "✅",
  x: "❌",
  question: "❓",
  exclamation: "❗",
  zzz: "💤",
  bug: "🐛",
  gear: "⚙️",
  lock: "🔒",
  key: "🔑",
  bell: "🔔",
  loudspeaker: "📢",
  speech_balloon: "💬",
  Check: "✅",
});

/**
 * Строит безопасный CDN URL для кастомного эмодзи Discord (`https://cdn.discordapp.com/emojis/{id}.{ext}`).
 *
 * @param {string} id
 * @param {boolean} [animated]
 * @returns {string}
 */
export function buildCustomEmojiUrl(id, animated = false) {
  if (typeof id !== "string" || !/^\d{1,32}$/.test(id)) {
    throw new TypeError("buildCustomEmojiUrl: id должен быть строкой из 1..=32 цифр");
  }
  const ext = animated ? "gif" : "webp";
  return `${DISCORD_EMOJI_CDN_BASE}/${id}.${ext}`;
}

/**
 * Пытается прочитать кастомное эмодзи Discord (`<:name:id>` или `<a:name:id>`)
 * с позиции `start` в строке `text`.
 *
 * @param {string} text
 * @param {number} start
 * @returns {{ token: CustomEmojiToken, end: number } | null}
 */
export function readCustomEmoji(text, start) {
  if (typeof text !== "string" || start < 0 || start >= text.length) return null;
  if (text.charAt(start) !== "<") return null;

  const match = CUSTOM_EMOJI_RE.exec(text.slice(start));
  if (!match) return null;

  const animated = match[1] === "a";
  const name = match[2] ?? "";
  const id = match[3] ?? "";
  if (!name || !id) return null;

  return {
    token: {
      type: "custom_emoji",
      id,
      name,
      animated,
      url: buildCustomEmojiUrl(id, animated),
      label: `:${name}:`,
    },
    end: start + match[0].length,
  };
}

/**
 * Пытается прочитать шорткод эмодзи (`:name:`) с позиции `start` в строке `text`.
 *
 * Не срабатывает внутри URL (`https://...`) или времени (`12:30:45`), если перед `:` стоит
 * буква/цифра/слэш или если имя отсутствует в словаре `shortcodeMap`.
 *
 * @param {string} text
 * @param {number} start
 * @param {Record<string, string>} [shortcodeMap]
 * @returns {{ token: UnicodeEmojiToken, end: number } | null}
 */
export function readShortcodeEmoji(text, start, shortcodeMap = DISCORD_SHORTCODE_EMOJIS) {
  if (typeof text !== "string" || start < 0 || start >= text.length) return null;
  if (text.charAt(start) !== ":") return null;

  // Защита от ложного срабатывания внутри схем/путей или слепленных двоеточий
  if (start > 0 && /[A-Za-z0-9/:]/.test(text.charAt(start - 1))) {
    return null;
  }

  const match = SHORTCODE_EMOJI_RE.exec(text.slice(start));
  if (!match) return null;

  const name = match[1] ?? "";
  const map = shortcodeMap && typeof shortcodeMap === "object" ? shortcodeMap : DISCORD_SHORTCODE_EMOJIS;
  const unicode = Object.prototype.hasOwnProperty.call(map, name) ? map[name] : undefined;
  if (typeof unicode !== "string" || unicode === "") return null;

  return {
    token: {
      type: "unicode_emoji",
      name,
      unicode,
      label: `:${name}:`,
    },
    end: start + match[0].length,
  };
}

/**
 * Пытается прочитать любое эмодзи Discord (`<:name:id>`, `<a:name:id>` или `:shortcode:`)
 * с позиции `start` в строке `text`.
 *
 * @param {string} text
 * @param {number} start
 * @param {Record<string, string>} [shortcodeMap]
 * @returns {{ token: EmojiToken, end: number } | null}
 */
export function readEmoji(text, start, shortcodeMap = DISCORD_SHORTCODE_EMOJIS) {
  if (typeof text !== "string" || start < 0 || start >= text.length) return null;
  const ch = text.charAt(start);
  if (ch === "<") return readCustomEmoji(text, start);
  if (ch === ":") return readShortcodeEmoji(text, start, shortcodeMap);
  return null;
}

/**
 * Извлекает все эмодзи (`custom_emoji` и `unicode_emoji`) из текста `text`
 * в порядке следования, пропуская инлайн-код (`` `...` ``) и блоки кода (` ```...``` `).
 *
 * @param {string} text
 * @param {Record<string, string>} [shortcodeMap]
 * @returns {EmojiToken[]}
 */
export function extractEmojis(text, shortcodeMap = DISCORD_SHORTCODE_EMOJIS) {
  if (typeof text !== "string") {
    throw new TypeError("extractEmojis: text должен быть строкой");
  }
  /** @type {EmojiToken[]} */
  const out = [];
  let index = 0;

  while (index < text.length) {
    const ch = text.charAt(index);

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

    const emoji = readEmoji(text, index, shortcodeMap);
    if (emoji) {
      out.push(emoji.token);
      index = emoji.end;
      continue;
    }

    index += 1;
  }

  return out;
}

/**
 * Заменяет известные шорткоды `:name:` в строке на их Unicode-символы,
 * не затрагивая содержимое инлайн-кода и блоков кода, а также кастомные `<:name:id>`.
 *
 * @param {string} text
 * @param {Record<string, string>} [shortcodeMap]
 * @returns {string}
 */
export function replaceEmojiShortcodes(text, shortcodeMap = DISCORD_SHORTCODE_EMOJIS) {
  if (typeof text !== "string") {
    throw new TypeError("replaceEmojiShortcodes: text должен быть строкой");
  }
  let out = "";
  let index = 0;

  while (index < text.length) {
    const ch = text.charAt(index);

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
          out += text.slice(index, cursor + run);
          index = cursor + run;
          closed = true;
          break;
        }
        cursor += run;
      }
      if (closed) continue;
    }

    if (ch === "<") {
      const custom = readCustomEmoji(text, index);
      if (custom) {
        out += text.slice(index, custom.end);
        index = custom.end;
        continue;
      }
    }

    if (ch === ":") {
      const sc = readShortcodeEmoji(text, index, shortcodeMap);
      if (sc) {
        out += sc.token.unicode;
        index = sc.end;
        continue;
      }
    }

    out += ch;
    index += 1;
  }

  return out;
}

/**
 * Создаёт безопасный DOM-элемент эмодзи (`span.emoji`) через `textContent` (без `<img>` и `innerHTML`).
 *
 * - Для `custom_emoji`:
 *   `<span class="emoji emoji--custom" role="img" aria-label=":name:" data-emoji-id="..." data-emoji-name="..." data-emoji-animated="true|false" data-emoji-url="https://cdn.discordapp.com/emojis/...">:name:</span>`
 * - Для `unicode_emoji`:
 *   `<span class="emoji emoji--unicode" role="img" aria-label=":name:" data-emoji-name="...">😄</span>`
 *
 * @param {Document} doc
 * @param {EmojiToken} token
 * @returns {HTMLElement}
 */
export function createEmojiElement(doc, token) {
  if (!doc || typeof doc.createElement !== "function") {
    throw new TypeError("createEmojiElement: нужен объект Document с методом createElement");
  }
  if (token.type === "custom_emoji") {
    const node = doc.createElement("span");
    node.className = token.animated
      ? "emoji emoji--custom emoji--animated"
      : "emoji emoji--custom";
    node.setAttribute("role", "img");
    node.setAttribute("aria-label", token.label);
    node.setAttribute("data-emoji-id", token.id);
    node.setAttribute("data-emoji-name", token.name);
    node.setAttribute("data-emoji-animated", token.animated ? "true" : "false");
    node.setAttribute("data-emoji-url", token.url);
    node.textContent = token.label;
    return node;
  }
  const node = doc.createElement("span");
  node.className = "emoji emoji--unicode";
  node.setAttribute("role", "img");
  node.setAttribute("aria-label", token.label);
  node.setAttribute("data-emoji-name", token.name);
  node.textContent = token.unicode;
  return node;
}
