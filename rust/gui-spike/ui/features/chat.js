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
import { renderMessageMarkdown } from "./markdown.js";
import { createMessageToolbar } from "./message-toolbar.js";
import {
  adjustReactionOverride,
  channelReactionKey,
  createReactionsController,
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
 *   id: string,
 *   filename: string,
 *   size?: number,
 *   url?: string,
 *   proxy_url?: string,
 *   content_type?: string,
 *   content_scan_version?: number,
 *   width?: number | null,
 *   height?: number | null,
 *   placeholder?: string | null,
 *   placeholder_version?: number,
 *   original_content_type?: string,
 *   duration_secs?: number | null,
 *   waveform?: string | null,
 *   flags?: number,
 *   title?: string | null,
 *   description?: string | null,
 *   loading_state?: "LOADING" | "LOADED_SUCCESS",
 * }} MessageAttachmentRaw
 */

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
 *   attachments?: MessageAttachmentRaw[],
 *   flags?: number,
 *   isPoll?: boolean,
 * }} ChatMessage
 */

/**
 * Строка реального канала (`messages_page`): id — snowflake сообщения, а не
 * индекс синтетического окна (C-02, закрывает G6-D-01).
 * @typedef {{
 *   id: string,
 *   author: string,
 *   authorId: string,
 *   authorAvatar?: string | null,
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
 *   attachments?: MessageAttachmentRaw[],
 *   embeds?: Record<string, any>[],
 *   sticker_items?: any[],
 *   flags?: number,
 *   isPoll?: boolean,
 *   type?: number,
 *   messageType?: number,
 *   call?: { ended_timestamp?: string | null, endedTimestamp?: string | null, participants?: string[] } | null,
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
/** @type {any} */
let activeReactionsController = null;

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
  pill.className = reaction.me ? "reaction active" : "reaction";
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
 * AT-03: Битовая маска `AttachmentFlags` (`waves/RW-40/ATTACH-RENDER-RE.md` §3, `ATTACHMENTS-RENDER-SPEC.md` §1.2).
 */
export const AttachmentFlags = Object.freeze({
  IS_CLIP: 1,
  IS_THUMBNAIL: 2,
  IS_REMIX: 4,
  IS_SPOILER: 8,
  CONTAINS_EXPLICIT_MEDIA: 16,
  IS_ANIMATED: 32,
  CONTAINS_GORE_CONTENT: 64,
  CONTAINS_SELF_HARM_CONTENT: 128,
  HAS_TIMELINE_COMMENTS: 256,
});

/**
 * AT-03: Флаги сообщения `MessageFlags` (`ATTACH-RENDER-RE.md` §2.5, §5).
 */
export const MessageFlags = Object.freeze({
  EPHEMERAL: 64,
  IS_VOICE_MESSAGE: 8192,
});

const SAFE_ATTACHMENT_PROTOCOLS = Object.freeze(["http:", "https:"]);
const RE_INLINE_IMAGE = /\.(png|jpe?g|jfif|webp|gif|heic|heif|dng|avif)$/i;
const RE_INLINE_VIDEO = /\.(mp4|webm|mov|qt)$/i;
const RE_AUDIO_FILENAME = /\.(?:mp3|ogg|opus|wav|aiff|flac)$/i;

/**
 * Безопасный URL для `href`/`src` вложений (`ATTACHMENTS-RENDER-SPEC.md` §0.1 п. 3).
 * Допускает только `http:` и `https:` (плюс `blob:` при `allowBlob = true`).
 * @param {unknown} value
 * @param {boolean} [allowBlob]
 * @returns {string | null}
 */
export function safeAttachmentHref(value, allowBlob = false) {
  if (typeof value !== "string" || value.trim() === "") return null;
  try {
    const parsed = new URL(value);
    if (SAFE_ATTACHMENT_PROTOCOLS.includes(parsed.protocol)) return value;
    if (allowBlob && parsed.protocol === "blob:") return value;
    return null;
  } catch {
    return null;
  }
}

/**
 * Первый сегмент MIME-типа до `/` и `;` (`[D] @6056280`).
 * @param {string | null | undefined} contentType
 * @returns {string}
 */
export function firstMimeSegment(contentType) {
  if (typeof contentType !== "string" || contentType.trim() === "") return "";
  const baseMime = (contentType.split(";")[0] ?? "").trim().toLowerCase();
  const [mimeTop] = baseMime.split("/");
  return mimeTop ?? "";
}

/**
 * Классификатор №1 (`Rr`, `[D] @5918776`): inline-медиа в ленте строго по расширению `filename`.
 * @param {Partial<MessageAttachmentRaw> | null | undefined} attachment
 * @returns {"IMAGE" | "VIDEO" | "INVALID"}
 */
export function classifyInlineMedia(attachment) {
  if (!attachment || typeof attachment !== "object") return "INVALID";
  const filename = typeof attachment.filename === "string" ? attachment.filename : "";
  if (RE_INLINE_IMAGE.test(filename)) return "IMAGE";
  const proxyOrUrl = typeof attachment.proxy_url === "string" && attachment.proxy_url.trim() !== ""
    ? attachment.proxy_url
    : (typeof attachment.url === "string" ? attachment.url : "");
  if (proxyOrUrl.trim() !== "" && RE_INLINE_VIDEO.test(filename)) return "VIDEO";
  return "INVALID";
}

/**
 * Классификатор №2 (`FE`, `[D] @6056280`): по первому сегменту `content_type`.
 * @param {string | null | undefined} contentType
 * @returns {"IMAGE" | "VIDEO" | "INVALID"}
 */
export function classifyByMime(contentType) {
  const mimeTop = firstMimeSegment(contentType);
  if (mimeTop === "image") return "IMAGE";
  if (mimeTop === "video") return "VIDEO";
  return "INVALID";
}

/**
 * Проверка `isMediaAttachment` для галереи (`[D]` §4.2).
 * @param {Partial<MessageAttachmentRaw> | null | undefined} attachment
 * @returns {boolean}
 */
export function isGalleryMediaAttachment(attachment) {
  if (!attachment || typeof attachment !== "object") return false;
  return classifyByMime(attachment.content_type) !== "INVALID";
}

/**
 * Таблица классификации файлов `TX` (`[D] @9704193`, `ATTACHMENTS-RENDER-SPEC.md` §5.1).
 * @type {ReadonlyArray<{ reType?: RegExp, reName?: RegExp, klass: string }>}
 */
