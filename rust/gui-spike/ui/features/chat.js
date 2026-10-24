// @ts-check
/**
 * Экран чата: PageStore + виртуализация с переменной высотой строки (D-04) и
 * extras строки P3-02 (W39h): day divider, reply-quote, reactions, embeds,
 * метка «изменено», unread-полоса 34 (D-13) и hover-панель R-16.
 *
 * Шаблон строки — DOM-CONTRACT §7.1: ровно два ребёнка `.author` и `.text`,
 * placeholder без детей, `data-group="start"|"continuation"`. Все extras живут
 * внутри `.text`, `.text-body` — всегда первый ребёнок (оракул индекса).
 *
 * История канала (C-02-PAGE + C-02-UI-OLDER) ходит в `messages_page` двумя
 * независимыми направлениями: `position:"latest"`/`cursor`+`nextCursor` — вниз,
 * `before`+`olderCursor` — вверх. Курсоры хранятся раздельно (`ChannelWindow`),
 * окно — не больше `CHANNEL_WINDOW_ROWS` строк.
 */
import { PageStore, ROW_HEIGHTS, createChatHeights, messageGroup, messageHeight, messageLines } from "../model.js";
import { byId, formatNumber } from "./dom.js";
import { UIError } from "./errors.js";
import { createEmbeds } from "./embeds.js";
import { createMessageToolbar } from "./message-toolbar.js";
import {
  adjustReactionOverride,
  channelReactionKey,
  reactionsFor,
  requestReaction,
} from "./reactions.js";
import { AUTHORS, extrasFor } from "./rows.js";
import { VirtualList } from "./virtual-list.js";

export const CHAT_ROWS = 100_000;
export const CHAT_PAGE_SIZE = 100;
export const CHAT_MAX_PAGES = 5;
/**
 * C-02: канал синтетического окна. Это режим бенча (`CHAT_ROWS`), а не реальный
 * канал: при выборе канала из `channels()` уходит настоящий snowflake.
 */
export const BENCH_CHANNEL_ID = "news";
/** C-02: `messages_page` отдаёт не более 100 строк за страницу (`MAX_PAGE_LIMIT`). */
export const CHANNEL_PAGE_LIMIT = 100;
/**
 * C-02-PAGE: насколько близко к нижнему краю списка считается «дочитал до
 * конца» — тогда подгружается следующая страница по `nextCursor` (px).
 */
export const CHANNEL_LOAD_EDGE_PX = 96;
/**
 * C-02-UI-OLDER: насколько близко к верхнему краю списка считается «дошёл до
 * начала окна» — тогда подгружается более ранняя история по `olderCursor` (px).
 */
export const CHANNEL_LOAD_TOP_EDGE_PX = 96;
/**
 * C-02-PAGE: бюджет окна истории. `data.rs UI_WINDOW_ROWS` = 500 — UI не
 * накапливает больше строк, даже если backend готов отдавать страницы дальше.
 */
export const CHANNEL_WINDOW_ROWS = 500;
/** D-13: «20+ новых сообщений» — счётчик полосы (не выдумка из загруженного окна). */
export const UNREAD_COUNT = 20;
/** Куда ведёт полоса непрочитанного: за 20 сообщений до конца канала. */
export const UNREAD_TARGET = CHAT_ROWS - UNREAD_COUNT - 1;
/**
 * M-07: окно троттлинга ack на канал. Это политика `[A]` (не восстановленное
 * значение Discord): открытие/фокус/скролл в низ в пределах окна не шлют
 * второй `ack_message`; контракт — `CONTRACTS/read-state.md`.
 */
export const ACK_THROTTLE_MS = 3_000;
/** Насколько близко к низу считается «скролл в низ» (px допуска). */
export const ACK_BOTTOM_EDGE_PX = 8;

const WORDS = [
  "привет", "как дела", "смотри", "готово", "тест", "волна", "ревью", "gate",
  "Rust", "Discord", "сборка", "макет", "пиксель", "контраст", "фокус",
];
const FILLER = "строка переноса демонстрирует высоту 22";
/** Вертикальный padding строки сообщения: `padding: 2px 16px` (--space-1, SPEC §5.2). */
const ROW_PAD_Y = 2;

/**
 * @typedef {{
 *   author: string,
 *   text: string,
 *   lines: number,
 *   group: "start" | "continuation",
 *   time: string,
 *   iso: string,
 *   hue: number,
 *   bot: boolean,
 *   pinned: boolean,
 * }} ChatMessage
 */

/**
 * Строка реального канала (`messages_page`): id — snowflake сообщения, а не
 * индекс синтетического окна (C-02, закрывает G6-D-01).
 * @typedef {{
 *   id: string,
 *   author: string,
 *   authorId: string,
 *   text: string,
 *   time: string,
 *   iso: string,
 *   day: string,
 *   dayLabel: string,
 *   group: "start" | "continuation",
 *   edited: boolean,
 *   bot: boolean,
 *   hue: number,
 *   channelId: string,
 *   reactions: import("./rows.js").Reaction[],
 * }} ChannelMessage
 */

/**
 * Окно истории реального канала. Направления курсоров хранятся раздельно:
 * `nextCursor`/`hasMore` — forward (`cursor`, строки новее), `olderCursor`/
 * `hasOlder` — older (`before`, строки старше; см. `data.rs` + C-02-BEFORE §1.2).
 * Путать их нельзя: `before`-страница тоже возвращает свой `nextCursor`.
 * @typedef {{
 *   items: ChannelMessage[],
 *   scroll: number,
 *   nextCursor: string | null,
 *   hasMore: boolean,
 *   olderCursor: string | null,
 *   hasOlder: boolean,
 *   total: number | null,
 *   pages: number,
 *   loading: boolean,
 *   olderLoading: boolean,
 * }} ChannelWindow
 */

/**
 * Строка чата по сквозному индексу: материализуется только для загруженной страницы.
 * @param {number} index
 * @returns {ChatMessage}
 */
export function makeMessage(index) {
  const author = `${AUTHORS[index % AUTHORS.length] ?? ""}${index % 97 === 0 ? `_${index}` : ""}`;
  const lines = messageLines(index);
  const head = `#${index} ${WORDS[index % WORDS.length] ?? ""} ${WORDS[(index * 7) % WORDS.length] ?? ""}`;
  // Текст длиннее — переносы визуально соответствуют +22 в модели высот.
  const text = lines === 1 ? head : `${head} — ${Array.from({ length: lines - 1 }, () => FILLER).join(" ")}`;
  const minute = String(index % 60).padStart(2, "0");
  const hour = String(20 + (index % 4)).padStart(2, "0");
  return {
    author,
    text,
    lines,
    group: messageGroup(index),
    time: `${hour}:${minute}`,
    iso: `2026-10-23T${hour}:${minute}:00Z`,
    hue: (index * 37) % 360,
    bot: index % 97 === 0,
    // Демо-флаг «закреплено» для секции «Закреплено» search overlay.
    pinned: index % 523 === 0,
  };
}

/**
 * Хук перерисовки одной строки: ставит `createChatScreen`, чтобы реакция,
 * поставленная из строки, обновляла именно этот узел (без полного paint).
 * @type {(index: number) => void}
 */
let refreshRowHook = () => {};

/**
 * @param {number} index
 * @returns {string}
 */
function authorOf(index) {
  return AUTHORS[index % AUTHORS.length] ?? "";
}

/**
 * Пилюля реакции: emoji + счётчик, состояние — `aria-pressed` (MSG-25).
 * @param {import("./rows.js").Reaction} reaction
 * @returns {HTMLElement}
 */
function reactionPill(reaction) {
  const pill = document.createElement("button");
  pill.className = "reaction";
  pill.setAttribute("type", "button");
  pill.setAttribute("data-emoji", reaction.emoji);
  pill.setAttribute("data-me", reaction.me ? "true" : "false");
  pill.setAttribute("aria-pressed", reaction.me ? "true" : "false");
  pill.setAttribute("aria-label", `Реакция ${reaction.emoji}, ${reaction.count}`);
  const glyph = document.createElement("span");
  glyph.className = "reaction-emoji";
  glyph.setAttribute("aria-hidden", "true");
  glyph.textContent = reaction.emoji;
  const count = document.createElement("span");
  count.className = "reaction-count";
  count.textContent = String(reaction.count);
  pill.append(glyph, count);
  return pill;
}

/**
 * Группа пилюль внутри `.text`. L-02-UI: один шаблон для bench-строки, строки
 * реального канала и live-реакции — разметка MSG-23…26 не дублируется.
 * @param {HTMLElement} row
 * @returns {HTMLElement | null}
 */
function reactionsGroupOf(row) {
  const text = /** @type {HTMLElement | null} */ (row.querySelector(".text"));
  if (!text) return null;
  const existing = /** @type {HTMLElement | null} */ (text.querySelector(".reactions-row"));
  if (existing) return existing;
  const group = document.createElement("div");
  group.className = "reactions-row";
  group.setAttribute("role", "group");
  group.setAttribute("aria-label", "Реакции");
  text.append(group);
  text.setAttribute("data-stack", "true");
  return group;
}

/**
 * @param {HTMLElement} row
 * @param {ChatMessage | null} item
 * @param {number} index
 */
