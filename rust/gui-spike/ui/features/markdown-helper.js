// @ts-check
/**
 * T-218 (RW-40 / G2): хелпер разметки сообщений Discord — спойлеры
 * (`||...||`), независимость от inline-кода и timestamp-теги `<t:UNIX[:STYLE]>`.
 *
 * Оракулы контрактов:
 * - клиент Discord: timestamp-стили `t T d D f F R`, по умолчанию `f`;
 * - правило клиента: `||...||` внутри inline-бэктиков спойлером НЕ считается;
 * - короткие русские относительные формы (`мин.`, `ч.`, `дн.`).
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

/**
 * Стиль по умолчанию, когда он не указан в теге.
 * @type {"f"}
 */
const DEFAULT_TIMESTAMP_STYLE = "f";

/**
 * Полный синтаксис тега: `<t:1700000000>` либо `<t:1700000000:R>`.
 * UNIX — целое (в том числе отрицательное), стиль — из `TIMESTAMP_STYLES`.
 */
const TIMESTAMP_TAG_PATTERN = /^<t:(-?\d+)(?::([tTdDfFR]))?>$/;

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
  if (!TIMESTAMP_STYLES.includes(style)) return null;

  return { unixSeconds, style };
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