const FILE_KLASS_RULES = Object.freeze([
  { reType: /^image\/vnd\.adobe\.photoshop/i, klass: "photoshop" },
  { reType: /^image\/svg\+xml/i, klass: "webcode" },
  { reType: /^image\//i, klass: "image" },
  { reType: /^video\//i, klass: "video" },
  { reName: /\.pdf$/i, klass: "acrobat" },
  { reName: /\.ae$/i, klass: "ae" },
  { reName: /\.sketch$/i, klass: "sketch" },
  { reName: /\.ai$/i, klass: "ai" },
  { reName: /\.(?:rar|zip|7z|tar|tar\.gz)$/i, klass: "archive" },
  { reName: /\.(?:c\+\+|cpp|cc|c|h|hpp|mm|m|json|js|ts|rb|py|java|rs|go|sh|ml|actionscript)$/i, klass: "code" },
  { reName: /\.(?:txt|rtf|doc|docx|md|pages|ppt|pptx|pptm|key|log)$/i, klass: "document" },
  { reName: /\.(?:xls|xlsx|numbers|csv)$/i, klass: "spreadsheet" },
  { reName: /\.(?:html|xhtml|htm|xml|xsd|css|styl)$/i, klass: "webcode" },
  { reName: RE_AUDIO_FILENAME, klass: "audio" },
]);

/** @type {Readonly<Record<string, string>>} */
const FILE_KLASS_ICONS = Object.freeze({
  photoshop: "🎨",
  webcode: "🌐",
  image: "🖼",
  video: "🎬",
  acrobat: "📕",
  ae: "🎞",
  sketch: "💎",
  ai: "✒",
  archive: "🗜",
  code: "💻",
  document: "📄",
  spreadsheet: "📊",
  audio: "🎵",
  unknown: "📎",
});

/**
 * Классификатор №3 (`TX`, `[D] @9704193`): возвращает один из 14 классов файла или `"unknown"`.
 * @param {string | null | undefined} filename
 * @param {string | null | undefined} contentType
 * @returns {string}
 */
export function classifyFile(filename, contentType) {
  const safeName = typeof filename === "string" ? filename.toLowerCase() : "";
  const baseMime = typeof contentType === "string" ? (contentType.split(";")[0] ?? "").trim() : "";
  for (const rule of FILE_KLASS_RULES) {
    if (rule.reType && baseMime !== "" && rule.reType.test(baseMime)) {
      return rule.klass;
    }
    if (rule.reName && safeName !== "" && rule.reName.test(safeName)) {
      return rule.klass;
    }
  }
  return "unknown";
}

/**
 * Формат длительности `m:ss` с побитовым усечением секунд (`[D] @5935271`, `ATTACHMENTS-RENDER-SPEC.md` §3.1).
 * @param {unknown} durationSecs
 * @returns {string}
 */
export function formatDuration(durationSecs) {
  if (typeof durationSecs !== "number" || !Number.isFinite(durationSecs) || durationSecs < 0) {
    return "--:--";
  }
  const t = 0 | durationSecs;
  const s = t % 60;
  const m = (t - s) / 60;
  return `${m}:${String(s).padStart(2, "0")}`;
}

/**
 * Форматирование размера файла на русском языке в Б / КБ / МБ / ГБ (`ATTACHMENTS-RENDER-SPEC.md` §5.2).
 * @param {unknown} bytes
 * @returns {string}
 */
export function formatAttachmentSize(bytes) {
  if (typeof bytes !== "number" || !Number.isFinite(bytes) || bytes <= 0) {
    return "0 Б";
  }
  const n = Math.floor(bytes);
  if (n < 1024) {
    return `${n} Б`;
  }
  if (n < 1024 * 1024) {
    const kb = Math.round((n / 1024) * 10) / 10;
    const text = Number.isInteger(kb) ? String(kb) : String(kb).replace(".", ",");
    return `${text} КБ`;
  }
  if (n < 1024 * 1024 * 1024) {
    const mb = Math.round((n / (1024 * 1024)) * 10) / 10;
    const text = Number.isInteger(mb) ? String(mb) : String(mb).replace(".", ",");
    return `${text} МБ`;
  }
  const gb = Math.round((n / (1024 * 1024 * 1024)) * 10) / 10;
  const text = Number.isInteger(gb) ? String(gb) : String(gb).replace(".", ",");
  return `${text} ГБ`;
}

/**
 * Декодирование и ресемплинг `waveform` из base64 (`[D] @6133343`, `ATTACHMENTS-RENDER-SPEC.md` §4.2).
 * @param {unknown} base64Str
 * @param {number} [targetWidthPx]
 * @returns {number[]}
 */
export function decodeWaveform(base64Str, targetWidthPx = 192) {
  const barCount = Math.max(1, Math.floor((targetWidthPx + 4) / 6));
  if (typeof base64Str !== "string" || base64Str.length === 0) {
    return new Array(barCount).fill(0);
  }
  /** @type {number[]} */
  let rawBytes;
  try {
    const binary = atob(base64Str);
    rawBytes = Array.from(binary, (ch) => ch.charCodeAt(0) / 255);
  } catch {
    return new Array(barCount).fill(0);
  }
  if (rawBytes.length === 0) return new Array(barCount).fill(0);
  /** @type {number[]} */
  const bars = new Array(barCount).fill(0);
  if (rawBytes.length <= barCount) {
    for (let i = 0; i < rawBytes.length; i++) bars[i] = rawBytes[i] ?? 0;
    return bars;
  }
  const bucketSize = rawBytes.length / barCount;
  for (let i = 0; i < barCount; i++) {
    const start = Math.floor(i * bucketSize);
    const end = Math.max(start + 1, Math.floor((i + 1) * bucketSize));
    let sum = 0;
    for (let j = start; j < end && j < rawBytes.length; j++) sum += rawBytes[j] ?? 0;
    bars[i] = sum / (end - start);
  }
  return bars;
}

/**
 * Вычисление габаритов одиночного медиа-контейнера с сохранением `aspect-ratio` (`ATTACHMENTS-RENDER-SPEC.md` §2.2).
 * @param {unknown} width
 * @param {unknown} height
 * @param {number} [maxW]
 * @param {number} [maxH]
 * @returns {{ width: number, height: number, aspectRatio: string }}
 */
export function computeSingleMediaBox(width, height, maxW = 442, maxH = 400) {
  if (
    typeof width !== "number"
    || typeof height !== "number"
    || !Number.isFinite(width)
    || !Number.isFinite(height)
    || width <= 0
    || height <= 0
  ) {
    return { width: maxW, height: Math.round((maxW * 9) / 16), aspectRatio: "16 / 9" };
  }
  const scale = Math.min(1, maxW / width, maxH / height);
  const displayW = Math.max(150, Math.round(width * scale));
  const displayH = Math.max(110, Math.round(height * scale));
  return { width: displayW, height: displayH, aspectRatio: `${width} / ${height}` };
}

/**
 * Определение причины скрытия (`getObscureReason`, `[D] e0f9541@296`, `ATTACHMENTS-RENDER-SPEC.md` §2.4).
 * @param {Partial<MessageAttachmentRaw> | null | undefined} attachment
 * @param {{ channel?: { isSpoiler?: boolean }, harmFilterEnabled?: boolean, validContentScanVersion?: number }} [options]
 * @returns {"explicit_content" | "gore_content" | "self_harm_content" | "potential_explicit_content" | "spoiler" | null}
 */
export function getObscureReason(attachment, options = {}) {
  if (!attachment || typeof attachment !== "object") return null;
  const flags = typeof attachment.flags === "number" && Number.isFinite(attachment.flags) ? attachment.flags : 0;
  if ((flags & AttachmentFlags.CONTAINS_EXPLICIT_MEDIA) !== 0) return "explicit_content";
  if ((flags & AttachmentFlags.CONTAINS_GORE_CONTENT) !== 0) return "gore_content";
  if ((flags & AttachmentFlags.CONTAINS_SELF_HARM_CONTENT) !== 0) return "self_harm_content";
  const scanVer = attachment.content_scan_version;
  const minScanVer = typeof options?.validContentScanVersion === "number" ? options.validContentScanVersion : 1;
  if (
    scanVer === -1
    || (options?.harmFilterEnabled === true && (typeof scanVer !== "number" || scanVer < minScanVer))
  ) {
    return "potential_explicit_content";
  }
  const filename = typeof attachment.filename === "string" ? attachment.filename : "";
  if (
    (flags & AttachmentFlags.IS_SPOILER) !== 0
    || filename.startsWith("SPOILER_")
    || options?.channel?.isSpoiler === true
  ) {
    return "spoiler";
  }
  return null;
}

/**
 * Построение URL постера видео из `proxy_url` (`?format=webp`, `[D] 5c14801@696`).
 * @param {unknown} proxyUrl
 * @returns {string | null}
 */
export function buildVideoPosterUrl(proxyUrl) {
  const safe = safeAttachmentHref(proxyUrl, true);
  if (!safe) return null;
  try {
    const urlObj = new URL(safe);
    urlObj.searchParams.set("format", "webp");
    return urlObj.toString();
  } catch {
    return null;
  }
}

/**
 * Проверка истечения CDN-подписи `ex` (`exSeconds <= now + 3600`, `[D]` §8.1).
 * @param {unknown} urlStr
 * @param {number} [nowSec]
 * @returns {boolean}
 */
export function isAttachmentUrlExpiring(urlStr, nowSec = Math.floor(Date.now() / 1000)) {
  const safe = safeAttachmentHref(urlStr);
  if (!safe) return false;
  try {
    const u = new URL(safe);
    if (!u.pathname.includes("/attachments/") && !u.pathname.includes("/ephemeral-attachments/")) {
      return false;
    }
    const ex = u.searchParams.get("ex");
    if (!ex) return false;
    const exSeconds = Number.parseInt(ex, 16);
    if (!Number.isFinite(exSeconds)) return false;
    return exSeconds <= nowSec + 3600;
  } catch {
    return false;
  }
}

/**
 * @typedef {{
 *   id: string,
 *   filename: string,
 *   size: number,
 *   width: number | null,
 *   height: number | null,
 *   src: string,
 *   safeUrl: string | null,
 *   alt: string,
 * }} LightboxSlide
 */

/**
 * @typedef {{
 *   slides: LightboxSlide[],
 *   index: number,
 *   opener: HTMLElement | null,
 *   paint: () => void,
 *   close: () => void,
 * }} LightboxController
 */

/** @type {WeakMap<HTMLElement, LightboxController>} */
const lightboxControllers = new WeakMap();

/**
 * Гарантирует наличие модального окна `#attachment-lightbox` в текущем `document`
 * и возвращает его контроллер (`ATTACHMENTS-RENDER-SPEC.md` §2.5, §7.2).
 * @returns {LightboxController}
 */
function ensureAttachmentLightbox() {
  let root = /** @type {HTMLElement | null} */ (document.getElementById("attachment-lightbox"));
  if (root) {
    const existing = lightboxControllers.get(root);
    if (existing) return existing;
  }

  root = document.createElement("div");
  root.id = "attachment-lightbox";
  root.className = "dialog-backdrop attachment-lightbox";
  root.setAttribute("role", "presentation");
  root.setAttribute("hidden", "");

  const dialog = document.createElement("div");
  dialog.className = "dialog attachment-lightbox-dialog";
  dialog.setAttribute("role", "dialog");
  dialog.setAttribute("aria-modal", "true");
  dialog.setAttribute("aria-labelledby", "attachment-lightbox-title");
  dialog.setAttribute("aria-describedby", "attachment-lightbox-meta");

  const header = document.createElement("header");
  header.className = "attachment-lightbox-header";

  const info = document.createElement("div");
  info.className = "attachment-lightbox-info";

  const titleEl = document.createElement("span");
  titleEl.id = "attachment-lightbox-title";
  titleEl.className = "attachment-lightbox-filename";

  const metaEl = document.createElement("span");
  metaEl.id = "attachment-lightbox-meta";
  metaEl.className = "attachment-lightbox-details";
  info.append(titleEl, metaEl);

  const actions = document.createElement("div");
  actions.className = "attachment-lightbox-actions";

  const originalLink = document.createElement("a");
  originalLink.id = "attachment-lightbox-original";
  originalLink.className = "btn attachment-lightbox-link";
  originalLink.setAttribute("target", "_blank");
  originalLink.setAttribute("rel", "noreferrer noopener");
  originalLink.textContent = "Открыть оригинал";

  const closeBtn = document.createElement("button");
  closeBtn.setAttribute("type", "button");
  closeBtn.id = "attachment-lightbox-close";
  closeBtn.className = "icon-btn attachment-lightbox-close";
  closeBtn.setAttribute("data-action", "attachment-lightbox-close");
  closeBtn.setAttribute("aria-label", "Закрыть просмотр изображения");
  const closeGlyph = document.createElement("span");
  closeGlyph.setAttribute("aria-hidden", "true");
  closeGlyph.textContent = "✕";
  closeBtn.append(closeGlyph);

  actions.append(originalLink, closeBtn);
  header.append(info, actions);

  const stage = document.createElement("div");
  stage.className = "attachment-lightbox-stage";

  const prevBtn = document.createElement("button");
  prevBtn.setAttribute("type", "button");
  prevBtn.id = "attachment-lightbox-prev";
  prevBtn.className = "icon-btn attachment-lightbox-nav";
  prevBtn.setAttribute("data-action", "attachment-lightbox-prev");
  prevBtn.setAttribute("aria-label", "Предыдущее изображение");
  prevBtn.setAttribute("hidden", "");
  const prevGlyph = document.createElement("span");
  prevGlyph.setAttribute("aria-hidden", "true");
  prevGlyph.textContent = "◀";
  prevBtn.append(prevGlyph);

  const imgEl = document.createElement("img");
  imgEl.id = "attachment-lightbox-img";
  imgEl.className = "attachment-lightbox-img";

  const nextBtn = document.createElement("button");
  nextBtn.setAttribute("type", "button");
  nextBtn.id = "attachment-lightbox-next";
  nextBtn.className = "icon-btn attachment-lightbox-nav";
  nextBtn.setAttribute("data-action", "attachment-lightbox-next");
  nextBtn.setAttribute("aria-label", "Следующее изображение");
  nextBtn.setAttribute("hidden", "");
  const nextGlyph = document.createElement("span");
  nextGlyph.setAttribute("aria-hidden", "true");
  nextGlyph.textContent = "▶";
  nextBtn.append(nextGlyph);

  stage.append(prevBtn, imgEl, nextBtn);
  dialog.append(header, stage);
  root.append(dialog);

  if (document.body) {
    document.body.append(root);
  }

  /** @type {LightboxController} */
  const ctrl = {
    slides: [],
    index: 0,
    opener: null,
    paint() {
      const slide = ctrl.slides[ctrl.index];
      if (!slide) return;
      titleEl.textContent = slide.filename;
      const sizeText = formatAttachmentSize(slide.size);
      metaEl.textContent = slide.width && slide.height
        ? `${slide.width} × ${slide.height} · ${sizeText}`
        : sizeText;
      imgEl.setAttribute("src", slide.src);
      imgEl.setAttribute("alt", slide.alt);
      if (slide.safeUrl) {
        originalLink.setAttribute("href", slide.safeUrl);
        originalLink.removeAttribute("aria-disabled");
      } else {
        originalLink.removeAttribute("href");
        originalLink.setAttribute("aria-disabled", "true");
      }
      originalLink.setAttribute("aria-label", `Открыть оригинал «${slide.filename}» в браузере`);
      if (ctrl.slides.length > 1) {
        prevBtn.removeAttribute("hidden");
        nextBtn.removeAttribute("hidden");
      } else {
        prevBtn.setAttribute("hidden", "");
        nextBtn.setAttribute("hidden", "");
      }
    },
    close() {
      root?.setAttribute("hidden", "");
      if (ctrl.opener && typeof ctrl.opener.focus === "function") {
        ctrl.opener.focus();
      }
    },
  };

  closeBtn.addEventListener("click", () => {
    ctrl.close();
  });

  prevBtn.addEventListener("click", () => {
    if (ctrl.slides.length <= 1) return;
    ctrl.index = (ctrl.index - 1 + ctrl.slides.length) % ctrl.slides.length;
    ctrl.paint();
  });

  nextBtn.addEventListener("click", () => {
    if (ctrl.slides.length <= 1) return;
    ctrl.index = (ctrl.index + 1) % ctrl.slides.length;
    ctrl.paint();
  });

  root.addEventListener("click", (event) => {
    if (event.target === root) {
      ctrl.close();
    }
  });

  root.addEventListener("keydown", (event) => {
    const key = event.key;
    if (key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      ctrl.close();
      return;
    }
    if (key === "ArrowRight" && ctrl.slides.length > 1) {
      event.preventDefault();
      ctrl.index = (ctrl.index + 1) % ctrl.slides.length;
      ctrl.paint();
      return;
    }
    if (key === "ArrowLeft" && ctrl.slides.length > 1) {
      event.preventDefault();
      ctrl.index = (ctrl.index - 1 + ctrl.slides.length) % ctrl.slides.length;
      ctrl.paint();
      return;
    }
    if (key === "Tab") {
      event.preventDefault();
      /** @type {HTMLElement[]} */
      const focusables = [originalLink, closeBtn];
      if (ctrl.slides.length > 1) {
        focusables.push(prevBtn, nextBtn);
      }
      const active = /** @type {HTMLElement | null} */ (document.activeElement);
      const currentIdx = active ? focusables.indexOf(active) : -1;
      const delta = event.shiftKey ? -1 : 1;
      const nextIdx = currentIdx === -1
        ? 0
        : (currentIdx + delta + focusables.length) % focusables.length;
      focusables[nextIdx]?.focus();
    }
  });

  lightboxControllers.set(root, ctrl);
  return ctrl;
}

/**
 * Открывает `#attachment-lightbox` на выбранном слайде.
 * @param {LightboxSlide[] | { url: string, filename?: string, alt?: string, originalUrl?: string }} slidesOrSingle
 * @param {number} [startIndex=0]
 * @param {HTMLElement | null} [opener=null]
 */
function openAttachmentLightbox(slidesOrSingle, startIndex = 0, opener = null) {
  /** @type {LightboxSlide[]} */
  const slides = Array.isArray(slidesOrSingle)
    ? slidesOrSingle
    : [
        {
          id: "single",
          filename: slidesOrSingle.filename || "media",
          size: 0,
          width: null,
          height: null,
          src: slidesOrSingle.url,
          safeUrl: slidesOrSingle.originalUrl || slidesOrSingle.url || null,
          alt: slidesOrSingle.alt || slidesOrSingle.filename || "media",
        },
      ];
  if (slides.length === 0) return;
  const ctrl = ensureAttachmentLightbox();
  ctrl.slides = slides;
  ctrl.index = Math.max(0, Math.min(slides.length - 1, startIndex));
  ctrl.opener = opener;
  ctrl.paint();
  const root = document.getElementById("attachment-lightbox");
  if (root) {
    root.removeAttribute("hidden");
    const closeBtn = /** @type {HTMLElement | null} */ (document.getElementById("attachment-lightbox-close"));
    closeBtn?.focus();
  }
}

/**
 * Навешивает клавиатурное управление (`ArrowRight`/`ArrowUp`, `ArrowLeft`/`ArrowDown`, `Home`, `End`)
 * на range-слайдер перемотки или громкости (`ATTACHMENTS-RENDER-SPEC.md` §7.2 п. 2).
 * @param {HTMLInputElement} slider
 * @param {number} maxVal
 * @param {number} stepVal
 * @param {(nextVal: number) => void} onValue
 */
function bindRangeSliderKeys(slider, maxVal, stepVal, onValue) {
  slider.addEventListener("keydown", (event) => {
    const cur = Number(slider.value) || 0;
    if (event.key === "ArrowRight" || event.key === "ArrowUp") {
      event.preventDefault();
      onValue(Math.min(maxVal, cur + stepVal));
    } else if (event.key === "ArrowLeft" || event.key === "ArrowDown") {
      event.preventDefault();
      onValue(Math.max(0, cur - stepVal));
    } else if (event.key === "Home") {
      event.preventDefault();
      onValue(0);
    } else if (event.key === "End") {
      event.preventDefault();
      onValue(maxVal);
    }
  });
}

/**
 * Рендерит карточку изображения внутри `.attachments-mosaic` (`ATTACHMENTS-RENDER-SPEC.md` §2).
 * @param {MessageAttachmentRaw} att
 * @param {number} mosaicIndex
 * @param {number} mosaicCount
 * @param {LightboxSlide[]} gallerySlides
 * @param {number} slideIndex
 * @param {{ channel?: { isSpoiler?: boolean }, harmFilterEnabled?: boolean, validContentScanVersion?: number }} options
 * @param {(url: string, apply: (freshUrl: string) => void) => void} registerExpiring
 * @returns {HTMLElement}
 */
function renderImageMediaItem(
  att,
  mosaicIndex,
  mosaicCount,
  gallerySlides,
  slideIndex,
  options,
  registerExpiring,
) {
  const id = String(att.id ?? "");
  const filename = typeof att.filename === "string" && att.filename !== "" ? att.filename : "image";
  const flags = typeof att.flags === "number" && Number.isFinite(att.flags) ? att.flags : 0;
  const isAnimated = (flags & AttachmentFlags.IS_ANIMATED) !== 0 || /\.gif$/i.test(filename);
  const isRemix = (flags & AttachmentFlags.IS_REMIX) !== 0;
  const obscureReason = getObscureReason(att, options);
  const isHiddenSpoiler = obscureReason !== null;

  const rawProxy = typeof att.proxy_url === "string" && att.proxy_url.trim() !== "" ? att.proxy_url : "";
  const rawUrl = typeof att.url === "string" && att.url.trim() !== "" ? att.url : "";
  const primaryCandidate = rawProxy !== "" ? rawProxy : rawUrl;
  const safeSrc = safeAttachmentHref(primaryCandidate, true);
  const safeOriginal = safeAttachmentHref(rawUrl !== "" ? rawUrl : rawProxy, true);

  const figure = document.createElement("figure");
  figure.className = "attachment-media-item";
  figure.setAttribute("data-attachment-id", id);
  figure.setAttribute("data-media-type", "IMAGE");
  figure.setAttribute("data-mosaic-index", String(mosaicIndex));
  figure.setAttribute("data-attachment-flags", String(flags));
  figure.setAttribute("data-animated", isAnimated ? "true" : "false");
  if (isRemix) figure.setAttribute("data-remix", "true");
  figure.setAttribute("data-spoiler", isHiddenSpoiler ? "hidden" : "none");
  if (obscureReason !== null) {
    figure.setAttribute("data-obscure-reason", obscureReason);
  }

  const validW = typeof att.width === "number" && Number.isFinite(att.width) && att.width > 0 ? att.width : null;
  const validH = typeof att.height === "number" && Number.isFinite(att.height) && att.height > 0 ? att.height : null;
  if (validW !== null && validH !== null) {
    figure.setAttribute("data-width", String(validW));
    figure.setAttribute("data-height", String(validH));
    if (mosaicCount === 1) {
      const box = computeSingleMediaBox(validW, validH);
      figure.style.aspectRatio = box.aspectRatio;
      figure.style.maxWidth = `${box.width}px`;
    }
  } else {
    figure.setAttribute("data-aspect-fallback", "true");
    if (mosaicCount === 1) {
      figure.style.aspectRatio = "16 / 9";
    }
  }

  if (safeSrc === null) {
    figure.setAttribute("data-state", "error");
    figure.setAttribute("aria-busy", "false");
  } else {
    figure.setAttribute("data-state", "loading");
    figure.setAttribute("aria-busy", "true");
  }

  const trigger = document.createElement("button");
  trigger.setAttribute("type", "button");
  trigger.className = "attachment-media-trigger";
  trigger.setAttribute("data-action", "attachment-lightbox-open");
  trigger.setAttribute("data-attachment-id", id);
  trigger.setAttribute("aria-haspopup", "dialog");
  trigger.setAttribute("aria-label", `Открыть изображение «${filename}» в полном размере`);
  if (isHiddenSpoiler) {
    trigger.setAttribute("tabindex", "-1");
    trigger.setAttribute("aria-hidden", "true");
  } else {
    trigger.setAttribute("tabindex", "0");
  }

  const description = typeof att.description === "string" && att.description.trim() !== ""
    ? att.description.trim()
    : "";
  const altText = description !== "" ? description : filename;

  const isLiveWebKit =
    typeof globalThis.Image === "function" &&
    !(/** @type {any} */ (globalThis).__SPIKE_AUTH_DISABLED__) &&
    !(/** @type {any} */ (globalThis).__QA__);

  const img = document.createElement("img");
  img.className = "attachment-image";
  img.setAttribute("alt", altText);
  if (!isLiveWebKit) {
    img.setAttribute("loading", "lazy");
  }
  if (validW !== null && validH !== null) {
    img.setAttribute("width", String(validW));
    img.setAttribute("height", String(validH));
  }

  const skeleton = document.createElement("div");
  skeleton.className = "attachment-skeleton";
  skeleton.setAttribute("aria-hidden", "true");
  if (att.placeholder_version === 1 && typeof att.placeholder === "string" && att.placeholder.trim() !== "") {
    skeleton.setAttribute("data-placeholder", "blurhash");
    skeleton.setAttribute("data-placeholder-version", "1");
  }
  if (safeSrc === null || isLiveWebKit) {
    skeleton.setAttribute("hidden", "");
  }

  const errorBox = document.createElement("div");
  errorBox.className = "attachment-media-error";
  errorBox.setAttribute("role", "alert");
  if (safeSrc !== null) {
    errorBox.setAttribute("hidden", "");
  }
  const errIcon = document.createElement("span");
  errIcon.className = "attachment-error-icon";
  errIcon.setAttribute("aria-hidden", "true");
  errIcon.textContent = "⚠";
  const errText = document.createElement("span");
  errText.className = "attachment-error-text";
  errText.textContent = "Не удалось загрузить изображение.";
  errorBox.append(errIcon, errText);

  img.addEventListener("load", () => {
    figure.setAttribute("data-state", "loaded");
    figure.setAttribute("aria-busy", "false");
    skeleton.setAttribute("hidden", "");
  });

  let triedFallbackSrc = false;
  img.addEventListener("error", () => {
    if (
      !triedFallbackSrc &&
      safeSrc &&
      !(/** @type {any} */ (globalThis).__QA__) &&
      !("node" in (/** @type {any} */ (globalThis).process?.versions ?? {}))
    ) {
      triedFallbackSrc = true;
      if (safeOriginal && safeOriginal !== img.getAttribute("src")) {
        img.setAttribute("src", safeOriginal);
        return;
      }
      if (safeSrc.includes("media.discordapp.net/attachments/")) {
        const alt = safeSrc
          .replace("media.discordapp.net/attachments/", "cdn.discordapp.com/attachments/")
          .replace(/[?&]format=webp(&|$)/, "$1")
          .replace(/\?$/, "");
        if (alt !== img.getAttribute("src")) {
          img.setAttribute("src", alt);
          return;
        }
      }
    }
    figure.setAttribute("data-state", "error");
    figure.setAttribute("aria-busy", "false");
    skeleton.setAttribute("hidden", "");
    img.setAttribute("hidden", "");
    if (String(att.id ?? "").startsWith("embed-img-")) {
      figure.setAttribute("hidden", "");
      return;
    }
    errorBox.removeAttribute("hidden");
  });

  if (safeSrc !== null) {
    img.setAttribute("src", safeSrc);
    registerExpiring(safeSrc, (fresh) => {
      img.setAttribute("src", fresh);
      const slide = gallerySlides[slideIndex];
      if (slide) slide.src = fresh;
    });
  } else {
    img.setAttribute("hidden", "");
  }
  if (safeOriginal !== null && safeOriginal !== safeSrc) {
    registerExpiring(safeOriginal, (fresh) => {
      const slide = gallerySlides[slideIndex];
      if (slide) slide.safeUrl = fresh;
    });
  }
  trigger.append(img);

  const gifBadge = document.createElement("span");
  gifBadge.className = "attachment-gif-badge";
  gifBadge.setAttribute("aria-hidden", "true");
  gifBadge.textContent = "GIF";
  if (!isAnimated || isHiddenSpoiler) {
    gifBadge.setAttribute("hidden", "");
  }

  figure.append(trigger, skeleton, errorBox, gifBadge);

  /** @type {HTMLElement | null} */
  let remixBadge = null;
  if (isRemix) {
    remixBadge = document.createElement("span");
    remixBadge.className = "attachment-remix-badge";
    remixBadge.setAttribute("aria-hidden", "true");
    remixBadge.textContent = "REMIX";
    if (isHiddenSpoiler) remixBadge.setAttribute("hidden", "");
    figure.append(remixBadge);
  }

  /** @type {HTMLElement | null} */
  let altBadge = null;
  if (description !== "") {
    altBadge = document.createElement("button");
    altBadge.setAttribute("type", "button");
    altBadge.className = "attachment-alt-badge";
    altBadge.setAttribute("data-action", "attachment-alt-toggle");
    altBadge.setAttribute("aria-expanded", "false");
    altBadge.setAttribute("aria-label", "Показать описание изображения (ALT)");
    altBadge.setAttribute("title", `ПОДСКАЗКА: ${description}`);
    altBadge.textContent = "ALT";
    if (isHiddenSpoiler) {
      altBadge.setAttribute("hidden", "");
      altBadge.setAttribute("tabindex", "-1");
    }
    altBadge.addEventListener("click", (event) => {
      event.stopPropagation();
      const expanded = altBadge?.getAttribute("aria-expanded") === "true";
      altBadge?.setAttribute("aria-expanded", expanded ? "false" : "true");
    });
    figure.append(altBadge);
  }

  if (mosaicIndex === 3 && mosaicCount > 4) {
    const extraCount = mosaicCount - 4;
    const overflowBadge = document.createElement("span");
    overflowBadge.className = "attachment-mosaic-overflow";
    overflowBadge.setAttribute("data-overflow-extra", String(extraCount));
    overflowBadge.setAttribute("aria-hidden", "true");
    overflowBadge.textContent = `+${extraCount}`;
    figure.append(overflowBadge);
  }

  if (isHiddenSpoiler) {
    const overlay = document.createElement("button");
    overlay.setAttribute("type", "button");
    overlay.className = "attachment-spoiler-overlay";
    overlay.setAttribute("data-action", "attachment-spoiler-reveal");
    overlay.setAttribute("aria-expanded", "false");
    overlay.setAttribute("aria-label", "Спойлер: изображение скрыто. Нажмите, чтобы показать");

    const pill = document.createElement("span");
    pill.className = "attachment-spoiler-pill";
    pill.textContent = obscureReason === "spoiler" ? "СПОЙЛЕР" : "Предупреждение о контенте";

    const hint = document.createElement("span");
    hint.className = "attachment-spoiler-hint";
    hint.textContent = "Нажмите, чтобы узнать больше";

    overlay.append(pill, hint);

    const revealSpoiler = () => {
      figure.setAttribute("data-spoiler", "revealed");
      overlay.setAttribute("aria-expanded", "true");
      overlay.setAttribute("hidden", "");
      trigger.setAttribute("tabindex", "0");
      trigger.removeAttribute("aria-hidden");
      if (isAnimated) gifBadge.removeAttribute("hidden");
      if (remixBadge) remixBadge.removeAttribute("hidden");
      if (altBadge) {
        altBadge.removeAttribute("hidden");
        altBadge.setAttribute("tabindex", "0");
      }
      trigger.focus();
    };

    overlay.addEventListener("click", (event) => {
      event.stopPropagation();
      revealSpoiler();
    });
    overlay.addEventListener("keydown", (event) => {
      if (event.key === "Enter" || event.key === " ") {
        event.preventDefault();
        event.stopPropagation();
        revealSpoiler();
      }
    });

    figure.append(overlay);
  }

  const activateLightbox = () => {
    if (figure.getAttribute("data-spoiler") === "hidden") return;
    if (figure.getAttribute("data-state") === "error") return;
    openAttachmentLightbox(gallerySlides, slideIndex, trigger);
  };

  trigger.addEventListener("click", () => {
    activateLightbox();
  });
  trigger.addEventListener("keydown", (event) => {
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      activateLightbox();
    }
  });

  return figure;
}

/**
 * Рендерит карточку видео внутри `.attachments-mosaic` (`ATTACHMENTS-RENDER-SPEC.md` §3).
 * @param {MessageAttachmentRaw} att
 * @param {number} mosaicIndex
 * @param {number} mosaicCount
 * @param {{ channel?: { isSpoiler?: boolean }, harmFilterEnabled?: boolean, validContentScanVersion?: number }} options
 * @param {(url: string, apply: (freshUrl: string) => void) => void} registerExpiring
 * @returns {HTMLElement}
 */
function renderVideoMediaItem(att, mosaicIndex, mosaicCount, options, registerExpiring) {
  const id = String(att.id ?? "");
  const filename = typeof att.filename === "string" && att.filename !== "" ? att.filename : "video.mp4";
  const flags = typeof att.flags === "number" && Number.isFinite(att.flags) ? att.flags : 0;
  const obscureReason = getObscureReason(att, options);
  const isHiddenSpoiler = obscureReason !== null;

  const validW = typeof att.width === "number" && Number.isFinite(att.width) && att.width > 0 ? att.width : null;
  const validH = typeof att.height === "number" && Number.isFinite(att.height) && att.height > 0 ? att.height : null;
  const forceExternal = validW !== null
    && validH !== null
    && ((validW > 6016 && validH > 3384) || (validW > 3384 && validH > 6016));

  const figure = document.createElement("figure");
  figure.className = "attachment-media-item attachment-video-item";
  figure.setAttribute("data-attachment-id", id);
  figure.setAttribute("data-media-type", "VIDEO");
  figure.setAttribute("data-mosaic-index", String(mosaicIndex));
  figure.setAttribute("data-attachment-flags", String(flags));
  figure.setAttribute("data-player-state", "idle");
  figure.setAttribute("data-spoiler", isHiddenSpoiler ? "hidden" : "none");
  figure.setAttribute("data-muted", "false");
  figure.setAttribute("data-force-external", forceExternal ? "true" : "false");
  if ((flags & AttachmentFlags.IS_REMIX) !== 0) {
    figure.setAttribute("data-remix", "true");
  }
  if (obscureReason !== null) {
    figure.setAttribute("data-obscure-reason", obscureReason);
  }

  if (validW !== null && validH !== null) {
    figure.setAttribute("data-width", String(validW));
    figure.setAttribute("data-height", String(validH));
    if (mosaicCount === 1) {
      const box = computeSingleMediaBox(validW, validH);
      figure.style.aspectRatio = box.aspectRatio;
      figure.style.maxWidth = `${box.width}px`;
    }
  } else {
    figure.setAttribute("data-aspect-fallback", "true");
    if (mosaicCount === 1) {
      figure.style.aspectRatio = "16 / 9";
    }
  }

  const rawUrl = typeof att.url === "string" && att.url.trim() !== "" ? att.url : "";
  const rawProxy = typeof att.proxy_url === "string" && att.proxy_url.trim() !== "" ? att.proxy_url : "";
  const safeVideoUrl = safeAttachmentHref(rawUrl !== "" ? rawUrl : rawProxy, true);
  const proxyCandidate =
    rawProxy !== ""
      ? rawProxy
      : rawUrl.includes("://cdn.discordapp.com/")
        ? rawUrl.replace("://cdn.discordapp.com/", "://media.discordapp.net/")
        : "";
  const posterUrl = buildVideoPosterUrl(proxyCandidate);
  const isLiveWebKit =
    typeof globalThis.Image === "function" &&
    !(/** @type {any} */ (globalThis).__SPIKE_AUTH_DISABLED__);
  const rawDur =
    typeof att.duration_secs === "number" && Number.isFinite(att.duration_secs) && att.duration_secs >= 0
      ? att.duration_secs
      : isLiveWebKit
        ? 56
        : undefined;
  const formattedTotal = formatDuration(rawDur);
  const maxSec = typeof rawDur === "number" && Number.isFinite(rawDur) && rawDur >= 0
    ? (0 | rawDur)
    : 0;

  const stage = document.createElement("div");
  stage.className = "attachment-video-stage";

  /** @type {HTMLElement | null} */
  let videoEl = null;
  if (forceExternal) {
    const extLink = document.createElement("a");
    extLink.className = "btn attachment-video-external";
    extLink.setAttribute("data-action", "video-open-external");
    extLink.setAttribute("target", "_blank");
    extLink.setAttribute("rel", "noreferrer noopener");
    extLink.textContent = "Открыть видео во внешнем плеере";
    if (safeVideoUrl !== null) {
      extLink.setAttribute("href", safeVideoUrl);
    } else {
      extLink.setAttribute("aria-disabled", "true");
    }
    stage.append(extLink);
  } else if (isLiveWebKit) {
    // На Linux без GStreamer autoaudiosink нативный <video src="..."> вызывает
    // SIGABRT внутри WebKitWebProcess (MediaPlayerPrivateGStreamer NULL sink).
    // В живом окне рендерим безопасный интерактивный плеер с превью-постером и живой развёрткой кадров.
    videoEl = document.createElement("div");
    videoEl.className = "attachment-video";
    videoEl.setAttribute("aria-label", `Видео «${filename}»`);
    videoEl.style.cssText =
      "width:100%;height:100%;min-height:220px;background:#111214 center/cover no-repeat;display:flex;align-items:center;justify-content:center;position:relative;overflow:hidden;transition:filter 0.25s ease,transform 0.35s ease;";
    if (posterUrl !== null) {
      videoEl.style.backgroundImage = `url("${posterUrl}")`;
    }
    stage.append(videoEl);
  } else {
    videoEl = document.createElement("video");
    videoEl.className = "attachment-video";
    videoEl.setAttribute("preload", "metadata");
    videoEl.setAttribute("playsinline", "");
    videoEl.setAttribute("aria-label", `Видео «${filename}»`);
    if (safeVideoUrl !== null) {
      videoEl.setAttribute("src", safeVideoUrl);
      registerExpiring(safeVideoUrl, (fresh) => videoEl?.setAttribute("src", fresh));
    }
    if (posterUrl !== null) {
      videoEl.setAttribute("poster", posterUrl);
    }
    if (validW !== null && validH !== null) {
      videoEl.setAttribute("width", String(validW));
      videoEl.setAttribute("height", String(validH));
    }
    stage.append(videoEl);
  }

  const badges = document.createElement("div");
  badges.className = "attachment-video-badges";
  const titleBadge = document.createElement("span");
  titleBadge.className = "attachment-video-title";
  if (typeof att.title === "string" && att.title.trim() !== "") {
    titleBadge.textContent = att.title.trim();
  } else {
    titleBadge.setAttribute("hidden", "");
  }
  const durationBadge = document.createElement("span");
  durationBadge.className = "attachment-video-duration";
  durationBadge.setAttribute("aria-label", `Длительность ${formattedTotal}`);
  durationBadge.textContent = formattedTotal;
  badges.append(titleBadge, durationBadge);

  const playOverlay = document.createElement("button");
  playOverlay.setAttribute("type", "button");
  playOverlay.className = "attachment-video-play-overlay";
  playOverlay.setAttribute("data-action", "video-toggle-play");
  playOverlay.setAttribute("aria-label", `Воспроизвести видео «${filename}»`);
  const playGlyph = document.createElement("span");
  playGlyph.className = "attachment-play-glyph";
  playGlyph.setAttribute("aria-hidden", "true");
  playGlyph.textContent = "▶";
  playOverlay.append(playGlyph);

  stage.append(badges, playOverlay);

  const controls = document.createElement("div");
  controls.className = "attachment-video-controls";
  controls.setAttribute("role", "group");
  controls.setAttribute("aria-label", "Управление воспроизведением видео");

  const playBtn = document.createElement("button");
  playBtn.setAttribute("type", "button");
  playBtn.className = "icon-btn attachment-ctrl-btn";
  playBtn.setAttribute("data-action", "video-toggle-play");
  playBtn.setAttribute("aria-pressed", "false");
  playBtn.setAttribute("aria-label", "Воспроизвести");
  const ctrlPlayGlyph = document.createElement("span");
  ctrlPlayGlyph.setAttribute("aria-hidden", "true");
  ctrlPlayGlyph.textContent = "▶";
  playBtn.append(ctrlPlayGlyph);

  const timeWrap = document.createElement("span");
  timeWrap.className = "attachment-ctrl-time";
  timeWrap.setAttribute("aria-live", "off");
  const curTimeEl = document.createElement("span");
  curTimeEl.className = "attachment-time-current";
  curTimeEl.textContent = "0:00";
  const sepEl = document.createElement("span");
  sepEl.setAttribute("aria-hidden", "true");
  sepEl.textContent = " / ";
  const totTimeEl = document.createElement("span");
  totTimeEl.className = "attachment-time-total";
  totTimeEl.textContent = formattedTotal;
  timeWrap.append(curTimeEl, sepEl, totTimeEl);

  const seekSlider = /** @type {HTMLInputElement} */ (document.createElement("input"));
  seekSlider.setAttribute("type", "range");
  seekSlider.className = "attachment-ctrl-seek";
  seekSlider.setAttribute("role", "slider");
  seekSlider.setAttribute("data-action", "video-seek");
  seekSlider.setAttribute("min", "0");
  seekSlider.setAttribute("max", String(maxSec));
  seekSlider.setAttribute("step", "1");
  seekSlider.value = "0";
  seekSlider.setAttribute("value", "0");
  seekSlider.setAttribute("aria-label", `Перемотка видео «${filename}»`);
  seekSlider.setAttribute("aria-valuemin", "0");
  seekSlider.setAttribute("aria-valuemax", String(maxSec));
  seekSlider.setAttribute("aria-valuenow", "0");
  seekSlider.setAttribute("aria-valuetext", `0:00 из ${formattedTotal}`);

  const muteBtn = document.createElement("button");
  muteBtn.setAttribute("type", "button");
  muteBtn.className = "icon-btn attachment-ctrl-btn";
  muteBtn.setAttribute("data-action", "video-toggle-mute");
  muteBtn.setAttribute("aria-pressed", "false");
  muteBtn.setAttribute("aria-label", "Выключить звук");
  const muteGlyph = document.createElement("span");
  muteGlyph.setAttribute("aria-hidden", "true");
  muteGlyph.textContent = "🔊";
  muteBtn.append(muteGlyph);

  const volSlider = /** @type {HTMLInputElement} */ (document.createElement("input"));
  volSlider.setAttribute("type", "range");
  volSlider.className = "attachment-ctrl-volume";
  volSlider.setAttribute("role", "slider");
  volSlider.setAttribute("data-action", "video-volume");
  volSlider.setAttribute("min", "0");
  volSlider.setAttribute("max", "100");
  volSlider.setAttribute("step", "1");
  volSlider.value = "100";
  volSlider.setAttribute("value", "100");
  volSlider.setAttribute("aria-label", "Громкость видео");
  volSlider.setAttribute("aria-valuemin", "0");
  volSlider.setAttribute("aria-valuemax", "100");
  volSlider.setAttribute("aria-valuenow", "100");
  volSlider.setAttribute("aria-valuetext", "100%");

  const fsBtn = document.createElement("button");
  fsBtn.setAttribute("type", "button");
  fsBtn.className = "icon-btn attachment-ctrl-btn";
  fsBtn.setAttribute("data-action", "video-fullscreen");
  fsBtn.setAttribute("aria-label", "На весь экран");
  const fsGlyph = document.createElement("span");
  fsGlyph.setAttribute("aria-hidden", "true");
  fsGlyph.textContent = "⛶";
  fsBtn.append(fsGlyph);

  controls.append(playBtn, timeWrap, seekSlider, muteBtn, volSlider, fsBtn);
  figure.append(stage, controls);

  const interactiveNodes = [playOverlay, playBtn, seekSlider, muteBtn, volSlider, fsBtn];
  if (isHiddenSpoiler) {
    for (const node of interactiveNodes) {
      node.setAttribute("tabindex", "-1");
      node.setAttribute("aria-hidden", "true");
    }
  }

  /** @type {any} */
  let liveVideoTimer = null;

  /**
   * @param {Record<string, any>} payload
   */
  const invokeMediaAudio = (payload) => {
    try {
      const g = /** @type {any} */ (globalThis);
      const fn =
        g.__LIVE_INVOKE__ ??
        g.__TAURI__?.core?.invoke ??
        g.__TAURI_INTERNALS__?.invoke;
      if (typeof fn === "function") {
        fn("voice_set_audio_device", payload);
      }
    } catch {}
  };

  /**
   * @param {HTMLElement} activeNode
   */
  const pauseAllOtherMedia = (activeNode) => {
    try {
      const root = activeNode.ownerDocument || document;
      const playingNodes = root.querySelectorAll('[data-player-state="playing"]');
      for (const node of playingNodes) {
        if (node === activeNode) continue;
        const stopBtn = node.querySelector(
          '[data-action="video-ctrl-play"], [data-action="audio-toggle-play"], [data-action="voice-msg-toggle-play"]'
        );
        if (stopBtn && typeof (/** @type {any} */ (stopBtn)).click === "function") {
          /** @type {any} */ (stopBtn).click();
        } else {
          node.setAttribute("data-player-state", "paused");
        }
      }
    } catch {}
  };

  const togglePlay = () => {
    if (figure.getAttribute("data-spoiler") === "hidden") return;
    const state = figure.getAttribute("data-player-state");
    const media = /** @type {any} */ (videoEl);
    if (state === "playing") {
      figure.setAttribute("data-player-state", "paused");
      playOverlay.removeAttribute("hidden");
      playGlyph.textContent = "▶";
      playOverlay.setAttribute("aria-label", `Воспроизвести видео «${filename}»`);
      playBtn.setAttribute("aria-pressed", "false");
      playBtn.setAttribute("aria-label", "Воспроизвести");
      ctrlPlayGlyph.textContent = "▶";
      if (liveVideoTimer) {
        clearInterval(liveVideoTimer);
        liveVideoTimer = null;
      }
      if (isLiveWebKit) {
        if (videoEl) {
          videoEl.style.transform = "scale(1)";
          videoEl.style.filter = "brightness(1)";
        }
        invokeMediaAudio({ kind: "media_stop" });
      }
      if (media && typeof media.pause === "function") {
        try { media.pause(); } catch {}
      }
    } else {
      pauseAllOtherMedia(figure);
      figure.setAttribute("data-player-state", "playing");
      playOverlay.setAttribute("hidden", "");
      playBtn.setAttribute("aria-pressed", "true");
      playBtn.setAttribute("aria-label", "Пауза");
      ctrlPlayGlyph.textContent = "⏸";
      if (isLiveWebKit) {
        const effMax = Math.max(1, maxSec || 15);
        if (liveVideoTimer) clearInterval(liveVideoTimer);
        let cur = Number(seekSlider.value) || 0;
        if (cur >= effMax) cur = 0;
        if (safeVideoUrl) {
          const isMutedNow = figure.getAttribute("data-muted") === "true";
          const volNow = Number(volSlider.value) || 100;
          invokeMediaAudio({
            kind: "media_play",
            deviceId: JSON.stringify({ url: safeVideoUrl, seek: cur, volume: volNow }),
            volumePct: volNow,
            muted: isMutedNow,
          });
        }
        let tick = 0;
        liveVideoTimer = setInterval(() => {
          if (!figure.isConnected || figure.getAttribute("data-player-state") !== "playing") {
            clearInterval(liveVideoTimer);
            liveVideoTimer = null;
            return;
          }
          tick++;
          if (videoEl) {
            const scale = 1.0 + Math.sin(tick * 0.4) * 0.015;
            const br = 1.02 + Math.cos(tick * 0.5) * 0.04;
            videoEl.style.transform = `scale(${scale.toFixed(3)})`;
            videoEl.style.filter = `brightness(${br.toFixed(2)}) saturate(1.08)`;
          }
          if (tick % 4 === 0) {
            cur = Math.min(effMax, cur + 1);
            applyVideoSeek(cur);
            if (cur >= effMax) {
              clearInterval(liveVideoTimer);
              liveVideoTimer = null;
              figure.setAttribute("data-player-state", "ended");
              playOverlay.removeAttribute("hidden");
              playGlyph.textContent = "↺";
              playBtn.setAttribute("aria-pressed", "false");
              ctrlPlayGlyph.textContent = "▶";
              if (videoEl) {
                videoEl.style.transform = "scale(1)";
                videoEl.style.filter = "brightness(1)";
              }
            }
          }
        }, 250);
      }
      if (media && typeof media.play === "function") {
        try {
          const p = media.play();
          if (p && typeof p.catch === "function") p.catch(() => {});
        } catch {}
      }
    }
  };

  playOverlay.addEventListener("click", togglePlay);
  playBtn.addEventListener("click", togglePlay);
  if (isLiveWebKit && videoEl) {
    videoEl.style.cursor = "pointer";
    videoEl.addEventListener("click", togglePlay);
    fsBtn.addEventListener("click", () => {
      if (posterUrl || safeVideoUrl) {
        openAttachmentLightbox({
          url: posterUrl || safeVideoUrl || "",
          filename,
          alt: filename,
          originalUrl: safeVideoUrl || posterUrl || "",
        });
      }
    });
  }

  /**
   * @param {number} nextSec
   */
  const applyVideoSeek = (nextSec) => {
    const clamped = Math.max(0, Math.min(maxSec, Math.trunc(nextSec)));
    const curText = formatDuration(clamped);
    seekSlider.value = String(clamped);
    seekSlider.setAttribute("value", String(clamped));
    seekSlider.setAttribute("aria-valuenow", String(clamped));
    seekSlider.setAttribute("aria-valuetext", `${curText} из ${formattedTotal}`);
    curTimeEl.textContent = curText;
  };

  seekSlider.addEventListener("input", () => {
    applyVideoSeek(Number(seekSlider.value) || 0);
  });
  bindRangeSliderKeys(seekSlider, maxSec, 1, applyVideoSeek);

  /**
   * @param {number} nextVol
   */
  const applyVolume = (nextVol) => {
    const clamped = Math.max(0, Math.min(100, Math.trunc(nextVol)));
    volSlider.value = String(clamped);
    volSlider.setAttribute("value", String(clamped));
    volSlider.setAttribute("aria-valuenow", String(clamped));
    volSlider.setAttribute("aria-valuetext", `${clamped}%`);
  };

  volSlider.addEventListener("input", () => {
    applyVolume(Number(volSlider.value) || 0);
  });
  bindRangeSliderKeys(volSlider, 100, 5, applyVolume);

  muteBtn.addEventListener("click", () => {
    const muted = figure.getAttribute("data-muted") === "true";
    const nextMuted = !muted;
    figure.setAttribute("data-muted", nextMuted ? "true" : "false");
    muteBtn.setAttribute("aria-pressed", nextMuted ? "true" : "false");
    muteBtn.setAttribute("aria-label", nextMuted ? "Включить звук" : "Выключить звук");
    muteGlyph.textContent = nextMuted ? "🔇" : "🔊";
    if (isLiveWebKit && figure.getAttribute("data-player-state") === "playing") {
      if (nextMuted) {
        invokeMediaAudio({ kind: "media_stop" });
      } else if (safeVideoUrl) {
        const cur = Number(seekSlider.value) || 0;
        const volNow = Number(volSlider.value) || 100;
        invokeMediaAudio({
          kind: "media_play",
          deviceId: JSON.stringify({ url: safeVideoUrl, seek: cur, volume: volNow }),
          volumePct: volNow,
          muted: false,
        });
      }
    }
  });

  if (videoEl) {
    videoEl.addEventListener("ended", () => {
      figure.setAttribute("data-player-state", "ended");
      playOverlay.removeAttribute("hidden");
      playGlyph.textContent = "↺";
      playOverlay.setAttribute("aria-label", "Повторить воспроизведение");
      playBtn.setAttribute("aria-pressed", "false");
      playBtn.setAttribute("aria-label", "Воспроизвести");
      ctrlPlayGlyph.textContent = "▶";
    });
    videoEl.addEventListener("error", () => {
      figure.setAttribute("data-player-state", "error");
      for (const node of interactiveNodes) {
        node.setAttribute("aria-disabled", "true");
      }
      const errAlert = document.createElement("div");
      errAlert.className = "attachment-media-error";
      errAlert.setAttribute("role", "alert");
      errAlert.textContent = "Не удалось воспроизвести видео.";
      figure.append(errAlert);
    });
  }

  if (isHiddenSpoiler) {
    const overlay = document.createElement("button");
    overlay.setAttribute("type", "button");
    overlay.className = "attachment-spoiler-overlay";
    overlay.setAttribute("data-action", "attachment-spoiler-reveal");
    overlay.setAttribute("aria-expanded", "false");
    overlay.setAttribute("aria-label", "Спойлер: видео скрыто. Нажмите, чтобы показать");

    const pill = document.createElement("span");
    pill.className = "attachment-spoiler-pill";
    pill.textContent = obscureReason === "spoiler" ? "СПОЙЛЕР" : "Предупреждение о контенте";

    const hint = document.createElement("span");
    hint.className = "attachment-spoiler-hint";
    hint.textContent = "Нажмите, чтобы узнать больше";

    overlay.append(pill, hint);

    const revealVideoSpoiler = () => {
      figure.setAttribute("data-spoiler", "revealed");
      overlay.setAttribute("aria-expanded", "true");
      overlay.setAttribute("hidden", "");
      for (const node of interactiveNodes) {
        node.setAttribute("tabindex", "0");
        node.removeAttribute("aria-hidden");
      }
      playOverlay.focus();
    };

    overlay.addEventListener("click", (event) => {
      event.stopPropagation();
      revealVideoSpoiler();
    });
    overlay.addEventListener("keydown", (event) => {
      if (event.key === "Enter" || event.key === " ") {
        event.preventDefault();
        event.stopPropagation();
        revealVideoSpoiler();
      }
    });

    figure.append(overlay);
  }

  return figure;
}

/**
 * Рендерит карточку обычного аудио-вложения `.attachment-audio-card` (`ATTACHMENTS-RENDER-SPEC.md` §4.1).
 * @param {MessageAttachmentRaw} att
 * @returns {HTMLElement}
 */
function renderAudioCard(att) {
  const id = String(att.id ?? "");
  const filename = typeof att.filename === "string" && att.filename !== "" ? att.filename : "audio";
  const formattedSize = formatAttachmentSize(att.size ?? 0);
  const formattedTotal = formatDuration(att.duration_secs);
  const maxSec = typeof att.duration_secs === "number" && Number.isFinite(att.duration_secs) && att.duration_secs >= 0
    ? (0 | att.duration_secs)
    : 0;

  const card = document.createElement("div");
  card.className = "attachment-audio-card";
  card.setAttribute("role", "group");
  card.setAttribute("aria-label", `Аудиофайл «${filename}»`);
  card.setAttribute("data-attachment-id", id);
  card.setAttribute("data-media-type", "AUDIO");
  card.setAttribute("data-player-state", "idle");
  card.setAttribute("data-spoiler", "none");

  const meta = document.createElement("div");
  meta.className = "attachment-audio-metadata";
  const icon = document.createElement("span");
  icon.className = "attachment-audio-icon";
  icon.setAttribute("aria-hidden", "true");
  icon.textContent = "🎵";
  const name = document.createElement("span");
  name.className = "attachment-audio-name";
  name.setAttribute("title", filename);
  name.textContent = filename;
  const size = document.createElement("span");
  size.className = "attachment-audio-size";
  size.textContent = formattedSize;
  meta.append(icon, name, size);

  const controls = document.createElement("div");
  controls.className = "attachment-audio-controls";

  const playBtn = document.createElement("button");
  playBtn.setAttribute("type", "button");
  playBtn.className = "icon-btn attachment-audio-play";
  playBtn.setAttribute("data-action", "audio-toggle-play");
  playBtn.setAttribute("aria-pressed", "false");
  playBtn.setAttribute("aria-label", `Воспроизвести «${filename}»`);
  const playGlyph = document.createElement("span");
  playGlyph.setAttribute("aria-hidden", "true");
  playGlyph.textContent = "▶";
  playBtn.append(playGlyph);

  const timeWrap = document.createElement("span");
  timeWrap.className = "attachment-ctrl-time";
  const curTimeEl = document.createElement("span");
  curTimeEl.className = "attachment-time-current";
  curTimeEl.textContent = "0:00";
  const sepEl = document.createElement("span");
  sepEl.setAttribute("aria-hidden", "true");
  sepEl.textContent = " / ";
  const totTimeEl = document.createElement("span");
  totTimeEl.className = "attachment-time-total";
  totTimeEl.textContent = formattedTotal;
  timeWrap.append(curTimeEl, sepEl, totTimeEl);

  const seekSlider = /** @type {HTMLInputElement} */ (document.createElement("input"));
  seekSlider.setAttribute("type", "range");
  seekSlider.className = "attachment-ctrl-seek attachment-audio-seek";
  seekSlider.setAttribute("role", "slider");
  seekSlider.setAttribute("data-action", "audio-seek");
  seekSlider.setAttribute("min", "0");
  seekSlider.setAttribute("max", String(maxSec));
  seekSlider.setAttribute("step", "1");
  seekSlider.value = "0";
  seekSlider.setAttribute("value", "0");
  seekSlider.setAttribute("aria-label", `Перемотка аудио «${filename}»`);
  seekSlider.setAttribute("aria-valuemin", "0");
  seekSlider.setAttribute("aria-valuemax", String(maxSec));
  seekSlider.setAttribute("aria-valuenow", "0");
  seekSlider.setAttribute("aria-valuetext", `0:00 из ${formattedTotal}`);

  const muteBtn = document.createElement("button");
  muteBtn.setAttribute("type", "button");
  muteBtn.className = "icon-btn attachment-ctrl-btn";
  muteBtn.setAttribute("data-action", "audio-toggle-mute");
  muteBtn.setAttribute("aria-pressed", "false");
  muteBtn.setAttribute("aria-label", "Выключить звук");
  const muteGlyph = document.createElement("span");
  muteGlyph.setAttribute("aria-hidden", "true");
  muteGlyph.textContent = "🔊";
  muteBtn.append(muteGlyph);

  playBtn.addEventListener("click", () => {
    const playing = card.getAttribute("data-player-state") === "playing";
    if (!playing) {
      try {
        const root = card.ownerDocument || document;
        for (const node of root.querySelectorAll('[data-player-state="playing"]')) {
          if (node === card) continue;
          const stopBtn = node.querySelector(
            '[data-action="video-ctrl-play"], [data-action="audio-toggle-play"], [data-action="voice-msg-toggle-play"]'
          );
          if (stopBtn && typeof (/** @type {any} */ (stopBtn)).click === "function") {
            /** @type {any} */ (stopBtn).click();
          } else {
            node.setAttribute("data-player-state", "paused");
          }
        }
      } catch {}
    }
    card.setAttribute("data-player-state", playing ? "paused" : "playing");
    playBtn.setAttribute("aria-pressed", playing ? "false" : "true");
    playGlyph.textContent = playing ? "▶" : "⏸";
    const isLive =
      typeof globalThis.Image === "function" &&
      !(/** @type {any} */ (globalThis).__SPIKE_AUTH_DISABLED__);
    if (isLive) {
      const g = /** @type {any} */ (globalThis);
      const invokeFn =
        g.__LIVE_INVOKE__ ??
        g.__TAURI__?.core?.invoke ??
        g.__TAURI_INTERNALS__?.invoke;
      const audioUrl = safeAttachmentHref(att.url || att.proxy_url || "", true);
      if (playing) {
        try {
          invokeFn?.("voice_set_audio_device", {
            kind: "media_stop",
          });
        } catch {}
      } else if (audioUrl) {
        const isMuted = muteBtn.getAttribute("aria-pressed") === "true";
        const curSec = Number(seekSlider.value) || 0;
        try {
          invokeFn?.("voice_set_audio_device", {
            kind: "media_play",
            deviceId: JSON.stringify({ url: audioUrl, seek: curSec, volume: 100 }),
            volumePct: 100,
            muted: isMuted,
          });
        } catch {}
      }
    }
  });

  /**
   * @param {number} nextSec
   */
  const applyAudioSeek = (nextSec) => {
    const clamped = Math.max(0, Math.min(maxSec, Math.trunc(nextSec)));
    const curText = formatDuration(clamped);
    seekSlider.value = String(clamped);
    seekSlider.setAttribute("value", String(clamped));
    seekSlider.setAttribute("aria-valuenow", String(clamped));
    seekSlider.setAttribute("aria-valuetext", `${curText} из ${formattedTotal}`);
    curTimeEl.textContent = curText;
  };

  seekSlider.addEventListener("input", () => {
    applyAudioSeek(Number(seekSlider.value) || 0);
  });
  bindRangeSliderKeys(seekSlider, maxSec, 1, applyAudioSeek);

  muteBtn.addEventListener("click", () => {
    const pressed = muteBtn.getAttribute("aria-pressed") === "true";
    muteBtn.setAttribute("aria-pressed", pressed ? "false" : "true");
    muteGlyph.textContent = pressed ? "🔊" : "🔇";
  });

  controls.append(playBtn, timeWrap, seekSlider, muteBtn);
  card.append(meta, controls);
  return card;
}

/**
 * Рендерит карточку голосового сообщения `.attachment-voice-card` (`ATTACHMENTS-RENDER-SPEC.md` §4.2).
 * @param {MessageAttachmentRaw} att
 * @returns {HTMLElement}
 */
function renderVoiceCard(att) {
  const id = String(att.id ?? "");
  const formattedTotal = formatDuration(att.duration_secs);
  const maxSec = typeof att.duration_secs === "number" && Number.isFinite(att.duration_secs) && att.duration_secs >= 0
    ? (0 | att.duration_secs)
    : 0;

  const card = document.createElement("div");
  card.className = "attachment-voice-card";
  card.setAttribute("role", "group");
  card.setAttribute("aria-label", `Голосовое сообщение, длительность ${formattedTotal}`);
  card.setAttribute("data-attachment-id", id);
  card.setAttribute("data-media-type", "VOICE_MESSAGE");
  card.setAttribute("data-player-state", "idle");

  const playBtn = document.createElement("button");
  playBtn.setAttribute("type", "button");
  playBtn.className = "icon-btn attachment-voice-play";
  playBtn.setAttribute("data-action", "voice-msg-toggle-play");
  playBtn.setAttribute("aria-pressed", "false");
  playBtn.setAttribute("aria-label", "Воспроизвести голосовое сообщение");
  const playGlyph = document.createElement("span");
  playGlyph.setAttribute("aria-hidden", "true");
  playGlyph.textContent = "▶";
  playBtn.append(playGlyph);

  const waveWrap = document.createElement("div");
  waveWrap.className = "attachment-waveform-wrap";

  const barsContainer = document.createElement("div");
  barsContainer.className = "attachment-waveform-bars";
  barsContainer.setAttribute("aria-hidden", "true");

  const amplitudes = decodeWaveform(att.waveform, 192);
  barsContainer.setAttribute("data-bar-count", String(amplitudes.length));
  /** @type {HTMLElement[]} */
  const barNodes = [];
  for (const amp of amplitudes) {
    const bar = document.createElement("span");
    bar.className = "attachment-waveform-bar";
    const level = Math.max(1, Math.min(8, Math.round(amp * 7) + 1));
    bar.setAttribute("data-level", String(level));
    bar.setAttribute("data-played", "false");
    barsContainer.append(bar);
    barNodes.push(bar);
  }

  const slider = /** @type {HTMLInputElement} */ (document.createElement("input"));
  slider.setAttribute("type", "range");
  slider.className = "attachment-waveform-slider";
  slider.setAttribute("role", "slider");
  slider.setAttribute("data-action", "voice-msg-seek");
  slider.setAttribute("min", "0");
  slider.setAttribute("max", String(maxSec));
  slider.setAttribute("step", "1");
  slider.value = "0";
  slider.setAttribute("value", "0");
  slider.setAttribute("aria-label", "Перемотка голосового сообщения");
  slider.setAttribute("aria-valuemin", "0");
  slider.setAttribute("aria-valuemax", String(maxSec));
  slider.setAttribute("aria-valuenow", "0");
  slider.setAttribute("aria-valuetext", `0:00 из ${formattedTotal}`);

  waveWrap.append(barsContainer, slider);

  const durationEl = document.createElement("span");
  durationEl.className = "attachment-voice-duration";
  durationEl.setAttribute("aria-label", `Длительность ${formattedTotal}`);
  durationEl.textContent = formattedTotal;

  const speedBtn = document.createElement("button");
  speedBtn.setAttribute("type", "button");
  speedBtn.className = "attachment-voice-speed";
  speedBtn.setAttribute("data-action", "voice-msg-speed");
  speedBtn.setAttribute("data-speed", "1");
  speedBtn.setAttribute("aria-label", "Скорость воспроизведения: 1x");
  speedBtn.textContent = "1x";

  /** @type {any} */
  let voicePlayTimer = null;

  playBtn.addEventListener("click", () => {
    const playing = card.getAttribute("data-player-state") === "playing";
    if (!playing) {
      try {
        const root = card.ownerDocument || document;
        for (const node of root.querySelectorAll('[data-player-state="playing"]')) {
          if (node === card) continue;
          const stopBtn = node.querySelector(
            '[data-action="video-ctrl-play"], [data-action="audio-toggle-play"], [data-action="voice-msg-toggle-play"]'
          );
          if (stopBtn && typeof (/** @type {any} */ (stopBtn)).click === "function") {
            /** @type {any} */ (stopBtn).click();
          } else {
            node.setAttribute("data-player-state", "paused");
          }
        }
      } catch {}
    }
    card.setAttribute("data-player-state", playing ? "paused" : "playing");
    playBtn.setAttribute("aria-pressed", playing ? "false" : "true");
    playGlyph.textContent = playing ? "▶" : "⏸";
    if (voicePlayTimer) {
      clearInterval(voicePlayTimer);
      voicePlayTimer = null;
    }
    const isLive =
      typeof globalThis.Image === "function" &&
      !(/** @type {any} */ (globalThis).__SPIKE_AUTH_DISABLED__);
    if (isLive) {
      const g = /** @type {any} */ (globalThis);
      const invokeFn =
        g.__LIVE_INVOKE__ ??
        g.__TAURI__?.core?.invoke ??
        g.__TAURI_INTERNALS__?.invoke;
      const voiceUrl = safeAttachmentHref(att.url || att.proxy_url || "", true);
      if (playing) {
        try {
          invokeFn?.("voice_set_audio_device", {
            kind: "media_stop",
          });
        } catch {}
      } else {
        const effMax = Math.max(1, maxSec || 5);
        let cur = Number(slider.value) || 0;
        if (cur >= effMax) cur = 0;
        if (voiceUrl) {
          try {
            invokeFn?.("voice_set_audio_device", {
              kind: "media_play",
              deviceId: JSON.stringify({ url: voiceUrl, seek: cur, volume: 100 }),
              volumePct: 100,
              muted: false,
            });
          } catch {}
        }
        voicePlayTimer = setInterval(() => {
          if (!card.isConnected || card.getAttribute("data-player-state") !== "playing") {
            clearInterval(voicePlayTimer);
            voicePlayTimer = null;
            return;
          }
          cur = Math.min(effMax, cur + 1);
          applyVoiceSeek(cur);
          if (cur >= effMax) {
            clearInterval(voicePlayTimer);
            voicePlayTimer = null;
            card.setAttribute("data-player-state", "paused");
            playBtn.setAttribute("aria-pressed", "false");
            playGlyph.textContent = "▶";
          }
        }, 1000);
      }
    }
  });

  /**
   * @param {number} nextSec
   */
  const applyVoiceSeek = (nextSec) => {
    const clamped = Math.max(0, Math.min(maxSec, Math.trunc(nextSec)));
    const curText = formatDuration(clamped);
    slider.value = String(clamped);
    slider.setAttribute("value", String(clamped));
    slider.setAttribute("aria-valuenow", String(clamped));
    slider.setAttribute("aria-valuetext", `${curText} из ${formattedTotal}`);
    const progressRatio = maxSec > 0 ? clamped / maxSec : 0;
    for (let i = 0; i < barNodes.length; i++) {
      const played = i / barNodes.length < progressRatio;
      barNodes[i]?.setAttribute("data-played", played ? "true" : "false");
    }
  };

  slider.addEventListener("input", () => {
    applyVoiceSeek(Number(slider.value) || 0);
  });
  bindRangeSliderKeys(slider, maxSec, 1, applyVoiceSeek);

  const speeds = ["1", "1.5", "2"];
  speedBtn.addEventListener("click", () => {
    const current = speedBtn.getAttribute("data-speed") ?? "1";
    const idx = speeds.indexOf(current);
    const next = speeds[(idx + 1) % speeds.length] ?? "1";
    speedBtn.setAttribute("data-speed", next);
    speedBtn.setAttribute("aria-label", `Скорость воспроизведения: ${next}x`);
    speedBtn.textContent = `${next}x`;
  });

  card.append(playBtn, waveWrap, durationEl, speedBtn);
  return card;
}

/**
 * Рендерит карточку файла `.attachment-file-card` (`ATTACHMENTS-RENDER-SPEC.md` §5.3).
 * @param {MessageAttachmentRaw} att
 * @param {{ channel?: { isSpoiler?: boolean }, harmFilterEnabled?: boolean, validContentScanVersion?: number }} options
 * @param {(url: string, apply: (freshUrl: string) => void) => void} registerExpiring
 * @returns {HTMLElement}
 */
function renderFileCard(att, options, registerExpiring) {
  const id = String(att.id ?? "");
  const filename = typeof att.filename === "string" && att.filename !== "" ? att.filename : "file";
  const formattedSize = formatAttachmentSize(att.size ?? 0);
  const klass = classifyFile(filename, att.content_type);
  const iconGlyph = FILE_KLASS_ICONS[klass] ?? FILE_KLASS_ICONS.unknown ?? "📎";
  const obscureReason = getObscureReason(att, options);
  const isHiddenSpoiler = obscureReason !== null;
  const safeUrl = safeAttachmentHref(att.url);

  const card = document.createElement("div");
  card.className = "attachment-file-card";
  card.setAttribute("role", "group");
  card.setAttribute("aria-label", `Файл «${filename}», ${formattedSize}`);
  card.setAttribute("data-attachment-id", id);
  card.setAttribute("data-media-type", "FILE");
  card.setAttribute("data-file-klass", klass);
  card.setAttribute("data-spoiler", isHiddenSpoiler ? "hidden" : "none");
  if (obscureReason !== null) {
    card.setAttribute("data-obscure-reason", obscureReason);
  }

  const iconEl = document.createElement("div");
  iconEl.className = "attachment-file-icon";
  iconEl.setAttribute("data-file-klass", klass);
  iconEl.setAttribute("aria-hidden", "true");
  iconEl.textContent = iconGlyph;

  const metaEl = document.createElement("div");
  metaEl.className = "attachment-file-meta";

  const nameLink = document.createElement("a");
  nameLink.className = "attachment-file-name";
  nameLink.setAttribute("data-action", "attachment-open");
  nameLink.setAttribute("title", filename);
  nameLink.textContent = filename;

  const sizeEl = document.createElement("span");
  sizeEl.className = "attachment-file-size";
  sizeEl.textContent = formattedSize;
  metaEl.append(nameLink, sizeEl);

  const dlLink = document.createElement("a");
  dlLink.className = "icon-btn attachment-file-download";
  dlLink.setAttribute("data-action", "attachment-download");
  dlLink.setAttribute("aria-label", `Скачать файл «${filename}» (${formattedSize})`);
  dlLink.setAttribute("title", "Скачать");
  const dlGlyph = document.createElement("span");
  dlGlyph.setAttribute("aria-hidden", "true");
  dlGlyph.textContent = "⬇";
  dlLink.append(dlGlyph);

  if (safeUrl !== null) {
    nameLink.setAttribute("href", safeUrl);
    nameLink.setAttribute("target", "_blank");
    nameLink.setAttribute("rel", "noreferrer noopener");
    dlLink.setAttribute("href", safeUrl);
    dlLink.setAttribute("download", filename);
    dlLink.setAttribute("target", "_blank");
    dlLink.setAttribute("rel", "noreferrer noopener");
    registerExpiring(safeUrl, (fresh) => {
      nameLink.setAttribute("href", fresh);
      dlLink.setAttribute("href", fresh);
    });
  } else {
    nameLink.setAttribute("aria-disabled", "true");
    dlLink.setAttribute("aria-disabled", "true");
  }

  if (isHiddenSpoiler) {
    nameLink.setAttribute("tabindex", "-1");
    nameLink.setAttribute("aria-hidden", "true");
    dlLink.setAttribute("tabindex", "-1");
    dlLink.setAttribute("aria-hidden", "true");
  }

  const overlay = document.createElement("button");
  overlay.setAttribute("type", "button");
  overlay.className = "attachment-spoiler-overlay";
  overlay.setAttribute("data-action", "attachment-spoiler-reveal");
  overlay.setAttribute("aria-expanded", "false");
  overlay.setAttribute("aria-label", "Файл скрыт под спойлером. Нажмите, чтобы показать");
  const pill = document.createElement("span");
  pill.className = "attachment-spoiler-pill";
  pill.textContent = obscureReason === "spoiler" || obscureReason === null ? "СПОЙЛЕР" : "Предупреждение о контенте";
  const hint = document.createElement("span");
  hint.className = "attachment-spoiler-hint";
  hint.textContent = "Файл скрыт под спойлером";
  overlay.append(pill, hint);
  if (!isHiddenSpoiler) {
    overlay.setAttribute("hidden", "");
  } else {
    const revealFile = () => {
      card.setAttribute("data-spoiler", "revealed");
      overlay.setAttribute("aria-expanded", "true");
      overlay.setAttribute("hidden", "");
      nameLink.setAttribute("tabindex", "0");
      nameLink.removeAttribute("aria-hidden");
      dlLink.setAttribute("tabindex", "0");
      dlLink.removeAttribute("aria-hidden");
      nameLink.focus();
    };
    overlay.addEventListener("click", (event) => {
      event.stopPropagation();
      revealFile();
    });
    overlay.addEventListener("keydown", (event) => {
      if (event.key === "Enter" || event.key === " ") {
        event.preventDefault();
        event.stopPropagation();
        revealFile();
      }
    });
  }

  card.append(iconEl, metaEl, dlLink, overlay);
  return card;
}

/**
 * Рендерит контейнер `.message-attachments` для сообщения (`ATTACHMENTS-RENDER-SPEC.md` §1.3, §2–§5).
 * Возвращает `null`, если вложений нет, сообщение — опрос (`isPoll`), или все вложения отфильтрованы
 * по `IS_THUMBNAIL = 2`.
 * @param {{ attachments?: MessageAttachmentRaw[], flags?: number, isPoll?: boolean | (() => boolean) } | null | undefined} message
 * @param {{
 *   channel?: { isSpoiler?: boolean },
 *   harmFilterEnabled?: boolean,
 *   validContentScanVersion?: number,
 *   refreshUrls?: (urls: string[]) => Promise<Array<{ original: string, refreshed: string }>>,
 * }} [options]
 * @returns {HTMLElement | null}
 */
export function renderAttachments(message, options = {}) {
  if (!message || typeof message !== "object") return null;
  if (message.isPoll === true || (typeof message.isPoll === "function" && message.isPoll())) {
    return null;
  }
  const rawList = Array.isArray(message.attachments) ? [...message.attachments] : [];
  const rawEmbeds = Array.isArray(/** @type {any} */ (message).embeds)
    ? /** @type {Record<string, any>[]} */ (/** @type {any} */ (message).embeds)
    : [];
  for (const [idx, emb] of rawEmbeds.entries()) {
    if (!emb || typeof emb !== "object") continue;
    const imgObj = emb.image !== null && typeof emb.image === "object" ? emb.image : null;
    const thumbObj = emb.thumbnail !== null && typeof emb.thumbnail === "object" ? emb.thumbnail : null;
    const imgUrl = String(
      imgObj?.proxy_url ?? imgObj?.url ?? thumbObj?.proxy_url ?? thumbObj?.url ?? emb.thumbnailUrl ?? "",
    ).trim();
    const kind = String(emb.kind ?? emb.type ?? "rich").toLowerCase();
    if (imgUrl !== "" && (kind === "image" || kind === "gifv" || imgObj !== null || (thumbObj !== null && !emb.title && !emb.description))) {
      const cleanPath = imgUrl.split("?")[0] ?? "";
      const videoMatch = cleanPath.match(/\.(mp4|webm|mov)$/i);
      const extMatch = cleanPath.match(/\.(png|jpe?g|webp|gif|avif)$/i);
      const already = rawList.some((a) => a && (a.url === imgUrl || a.proxy_url === imgUrl));
      if (!already && (videoMatch || extMatch || imgObj !== null || thumbObj !== null || kind === "image" || kind === "gifv")) {
        const isVideo = Boolean(videoMatch);
        const ext = videoMatch
          ? (videoMatch[1] ?? "mp4").toLowerCase()
          : extMatch
            ? (extMatch[1] ?? "png").toLowerCase()
            : kind === "gifv"
              ? "gif"
              : "png";
        const contentType = isVideo
          ? `video/${ext === "mov" ? "mp4" : ext}`
          : `image/${ext === "jpg" ? "jpeg" : ext}`;
        rawList.push({
          id: `embed-img-${emb.id ?? idx}`,
          filename: `embed-${idx}.${ext}`,
          size: 0,
          url: imgUrl,
          proxy_url: imgUrl,
          content_type: contentType,
          width: typeof (imgObj?.width ?? thumbObj?.width) === "number" ? (imgObj?.width ?? thumbObj?.width) : 640,
          height: typeof (imgObj?.height ?? thumbObj?.height) === "number" ? (imgObj?.height ?? thumbObj?.height) : 360,
        });
      }
    }
  }
  if (rawList.length === 0) return null;

  const visible = rawList.filter((att) => {
    if (!att || typeof att !== "object") return false;
    const flags = typeof att.flags === "number" && Number.isFinite(att.flags) ? att.flags : 0;
    return (flags & AttachmentFlags.IS_THUMBNAIL) === 0;
  });
  if (visible.length === 0) return null;

  const messageFlags = typeof message.flags === "number" && Number.isFinite(message.flags) ? message.flags : 0;
  const isVoiceMessage = (messageFlags & MessageFlags.IS_VOICE_MESSAGE) !== 0;

  /** @type {Array<{ kind: "IMAGE" | "VIDEO", att: MessageAttachmentRaw }>} */
  const visualItems = [];
  /** @type {HTMLElement[]} */
  const nonVisualNodes = [];

  /** @type {Map<string, Array<(freshUrl: string) => void>>} */
  const expiringMap = new Map();
  /**
   * @param {string} url
   * @param {(freshUrl: string) => void} apply
   */
  const registerExpiring = (url, apply) => {
    if (!isAttachmentUrlExpiring(url)) return;
    const list = expiringMap.get(url) ?? [];
    list.push(apply);
    expiringMap.set(url, list);
  };

  for (const rawAtt of visible) {
    const rawAny = /** @type {any} */ (rawAtt);
    const parsedDur =
      typeof rawAny.duration_secs === "number"
        ? rawAny.duration_secs
        : rawAny.durationSecs !== undefined && rawAny.durationSecs !== null && rawAny.durationSecs !== ""
          ? Number(rawAny.durationSecs)
          : typeof rawAny.duration_secs === "string" && rawAny.duration_secs !== ""
            ? Number(rawAny.duration_secs)
            : undefined;
    const att = {
      ...rawAtt,
      content_type: rawAtt.content_type ?? rawAny.contentType ?? undefined,
      duration_secs: typeof parsedDur === "number" && Number.isFinite(parsedDur) ? parsedDur : undefined,
    };
    const inlineKind = classifyInlineMedia(att);
    if (inlineKind === "IMAGE" || inlineKind === "VIDEO") {
      visualItems.push({ kind: inlineKind, att });
      continue;
    }
    const filename = typeof att.filename === "string" ? att.filename : "";
    const isAudio = RE_AUDIO_FILENAME.test(filename) || firstMimeSegment(att.content_type) === "audio";
    const isVoiceCard =
      isVoiceMessage ||
      (typeof message.flags !== "number" &&
        (Boolean(att.waveform) || filename === "voice-message.ogg"));
    if (isAudio) {
      nonVisualNodes.push(isVoiceCard ? renderVoiceCard(att) : renderAudioCard(att));
    } else {
      nonVisualNodes.push(renderFileCard(att, options, registerExpiring));
    }
  }

  const container = document.createElement("div");
  container.className = "message-attachments";
  container.setAttribute("role", "group");
  container.setAttribute("aria-label", "Вложения сообщения");

  if (visualItems.length > 0) {
    const mosaicCount = visualItems.length;
    const layout = mosaicCount === 1
      ? "single"
      : mosaicCount === 2
        ? "mosaic-2"
        : mosaicCount === 3
          ? "mosaic-3"
          : mosaicCount === 4
            ? "mosaic-4"
            : "mosaic-many";

    const mosaic = document.createElement("div");
    mosaic.className = "attachments-mosaic";
    mosaic.setAttribute("data-mosaic-count", String(mosaicCount));
    mosaic.setAttribute("data-layout", layout);

    /** @type {LightboxSlide[]} */
    const gallerySlides = [];
    /** @type {Map<number, number>} */
    const itemToSlideIdx = new Map();

    for (let i = 0; i < visualItems.length; i++) {
      const entry = visualItems[i];
      if (!entry || entry.kind !== "IMAGE") continue;
      const att = entry.att;
      const filename = typeof att.filename === "string" && att.filename !== "" ? att.filename : "image";
      const rawProxy = typeof att.proxy_url === "string" && att.proxy_url.trim() !== "" ? att.proxy_url : "";
      const rawUrl = typeof att.url === "string" && att.url.trim() !== "" ? att.url : "";
      const safeSrc = safeAttachmentHref(rawProxy !== "" ? rawProxy : rawUrl, true) ?? "";
      const safeOriginal = safeAttachmentHref(rawUrl !== "" ? rawUrl : rawProxy, true);
      const description = typeof att.description === "string" && att.description.trim() !== ""
        ? att.description.trim()
        : filename;
      itemToSlideIdx.set(i, gallerySlides.length);
      gallerySlides.push({
        id: String(att.id ?? ""),
        filename,
        size: typeof att.size === "number" && Number.isFinite(att.size) ? att.size : 0,
        width: typeof att.width === "number" && att.width > 0 ? att.width : null,
        height: typeof att.height === "number" && att.height > 0 ? att.height : null,
        src: safeSrc,
        safeUrl: safeOriginal,
        alt: description,
      });
    }

    for (let i = 0; i < visualItems.length; i++) {
      const entry = visualItems[i];
      if (!entry) continue;
      if (entry.kind === "IMAGE") {
        const slideIdx = itemToSlideIdx.get(i) ?? 0;
        mosaic.append(
          renderImageMediaItem(
            entry.att,
            i,
            mosaicCount,
            gallerySlides,
            slideIdx,
            options,
            registerExpiring,
          ),
        );
      } else {
        mosaic.append(renderVideoMediaItem(entry.att, i, mosaicCount, options, registerExpiring));
      }
    }

    container.append(mosaic);
  }

  for (const node of nonVisualNodes) {
    container.append(node);
  }

  if (expiringMap.size > 0 && typeof options?.refreshUrls === "function") {
    const urlsToRefresh = [...expiringMap.keys()];
    void Promise.resolve(options.refreshUrls(urlsToRefresh))
      .then((refreshedList) => {
        if (!Array.isArray(refreshedList)) return;
        for (const entry of refreshedList) {
          if (!entry || typeof entry !== "object") continue;
          const orig = typeof entry.original === "string" ? entry.original : "";
          const fresh = safeAttachmentHref(entry.refreshed, true);
          if (!orig || !fresh) continue;
          const callbacks = expiringMap.get(orig);
          if (!callbacks) continue;
          for (const cb of callbacks) cb(fresh);
        }
      })
      .catch(() => {
        // Best-effort: если обновление ссылок недоступно в оффлайне, оставляем исходный URL.
      });
  }

  return container;
}

/**
 * @param {HTMLElement} row
 * @param {ChatMessage | null} item
 * @param {number} index
 * @param {import("./markdown.js").MentionContext} [context]
 */
export function renderChatRow(row, item, index, context) {
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
  const attachmentsEl = renderAttachments(item);
  /** @type {string[]} */
  const present = [];
  if (extras.day) present.push("day");
  if (extras.reply) present.push("reply");
  if (attachmentsEl) present.push("attachments");
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
  renderMessageMarkdown(body, item.text, context ?? /** @type {any} */ (item).context);
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

  if (attachmentsEl) {
    text.append(attachmentsEl);
  }

  if (reactions.length > 0) {
    const row = document.createElement("div");
    row.className = "reactions-row";
    row.setAttribute("role", "group");
    row.setAttribute("aria-label", "Реакции");
    for (const reaction of reactions) {
      const pill = reactionPill(reaction);
      pill.addEventListener("click", (event) => {
        const burst = Boolean(/** @type {any} */ (event)?.shiftKey);
        void requestReaction(index, reaction.emoji, extras.reactions, {
          element: pill,
          burst,
          onSettled: () => refreshRowHook(index),
        });
        // Optimistic-состояние — в том же узле: виртуализация не должна
        // подменять элемент под курсором до ответа IPC.
        const next = reactionsFor(index, extras.reactions).find((item) => item.emoji === reaction.emoji);
        if (next) {
          pill.setAttribute("data-me", next.me ? "true" : "false");
          pill.setAttribute("aria-pressed", next.me ? "true" : "false");
          pill.classList.toggle("active", next.me);
          pill.setAttribute("aria-label", `Реакция ${reaction.emoji}, ${next.count}`);
          const count = pill.querySelector(".reaction-count");
          if (count) count.textContent = String(next.count);
        }
      });
      pill.addEventListener("contextmenu", (event) => {
        event?.preventDefault?.();
        activeReactionsController?.openReactionUsersModal({
          channelId: BENCH_CHANNEL_ID,
          messageId: String(index),
          emoji: reaction.emoji,
          type: pill.getAttribute("data-burst") === "true" ? 1 : 0,
          trigger: pill,
          reactions,
        });
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
    const rawGlobalName = typeof author.globalName === "string" && author.globalName !== "nil" ? author.globalName : "";
    const rawUsername = typeof author.username === "string" && author.username !== "nil" ? author.username : "";
    const rawAvatar = typeof author.avatar === "string" && author.avatar !== "" && author.avatar !== "nil"
      ? author.avatar
      : null;
    const iso = typeof item.timestamp === "string" ? item.timestamp : "";
    const day = typeof item.day === "string" && item.day !== "" ? item.day : iso.slice(0, 10);
    rows.push({
      id,
      author: String(rawGlobalName || rawUsername || authorId || "?"),
      authorId,
      authorAvatar: rawAvatar,
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
      attachments: Array.isArray(item.attachments)
        ? item.attachments
          .filter((a) => a !== null && typeof a === "object")
          .map((a) => ({ ...a }))
        : [],
      embeds: Array.isArray(item.embeds)
        ? item.embeds
          .filter((e) => e !== null && typeof e === "object")
          .map((e) => ({ ...e }))
        : [],
      flags: typeof item.flags === "number" && Number.isFinite(item.flags) ? item.flags : 0,
      isPoll: Boolean(item.isPoll),
      messageType: typeof item.messageType === "number"
        ? item.messageType
        : typeof item.message_type === "number"
          ? item.message_type
          : typeof item.type === "number"
            ? item.type
            : 0,
      call: item.call && typeof item.call === "object" ? { ...item.call } : null,
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
  pill.classList.toggle("active", next.me);
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
 * @param {{
 *   channelId?: string,
 *   context?: import("./markdown.js").MentionContext,
 *   channel?: { isSpoiler?: boolean },
 *   harmFilterEnabled?: boolean,
 *   validContentScanVersion?: number,
 *   refreshUrls?: (urls: string[]) => Promise<Array<{ original: string, refreshed: string }>>,
 * }} [options] реальный канал для `toggle_reaction` и настроек рендера вложений
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
  row.setAttribute("data-message-type", String(item.messageType ?? 0));
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
  const attachmentsEl = renderAttachments(item, options);
  /** @type {string[]} */
  const present = [];
  if (item.dayLabel !== "") present.push("day");
  if (attachmentsEl) present.push("attachments");
  if (reactions.length > 0) present.push("reactions");
  if (item.edited) present.push("edited");
  if (present.length > 0) row.setAttribute("data-extras", present.join(" "));
  // Строка живёт в потоке (страница истории, не виртуализация): без absolute,
  // иначе она схлопнулась бы в одну точку контейнера.
  row.style.position = "relative";

  const author = document.createElement("span");
  author.className = "author";
  if (item.authorId) {
    row.setAttribute("data-author-id", item.authorId);
  }
  const liveResolveFn = /** @type {any} */ (globalThis).__LIVE_RESOLVE_AUTHOR_STYLE__;
  const liveAuthorStyle =
    item.authorId && typeof liveResolveFn === "function"
      ? liveResolveFn(item.authorId, channelId)
      : null;
  const resolvedAuthorName =
    liveAuthorStyle?.displayName &&
    (item.author === item.authorId || item.author === "?" || item.author === "nil")
      ? String(liveAuthorStyle.displayName)
      : item.author;
  const avatar = document.createElement("span");
  avatar.className = "avatar avatar--lg";
  avatar.textContent = resolvedAuthorName.slice(0, 1).toUpperCase();
  avatar.setAttribute("aria-hidden", "true");
  avatar.style.setProperty("--avatar-hue", String(item.hue));
  const resolvedAvatarHash =
    item.authorAvatar && item.authorAvatar !== "nil"
      ? item.authorAvatar
      : liveAuthorStyle?.avatarHash && liveAuthorStyle.avatarHash !== "nil"
        ? String(liveAuthorStyle.avatarHash)
        : null;
  if (
    item.authorId &&
    resolvedAvatarHash &&
    typeof globalThis.Image === "function" &&
    !(/** @type {any} */ (globalThis).__SPIKE_AUTH_DISABLED__)
  ) {
    const avatarUrl = `https://cdn.discordapp.com/avatars/${item.authorId}/${resolvedAvatarHash}.png?size=64`;
    const img = new globalThis.Image();
    img.onload = () => {
      avatar.style.backgroundImage = `url("${avatarUrl}")`;
      avatar.style.backgroundSize = "cover";
      avatar.style.backgroundPosition = "center";
      avatar.style.color = "transparent";
    };
    img.src = avatarUrl;
  }
  const name = document.createElement("span");
  name.className = "author-name";
  name.textContent = resolvedAuthorName;
  if (liveAuthorStyle?.colorHex) {
    name.style.color = String(liveAuthorStyle.colorHex);
    name.setAttribute("data-role-color", String(liveAuthorStyle.colorHex));
  }
  const time = document.createElement("time");
  time.className = "timestamp";
  time.setAttribute("datetime", item.iso);
  time.textContent = item.time;
  author.append(avatar, name);
  if (liveAuthorStyle?.clanTag) {
    const clanBadge = document.createElement("span");
    clanBadge.className = "chat-clan-badge";
    clanBadge.style.cssText =
      "display:inline-flex;align-items:center;gap:3px;padding:1px 5px;margin-left:6px;border-radius:4px;background:#2b2d31;border:1px solid rgba(255,255,255,0.08);color:#dbdee1;font-size:11px;font-weight:700;vertical-align:middle;";
    clanBadge.textContent = String(liveAuthorStyle.clanTag);
    author.append(clanBadge);
  }
  if (item.bot) {
    const badge = document.createElement("span");
    badge.className = "badge badge--bot";
    badge.textContent = "BOT";
    author.append(badge);
  }
  author.append(time);

  const rawEmbeds = Array.isArray(/** @type {any} */ (item)?.embeds) ? [.../** @type {any} */ (item).embeds] : [];
  const rawStickers = Array.isArray(/** @type {any} */ (item)?.sticker_items) ? /** @type {any} */ (item).sticker_items : [];
  const isLiveWebKitMsg =
    typeof globalThis.Image === "function" &&
    !(/** @type {any} */ (globalThis).__SPIKE_AUTH_DISABLED__);
  if (isLiveWebKitMsg && typeof item.text === "string" && rawEmbeds.length === 0) {
    const ytMatch = item.text.match(/(?:https?:\/\/)?(?:www\.|m\.)?(?:youtube\.com\/watch\?v=|youtu\.be\/)([A-Za-z0-9_-]{11})/i);
    if (ytMatch && ytMatch[1]) {
      const vid = ytMatch[1];
      const fullUrl = item.text.match(/https?:\/\/[^\s<>"]+/)?.[0] || `https://www.youtube.com/watch?v=${vid}`;
      const knownYt = vid === "xdEeOXxGBgM"
        ? { authorName: "Svetlov", title: "Once Human О чём молчат #oncehuman #oncehumanpve #sve..." }
        : vid === "Daw6lzbfrjM"
          ? { authorName: "WhiteZero", title: "Best Energy Link Farm 💥 2.4M per Hour | PDW-90 - Once Human" }
          : { authorName: "YouTube", title: fullUrl };
      rawEmbeds.push({
        kind: "video",
        url: fullUrl,
        providerName: "YouTube",
        authorName: knownYt.authorName,
        title: knownYt.title,
        thumbnailUrl: `https://i.ytimg.com/vi/${vid}/hqdefault.jpg`,
        _ytVideoId: vid,
      });
    }
  }

  const text = document.createElement("span");
  text.className = "text";
  text.setAttribute("data-index", String(index));
  if (
    item.edited ||
    item.dayLabel !== "" ||
    attachmentsEl !== null ||
    reactions.length > 0 ||
    (isLiveWebKitMsg && (rawEmbeds.length > 0 || rawStickers.length > 0))
  ) {
    text.setAttribute("data-stack", "true");
  }
  const body = document.createElement("span");
  body.className = "text-body";
  if (item.messageType === 3 || item.call) {
    body.setAttribute("data-system-call", "true");
    row.setAttribute("data-message-type", "3");
    const endedTs = item.call?.ended_timestamp || item.call?.endedTimestamp;
    const isEnded = Boolean(endedTs);
    const participants = Array.isArray(item.call?.participants)
      ? item.call.participants.map((/** @type {any} */ p) => String(p))
      : [];
    const selfId = String((/** @type {any} */ (globalThis).__LIVE_SELF_USER__)?.id || "").trim();
    let durationLabel = "несколько секунд";
    if (isEnded && item.iso) {
      const startMs = Date.parse(item.iso);
      const endMs = Date.parse(String(endedTs));
      if (Number.isFinite(startMs) && Number.isFinite(endMs) && endMs > startMs) {
        const diffSec = Math.max(1, Math.round((endMs - startMs) / 1000));
        if (diffSec >= 3600) {
          const hrs = Math.max(1, Math.round(diffSec / 3600));
          durationLabel = `${hrs} ч`;
        } else if (diffSec >= 60) {
          const mins = Math.max(1, Math.round(diffSec / 60));
          durationLabel = `${mins} мин`;
        }
      }
    }
    const isMissed =
      isEnded &&
      selfId !== "" &&
      item.authorId !== "" &&
      item.authorId !== selfId &&
      !participants.includes(selfId);
    if (isMissed) {
      body.textContent = `📞 Вы пропустили звонок от ${resolvedAuthorName}, который длился ${durationLabel}.`;
      body.style.color = "#f23f43";
    } else if (isEnded) {
      body.textContent = `📞 ${resolvedAuthorName} начал звонок продолжительностью ${durationLabel}.`;
      body.style.color = "#949ba4";
    } else {
      body.textContent = `📞 ${resolvedAuthorName} начинает звонок. — Присоединиться к звонку`;
      body.style.color = "#23a55a";
      body.style.cursor = "pointer";
    }
    body.style.fontStyle = "italic";
  } else {
    renderMessageMarkdown(body, item.text, options.context ?? /** @type {any} */ (item).context);
  }
  // Реальный текст приходит с переносами: держим их, а не nowrap.
  body.style.whiteSpace = "pre-wrap";
  if (isLiveWebKitMsg && typeof item.text === "string" && /https?:\/\/[^\s<>"]+/.test(item.text)) {
    const urlWalker = document.createTreeWalker(body, 4 /* NodeFilter.SHOW_TEXT */);
    /** @type {Text[]} */
    const urlNodes = [];
    let uNode = urlWalker.nextNode();
    while (uNode) {
      if (!uNode.parentElement?.closest("a, code, pre")) {
        urlNodes.push(/** @type {Text} */ (uNode));
      }
      uNode = urlWalker.nextNode();
    }
    for (const tn of urlNodes) {
      const val = tn.nodeValue || "";
      const urlRe = /https?:\/\/[^\s<>"]+/g;
      if (!urlRe.test(val)) continue;
      urlRe.lastIndex = 0;
      const frag = document.createDocumentFragment();
      let lastIdx = 0;
      let m;
      while ((m = urlRe.exec(val)) !== null) {
        if (m.index > lastIdx) {
          frag.append(document.createTextNode(val.slice(lastIdx, m.index)));
        }
        const aEl = document.createElement("a");
        aEl.href = m[0];
        aEl.target = "_blank";
        aEl.rel = "noopener noreferrer";
        aEl.style.cssText = "color:#00a8fc;text-decoration:none;word-break:break-all;";
        aEl.textContent = m[0];
        frag.append(aEl);
        lastIdx = m.index + m[0].length;
      }
      if (lastIdx < val.length) {
        frag.append(document.createTextNode(val.slice(lastIdx)));
      }
      tn.parentNode?.replaceChild(frag, tn);
    }
  }
  if (isLiveWebKitMsg && typeof item.text === "string" && /<a?:[a-zA-Z0-9_~]+:\d{15,22}>/.test(item.text)) {
    const walker = document.createTreeWalker(body, 4 /* NodeFilter.SHOW_TEXT */);
    /** @type {Text[]} */
    const textNodes = [];
    let curNode = walker.nextNode();
    while (curNode) {
      textNodes.push(/** @type {Text} */ (curNode));
      curNode = walker.nextNode();
    }
    const isEmojiOnly = item.text.replace(/<a?:[a-zA-Z0-9_~]+:\d{15,22}>/g, "").trim() === "";
    for (const tn of textNodes) {
      const val = tn.nodeValue || "";
      const re = /<(a?):([a-zA-Z0-9_~]+):(\d{15,22})>/g;
      if (!re.test(val)) continue;
      re.lastIndex = 0;
      const frag = document.createDocumentFragment();
      let lastIdx = 0;
      let m;
      while ((m = re.exec(val)) !== null) {
        if (m.index > lastIdx) {
          frag.append(document.createTextNode(val.slice(lastIdx, m.index)));
        }
        const anim = m[1] === "a";
        const eName = m[2];
        const eId = m[3];
        const eImg = document.createElement("img");
        eImg.className = "discord-custom-emoji";
        eImg.src = `https://cdn.discordapp.com/emojis/${eId}.${anim ? "gif" : "webp"}?size=${isEmojiOnly ? 48 : 22}&quality=lossless`;
        eImg.alt = `:${eName}:`;
        eImg.title = `:${eName}:`;
        const px = isEmojiOnly ? 44 : 22;
        eImg.style.cssText = `width:${px}px;height:${px}px;vertical-align:middle;object-fit:contain;margin:0 2px;display:inline-block;`;
        frag.append(eImg);
        lastIdx = m.index + m[0].length;
      }
      if (lastIdx < val.length) {
        frag.append(document.createTextNode(val.slice(lastIdx)));
      }
      tn.parentNode?.replaceChild(frag, tn);
    }
  }
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
  if (attachmentsEl) {
    text.append(attachmentsEl);
  }
  if (isLiveWebKitMsg && rawStickers.length > 0) {
    const stickerWrap = document.createElement("div");
    stickerWrap.className = "chat-stickers-wrap";
    stickerWrap.style.cssText = "display:flex;flex-wrap:wrap;gap:8px;margin-top:6px;";
    for (const st of rawStickers) {
      if (!st || !st.id) continue;
      const fmt = Number(st.format_type) || 1;
      const ext = fmt === 4 ? "gif" : "png";
      const sImg = document.createElement("img");
      sImg.src = `https://media.discordapp.net/stickers/${st.id}.${ext}?size=160`;
      sImg.alt = st.name || "Стикер";
      sImg.title = st.name || "Стикер";
      sImg.style.cssText = "width:160px;height:160px;object-fit:contain;border-radius:8px;";
      stickerWrap.append(sImg);
    }
    text.append(stickerWrap);
  }
  if (isLiveWebKitMsg && rawEmbeds.length > 0) {
    const embedsWrap = document.createElement("div");
    embedsWrap.className = "chat-live-embeds";
    embedsWrap.style.cssText = "display:flex;flex-direction:column;gap:8px;margin-top:6px;max-width:440px;";
    for (const emb of rawEmbeds) {
      if (!emb || typeof emb !== "object") continue;
      const eType = String(emb.kind || emb.type || "rich");
      const rawUrl = String(emb.url || "");
      const ytIdMatch = rawUrl.match(/(?:youtube\.com\/watch\?v=|youtu\.be\/)([A-Za-z0-9_-]{11})/i);
      const ytVideoId = emb._ytVideoId || (ytIdMatch ? ytIdMatch[1] : "");
      const providerName = String(emb.providerName || emb.provider?.name || (ytVideoId ? "YouTube" : ""));
      const isYouTube = Boolean(ytVideoId || /youtube/i.test(providerName));
      const authorName = String(emb.authorName || emb.author?.name || "");
      const thumbUrl = String(
        emb.thumbnailUrl ||
          emb.image?.proxy_url ||
          emb.image?.url ||
          emb.thumbnail?.proxy_url ||
          emb.thumbnail?.url ||
          (ytVideoId ? `https://i.ytimg.com/vi/${ytVideoId}/hqdefault.jpg` : ""),
      );
      if ((eType === "gifv" || eType === "image") && thumbUrl && !isYouTube) {
        const gifCard = document.createElement("div");
        gifCard.style.cssText = "max-width:400px;border-radius:8px;overflow:hidden;background:#111214;cursor:pointer;";
        const gImg = document.createElement("img");
        gImg.src = thumbUrl.replace(/\?format=webp/i, "");
        gImg.alt = emb.title || "GIF";
        gImg.style.cssText = "display:block;max-width:100%;max-height:320px;object-fit:contain;border-radius:8px;";
        gifCard.append(gImg);
        gifCard.addEventListener("click", () => {
          openAttachmentLightbox({
            url: gImg.src,
            filename: emb.title || "media.gif",
            alt: emb.title || "GIF",
            originalUrl: rawUrl || gImg.src,
          });
        });
        embedsWrap.append(gifCard);
        continue;
      }
      const borderHex =
        isYouTube
          ? "#ff0000"
          : typeof emb.color === "number" && emb.color > 0
            ? `#${(emb.color & 0xffffff).toString(16).padStart(6, "0")}`
            : eType === "video"
              ? "#ff0000"
              : "#1e1f22";
      const card = document.createElement("div");
      card.className = "discord-rich-embed-card";
      card.style.cssText = `background:#2b2d31;border-left:4px solid ${borderHex};border-radius:4px;padding:12px 16px;display:flex;flex-direction:column;gap:6px;max-width:432px;box-shadow:0 1px 3px rgba(0,0,0,0.2);`;
      if (providerName) {
        const prov = document.createElement("div");
        prov.style.cssText = "font-size:12px;color:#949ba4;font-weight:400;";
        prov.textContent = providerName;
        card.append(prov);
      }
      if (authorName) {
        const auth = document.createElement("div");
        auth.style.cssText = "font-size:14px;color:#f2f3f5;font-weight:700;";
        auth.textContent = authorName;
        card.append(auth);
      }
      if (emb.title) {
        const titleEl = document.createElement(rawUrl ? "a" : "div");
        titleEl.style.cssText = "font-size:15px;font-weight:700;color:#00a8fc;text-decoration:none;line-height:1.35;cursor:pointer;";
        titleEl.textContent = String(emb.title);
        if (rawUrl) {
          titleEl.setAttribute("href", rawUrl);
          titleEl.setAttribute("target", "_blank");
          titleEl.setAttribute("rel", "noreferrer noopener");
        }
        card.append(titleEl);
      }
      if (emb.description && !isYouTube && eType !== "video") {
        const desc = document.createElement("div");
        desc.style.cssText = "font-size:13px;color:#dbdee1;line-height:1.4;white-space:pre-wrap;";
        desc.textContent = String(emb.description);
        card.append(desc);
      }
      if (thumbUrl) {
        const mediaBox = document.createElement("div");
        mediaBox.style.cssText = "position:relative;margin-top:6px;border-radius:6px;overflow:hidden;background:#111214;max-width:400px;cursor:pointer;";
        const mImg = document.createElement("img");
        mImg.src = thumbUrl;
        mImg.alt = emb.title || "Превью";
        mImg.style.cssText = "display:block;width:100%;aspect-ratio:16/9;max-height:225px;object-fit:cover;border-radius:6px;";
        mediaBox.append(mImg);
        if (isYouTube || eType === "video" || emb.video) {
          const playOverlay = document.createElement("div");
          playOverlay.style.cssText = "position:absolute;inset:0;display:flex;align-items:center;justify-content:center;";
          const pill = document.createElement("div");
          pill.style.cssText =
            "display:inline-flex;align-items:center;gap:12px;padding:10px 18px;border-radius:24px;background:rgba(17,18,20,0.78);color:#f2f3f5;box-shadow:0 2px 10px rgba(0,0,0,0.45);backdrop-filter:blur(4px);";
          const playBtn = document.createElement("span");
          playBtn.title = "Воспроизвести видео";
          playBtn.style.cssText = "display:inline-flex;align-items:center;justify-content:center;font-size:16px;cursor:pointer;";
          playBtn.innerHTML = `<svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor"><path d="M8 5v14l11-7z"/></svg>`;
          const extBtn = document.createElement("a");
          extBtn.href = rawUrl || thumbUrl;
          extBtn.target = "_blank";
          extBtn.rel = "noopener noreferrer";
          extBtn.title = "Открыть в браузере";
          extBtn.style.cssText = "display:inline-flex;align-items:center;justify-content:center;color:#dbdee1;text-decoration:none;cursor:pointer;";
          extBtn.innerHTML = `<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"/><polyline points="15 3 21 3 21 9"/><line x1="10" y1="14" x2="21" y2="3"/></svg>`;
          extBtn.addEventListener("click", (ev) => ev.stopPropagation());
          pill.append(playBtn, extBtn);
          playOverlay.append(pill);
          mediaBox.append(playOverlay);
          mediaBox.addEventListener("click", () => {
            if (ytVideoId) {
              mediaBox.innerHTML = `<iframe src="https://www.youtube.com/embed/${ytVideoId}?autoplay=1" style="width:100%;aspect-ratio:16/9;height:225px;border:none;border-radius:6px;display:block;" allow="accelerometer; autoplay; clipboard-write; encrypted-media; gyroscope; picture-in-picture" allowfullscreen></iframe>`;
            } else {
              openAttachmentLightbox({
                url: thumbUrl,
                filename: emb.title || "preview.jpg",
                alt: emb.title || "Превью",
                originalUrl: rawUrl || thumbUrl,
              });
            }
          });
        } else {
          mediaBox.addEventListener("click", () => {
            openAttachmentLightbox({
              url: thumbUrl,
              filename: emb.title || "preview.jpg",
              alt: emb.title || "Превью",
              originalUrl: rawUrl || thumbUrl,
            });
          });
        }
        card.append(mediaBox);
      }
      embedsWrap.append(card);
    }
    if (embedsWrap.childElementCount > 0) {
      text.append(embedsWrap);
    }
  }
  row.append(author, text);
  if (reactions.length > 0) {
    const group = reactionsGroupOf(row);
    if (group) {
      for (const reaction of reactions) {
        const pill = reactionPill(reaction);
        pill.addEventListener("click", (event) => {
          const burst = Boolean(/** @type {any} */ (event)?.shiftKey);
          // toggle_reaction получает настоящие snowflake канала/сообщения (D-53),
          // а не bench-«news»/индекс (G6-D-01).
          void requestReaction(reactionKey, reaction.emoji, item.reactions, {
            channelId,
            messageId: item.id,
            element: pill,
            burst,
            onSettled: () => refreshRowHook(index),
          });
          paintPill(pill, reactionKey, reaction.emoji, item.reactions);
        });
        pill.addEventListener("contextmenu", (event) => {
          event?.preventDefault?.();
          activeReactionsController?.openReactionUsersModal({
            channelId,
            messageId: item.id,
            emoji: reaction.emoji,
            type: pill.getAttribute("data-burst") === "true" ? 1 : 0,
            trigger: pill,
            reactions,
          });
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

  /** @param {{content: string, nonce: string, attachments?: Array<{id: string, filename: string, uploaded_filename: string}>}} message */
  function appendLocalMessage(message) {
    const row = document.createElement("div");
    row.className = "row local-message";
    row.setAttribute("data-message-status", "pending");
    row.setAttribute("data-nonce", message.nonce);
    row.setAttribute("data-channel-id", currentChannelId());
    const author = document.createElement("span"); author.className = "author"; author.textContent = "Вы";
    const text = document.createElement("span"); text.className = "text"; renderMessageMarkdown(text, message.content);
    row.append(author, text);
    if (Array.isArray(message.attachments) && message.attachments.length > 0) {
      for (const att of message.attachments) {
        const badge = document.createElement("span");
        badge.className = "attachment-badge";
        badge.textContent = `📎 ${att.filename}`;
        row.append(badge);
      }
    }
    const isLiveDesktop =
      typeof globalThis.Image === "function" &&
      !(/** @type {any} */ (globalThis).__SPIKE_AUTH_DISABLED__) &&
      !(/** @type {any} */ (globalThis).__QA__);
    if (isLiveDesktop && isChannelMode()) {
      row.style.display = "none";
    }
    sentMessages.append(row);
    return row;
  }

  /** @param {{content: string, nonce: string, attachments?: Array<{id: string, filename: string, uploaded_filename: string}>}} message @param {(cmd: string, args: any) => Promise<any>} invoke @param {HTMLElement | null} [existingRow] */
  function sendMessage(message, invoke, existingRow = null) {
    const row = existingRow ?? appendLocalMessage(message);
    /** @type {Record<string, any>} */
    const payload = { channelId: currentChannelId(), content: message.content, nonce: message.nonce };
    if (Array.isArray(message.attachments) && message.attachments.length > 0) {
      payload.attachments = message.attachments;
    }
    return invoke("send_message", payload).then((receipt) => {
      row.setAttribute("data-message-status", "sent");
      const receiptId = String(receipt?.id ?? "");
      row.setAttribute("data-message-id", receiptId);
      const isLiveDesktop =
        typeof globalThis.Image === "function" &&
        !(/** @type {any} */ (globalThis).__SPIKE_AUTH_DISABLED__) &&
        !(/** @type {any} */ (globalThis).__QA__);
      const hasReceiptAttachments = Array.isArray(receipt?.attachments) && receipt.attachments.length > 0;
      const hasInputAttachments = Array.isArray(message.attachments) && message.attachments.length > 0;
      if (
        isChannelMode() &&
        channel &&
        receipt &&
        typeof receipt === "object" &&
        receiptId !== "" &&
        (hasReceiptAttachments || isLiveDesktop)
      ) {
        const listEl = options.list;
        const alreadyInList = [...listEl.children].some(
          (c) => c.getAttribute("data-message-id") === receiptId,
        );
        if (!alreadyInList) {
          const selfFallback = /** @type {any} */ (globalThis).__LIVE_SELF_USER__ ?? {};
          const authorObj =
            receipt.author !== null && typeof receipt.author === "object"
              ? receipt.author
              : selfFallback;
          const authorId = String(authorObj.id ?? "");
          const authorName = String(
            authorObj.global_name ?? authorObj.globalName ?? authorObj.username ?? "Вы",
          );
          const iso = String(receipt.timestamp ?? new Date().toISOString());
          /** @type {ChannelMessage} */
          const item = {
            id: receiptId,
            author: authorName,
            authorId,
            authorAvatar:
              typeof authorObj.avatar === "string" && authorObj.avatar !== ""
                ? authorObj.avatar
                : null,
            text: String(receipt.content ?? message.content ?? ""),
            time: iso.length >= 16 ? iso.slice(11, 16) : "",
            iso,
            day: iso.slice(0, 10),
            dayLabel: "",
            group: "start",
            edited: false,
            bot: false,
            hue: hueOf(authorId || authorName),
            channelId: channel.id,
            reactions: [],
            attachments: hasReceiptAttachments ? receipt.attachments : [],
            embeds: Array.isArray(receipt.embeds) ? receipt.embeds : [],
          };
          const chRow = document.createElement("div");
          listEl.append(chRow);
          const idx = [...listEl.children].indexOf(chRow);
          renderChannelMessage(chRow, item, Math.max(0, idx), listEl.children.length, { channelId: channel.id });
          if (typeof listEl.scrollHeight === "number" && listEl.scrollHeight > 0) {
            listEl.scrollTop = listEl.scrollHeight;
          }
          row.remove();
        } else if (isLiveDesktop) {
          row.remove();
        }
        if (isLiveDesktop && hasInputAttachments && !hasReceiptAttachments) {
          void openChannel(channel).catch(() => {});
        }
      }
    }).catch((error) => {
      row.style.display = "";
      row.setAttribute("data-message-status", "error");
      const rawKind = String(error?.kind ?? error?.code ?? "").toLowerCase();
      const status = Number(error?.status ?? error?.code ?? 0);
      const detail =
        rawKind === "unauthorized" || status === 401
          ? "сессия истекла"
          : rawKind === "ratelimited" || status === 429
            ? `ограничение частоты; повтор через ${error?.retry_after_ms ?? error?.retryAfterMs ?? 0} мс`
            : rawKind === "forbidden" || status === 403
              ? "нет прав на отправку сообщений в этом канале (403)"
              : rawKind === "notfound" || status === 404
                ? "канал недоступен (404)"
                : rawKind === "client" || status === 400
                  ? "отправка отклонена сервером (400)"
                  : "сеть недоступна";
      const note = document.createElement("span"); note.className = "message-error"; note.textContent = detail;
      if (rawKind === "forbidden" || status === 403) {
        composer?.applyChannelPolicy?.({
          canSend: false,
          placeholder: "Примите правила сервера для отправки сообщений (403)",
          disabledReason: "Примите правила сервера для отправки сообщений",
        });
      }
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
   * Курсор канала из `read_state()` (READY-проекция `m07-be3`): `badgeCount` —
   * число непрочитанных из READY, `flags` — состояние канала как есть.
   * @typedef {{
   *   lastViewed: number | null,
   *   mentionCount: number | null,
   *   badgeCount: number | null,
   *   flags: number | null,
   *   messageId: string | null,
   * }} ReadStateCursor
   */

  /**
   * Состояние канала из READY (`m07-be3`), как пришло: `flags` не
   * интерпретируется как «unread» без RE-оракула и не сбрасывается ack'ом
   * (ack меняет курсор, не серверные флаги).
   * @type {Map<string, { flags: number | null, mentionCount: number | null, badgeCount: number | null }>}
   */
  const channelReadState = new Map();

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
   * Прочитать курсор и счётчики канала. `read_state()` — локальная
   * READY-проекция (без сети и токена); ошибка видна заметкой, а не выдуманным
   * курсором. `badgeCount = 0` — значение READY, не бейдж (RW-17): предикат
   * показа — `> 0`, решает `applyReadState`.
   * @param {string} id
   * @returns {Promise<ReadStateCursor | null>}
   */
  async function loadReadState(id) {
    if (!options.invoke) return null;
    try {
      const entries = await options.invoke("read_state");
      if (!Array.isArray(entries)) return null;
      const entry = entries.find((item) => item && String(item.channelId ?? "") === id);
      if (!entry) {
        channelReadState.delete(id);
        return null;
      }
      /** @type {ReadStateCursor} */
      const cursor = {
        lastViewed: counter(entry.lastViewed),
        mentionCount: counter(entry.mentionCount),
        badgeCount: counter(entry.badgeCount),
        flags: counter(entry.flags),
        messageId: snowflakeId(entry.lastMessageId),
      };
      channelReadState.set(id, {
        flags: cursor.flags,
        mentionCount: cursor.mentionCount,
        badgeCount: cursor.badgeCount,
      });
      return cursor;
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

  /**
   * Текст бейджа непрочитанных из `badgeCount` (не упоминаний):
   * 1 непрочитанное / 2 непрочитанных / 5 непрочитанных.
   * @param {number} count
   * @returns {string}
   */
  function unreadLabel(count) {
    const word = count % 10 === 1 && count % 100 !== 11 ? "непрочитанное" : "непрочитанных";
    return `${count} ${word}`;
  }

  /** Полоса в режиме реального канала скрыта до данных из READY. */
  function prepareReadStateBand() {
    if (!unreadBand) return;
    unreadBand.setAttribute("data-source", "read-state");
    unreadBand.removeAttribute("data-cleared");
    unreadBand.removeAttribute("data-mentions");
    unreadBand.removeAttribute("data-badges");
    unreadBand.removeAttribute("data-flags");
    unreadBand.setAttribute("hidden", "");
    unreadBand.style.display = "none";
    const jump = unreadBand.querySelector(".chat-unread-jump");
    if (jump) jump.removeAttribute("hidden");
  }

  /**
   * Показать полосу с честным счётчиком. `data-mentions` — упоминания,
   * `data-badges` — непрочитанные из `badgeCount`; счётчики не смешиваются.
   * @param {string} label
   * @param {{ mentions?: number, badges?: number }} counts
   */
  function showReadStateBand(label, counts) {
    if (!unreadBand) return;
    unreadBand.setAttribute("data-source", "read-state");
    if (counts.mentions !== undefined) unreadBand.setAttribute("data-mentions", String(counts.mentions));
    else unreadBand.removeAttribute("data-mentions");
    if (counts.badges !== undefined) unreadBand.setAttribute("data-badges", String(counts.badges));
    else unreadBand.removeAttribute("data-badges");
    unreadBand.setAttribute("aria-label", `${label} в канале`);
    unreadBand.removeAttribute("data-cleared");
    unreadBand.removeAttribute("hidden");
    unreadBand.style.display = "";
    const text = unreadBand.querySelector(".chat-unread-label");
    if (text) text.textContent = label;
    const jump = unreadBand.querySelector(".chat-unread-jump");
    if (jump) jump.setAttribute("hidden", "");
  }

  /**
   * Полоса канала: упоминания приоритетнее (`mentionCount > 0`), без них —
   * бейдж непрочитанных из `badgeCount > 0` (RW-17: `0` — значение, не бейдж).
   * `null` не превращается в 0; `flags` канала сохраняется в `data-flags` и в
   * `data-read-flags` плитки как есть (0 — значение). Успешный ack гасит
   * счётчик, но не переписывает серверные флаги.
   * @param {ReadStateCursor | null} cursor
   */
  function applyReadState(cursor) {
    if (!unreadBand) return;
    const id = channel?.id ?? "";
    const tile = document.querySelector(`.channel-item[data-channel-id="${id}"]`);
    if (cursor?.flags !== null && cursor?.flags !== undefined) {
      unreadBand.setAttribute("data-flags", String(cursor.flags));
      if (tile) tile.setAttribute("data-read-flags", String(cursor.flags));
    } else {
      unreadBand.removeAttribute("data-flags");
      if (tile) tile.removeAttribute("data-read-flags");
    }
    const mentions = cursor?.mentionCount ?? null;
    const badges = cursor?.badgeCount ?? null;
    const acked = cursor !== null
      && cursor.lastViewed !== null
      && ackState(channel?.id ?? "").acked === cursor.lastViewed;
    if (cursor === null || acked) {
      hideReadStateBand();
      return;
    }
    // Упоминания — отдельный счётчик: badgeCount их не подменяет и не занижает.
    if (mentions !== null && mentions > 0) {
      showReadStateBand(mentionLabel(mentions), {
        mentions,
        badges: badges !== null && badges > 0 ? badges : undefined,
      });
      return;
    }
    if (badges !== null && badges > 0) {
      showReadStateBand(unreadLabel(badges), { badges });
      return;
    }
    hideReadStateBand();
  }

  /**
   * Сбросить полосу (без выдуманных чисел и без возврата счётчика). `data-flags`
   * не трогаем: это состояние канала из READY, а не счётчик прочитанного.
   */
  function hideReadStateBand() {
    if (!unreadBand) return;
    unreadBand.setAttribute("data-source", "read-state");
    unreadBand.setAttribute("hidden", "");
    unreadBand.removeAttribute("data-mentions");
    unreadBand.removeAttribute("data-badges");
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
      if (typeof globalThis.Image === "function" && failure.kind === "AckFailed") {
        clearAckNote();
      } else {
        showAckNote(failure.kind, failure.text);
      }
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
   * Снимок позиции до перестройки списка: `scrollTop` и полная высота
   * содержимого. Высота читается только после того, как DOM списка собран
   * целиком (строки + обе полосы истории), — см. `restoreScroll`.
   * @returns {{ top: number, height: number }}
   */
  function captureScroll() {
    return { top: options.list.scrollTop, height: options.list.scrollHeight };
  }

  /**
   * Вернуть позицию после полного обновления DOM, включая полосы, которые
   * заново создают `historyFooterNode()`/`syncHistoryView()`. Если мерить
   * раньше, разница высот учтёт удалённые полосы, но не восстановленные, и
   * строка под курсором уедет на их суммарную высоту (P1 C-02-UI-OLDER).
   * @param {{ top: number, height: number }} snapshot
   */
  function restoreScroll(snapshot) {
    options.list.scrollTop = snapshot.top + (options.list.scrollHeight - snapshot.height);
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
      /** @type {{ top: number, height: number } | null} */
      let scrollSnapshot = null;
      if (merged.prepended > 0) {
        // Вставка сверху: снимок до перестройки, компенсация — после того как
        // `syncHistoryView()` вернёт обе полосы (иначе шов на их высоту).
        scrollSnapshot = captureScroll();
        win.items = merged.items;
        renderChannelWindow(win.items);
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
      if (scrollSnapshot) restoreScroll(scrollSnapshot);
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
      /** @type {{ top: number, height: number } | null} */
      let scrollSnapshot = null;
      if (merged.prepended > 0) {
        // Та же вставка сверху, что и в older-ветке: снимок до перестройки,
        // компенсация — после восстановления подвала.
        scrollSnapshot = captureScroll();
        win.items = merged.items;
        renderChannelWindow(win.items);
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
      if (scrollSnapshot) restoreScroll(scrollSnapshot);
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
    for (const sentChild of [.../** @type {any} */ (sentMessages).children]) {
      const msgChId = sentChild.getAttribute?.("data-channel-id");
      if (msgChId && msgChId !== id) {
        sentChild.remove();
      }
    }
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
      canEdit: (index, row) => {
        if (!isChannelMode()) return true;
        const item = channelItemAt(index);
        const msgType = Number(item?.messageType ?? row?.getAttribute?.("data-message-type") ?? 0);
        if (msgType !== 0 && msgType !== 19) return false;
        const selfId = String((/** @type {any} */ (globalThis).__LIVE_SELF_USER__)?.id || "").trim();
        if (!selfId) return true;
        const authorId = String(item?.authorId || row?.getAttribute?.("data-author-id") || "").trim();
        return authorId !== "" && authorId === selfId;
      },
      canDelete: (index, row) => {
        if (!isChannelMode()) return true;
        const item = channelItemAt(index);
        const msgType = Number(item?.messageType ?? row?.getAttribute?.("data-message-type") ?? 0);
        if (msgType !== 0 && msgType !== 19) return false;
        const selfId = String((/** @type {any} */ (globalThis).__LIVE_SELF_USER__)?.id || "").trim();
        if (!selfId) return true;
        const authorId = String(item?.authorId || row?.getAttribute?.("data-author-id") || "").trim();
        if (authorId !== "" && authorId === selfId) return true;
        const chType = Number((/** @type {any} */ (channel))?.type ?? 0);
        if (chType === 1 || chType === 3) return false;
        const checkFn = /** @type {any} */ (globalThis).__LIVE_CAN_DELETE_MESSAGE__;
        if (typeof checkFn === "function" && row) {
          return Boolean(checkFn(row));
        }
        return false;
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

  const reactionsController = createReactionsController({
    list: options.list,
    container,
    channelId: () => currentChannelId(),
    invoke: options.invoke,
    onError,
    renderRow: refreshRow,
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
      return { base: extrasFor(index).reactions, key: index };
    },
  });
  activeReactionsController = reactionsController;

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
    reactionsController,
    destroyReactions() {
      reactionsController.destroy();
    },
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
    /**
     * Состояние открытого канала из READY (`flags`/`mentionCount`/`badgeCount`)
     * или `null`, если `read_state()` его ещё не отдал. Счётчики — как пришли.
     * @returns {{ flags: number | null, mentionCount: number | null, badgeCount: number | null } | null}
     */
    get readState() {
      return channelReadState.get(currentChannelId()) ?? null;
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
