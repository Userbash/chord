// @ts-check
/**
 * T-218 / RW-53 W03 (G2): модульный чистый AST-токеннизатор и хелперы разметки
 * сообщений Discord — спойлеры (`||...||`), inline-код, подсветка codeblock,
 * безопасные ссылки и timestamp-теги `<t:UNIX[:STYLE]>`.
 *
 * Оракулы контрактов:
 * - клиент Discord: timestamp-стили `t T d D f F R`, по умолчанию `f`;
 * - правило клиента: `||...||` внутри inline-бэктиков спойлером НЕ считается;
 * - короткие русские относительные формы (`мин.`, `ч.`, `дн.`);
 * - линейный O(n) разбор без катастрофического backtracking регулярных выражений.
 *
 * Модуль чистый: без DOM и сетевых вызовов.
 *
 * @module ui/features/markdown-helper
 */

/**
 * Допустимые стили Discord timestamp-тега (порядок как в клиенте).
 * @type {ReadonlyArray<string>}
 */
export const TIMESTAMP_STYLES = Object.freeze(["t", "T", "d", "D", "f", "F", "R"]);

/** @type {ReadonlySet<string>} */
const TIMESTAMP_STYLE_SET = new Set(TIMESTAMP_STYLES);

/**
 * Стиль по умолчанию, когда он не указан в теге.
 * @type {"f"}
 */
const DEFAULT_TIMESTAMP_STYLE = "f";

/** Только эти схемы допустимы в `href` (M-08, P0). */
const SAFE_PROTOCOLS = Object.freeze(["http:", "https:"]);

/**
 * Полный синтаксис тега: `<t:1700000000>` либо `<t:1700000000:R>`.
 * UNIX — целое (в том числе отрицательное), стиль — из `TIMESTAMP_STYLES`.
 */
const TIMESTAMP_TAG_PATTERN = /^<t:(-?\d+)(?::([tTdDfFR]))?>$/;

/** @typedef {{ type: "timestamp", unixSeconds: number, style: string, raw: string }} TimestampToken */
/** @typedef {{ type: "plain" | "keyword" | "string" | "comment" | "number", value: string }} CodeHighlightToken */

/** @type {ReadonlySet<string>} */
const CODE_KEYWORDS = new Set([
  "as", "async", "await", "break", "case", "catch", "class", "const", "continue",
  "crate", "default", "def", "delete", "do", "elif", "else", "enum", "export",
  "extends", "false", "finally", "fn", "for", "from", "function", "if", "impl",
  "import", "in", "instanceof", "interface", "let", "loop", "match", "mod", "mut",
  "new", "null", "None", "pub", "return", "self", "Self", "static", "struct",
  "super", "switch", "this", "throw", "trait", "true", "True", "False", "try",
  "type", "typeof", "use", "var", "void", "where", "while", "with", "yield",
]);

/** @type {ReadonlySet<string>} */
const HIGHLIGHT_LANGUAGES = new Set([
  "js", "javascript", "mjs", "cjs", "ts", "typescript", "jsx", "tsx",
  "rs", "rust", "py", "python", "json", "sh", "bash", "zsh", "c", "cpp", "go", "java",
]);

/**
 * @param {string} ch
 * @returns {boolean}
 */
function isAsciiDigit(ch) {
  return ch >= "0" && ch <= "9";
}

/**
 * @param {string} ch
 * @returns {boolean}
 */
function isIdentStart(ch) {
  return (ch >= "a" && ch <= "z") || (ch >= "A" && ch <= "Z") || ch === "_" || ch === "$";
}

/**
 * @param {string} ch
 * @returns {boolean}
 */
function isIdentPart(ch) {
  return isIdentStart(ch) || isAsciiDigit(ch);
}

/**
 * Проверяет, является ли символ буквой/цифрой (для границ `_italic_`).
 * @param {string} ch
 * @returns {boolean}
 */
export function isWordChar(ch) {
  if (!ch) return false;
  return /[\p{L}\p{N}]/u.test(ch);
}

/**
 * Безопасный `href`: только абсолютный http(s). Всё прочее — `null`.
 * @param {string} value
 * @returns {string | null}
 */
export function safeHref(value) {
  const raw = value.trim();
  if (raw === "") return null;
  /** @type {URL} */
  let url;
  try {
    url = new URL(raw);
  } catch {
    return null;
  }
  return SAFE_PROTOCOLS.includes(url.protocol) ? url.href : null;
}

/**
 * Разбирает Discord timestamp-тег.
 *
 * @param {unknown} tag Строка вида `<t:UNIX>` или `<t:UNIX:STYLE>`.
 * @returns {{ unixSeconds: number, style: string } | null} Разобранный тег либо `null`.
 */
export function parseDiscordTimestampTag(tag) {
  if (typeof tag !== "string") return null;
  const match = TIMESTAMP_TAG_PATTERN.exec(tag);
  if (match === null) return null;

  const unixSeconds = Number(match[1]);
  if (!Number.isSafeInteger(unixSeconds)) return null;

  const rawStyle = match[2];
  const style = rawStyle === undefined ? DEFAULT_TIMESTAMP_STYLE : rawStyle;
  if (!TIMESTAMP_STYLE_SET.has(style)) return null;

  return { unixSeconds, style };
}