export function renderChatRow(row, item, index) {
  // Переход row↔placeholder переиспользует тот же элемент: чистим детей и классы,
  // иначе placeholder унаследует старый текст, а строка задвоит его.
  row.className = "row";
  row.replaceChildren();
  row.removeAttribute("data-extras");
  if (item === null) {
    // Placeholder окна данных: только скелетон-фон, детей нет (§7.1).
    row.classList.add("placeholder");
    row.removeAttribute("role");
    row.removeAttribute("aria-label");
    return;
  }
  const extras = extrasFor(index);
  // Видимые реакции = базовый набор строки + локальные переключения (не сеть).
  const reactions = reactionsFor(index, extras.reactions);
  /** @type {string[]} */
  const present = [];
  if (extras.day) present.push("day");
  if (extras.reply) present.push("reply");
  if (reactions.length > 0) present.push("reactions");
  if (extras.embeds.length > 0) present.push("embeds");
  if (extras.edited) present.push("edited");
  if (present.length > 0) row.setAttribute("data-extras", present.join(" "));

  // AT-SPI/скринридер: виртуализованная строка — listitem с человекочитаемым именем.
  row.setAttribute("role", "listitem");
  row.setAttribute("data-index", String(index));
  row.setAttribute("data-message-id", String(index));
  row.setAttribute("data-group", item.group);
  row.setAttribute("aria-setsize", String(CHAT_ROWS));
  row.setAttribute("aria-posinset", String(index + 1));
  row.setAttribute(
    "aria-label",
    `${item.author}, ${item.time}: ${item.text}`
      + (extras.edited ? " (изменено)" : "")
      + (extras.reply ? `, ответ на ${extras.reply.author}` : ""),
  );

  const author = document.createElement("span");
  author.className = "author";
  const avatar = document.createElement("span");
  avatar.className = "avatar avatar--lg";
  avatar.textContent = item.author.slice(0, 1).toUpperCase();
  avatar.setAttribute("aria-hidden", "true");
  avatar.style.setProperty("--avatar-hue", String(item.hue));
  const name = document.createElement("span");
  name.className = "author-name";
  name.textContent = item.author;
  const time = document.createElement("time");
  time.className = "timestamp";
  time.setAttribute("datetime", item.iso);
  time.textContent = item.time;
  author.append(avatar, name);
  if (item.bot) {
    const badge = document.createElement("span");
    badge.className = "badge badge--bot";
    badge.textContent = "BOT";
    author.append(badge);
  }
  author.append(time);

  const text = document.createElement("span");
  text.className = "text";
  // Стабильный маркер строки для optimistic-реакций: после перерисовки
  // тест/диагностика находят владельца пилюли, не заглядывая в виртуализатор.
  text.setAttribute("data-index", String(index));
  // Высота контейнера текста считается по переносам (D-04), а не по замороженному
  // треку: строки × 22 + строка автора у начала группы. Padding'и строки 2+2
  // (`--space-1`, SPEC §5.2) уже вычтены моделью высот — поэтому высота .text
  // равна messageHeight(index) − 4 и не зависит от того, отрисована ли строка
  // виртуализатором (renderChatRow экспортируется и для прямого рендера).
  const textHeight = messageHeight(index) - 2 * ROW_PAD_Y;
  text.style.height = `${textHeight}px`;
  if (present.length > 0) text.setAttribute("data-stack", "true");

  // Первый ребёнок .text всегда .text-body: textContent начинается с «#<index>».
  const body = document.createElement("span");
  body.className = "text-body";
  body.textContent = item.text;
  // Переносы: chat.css держит `white-space: nowrap` (зона W39c2), поэтому для
  // многострочных строк включаем перенос inline — иначе +22 не виден.
  if (item.lines > 1) body.style.whiteSpace = "normal";
  text.append(body);

  if (extras.edited) {
    const edited = document.createElement("span");
    edited.className = "edited";
    edited.textContent = "(изменено)";
    text.append(edited);
  }

  if (extras.day) {
    const divider = document.createElement("div");
    divider.className = "day-divider";
    divider.setAttribute("role", "separator");
    divider.setAttribute("aria-label", `Начало дня: ${extras.day.iso.slice(0, 10)}`);
    divider.setAttribute("data-iso", extras.day.iso.slice(0, 10));
    const label = document.createElement("span");
    label.className = "day-divider-label";
    label.textContent = extras.day.label;
    divider.append(label);
    text.append(divider);
  }

  if (extras.reply) {
    const quote = document.createElement("div");
    quote.className = "reply-quote";
    quote.setAttribute("data-reply-author", extras.reply.author);
    const quoteAvatar = document.createElement("span");
    quoteAvatar.className = "avatar avatar--xs";
    quoteAvatar.setAttribute("aria-hidden", "true");
    quoteAvatar.style.setProperty("--avatar-hue", String(extras.reply.hue));
    quoteAvatar.textContent = extras.reply.author.slice(0, 1).toUpperCase();
    const quoteAuthor = document.createElement("span");
    quoteAuthor.className = "reply-quote-author";
    quoteAuthor.textContent = extras.reply.author;
    const quoteText = document.createElement("span");
    quoteText.className = "reply-quote-text";
    quoteText.textContent = extras.reply.text;
    quote.append(quoteAvatar, quoteAuthor, quoteText);
    text.append(quote);
  }

  if (reactions.length > 0) {
    const row = document.createElement("div");
    row.className = "reactions-row";
    row.setAttribute("role", "group");
    row.setAttribute("aria-label", "Реакции");
    for (const reaction of reactions) {
      const pill = reactionPill(reaction);
      pill.addEventListener("click", () => {
        void requestReaction(index, reaction.emoji, extras.reactions, {
          element: pill,
          onSettled: () => refreshRowHook(index),
        });
        // Optimistic-состояние — в том же узле: виртуализация не должна
        // подменять элемент под курсором до ответа IPC.
        const next = reactionsFor(index, extras.reactions).find((item) => item.emoji === reaction.emoji);
        if (next) {
          pill.setAttribute("data-me", next.me ? "true" : "false");
          pill.setAttribute("aria-pressed", next.me ? "true" : "false");
          pill.setAttribute("aria-label", `Реакция ${reaction.emoji}, ${next.count}`);
          const count = pill.querySelector(".reaction-count");
          if (count) count.textContent = String(next.count);
        }
      });
      row.append(pill);
    }
    text.append(row);
  }

  const embeds = createEmbeds(extras.embeds);
  if (embeds) text.append(embeds);

  row.append(author, text);
}

/**
 * Стабильный оттенок аватара по id автора (не зависит от индекса окна).
 * @param {string} key
 * @returns {number}
 */
function hueOf(key) {
  let hash = 0;
  for (const char of key) hash = (hash * 31 + (char.codePointAt(0) ?? 0)) % 360;
  return hash;
}

/**
 * `reactions[]` строки в форме backend `MessageReaction` (`data.rs`):
 * `{ emoji, count, me }`. Битые элементы (null, без emoji, count не число)
 * пропускаются/нормализуются, а не роняют окно канала (M-06).
 * @param {unknown} value
 * @returns {import("./rows.js").Reaction[]}
 */
export function parseChannelReactions(value) {
  if (!Array.isArray(value)) return [];
  /** @type {import("./rows.js").Reaction[]} */
  const rows = [];
  for (const entry of value) {
    if (entry === null || typeof entry !== "object") continue;
    const bag = /** @type {Record<string, unknown>} */ (entry);
    const emoji = typeof bag.emoji === "string" ? bag.emoji.trim() : "";
    if (emoji === "") continue;
    const raw = typeof bag.count === "number" && Number.isFinite(bag.count) ? bag.count : 0;
    rows.push({
      emoji,
      count: Math.max(0, Math.trunc(raw)),
      me: bag.me === true,
    });
  }
  return rows;
}

/**
 * `messages_page` → строки реального канала. Форма ответа — `MessagesPage`
 * (`data.rs`): `items[].id/author/content/timestamp/edited/group/reactions`.
 * Запись без строкового `id` — ошибка IPC, а не строка с выдуманным id (G6-D-01).
 * @param {string} channelId
 * @param {unknown} payload
 * @returns {ChannelMessage[]}
 */
export function parseChannelPage(channelId, payload) {
  const bag = payload !== null && typeof payload === "object"
    ? /** @type {Record<string, unknown>} */ (payload)
    : {};
  if (!Array.isArray(bag.items)) {
    throw new UIError("IPC_FAILED", `messages_page(${channelId}): в ответе нет items[]`);
  }
  /** @type {ChannelMessage[]} */
  const rows = [];
  /** @type {string} */
  let previousDay = "";
  for (const [index, value] of bag.items.entries()) {
    const item = value !== null && typeof value === "object"
      ? /** @type {Record<string, any>} */ (value)
      : {};
    const id = typeof item.id === "string" ? item.id : "";
    if (id === "") {
      throw new UIError("IPC_FAILED", `messages_page(${channelId}): строка ${index} без id`);
    }
    const author = item.author !== null && typeof item.author === "object"
      ? /** @type {Record<string, any>} */ (item.author)
      : {};
    const authorId = String(author.id ?? "");
    const iso = typeof item.timestamp === "string" ? item.timestamp : "";
    const day = typeof item.day === "string" && item.day !== "" ? item.day : iso.slice(0, 10);
    rows.push({
      id,
      author: String(author.globalName ?? author.username ?? authorId ?? "?"),
      authorId,
      text: String(item.content ?? ""),
      time: iso.length >= 16 ? iso.slice(11, 16) : "",
      iso,
      day,
      dayLabel: day !== "" && day !== previousDay ? day : "",
      group: item.group === "continuation" ? "continuation" : "start",
      edited: item.edited === true,
      bot: author.bot === true,
      hue: hueOf(authorId || String(author.username ?? "")),
      channelId: typeof item.channelId === "string" && item.channelId !== ""
        ? item.channelId
        : channelId,
      reactions: parseChannelReactions(item.reactions),
    });
    previousDay = day;
  }
  return rows;
}

/**
 * Порядок строк истории как у backend (`data.rs::collect_messages`): `timestamp`,
 * затем `id`. Пустой `iso` не выдумывает время — строка идёт первой.
 * @param {ChannelMessage} left
 * @param {ChannelMessage} right
 * @returns {number}
 */
function compareChannelItems(left, right) {
  if (left.iso !== right.iso) return left.iso < right.iso ? -1 : 1;
  return left.id < right.id ? -1 : left.id > right.id ? 1 : 0;
}

/**
 * Слияние страниц истории: дедуп по id (уже загруженная строка побеждает —
 * в ней могла быть optimistic-правка) и порядок backend. `prepended` — сколько
 * новых строк встало перед текущим началом окна (для сохранения позиции).
 * @param {ChannelMessage[]} existing
 * @param {ChannelMessage[]} incoming
 * @returns {{ items: ChannelMessage[], added: number, prepended: number }}
 */
export function mergeChannelItems(existing, incoming) {
  const known = new Set(existing.map((item) => item.id));
  /** @type {ChannelMessage[]} */
  const fresh = [];
  for (const item of incoming) {
    if (known.has(item.id)) continue;
    known.add(item.id);
    fresh.push(item);
  }
  if (fresh.length === 0) return { items: existing, added: 0, prepended: 0 };
  const first = existing[0];
  const prepended = first
    ? fresh.filter((item) => compareChannelItems(item, first) < 0).length
    : 0;
  const items = prepended === 0
    ? [...existing, ...fresh]
    : [...existing, ...fresh].sort(compareChannelItems);
  return { items, added: fresh.length, prepended };
}

/**
 * Метаданные страницы `messages_page` (`data.rs MessagesPage`): курсоры двух
 * направлений читаются раздельно и не подменяют друг друга. Пустая строка и
 * отсутствующее поле — «курсора нет» (`null`), а не выдуманный id.
 * @param {unknown} payload
 * @returns {{
 *   nextCursor: string | null,
 *   hasMore: boolean,
 *   olderCursor: string | null,
 *   hasOlder: boolean,
 *   total: number | null,
 * }}
 */
export function pageMeta(payload) {
  const bag = payload !== null && typeof payload === "object"
    ? /** @type {Record<string, unknown>} */ (payload)
    : {};
  const rawNext = typeof bag.nextCursor === "string" ? bag.nextCursor.trim() : "";
  const rawOlder = typeof bag.olderCursor === "string" ? bag.olderCursor.trim() : "";
  const nextCursor = rawNext === "" ? null : rawNext;
  const olderCursor = rawOlder === "" ? null : rawOlder;
  return {
    nextCursor,
    hasMore: bag.hasMore === true && nextCursor !== null,
    olderCursor,
    hasOlder: bag.hasOlder === true && olderCursor !== null,
    total: typeof bag.total === "number" && Number.isFinite(bag.total) ? bag.total : null,
  };
}

/**
 * Видимый текст отказа `messages_page` по машинному `kind`/статусу — без тела
 * ответа и без токена. 404/401 не превращаются в пустой список.
 * @param {unknown} error
 * @returns {string}
 */
export function channelFailureText(error) {
  const bag = error !== null && typeof error === "object"
    ? /** @type {Record<string, any>} */ (error)
    : {};
  const kind = String(bag.kind ?? "");
  const status = Number(bag.status ?? bag.code ?? 0);
  const message = error instanceof Error ? error.message : String(bag.message ?? "");
  if (status === 401 || /unauthor/i.test(kind) || /401|unauthor|сесси/i.test(message)) {
    return "Сессия истекла. Войдите снова.";
  }
  if (status === 404 || /notfound/i.test(kind) || /404|не найден/i.test(message)) {
    return "Канал не найден или недоступен.";
  }
  if (status === 429 || /ratelimit/i.test(kind)) {
    return "Слишком много запросов. Повторите позже.";
  }
  return "Не удалось загрузить сообщения канала.";
}

/**
 * Optimistic-состояние в том же узле: виртуализация/повторный рендер не должны
 * подменять элемент под курсором до ответа IPC (M-06, bench B3b).
 * @param {HTMLElement} pill
 * @param {number | string} key
 * @param {string} emoji
 * @param {import("./rows.js").Reaction[]} base
 */
function paintPill(pill, key, emoji, base) {
  const next = reactionsFor(key, base).find((item) => item.emoji === emoji);
  if (!next) return;
  pill.setAttribute("data-me", next.me ? "true" : "false");
  pill.setAttribute("aria-pressed", next.me ? "true" : "false");
  pill.setAttribute("aria-label", `Реакция ${emoji}, ${next.count}`);
  const count = pill.querySelector(".reaction-count");
  if (count) count.textContent = String(next.count);
}

/**
 * Строка реального канала — та же структура, что у синтетической
 * (DOM-CONTRACT §7.1: ровно `.author` + `.text`, `.text-body` первым ребёнком),
 * но id/автор/текст/группа — из `messages_page`.
 * @param {HTMLElement} row
 * @param {ChannelMessage} item
 * @param {number} index
 * @param {number} total
 * @param {{ channelId?: string }} [options] реальный канал для `toggle_reaction`
 */
export function renderChannelMessage(row, item, index, total, options = {}) {
  row.className = "row channel-message";
  row.replaceChildren();
  row.removeAttribute("data-extras");
  row.setAttribute("role", "listitem");
  row.setAttribute("data-index", String(index));
  row.setAttribute("data-message-id", item.id);
  row.setAttribute("data-group", item.group);
  row.setAttribute("data-channel-message", "true");
  row.setAttribute("aria-setsize", String(total));
  row.setAttribute("aria-posinset", String(index + 1));
  row.setAttribute(
    "aria-label",
    `${item.author}, ${item.time}: ${item.text}${item.edited ? " (изменено)" : ""}`,
  );
  // Ключ состояния — реальный канал/сообщение, а не bench-индекс (M-06).
  const channelId = String(options.channelId ?? item.channelId ?? "");
  const reactionKey = channelReactionKey(channelId, item.id);
  const reactions = reactionsFor(reactionKey, item.reactions);
  /** @type {string[]} */
  const present = [];
  if (item.dayLabel !== "") present.push("day");
  if (reactions.length > 0) present.push("reactions");
  if (item.edited) present.push("edited");
  if (present.length > 0) row.setAttribute("data-extras", present.join(" "));
  // Строка живёт в потоке (страница истории, не виртуализация): без absolute,
  // иначе она схлопнулась бы в одну точку контейнера.
  row.style.position = "relative";

  const author = document.createElement("span");
  author.className = "author";
  const avatar = document.createElement("span");
  avatar.className = "avatar avatar--lg";
  avatar.textContent = item.author.slice(0, 1).toUpperCase();
  avatar.setAttribute("aria-hidden", "true");
  avatar.style.setProperty("--avatar-hue", String(item.hue));
  const name = document.createElement("span");
  name.className = "author-name";
  name.textContent = item.author;
  const time = document.createElement("time");
  time.className = "timestamp";
  time.setAttribute("datetime", item.iso);
  time.textContent = item.time;
  author.append(avatar, name);
  if (item.bot) {
    const badge = document.createElement("span");
    badge.className = "badge badge--bot";
    badge.textContent = "BOT";
    author.append(badge);
  }
  author.append(time);

  const text = document.createElement("span");
  text.className = "text";
  text.setAttribute("data-index", String(index));
  if (item.edited || item.dayLabel !== "" || reactions.length > 0) {
    text.setAttribute("data-stack", "true");
  }
  const body = document.createElement("span");
  body.className = "text-body";
  body.textContent = item.text;
  // Реальный текст приходит с переносами: держим их, а не nowrap.
  body.style.whiteSpace = "pre-wrap";
  text.append(body);
  if (item.edited) {
    const edited = document.createElement("span");
    edited.className = "edited";
    edited.textContent = "(изменено)";
    text.append(edited);
  }
  if (item.dayLabel !== "") {
    const divider = document.createElement("div");
    divider.className = "day-divider";
    divider.setAttribute("role", "separator");
    divider.setAttribute("aria-label", `Начало дня: ${item.dayLabel}`);
    divider.setAttribute("data-iso", item.dayLabel);
    const label = document.createElement("span");
    label.className = "day-divider-label";
    label.textContent = item.dayLabel;
    divider.append(label);
    text.append(divider);
  }
  row.append(author, text);
  if (reactions.length > 0) {
    const group = reactionsGroupOf(row);
    if (group) {
      for (const reaction of reactions) {
        const pill = reactionPill(reaction);
        pill.addEventListener("click", () => {
          // toggle_reaction получает настоящие snowflake канала/сообщения (D-53),
          // а не bench-«news»/индекс (G6-D-01).
          void requestReaction(reactionKey, reaction.emoji, item.reactions, {
            channelId,
            messageId: item.id,
            element: pill,
            onSettled: () => refreshRowHook(index),
          });
          paintPill(pill, reactionKey, reaction.emoji, item.reactions);
        });
        group.append(pill);
      }
    }
  }
}

/**
 * Полоса непрочитанного «20+ новых сообщений» (DIV-01…04, D-13: 34 border-box).
 * Живёт рядом со списком, а не внутри: §6.1 — у `#chat-list` только sizer.
 * @param {{ onJump?: () => void }} [options]
 * @returns {HTMLElement}
 */
export function createUnreadDivider(options = {}) {
  const band = document.createElement("div");
  band.className = "chat-unread-divider";
  band.setAttribute("role", "separator");
  band.setAttribute("aria-label", `${UNREAD_COUNT}+ новых сообщений`);
  // Высота — токен `--unread-banner-h` (34), в разметке дублируем числом для
  // диагностики/тестов: CSS-каскад в шиме не считается.
  band.setAttribute("data-height", "34");
  band.setAttribute("data-count", String(UNREAD_COUNT));
  const label = document.createElement("span");
  label.className = "chat-unread-label";
  label.textContent = `${UNREAD_COUNT}+ новых сообщений`;
  const jump = document.createElement("button");
  jump.className = "chat-unread-jump";
  jump.setAttribute("type", "button");
  jump.setAttribute("aria-label", `Перейти к ${UNREAD_COUNT} новым сообщениям`);
  jump.textContent = "Перейти";
  band.append(label, jump);
  jump.addEventListener("click", () => options.onJump?.());
  return band;
}

/**
 * @param {{
 *   fetchPage?: (page: number) => ChatMessage[],
 * }} [options]
 * @returns {PageStore}
 */
export function createChatStore(options = {}) {
  return new PageStore({
    totalRows: CHAT_ROWS,
    pageSize: CHAT_PAGE_SIZE,
    maxPages: CHAT_MAX_PAGES,
    fetchPage: options.fetchPage ?? ((page) => {
      const start = page * CHAT_PAGE_SIZE;
      const end = Math.min(CHAT_ROWS, start + CHAT_PAGE_SIZE);
      return Array.from({ length: Math.max(0, end - start) }, (_, offset) => makeMessage(start + offset));
    }),
  });
}