/**
 * Линейный (O(1) / без backtracking) сканер `<t:UNIX[:STYLE]>` начиная с `start`.
 *
 * @param {string} text
 * @param {number} start
 * @returns {{ token: TimestampToken, end: number } | null}
 */
export function readTimestampTag(text, start) {
  if (!text.startsWith("<t:", start)) return null;
  let cursor = start + 3;
  if (text[cursor] === "-") cursor += 1;
  const digitsStart = cursor;
  while (cursor < text.length && isAsciiDigit(text[cursor] ?? "")) {
    cursor += 1;
  }
  if (cursor === digitsStart) return null;

  /** @type {string} */
  let style = DEFAULT_TIMESTAMP_STYLE;
  if (text[cursor] === ":") {
    const candidate = text[cursor + 1] ?? "";
    if (!TIMESTAMP_STYLE_SET.has(candidate)) return null;
    style = candidate;
    cursor += 2;
  }

  if (text[cursor] !== ">") return null;
  const end = cursor + 1;
  const raw = text.slice(start, end);
  const parsed = parseDiscordTimestampTag(raw);
  if (!parsed) return null;

  return {
    token: {
      type: "timestamp",
      unixSeconds: parsed.unixSeconds,
      style: parsed.style,
      raw,
    },
    end,
  };
}

/**
 * Инлайн-код: обратные кавычки одинаковой длины открывают и закрывают фрагмент.
 * @param {string} text
 * @param {number} start
 * @returns {{ value: string, end: number } | null}
 */