/**
 * Индекс высот с «дырами» удалённых строк (tombstones). Удалённая строка
 * получает высоту 0, а все строки ниже сдвигаются вверх — иначе после delete
 * остаётся пустая полоса, а виртуализация продолжает считать её место.
 * Геометрия базового `HeightIndex` (model.js) не меняется: надстройка только
 * вычитает известные удаления, поэтому перезагрузка страницы из PageStore
 * возвращает тот же результат.
 * @param {import("../model.js").HeightIndex} base
 */
function createDeletionHeights(base) {
  /** @type {Set<number>} */
  const deleted = new Set();
  /** @param {number} index */
  const removedBefore = (index) => {
    let sum = 0;
    for (const removed of deleted) {
      if (removed < index) sum += base.height(removed);
    }
    return sum;
  };
  return {
    /** @param {number} index */
    mark(index) { deleted.add(index); },
    /** @param {number} index */
    has(index) { return deleted.has(index); },
    get totalHeight() { return base.totalHeight - removedBefore(base.totalRows); },
    /** @param {number} index */
    height(index) { return deleted.has(index) ? 0 : base.height(index); },
    /** @param {number} index */
    top(index) { return base.top(index) - removedBefore(index); },
    /** @param {number} offset */
    indexAt(offset) {
      if (!Number.isFinite(offset) || offset <= 0) return 0;
      if (offset >= /** @type {number} */ (this.totalHeight)) return base.totalRows - 1;
      let low = 0;
      let high = base.totalRows - 1;
      while (low < high) {
        const middle = (low + high + 1) >> 1;
        if (/** @type {number} */ (this.top(middle)) <= offset) low = middle;
        else high = middle - 1;
      }
      return low;
    },
  };
}

/**
 * @param {{
 *   list: HTMLElement,
 *   search?: HTMLInputElement | null,
 *   countLabel?: HTMLElement | null,
 *   onError?: (error: UIError) => void,
 *   invoke?: (command: string, args?: any) => Promise<any>,
 *   now?: () => number,
 * }} options
 */