export function readCodeSpan(text, start) {
  let length = 0;
  while (text.charAt(start + length) === "`") length += 1;
  if (length === 0) return null;
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
 * Возвращает индекс сразу после закрывающего участка inline-кода,
 * либо длину строки, если код не закрыт.
 *
 * @param {string} text
 * @param {number} start Индекс открывающего бэктика.
 * @param {number} runLength Длина серии бэктиков.
 * @returns {number}
 */
function skipInlineCode(text, start, runLength) {
  const fence = "`".repeat(runLength);
  const end = text.indexOf(fence, start + runLength);
  return end === -1 ? text.length : end + runLength;
}

/**
 * Собирает непустые спойлеры `||content||`, игнорируя inline-бэктики.
 *
 * @param {string} text
 * @returns {string[]} Обрезанные по краям спойлеры в порядке появления.
 */
export function extractSpoilerSpans(text) {
  if (typeof text !== "string" || text === "") return [];

  /** @type {string[]} */
  const spans = [];
  let i = 0;

  while (i < text.length) {
    const ch = text[i];

    if (ch === "`") {
      let runLength = 0;
      while (text[i + runLength] === "`") runLength += 1;
      i = skipInlineCode(text, i, runLength);
      continue;
    }

    if (ch === "|" && text[i + 1] === "|") {
      const closeStart = findSpoilerClose(text, i + 2);
      if (closeStart === -1) {
        i += 2;
        continue;
      }
      const inner = text.slice(i + 2, closeStart).trim();
      if (inner !== "") spans.push(inner);
      i = closeStart + 2;
      continue;
    }

    i += 1;
  }

  return spans;
}

/**
 * Ищет закрывающие `||`, пропуская inline-код.
 *
 * @param {string} text
 * @param {number} start Индекс сразу после открывающих `||`.
 * @returns {number} Индекс закрывающих `||` либо `-1`.
 */
function findSpoilerClose(text, start) {
  let i = start;

  while (i < text.length) {
    if (text[i] === "`") {
      let runLength = 0;
      while (text[i + runLength] === "`") runLength += 1;
      i = skipInlineCode(text, i, runLength);
      continue;
    }
    if (text[i] === "|" && text[i + 1] === "|") return i;
    i += 1;
  }

  return -1;
}

/**
 * Короткая русская форма относительного времени для timestamp-тегов.
 * Диапазоны: `< 60 c` — «только что»/«через несколько секунд»,
 * `< 1 ч` — минуты, `< 1 сут` — часы, далее — дни (округление вниз).
 *
 * @param {number} unixSeconds Момент в секундах UNIX.
 * @param {number} nowSeconds Текущий момент в секундах UNIX.
 * @returns {string}
 */
export function formatRelativeTimestampRu(unixSeconds, nowSeconds) {
  const diff = nowSeconds - unixSeconds;
  const abs = Math.abs(diff);
  const future = diff < 0;

  if (abs < 60) {
    return future ? "через несколько секунд" : "только что";
  }
  if (abs < 3600) {
    const minutes = Math.floor(abs / 60);
    return future ? `через ${minutes} мин.` : `${minutes} мин. назад`;
  }
  if (abs < 86400) {
    const hours = Math.floor(abs / 3600);
    return future ? `через ${hours} ч.` : `${hours} ч. назад`;
  }
  const days = Math.floor(abs / 86400);
  return future ? `через ${days} дн.` : `${days} дн. назад`;
}

/**
 * Форматирует Discord timestamp для отображения в DOM.
 *
 * @param {number} unixSeconds
 * @param {string} [style]
 * @param {number} [nowSeconds]
 * @returns {string}
 */
export function formatTimestampText(
  unixSeconds,
  style = DEFAULT_TIMESTAMP_STYLE,
  nowSeconds = Math.floor(Date.now() / 1000),
) {
  if (style === "R") {
    return formatRelativeTimestampRu(unixSeconds, nowSeconds);
  }
  const date = new Date(unixSeconds * 1000);
  if (Number.isNaN(date.getTime())) return String(unixSeconds);

  /** @type {Intl.DateTimeFormatOptions} */
  let options;
  switch (style) {
    case "t":
      options = { hour: "2-digit", minute: "2-digit" };
      break;
    case "T":
      options = { hour: "2-digit", minute: "2-digit", second: "2-digit" };
      break;
    case "d":
      options = { year: "numeric", month: "2-digit", day: "2-digit" };
      break;
    case "D":
      options = { year: "numeric", month: "long", day: "numeric" };
      break;
    case "F":
      options = {
        weekday: "long",
        year: "numeric",
        month: "long",
        day: "numeric",
        hour: "2-digit",
        minute: "2-digit",
      };
      break;
    case "f":
    default:
      options = {
        year: "numeric",
        month: "long",
        day: "numeric",
        hour: "2-digit",
        minute: "2-digit",
      };
      break;
  }
  return new Intl.DateTimeFormat("ru-RU", options).format(date);
}

/**
 * Линейный (O(n)) токенизатор подсветки синтаксиса для блоков кода.
 * Не использует сложные регулярные выражения, исключает backtracking и XSS.
 *
 * @param {string} code
 * @param {string} [language]
 * @returns {CodeHighlightToken[]}
 */
export function tokenizeCodeLines(code, language = "") {
  if (typeof code !== "string" || code === "") return [];
  const normalizedLang = language.trim().toLowerCase();
  if (!HIGHLIGHT_LANGUAGES.has(normalizedLang)) {
    return [{ type: "plain", value: code }];
  }

  /** @type {CodeHighlightToken[]} */
  const tokens = [];
  let plainBuffer = "";
  let i = 0;

  const flushPlain = () => {
    if (plainBuffer !== "") {
      tokens.push({ type: "plain", value: plainBuffer });
      plainBuffer = "";
    }
  };

  while (i < code.length) {
    const ch = code[i] ?? "";
    const next = code[i + 1] ?? "";

    // Однострочный комментарий // или # (для python/shell)
    if (
      (ch === "/" && next === "/") ||
      (ch === "#" && (normalizedLang === "py" || normalizedLang === "python" || normalizedLang === "sh" || normalizedLang === "bash" || normalizedLang === "zsh"))
    ) {
      flushPlain();
      const nl = code.indexOf("\n", i);
      const end = nl === -1 ? code.length : nl;
      tokens.push({ type: "comment", value: code.slice(i, end) });
      i = end;
      continue;
    }

    // Многострочный комментарий /* ... */
    if (ch === "/" && next === "*") {
      flushPlain();
      const close = code.indexOf("*/", i + 2);
      const end = close === -1 ? code.length : close + 2;
      tokens.push({ type: "comment", value: code.slice(i, end) });
      i = end;
      continue;
    }

    // Строковый литерал '...', "...", `...`
    if (ch === "'" || ch === '"' || ch === "`") {
      flushPlain();
      const quote = ch;
      let cursor = i + 1;
      while (cursor < code.length) {
        const cur = code[cursor];
        if (cur === "\\") {
          cursor += 2;
          continue;
        }
        if (cur === quote) {
          cursor += 1;
          break;
        }
        if (cur === "\n" && quote !== "`") {
          break;
        }
        cursor += 1;
      }
      tokens.push({ type: "string", value: code.slice(i, cursor) });
      i = cursor;
      continue;
    }

    // Число
    if (isAsciiDigit(ch) && (i === 0 || !isIdentPart(code[i - 1] ?? ""))) {
      flushPlain();
      let cursor = i + 1;
      while (cursor < code.length) {
        const cur = code[cursor] ?? "";
        if (isAsciiDigit(cur) || cur === "." || cur === "_" || cur === "x" || cur === "X" || (cur >= "a" && cur <= "f") || (cur >= "A" && cur <= "F")) {
          cursor += 1;
        } else {
          break;
        }
      }
      tokens.push({ type: "number", value: code.slice(i, cursor) });
      i = cursor;
      continue;
    }

    // Идентификатор / ключевое слово
    if (isIdentStart(ch)) {
      let cursor = i + 1;
      while (cursor < code.length && isIdentPart(code[cursor] ?? "")) {
        cursor += 1;
      }
      const word = code.slice(i, cursor);
      if (CODE_KEYWORDS.has(word)) {
        flushPlain();
        tokens.push({ type: "keyword", value: word });
      } else {
        plainBuffer += word;
      }
      i = cursor;
      continue;
    }

    plainBuffer += ch;
    i += 1;
  }

  flushPlain();
  return tokens;
}