export function createChatScreen(options) {
  const store = createChatStore();
  const heights = createDeletionHeights(createChatHeights(CHAT_ROWS));
  // Tombstone по индексу: PageStore может перезагрузить страницу, но удалённое
  // сообщение не должно вернуться. `null` — строка без содержимого и высоты.
  const source = {
    totalRows: store.totalRows,
    getRow: (/** @type {number} */ index) => (heights.has(index) ? null : store.getRow(index)),
    ensureRange: (/** @type {number} */ first, /** @type {number} */ last) => store.ensureRange(first, last),
  };
  const list = new VirtualList({
    container: options.list,
    source,
    heights: /** @type {any} */ (heights),
    render: renderChatRow,
    onError: options.onError,
  });
  // D-04: модель высот — единственный источник (26/48/+22), строка-трек 44 из
  // RW-38 больше не участвует. Строку собираем из той же константы модели,
  // чтобы диагностика не разошлась с рендером.
  options.list.setAttribute(
    "data-row-height",
    `${ROW_HEIGHTS.continuation}/${ROW_HEIGHTS.groupStart}/+${ROW_HEIGHTS.lineExtra}`,
  );
  // Чат открывается на последнем сообщении, как в реальном клиенте.
  list.scrollToIndex(CHAT_ROWS - 1);

  const onError = options.onError ?? (() => {});
  const container = /** @type {HTMLElement | null} */ (
    /** @type {any} */ (options.list).parentNode ?? null
  );
  const sentMessages = document.createElement("div");
  sentMessages.className = "chat-sent-messages";
  sentMessages.setAttribute("aria-live", "polite");
  if (container) container.append(sentMessages);

  // M-07: единственная полоса (в бенче — «20+ новых», в реальном канале —
  // честный mention-счётчик из `read_state()`); ошибки ack — отдельной
  // видимой заметкой рядом, без оверлея и без retry-цикла.
  const unreadBand = container
    ? createUnreadDivider({ onJump: () => jumpTo(UNREAD_TARGET) })
    : null;
  if (container && unreadBand) {
    unreadBand.setAttribute("data-source", "bench");
    container.append(unreadBand);
  }
  const ackNote = container ? document.createElement("p") : null;
  if (container && ackNote) {
    ackNote.className = "chat-ack-note";
    ackNote.setAttribute("hidden", "");
    ackNote.style.display = "none";
    container.append(ackNote);
  }

  /** @param {{content: string, nonce: string}} message */
  function appendLocalMessage(message) {
    const row = document.createElement("div");
    row.className = "row local-message";
    row.setAttribute("data-message-status", "pending");
    row.setAttribute("data-nonce", message.nonce);
    const author = document.createElement("span"); author.className = "author"; author.textContent = "Вы";
    const text = document.createElement("span"); text.className = "text"; text.textContent = message.content;
    row.append(author, text); sentMessages.append(row);
    return row;
  }

  /** @param {{content: string, nonce: string}} message @param {(cmd: string, args: any) => Promise<any>} invoke @param {HTMLElement | null} [existingRow] */
  function sendMessage(message, invoke, existingRow = null) {
    const row = existingRow ?? appendLocalMessage(message);
    return invoke("send_message", { channelId: currentChannelId(), content: message.content, nonce: message.nonce }).then((receipt) => {
      row.setAttribute("data-message-status", "sent");
      row.setAttribute("data-message-id", String(receipt?.id ?? ""));
    }).catch((error) => {
      row.setAttribute("data-message-status", "error");
      const kind = error?.kind ?? error?.code;
      const detail = kind === "Unauthorized" || error?.status === 401 ? "сессия истекла" : kind === "RateLimited" || error?.status === 429 ? `ограничение частоты; повтор через ${error?.retry_after_ms ?? error?.retryAfterMs ?? 0} мс` : "сеть недоступна";
      const note = document.createElement("span"); note.className = "message-error"; note.textContent = detail;
      const retry = document.createElement("button"); retry.type = "button"; retry.textContent = "Повторить"; retry.setAttribute("data-action", "retry");
      retry.addEventListener("click", () => {
        note.remove();
        retry.remove();
        row.setAttribute("data-message-status", "pending");
        void sendMessage(message, invoke, row);
      });
      row.append(note, retry);
      returnMessageToComposer?.(message.content);
    });
  }
  /** @type {((text: string) => void) | null} */
  let returnMessageToComposer = null;

  /* ---------- C-02: реальный канал рядом с бенч-окном ---------- */

  /** @type {"bench" | "channel"} */
  let mode = "bench";
  /** @type {{ id: string, name: string } | null} */
  let channel = null;
  /**
   * Окно/позиция каждого открытого канала (per-channel, не общий скролл) +
   * курсор пагинации. `scroll` — позиция на момент ухода из канала.
   * @type {Map<string, ChannelWindow>}
   */
  const channelWindows = new Map();
  /** Подвал истории (`#chat-list .chat-history-more`): последний ребёнок списка. @type {HTMLElement | null} */
  let historyFooter = null;
  /**
   * C-02-UI-OLDER: верхняя полоса истории (`#chat-list .chat-history-older`) —
   * первый ребёнок списка, не строка (в подсчёте строк/`aria-setsize` не участвует).
   * @type {HTMLElement | null}
   */
  let olderBar = null;
  /** Синтетическое окно снимается один раз; назад его не подмешиваем. */
  let benchDetached = false;
  /** Токен запроса истории: поздний ответ прошлого канала не рисуется. */
  let openToken = 0;

  /** @returns {boolean} */
  function isChannelMode() {
    return mode === "channel";
  }

  /**
   * Канал для live-команд: реальный snowflake в режиме канала, синтетический
   * `news` — только для бенч-окна (G6-D-01).
   * @returns {string}
   */
  function currentChannelId() {
    return isChannelMode() && channel ? channel.id : BENCH_CHANNEL_ID;
  }

  /** @returns {ChannelMessage[]} */
  function channelItems() {
    if (!channel) return [];
    return channelWindows.get(channel.id)?.items ?? [];
  }

  /** @param {number} index @returns {ChannelMessage | undefined} */
  function channelItemAt(index) {
    return channelItems()[index];
  }

  /* ---------- M-07: read_state → ack (open/focus/scroll-to-bottom) ---------- */

  /** Часы троттлинга: `performance.now` не прыгает от перевода системного времени. */
  const now = typeof options.now === "function" ? options.now : () => performance.now();

  /**
   * Курсор канала из `read_state()` (READY-проекция `m07-be2`).
   * @typedef {{ lastViewed: number | null, mentionCount: number | null, messageId: string | null }} ReadStateCursor
   */

  /**
   * Счётчик READY: только целое неотрицательное число. Строка/`null`/badge не
   * превращаются в 0 — UI не выдумывает read-state.
   * @param {unknown} value
   * @returns {number | null}
   */
  function counter(value) {
    return typeof value === "number" && Number.isInteger(value) && value >= 0 ? value : null;
  }

  /**
   * Snowflake из READY — строка цифр. Число/`null` не подставляем.
   * @param {unknown} value
   * @returns {string | null}
   */
  function snowflakeId(value) {
    return typeof value === "string" && /^\d{1,20}$/.test(value) ? value : null;
  }

  /** @type {Map<string, { acked: number | null, failedFor: number | null, attemptedAt: number | null, pending: boolean }>} */
  const ackLog = new Map();

  /** @param {string} id */
  function ackState(id) {
    let entry = ackLog.get(id);
    if (!entry) {
      entry = { acked: null, failedFor: null, attemptedAt: null, pending: false };
      ackLog.set(id, entry);
    }
    return entry;
  }

  /**
   * Прочитать курсор канала. `read_state()` — локальная READY-проекция (без
   * сети и токена); ошибка видна заметкой, а не выдуманным курсором.
   * @param {string} id
   * @returns {Promise<ReadStateCursor | null>}
   */
  async function loadReadState(id) {
    if (!options.invoke) return null;
    try {
      const entries = await options.invoke("read_state");
      if (!Array.isArray(entries)) return null;
      const entry = entries.find((item) => item && String(item.channelId ?? "") === id);
      if (!entry) return null;
      return {
        lastViewed: counter(entry.lastViewed),
        mentionCount: counter(entry.mentionCount),
        messageId: snowflakeId(entry.lastMessageId),
      };
    } catch (error) {
      showAckNote("ReadState", "Не удалось прочитать состояние канала");
      return null;
    }
  }

  /**
   * Текст полосы упоминаний: 1 упоминание / 2 упоминания / 5 упоминаний.
   * @param {number} count
   * @returns {string}
   */
  function mentionLabel(count) {
    const mod10 = count % 10;
    const mod100 = count % 100;
    const word = mod10 === 1 && mod100 !== 11
      ? "упоминание"
      : mod10 >= 2 && mod10 <= 4 && (mod100 < 12 || mod100 > 14)
        ? "упоминания"
        : "упоминаний";
    return `${count} ${word}`;
  }

  /** Полоса в режиме реального канала скрыта до данных из READY. */
  function prepareReadStateBand() {
    if (!unreadBand) return;
    unreadBand.setAttribute("data-source", "read-state");
    unreadBand.removeAttribute("data-cleared");
    unreadBand.removeAttribute("data-mentions");
    unreadBand.setAttribute("hidden", "");
    unreadBand.style.display = "none";
    const jump = unreadBand.querySelector(".chat-unread-jump");
    if (jump) jump.removeAttribute("hidden");
  }

  /**
   * Полоса упоминаний: показывается только при `mentionCount > 0` из READY и
   * гаснет, если этот `lastViewed` уже подтверждён. `null`/badge — не 0.
   * @param {ReadStateCursor | null} cursor
   */
  function applyReadState(cursor) {
    if (!unreadBand) return;
    const mentions = cursor?.mentionCount ?? null;
    const acked = cursor !== null
      && cursor.lastViewed !== null
      && ackState(channel?.id ?? "").acked === cursor.lastViewed;
    if (cursor === null || mentions === null || mentions <= 0 || acked) {
      hideReadStateBand();
      return;
    }
    const label = mentionLabel(mentions);
    unreadBand.setAttribute("data-source", "read-state");
    unreadBand.setAttribute("data-mentions", String(mentions));
    unreadBand.setAttribute("aria-label", `${label} в канале`);
    unreadBand.removeAttribute("data-cleared");
    unreadBand.removeAttribute("hidden");
    unreadBand.style.display = "";
    const text = unreadBand.querySelector(".chat-unread-label");
    if (text) text.textContent = label;
    const jump = unreadBand.querySelector(".chat-unread-jump");
    if (jump) jump.setAttribute("hidden", "");
  }

  /** Сбросить полосу (без выдуманных чисел и без возврата счётчика). */
  function hideReadStateBand() {
    if (!unreadBand) return;
    unreadBand.setAttribute("data-source", "read-state");
    unreadBand.setAttribute("hidden", "");
    unreadBand.removeAttribute("data-mentions");
    unreadBand.style.display = "none";
  }

  /**
   * @param {string} kind
   * @param {string} text
   */
  function showAckNote(kind, text) {
    if (!ackNote) return;
    ackNote.textContent = text;
    ackNote.setAttribute("role", "alert");
    ackNote.setAttribute("data-state", "error");
    ackNote.setAttribute("data-kind", kind);
    ackNote.removeAttribute("hidden");
    ackNote.style.display = "";
  }

  function clearAckNote() {
    if (!ackNote) return;
    ackNote.textContent = "";
    ackNote.setAttribute("hidden", "");
    ackNote.style.display = "none";
    ackNote.removeAttribute("role");
    ackNote.removeAttribute("data-state");
    ackNote.removeAttribute("data-kind");
  }

  /**
   * Тип и текст ошибки ack: 401/429 объясняются, остальное — общий отказ.
   * Тело ответа и токен в текст не попадают.
   * @param {unknown} error
   * @returns {{ kind: string, text: string }}
   */
  function ackFailure(error) {
    const bag = error !== null && typeof error === "object"
      ? /** @type {Record<string, any>} */ (error)
      : {};
    const kind = String(bag.kind ?? "");
    const status = Number(bag.status ?? bag.code ?? 0);
    if (status === 401 || /unauthor/i.test(kind)) {
      return { kind: "Unauthorized", text: "Не удалось отметить канал прочитанным: сессия истекла" };
    }
    if (status === 429 || /ratelimit/i.test(kind)) {
      return { kind: "RateLimited", text: "Не удалось отметить канал прочитанным: слишком часто, повторите позже" };
    }
    return { kind: "AckFailed", text: "Не удалось отметить канал прочитанным" };
  }

  /**
   * `messageId` для ack: snowflake из READY, иначе последнее реально
   * загруженное сообщение канала. Индекс бенча сюда не попадает.
   * @param {string} id
   * @param {ReadStateCursor | null} cursor
   * @returns {string | null}
   */
  function ackMessageId(id, cursor) {
    if (cursor?.messageId) return cursor.messageId;
    const items = channelWindows.get(id)?.items ?? [];
    const last = items[items.length - 1];
    return last ? snowflakeId(last.id) : null;
  }

  /**
   * Один ack на (канал, `lastViewed`): троттлинг `ACK_THROTTLE_MS`, дедуп
   * успешного курсора и отсутствие retry-цикла после ошибки.
   * @param {string} id
   */
  async function ackIfNeeded(id) {
    // Синтетический бенч (`news`, 100k строк) не имеет read_state — не ack'аем.
    if (!isChannelMode() || channel?.id !== id || id === BENCH_CHANNEL_ID) return;
    const cursor = await loadReadState(id);
    // Пока ждали READY, пользователь мог уйти в другой канал.
    if (!isChannelMode() || channel?.id !== id) return;
    applyReadState(cursor);
    if (cursor === null || cursor.lastViewed === null) return;
    const entry = ackState(id);
    if (entry.pending) return;
    if (entry.acked === cursor.lastViewed) return;
    if (entry.failedFor === cursor.lastViewed) return;
    const at = now();
    if (entry.attemptedAt !== null && at - entry.attemptedAt < ACK_THROTTLE_MS) return;
    const messageId = ackMessageId(id, cursor);
    if (messageId === null) return;
    entry.pending = true;
    entry.attemptedAt = at;
    try {
      if (!options.invoke) return;
      await options.invoke("ack_message", {
        channelId: id,
        messageId,
        lastViewed: cursor.lastViewed,
        token: null,
      });
      entry.acked = cursor.lastViewed;
      entry.failedFor = null;
      clearAckNote();
      // Прочитано: полоса и точка в дереве гаснут (не выдуманный счётчик).
      if (channel?.id === id) {
        hideReadStateBand();
        if (unreadBand) unreadBand.setAttribute("data-cleared", "true");
      }
      const tile = document.querySelector(`.channel-item[data-channel-id="${id}"]`);
      if (tile) tile.classList.remove("is-unread");
    } catch (error) {
      entry.failedFor = cursor.lastViewed;
      const failure = ackFailure(error);
      showAckNote(failure.kind, failure.text);
    } finally {
      entry.pending = false;
    }
  }

  /**
   * Снять синтетические узлы `#chat-list` (sizer + строки виртуализации), не
   * трогая сам `VirtualList`: бенч-путь остаётся доступен диагностике/снапшоту.
   */
  function detachBench() {
    if (benchDetached) return;
    benchDetached = true;
    for (const node of [.../** @type {any} */ (options.list).children]) node.remove();
    historyFooter = null;
    olderBar = null;
  }

  /**
   * Видимое состояние реального канала: loading/empty/error. У `#chat-list` в
   * реальном режиме нет sizer'а, поэтому состояние — его единственный ребёнок.
   * @param {"loading" | "empty" | "error"} kind
   * @param {string} text
   * @param {string} [retryChannelId]
   */
  function renderChannelState(kind, text, retryChannelId = "") {
    const host = options.list;
    clearOlderBar();
    host.replaceChildren();
    historyFooter = null;
    const node = document.createElement("div");
    node.className = "chat-channel-state";
    node.setAttribute("data-state", kind);
    node.setAttribute("role", kind === "error" ? "alert" : "status");
    // Текст — отдельным узлом: в DOM-шиме `textContent` элемента с детьми не
    // включает собственный текст, а состояние должно быть читаемым в тестах.
    const message = document.createElement("p");
    message.className = "chat-channel-state-text";
    message.textContent = text;
    node.append(message);
    if (retryChannelId !== "") {
      const retry = document.createElement("button");
      retry.className = "chat-channel-retry";
      retry.type = "button";
      retry.textContent = "Повторить";
      retry.addEventListener("click", () => {
        void openChannel({ id: retryChannelId, name: channel?.name ?? retryChannelId }, { force: true });
      });
      node.append(retry);
    }
    host.append(node);
    host.setAttribute("aria-busy", kind === "loading" ? "true" : "false");
  }

  /**
   * Отрисовать окно реального канала из уже полученной истории.
   * @param {ChannelMessage[]} items
   */
  function renderChannelWindow(items) {
    if (items.length === 0) {
      renderChannelState("empty", "Здесь пока нет сообщений");
      return;
    }
    const host = options.list;
    clearOlderBar();
    host.replaceChildren();
    historyFooter = null;
    items.forEach((item, index) => {
      const row = document.createElement("div");
      renderChannelMessage(row, item, index, items.length, { channelId: channel?.id ?? item.channelId });
      host.append(row);
    });
    host.setAttribute("aria-busy", "false");
  }

  /**
   * Дописать строки в конец окна, не перерисовывая уже показанные (якорь и
   * позиция скролла сохраняются).
   * @param {ChannelMessage[]} items
   * @param {number} fromIndex
   */
  function appendChannelRows(items, fromIndex) {
    const host = options.list;
    // Подвал снимаем до добавления: в шиме нет insertBefore, а подвал должен
    // остаться последним ребёнком `#chat-list`.
    clearHistoryFooter();
    for (let index = fromIndex; index < items.length; index++) {
      const item = items[index];
      if (!item) continue;
      const row = document.createElement("div");
      renderChannelMessage(row, item, index, items.length, {
        channelId: channel?.id ?? item.channelId,
      });
      host.append(row);
    }
    // aria-setsize растёт у всего списка, а не только у новых строк.
    for (const child of /** @type {HTMLElement[]} */ ([...host.children])) {
      if (child.getAttribute("data-message-id")) {
        child.setAttribute("aria-setsize", String(items.length));
      }
    }
  }

  /* ---------- C-02-PAGE: подвал истории (nextCursor/hasMore) ---------- */

  /**
   * Текст подвала по состоянию. Ошибка приходит снаружи (`channelFailureText`).
   * @param {"idle" | "loading" | "error" | "end" | "limit"} state
   * @returns {string}
   */
  function historyFooterText(state) {
    if (state === "loading") return "Загрузка сообщений…";
    if (state === "end") return "Больше сообщений нет";
    if (state === "limit") return `Достигнут предел окна истории (${CHANNEL_WINDOW_ROWS} сообщений)`;
    return "Прокрутите к нижнему краю, чтобы загрузить ещё";
  }

  /**
   * Подвал истории — последний ребёнок `#chat-list`, не строка: в подсчёте
   * строк/`aria-setsize` не участвует. `error` — `role="alert"` + повтор.
   * @param {"idle" | "loading" | "error" | "end" | "limit"} state
   * @param {string} [text]
   * @returns {HTMLElement}
   */
  function historyFooterNode(state, text = "") {
    const host = options.list;
    if (!historyFooter || historyFooter.parentNode !== host) {
      historyFooter = document.createElement("div");
      historyFooter.className = "chat-history-more";
      const message = document.createElement("p");
      message.className = "chat-history-more-text";
      historyFooter.append(message);
      host.append(historyFooter);
    }
    const footer = historyFooter;
    // Не `data-state`: `#chat-list [data-state]` — контракт состояний канала
    // (loading/empty/error) из C-02-UI; подвал истории им не является.
    footer.setAttribute("data-page-state", state);
    footer.setAttribute("role", state === "error" ? "alert" : "status");
    const message = footer.querySelector(".chat-history-more-text");
    if (message) message.textContent = text !== "" ? text : historyFooterText(state);
    const existing = footer.querySelector(".chat-history-more-retry");
    if (state === "error") {
      if (!existing) {
        const retry = document.createElement("button");
        retry.className = "chat-history-more-retry";
        retry.type = "button";
        retry.textContent = "Повторить";
        retry.addEventListener("click", () => { void loadMoreHistory(); });
        footer.append(retry);
      }
    } else if (existing) {
      existing.remove();
    }
    return footer;
  }

  /** Убрать подвал (список заменяется целиком/канал закрыт). */
  function clearHistoryFooter() {
    historyFooter?.remove();
    historyFooter = null;
  }

  /* ---------- C-02-UI-OLDER: верхняя полоса истории (hasOlder/olderCursor) ---------- */

  /**
   * Текст верхней полосы по состоянию. Ошибка приходит снаружи
   * (`channelFailureText`) — как и у нижнего подвала.
   * @param {"idle" | "loading" | "error" | "end" | "limit"} state
   * @returns {string}
   */
  function olderBarText(state) {
    if (state === "loading") return "Загрузка предыдущих сообщений…";
    if (state === "end") return "Это начало истории";
    if (state === "limit") return `Достигнут предел окна истории (${CHANNEL_WINDOW_ROWS} сообщений)`;
    return "Прокрутите вверх, чтобы загрузить более ранние сообщения";
  }

  /**
   * Верхняя полоса истории — первый ребёнок `#chat-list`, не строка: в подсчёте
   * строк/`aria-setsize` не участвует. `data-page-state` (а не `data-state`) —
   * чтобы не спорить с контрактом состояний канала. `error` — `role="alert"` +
   * повтор того же шага (`before`/`olderCursor` не сдвигаются).
   * @param {"idle" | "loading" | "error" | "end" | "limit"} state
   * @param {string} [text]
   * @returns {HTMLElement}
   */
  function olderBarNode(state, text = "") {
    const host = options.list;
    if (!olderBar || olderBar.parentNode !== host) {
      olderBar = document.createElement("div");
      olderBar.className = "chat-history-older";
      const message = document.createElement("p");
      message.className = "chat-history-older-text";
      olderBar.append(message);
      const first = host.firstChild;
      // Шим не знает insertBefore (как и `appendChannelRows`): там полоса
      // встаёт перед строками тем же replaceChildren без потери узлов.
      if (first && first !== olderBar && typeof /** @type {any} */ (host).insertBefore === "function") {
        /** @type {any} */ (host).insertBefore(olderBar, first);
      } else {
        host.replaceChildren(olderBar, ...host.children);
      }
    }
    const bar = olderBar;
    bar.setAttribute("data-page-state", state);
    bar.setAttribute("role", state === "error" ? "alert" : "status");
    const message = bar.querySelector(".chat-history-older-text");
    if (message) message.textContent = text !== "" ? text : olderBarText(state);
    const existing = bar.querySelector(".chat-history-older-retry");
    if (state === "error") {
      if (!existing) {
        const retry = document.createElement("button");
        retry.className = "chat-history-older-retry";
        retry.type = "button";
        retry.textContent = "Повторить";
        retry.addEventListener("click", () => { void loadOlderHistory(); });
        bar.append(retry);
      }
    } else if (existing) {
      existing.remove();
    }
    return bar;
  }

  /** Убрать верхнюю полосу (список заменяется целиком/канал закрыт). */
  function clearOlderBar() {
    olderBar?.remove();
    olderBar = null;
  }

  /**
   * Показать оба направления истории по состоянию окна: низ — forward
   * (`hasMore`), верх — older (`hasOlder`). Направления не смешиваются.
   * @param {ChannelWindow} win
   */
  function syncHistoryView(win) {
    if (win.items.length === 0) return;
    historyFooterNode(win.hasMore ? "idle" : "end");
    olderBarNode(win.hasOlder ? "idle" : "end");
  }

  /**
   * Открытие канала — на последнем сообщении: `scrollTop` в низ списка (хвост),
   * а не в начало. В шиме layout не считается, там позиция остаётся 0.
   */
  function scrollChannelToEnd() {
    const host = options.list;
    host.scrollTop = Math.max(0, host.scrollHeight - host.clientHeight);
  }

  /**
   * Подгрузить более раннюю историю по `olderCursor` (`before`). Guard: реальный
   * канал, `hasOlder`, нет параллельного запроса, окно ≤ `CHANNEL_WINDOW_ROWS`.
   * В отличие от `loadMoreHistory`, эта ветка не читает и не пишет
   * `nextCursor`/`hasMore`: ответ на `before` тоже несёт forward-метаданные
   * своего окна (C-02-BEFORE §6.2), и принять их за курсор «вниз» нельзя.
   * @returns {Promise<void>}
   */
  async function loadOlderHistory() {
    if (!isChannelMode() || !channel || !options.invoke) return;
    const id = channel.id;
    const win = channelWindows.get(id);
    if (!win || win.olderLoading || win.loading || !win.hasOlder) return;
    if (win.items.length >= CHANNEL_WINDOW_ROWS) {
      olderBarNode("limit");
      return;
    }
    const before = win.olderCursor;
    if (before === null || before === "") {
      win.hasOlder = false;
      olderBarNode("end");
      return;
    }
    win.olderLoading = true;
    const token = openToken;
    olderBarNode("loading");
    options.list.setAttribute("aria-busy", "true");
    try {
      const payload = await options.invoke("messages_page", {
        channelId: id,
        before,
        limit: CHANNEL_PAGE_LIMIT,
      });
      if (token !== openToken || !isChannelMode() || channel?.id !== id) return;
      const page = parseChannelPage(id, payload);
      const meta = pageMeta(payload);
      const merged = mergeChannelItems(win.items, page);
      // Курсор не сдвинулся и новых строк нет — начало выборки, а не повод
      // крутить запросы (битый/повторный ответ backend).
      const stalled = merged.added === 0
        && (meta.olderCursor === null || meta.olderCursor === before);
      if (merged.prepended > 0) {
        const beforeTop = options.list.scrollTop;
        const beforeHeight = options.list.scrollHeight;
        win.items = merged.items;
        renderChannelWindow(win.items);
        // Вставка сверху: строка под курсором остаётся на месте. В DOM-шиме
        // layout не считается (delta = 0), в браузере — компенсация высотой.
        options.list.scrollTop = beforeTop + (options.list.scrollHeight - beforeHeight);
      } else if (merged.added > 0) {
        // Ответ не старше окна (битый `before`): строки не теряем — дописываем.
        win.items = merged.items;
        appendChannelRows(merged.items, merged.items.length - merged.added);
      }
      win.pages += 1;
      if (meta.total !== null) win.total = meta.total;
      win.olderCursor = meta.olderCursor;
      win.hasOlder = meta.hasOlder && !stalled;
      if (win.items.length >= CHANNEL_WINDOW_ROWS && win.hasOlder) {
        historyFooterNode(win.hasMore ? "idle" : "end");
        olderBarNode("limit");
      } else {
        syncHistoryView(win);
      }
      options.list.setAttribute("aria-busy", "false");
    } catch (error) {
      if (token !== openToken || !isChannelMode() || channel?.id !== id) return;
      // Хвост/уже загруженные строки остаются: ошибка — видимая полоса сверху,
      // а не пустой список. Повтор шлёт тот же `before`.
      const bar = olderBarNode("error", channelFailureText(error));
      bar.setAttribute("data-error-kind", String(/** @type {any} */ (error)?.kind ?? ""));
      options.list.setAttribute("aria-busy", "false");
    } finally {
      win.olderLoading = false;
    }
  }

  /**
   * Подгрузить следующую страницу истории по `nextCursor`. Guard: реальный
   * канал, `hasMore`, нет параллельной подгрузки (в любом направлении — окно
   * одно), окно ≤ `CHANNEL_WINDOW_ROWS`. Курсор и `limit` — единственные
   * аргументы (без token); первая страница курсор не шлёт (см. `openChannel`).
   * @returns {Promise<void>}
   */
  async function loadMoreHistory() {
    if (!isChannelMode() || !channel || !options.invoke) return;
    const id = channel.id;
    const win = channelWindows.get(id);
    if (!win || win.loading || win.olderLoading || !win.hasMore) return;
    if (win.items.length >= CHANNEL_WINDOW_ROWS) {
      historyFooterNode("limit");
      return;
    }
    const cursor = win.nextCursor;
    if (cursor === null || cursor === "") {
      win.hasMore = false;
      historyFooterNode("end");
      return;
    }
    win.loading = true;
    const token = openToken;
    historyFooterNode("loading");
    options.list.setAttribute("aria-busy", "true");
    try {
      const payload = await options.invoke("messages_page", {
        channelId: id,
        cursor,
        limit: CHANNEL_PAGE_LIMIT,
      });
      if (token !== openToken || !isChannelMode() || channel?.id !== id) return;
      const page = parseChannelPage(id, payload);
      const meta = pageMeta(payload);
      const nextCursor = meta.nextCursor;
      const merged = mergeChannelItems(win.items, page);
      // Курсор не сдвинулся и новых строк нет — конец выборки, а не повод
      // крутить запросы (битый/повторный ответ backend).
      const stalled = merged.added === 0 && (nextCursor === null || nextCursor === cursor);
      if (merged.prepended > 0) {
        const beforeTop = options.list.scrollTop;
        const beforeHeight = options.list.scrollHeight;
        win.items = merged.items;
        renderChannelWindow(win.items);
        // Вставка сверху: возвращаем визуальную позицию на высоту вставленного.
        // В DOM-шиме layout не считается (delta = 0), в браузере — сохраняет
        // строку под курсором на месте.
        options.list.scrollTop = beforeTop + (options.list.scrollHeight - beforeHeight);
      } else if (merged.added > 0) {
        win.items = merged.items;
        appendChannelRows(merged.items, merged.items.length - merged.added);
      }
      win.pages += 1;
      if (meta.total !== null) win.total = meta.total;
      // C-02-UI-OLDER: forward-ветка владеет только своей парой курсоров.
      // `olderCursor`/`hasOlder` не переписываются ответом `cursor`-страницы.
      win.nextCursor = nextCursor;
      win.hasMore = meta.hasMore && !stalled;
      options.list.setAttribute("aria-busy", "false");
      historyFooterNode(win.hasMore ? "idle" : "end");
    } catch (error) {
      if (token !== openToken || !isChannelMode() || channel?.id !== id) return;
      // Первая страница остаётся: ошибка подгрузки — видимый подвал (404/401),
      // а не пустой список и не потеря уже загруженных строк.
      const footer = historyFooterNode("error", channelFailureText(error));
      footer.setAttribute("data-error-kind", String(/** @type {any} */ (error)?.kind ?? ""));
      options.list.setAttribute("aria-busy", "false");
    } finally {
      win.loading = false;
    }
  }

  /**
   * Перерисовать одну строку реального канала (edit/optimistic), не теряя окно.
   * @param {number} index
   */
  function renderChannelRowAt(index) {
    const item = channelItemAt(index);
    if (!item) return;
    const node = [.../** @type {any} */ (options.list).children].find(
      (child) => child.getAttribute("data-index") === String(index)
        && child.getAttribute("data-message-id") === item.id,
    );
    if (!node) return;
    renderChannelMessage(
      /** @type {HTMLElement} */ (node),
      item,
      index,
      channelItems().length,
      { channelId: channel?.id ?? item.channelId },
    );
  }

  /**
   * Открыть реальный канал: `messages_page` с настоящим `channelId`, история в
   * `#chat-list`, у каждого канала своё окно и позиция.
   * @param {{ id: string, name?: string }} target
   * @param {{ force?: boolean }} [controls]
   * @returns {Promise<void>}
   */
  async function openChannel(target, controls = {}) {
    const id = String(target?.id ?? "").trim();
    if (id === "") throw new UIError("ACTION_FAILED", "openChannel: пустой channelId");
    const name = String(target?.name ?? "") || id;
    const force = controls.force === true;
    const host = options.list;
    // Повторный клик по уже открытому каналу — только позиция, без IPC истории;
    // ack остаётся идемпотентным (дедуп по lastViewed + троттлинг).
    if (!force && isChannelMode() && channel?.id === id && channelWindows.has(id)) {
      host.scrollTop = channelWindows.get(id)?.scroll ?? 0;
      void ackIfNeeded(id);
      return;
    }
    if (isChannelMode() && channel) {
      const previous = channelWindows.get(channel.id);
      if (previous) previous.scroll = host.scrollTop;
    }
    mode = "channel";
    channel = { id, name };
    detachBench();
    host.setAttribute("data-mode", "channel");
    host.setAttribute("data-channel-id", id);
    const header = document.querySelector("#chat-header");
    if (header) header.setAttribute("data-channel-id", id);
    const title = document.querySelector(".chat-title-text");
    if (title) title.textContent = name;
    // Полоса «20+ новых» относится к синтетическому окну: в реальном канале
    // показываем только mention-счётчик из `read_state()` (или ничего).
    prepareReadStateBand();
    // M-06: быстрые реакции работают и в реальном канале — они получают
    // channelId/messageId через `reactionTargetOf` панели, поэтому скрывать их
    // больше не нужно (раньше ушли бы в bench «news»).
    // Ручной retry начинает окно заново: старый кэш не должен «победить» ошибку.
    if (force) channelWindows.delete(id);
    const cached = channelWindows.get(id);
    if (cached) {
      renderChannelWindow(cached.items);
      syncHistoryView(cached);
      host.scrollTop = cached.scroll;
      void ackIfNeeded(id);
      return;
    }
    const token = ++openToken;
    renderChannelState("loading", "Загрузка сообщений…");
    try {
      if (!options.invoke) throw new UIError("IPC_FAILED", "openChannel: нет IPC-пути");
      // C-02-UI-OLDER: открытие — на последнем сообщении: `position:"latest"`
      // отдаёт хвост канала; ни `cursor`, ни `before` в запросе нет.
      const payload = await options.invoke("messages_page", {
        channelId: id,
        position: "latest",
        limit: CHANNEL_PAGE_LIMIT,
      });
      if (token !== openToken || !isChannelMode() || channel?.id !== id) return;
      const items = parseChannelPage(id, payload);
      const meta = pageMeta(payload);
      /** @type {ChannelWindow} */
      const win = {
        items,
        scroll: 0,
        nextCursor: meta.nextCursor,
        hasMore: meta.hasMore,
        // Курсор «вверх» — своя пара: `olderCursor` не переиспользуется как
        // forward-курсор (и наоборот).
        olderCursor: meta.olderCursor,
        hasOlder: meta.hasOlder,
        total: meta.total ?? items.length,
        pages: 1,
        loading: false,
        olderLoading: false,
      };
      channelWindows.set(id, win);
      renderChannelWindow(items);
      syncHistoryView(win);
      // Открытие — на последнем сообщении (хвост), а не в начале истории.
      scrollChannelToEnd();
      // M-07: открытие реального канала — один ack с курсором READY.
      void ackIfNeeded(id);
    } catch (error) {
      if (token !== openToken || !isChannelMode() || channel?.id !== id) return;
      renderChannelState("error", channelFailureText(error), id);
    }
  }

  /** @param {number} index */
  function refreshRow(index) {
    if (isChannelMode()) {
      renderChannelRowAt(index);
      return;
    }
    const node = list.rows.get(index);
    const item = store.getRow(index);
    if (!node || item === undefined) return;
    renderChatRow(node, item, index);
  }
  refreshRowHook = refreshRow;

  /* ---------- L-02-UI: live-реакции из dispatch ---------- */

  /**
   * Live-счётчик dispatch-события по ключу `channel:…:emoji`. Видимый счётчик =
   * базовый `item.reactions` (messages_page) + этот live-слой: второго источника
   * истины для реакций нет (PLAN §3.5).
   * @type {Map<string, number>}
   */
  const liveReactionCounts = new Map();

  /**
   * Применить live-реакцию к открытому каналу, не пересобирая `#chat-list`.
   * @param {{ channelId: string, messageId: string, emoji: string, count: number, me: boolean, removed: boolean }} change
   * @returns {"applied" | "not-open" | "not-loaded"}
   */
  function applyReaction(change) {
    if (!isChannelMode() || !channel || channel.id !== change.channelId) return "not-open";
    const item = channelItems().find((entry) => entry.id === change.messageId);
    const row = /** @type {HTMLElement | null} */ (
      [.../** @type {any} */ (options.list).children].find(
        (child) => child.getAttribute("data-message-id") === change.messageId
          && child.getAttribute("data-channel-message") === "true",
      ) ?? null
    );
    if (!item || !row) return "not-loaded";
    const key = channelReactionKey(change.channelId, change.messageId);
    const emoji = change.emoji;
    const visible = reactionsFor(key, item.reactions).find((reaction) => reaction.emoji === emoji);
    const currentCount = visible?.count ?? 0;
    const currentMe = visible?.me ?? false;
    const liveKey = `${key}:${emoji}`;
    const previousLive = liveReactionCounts.get(liveKey) ?? 0;
    const liveCount = Math.max(0, Math.trunc(Number(change.count) || 0));
    if (liveCount === previousLive && !change.removed) return "applied";
    liveReactionCounts.set(liveKey, liveCount);
    const displayCount = Math.max(0, currentCount - previousLive + liveCount);
    // `me` живёт по смыслу события: ADD себя включает, REMOVE себя снимает,
    // чужие события мою отметку не трогают.
    const nextMe = change.me ? !change.removed : currentMe;
    adjustReactionOverride(key, emoji, displayCount - currentCount, nextMe, item.reactions);

    const text = /** @type {HTMLElement | null} */ (row.querySelector(".text"));
    if (!text) return "not-loaded";
    const group = /** @type {HTMLElement | null} */ (text.querySelector(".reactions-row"));
    const existing = /** @type {HTMLElement | null} */ (
      [...(group?.querySelectorAll(".reaction") ?? [])].find(
        (pill) => pill.getAttribute("data-emoji") === emoji,
      ) ?? null
    );
    if (displayCount <= 0) {
      // Пилюля с нулём — «призрак»: удаляем и её, и пустую группу.
      existing?.remove();
      if (group && group.querySelectorAll(".reaction").length === 0) group.remove();
      return "applied";
    }
    const target = reactionsGroupOf(row);
    if (!target) return "not-loaded";
    if (!existing) {
      const pill = reactionPill({ emoji, count: displayCount, me: nextMe });
      const channelId = change.channelId;
      const messageId = change.messageId;
      const base = item.reactions;
      pill.addEventListener("click", () => {
        void requestReaction(key, emoji, base, {
          channelId,
          messageId,
          element: pill,
          onSettled: () => {
            const position = channelItems().findIndex((entry) => entry.id === messageId);
            if (position >= 0) renderChannelRowAt(position);
          },
        });
        paintPill(pill, key, emoji, base);
      });
      target.append(pill);
    } else {
      paintPill(existing, key, emoji, item.reactions);
    }
    return "applied";
  }

  /** @type {import("./composer.js").Composer | null} */
  let composer = null;

  /**
   * Пересобрать видимое окно после изменения геометрии (delete): старые узлы
   * держат прежние `top`, поэтому снимаем их и просим виртуализацию нарисовать
   * заново по сдвинутому индексу высот.
   */
  function relayout() {
    for (const node of list.rows.values()) node.remove();
    list.rows.clear();
    list.sizer.style.height = `${heights.totalHeight}px`;
    list.paint();
  }

  const toolbar = container
    ? createMessageToolbar({
      list: options.list,
      container,
      renderRow: refreshRow,
      indexLabel: (index) => {
        if (isChannelMode()) {
          const item = channelItemAt(index);
          return item ? `${item.author}, ${item.time}` : String(index);
        }
        return `${authorOf(index)}, ${makeMessage(index).time}`;
      },
      authorOf: (index) => (isChannelMode() ? channelItemAt(index)?.author ?? "" : authorOf(index)),
      getComposer: () => composer,
      textOf: (index) => (isChannelMode() ? channelItemAt(index)?.text ?? "" : store.getRow(index)?.text ?? ""),
      messageIdOf: (index) => {
        if (isChannelMode()) return channelItemAt(index)?.id ?? String(index);
        return String(index);
      },
      reactionTargetOf: (index) => {
        if (isChannelMode()) {
          const item = channelItemAt(index);
          return {
            base: item?.reactions ?? [],
            key: channelReactionKey(channel?.id ?? "", item?.id ?? String(index)),
            channelId: channel?.id ?? "",
            messageId: item?.id ?? "",
          };
        }
        // Bench-окно: синтетический набор строки и числовой индекс (B3b).
        return { base: extrasFor(index).reactions, key: index };
      },
      onEdit: async (index, content) => {
        if (isChannelMode()) {
          const item = channelItemAt(index);
          if (!item || !options.invoke || !channel) return;
          const previousText = item.text;
          const previousEdited = item.edited;
          item.text = content;
          item.edited = true;
          refreshRow(index);
          try {
            await options.invoke("edit_message", {
              channelId: channel.id,
              messageId: item.id,
              content,
            });
          } catch (error) {
            item.text = previousText;
            item.edited = previousEdited;
            refreshRow(index);
            onError(error instanceof UIError ? error : new UIError("ACTION_FAILED", error instanceof Error ? error.message : "Не удалось изменить сообщение", error));
          }
          return;
        }
        const item = store.getRow(index);
        if (!item || !options.invoke) return;
        const previous = item.text;
        item.text = content;
        refreshRow(index);
        try {
          await options.invoke("edit_message", { channelId: BENCH_CHANNEL_ID, messageId: String(index), content });
        } catch (error) {
          item.text = previous;
          refreshRow(index);
          onError(error instanceof UIError ? error : new UIError("ACTION_FAILED", error instanceof Error ? error.message : "Не удалось изменить сообщение", error));
        }
      },
      onDelete: async (index) => {
        if (isChannelMode()) {
          const item = channelItemAt(index);
          if (!item || !options.invoke || !channel) return;
          const node = [.../** @type {any} */ (options.list).children].find(
            (child) => child.getAttribute("data-message-id") === item.id,
          );
          if (!node) return;
          node.setAttribute("data-state", "deleted");
          node.style.display = "none";
          try {
            await options.invoke("delete_message", { channelId: channel.id, messageId: item.id });
            const window_ = channelWindows.get(channel.id);
            if (window_) {
              const position = window_.items.findIndex((entry) => entry.id === item.id);
              if (position >= 0) window_.items.splice(position, 1);
              renderChannelWindow(window_.items);
              syncHistoryView(window_);
            }
            toolbar?.close();
          } catch (error) {
            node.style.display = "";
            node.removeAttribute("data-state");
            onError(error instanceof UIError ? error : new UIError("ACTION_FAILED", error instanceof Error ? error.message : "Не удалось удалить сообщение", error));
          }
          return;
        }
        const item = store.getRow(index);
        const node = list.rows.get(index);
        if (!item || !node || !options.invoke) return;
        node.setAttribute("data-state", "deleted");
        node.style.display = "none";
        try {
          await options.invoke("delete_message", { channelId: BENCH_CHANNEL_ID, messageId: String(index) });
          // Tombstone до конца сессии: PageStore после выгрузки страницы отдаст
          // строку снова, поэтому удаление живёт в индексе высот/источнике.
          heights.mark(index);
          node.removeAttribute("data-state");
          node.style.display = "";
          relayout();
        } catch (error) {
          node.style.display = "";
          node.removeAttribute("data-state");
          onError(error instanceof UIError ? error : new UIError("ACTION_FAILED", error instanceof Error ? error.message : "Не удалось удалить сообщение", error));
        }
      },
      onError,
    })
    : null;

  /**
   * @param {number} index
   */
  function jumpTo(index) {
    try {
      list.scrollToIndex(index);
      const node = list.rows.get(index);
      if (node) node.classList.add("is-highlight");
      // Подсветка живёт 1000 ms (SRCH-16); таймер не держит процесс.
      setTimeout(() => {
        const current = list.rows.get(index);
        if (current) current.classList.remove("is-highlight");
      }, 1000);
    } catch (error) {
      onError(error instanceof UIError ? error : new UIError("ACTION_FAILED", "переход к сообщению", error));
    }
  }

  // M-07: триггеры ack — открытие (в openChannel), фокус списка и скролл в низ.
  // Только реальный канал: на бенч-окне `ackIfNeeded` выходит по `isChannelMode`.
  const onScrollAck = () => {
    if (!isChannelMode() || !channel) return;
    const remaining = options.list.scrollHeight - options.list.clientHeight - options.list.scrollTop;
    if (remaining <= ACK_BOTTOM_EDGE_PX) void ackIfNeeded(channel.id);
  };
  // C-02-PAGE: тот же нижний край списка — точка подгрузки следующей страницы
  // (`nextCursor`). В бенч-окне guard `isChannelMode` не пускает запрос.
  // C-02-UI-OLDER: верхний край — более ранняя история по `olderCursor`/`before`.
  // Порядок важен: при вырожденной геометрии (контент короче окна) оба края
  // «достигнуты» — приоритет у верха, потому что `loadOlderHistory` без
  // `hasOlder`/курсора возвращается синхронно и не блокирует forward; окно
  // мутирует только один запрос за раз (`loading`/`olderLoading`).
  const onScrollHistory = () => {
    if (!isChannelMode() || !channel) return;
    const remaining = options.list.scrollHeight - options.list.clientHeight - options.list.scrollTop;
    if (options.list.scrollTop <= CHANNEL_LOAD_TOP_EDGE_PX) void loadOlderHistory();
    if (remaining <= CHANNEL_LOAD_EDGE_PX) void loadMoreHistory();
  };
  const onFocusAck = () => {
    if (isChannelMode() && channel) void ackIfNeeded(channel.id);
  };
  options.list.addEventListener("scroll", onScrollAck, { passive: true });
  options.list.addEventListener("scroll", onScrollHistory, { passive: true });
  options.list.addEventListener("focus", onFocusAck);
  options.list.addEventListener("focusin", onFocusAck);

  return {
    store,
    list,
    toolbar,
    refreshRow,
    jumpTo,
    /** @param {import("./composer.js").Composer} instance */
    bindComposer(instance) {
      composer = instance;
      returnMessageToComposer = (text) => {
        const input = document.getElementById("composer-input");
        if (!input) return;
        const field = /** @type {HTMLTextAreaElement} */ (/** @type {unknown} */ (input));
        // P2: не затирать черновик, набранный пользователем, пока IPC отвечал.
        if (field.value.trim() !== "") return;
        field.value = text;
        instance.refresh();
      };
    },
    sendMessage,
    openChannel,
    /** L-02-UI: live-пилюля из dispatch-события (PLAN §3.5). */
    applyReaction,
    /** @returns {string} реальный channelId или bench-«news» (G6-D-01) */
    get channelId() {
      return currentChannelId();
    },
    /** @returns {{ id: string, name: string } | null} */
    get channel() {
      return channel;
    },
    /** @returns {"bench" | "channel"} */
    get mode() {
      return mode;
    },
    /** @returns {number} высота полосы непрочитанного из токена D-13 */
    get unreadHeight() {
      return 34;
    },
  };
}

/** @typedef {ReturnType<typeof createChatScreen>} ChatScreen */

/**
 * Хелпер для app.js: контейнер и поле поиска чата из разметки.
 * @param {(error: UIError) => void} onError
 * @returns {ChatScreen}
 */
/** @param {(error: UIError) => void} onError @param {(cmd: string, args: any) => Promise<any>} [invoke] */
export function initChatScreen(onError, invoke) {
  const list = byId("chat-list");
  if (!list) throw new UIError("ELEMENT_MISSING", "в DOM нет #chat-list");
  return createChatScreen({ list, onError, invoke });
}
