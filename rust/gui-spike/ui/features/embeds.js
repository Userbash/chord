// @ts-check
/// <reference lib="dom" />
/**
 * Embeds и вложения (SPEC-P3-02 §2, EMB-01…EMB-12).
 *
 * Все размеры — токены/шкала `tokens.css`; сеть не используется, поэтому
 * image-embed рендерится заглушкой `--surface-3` с иконкой 48 (EMB-07) и
 * `aria-hidden` (не озвучиваем пустую картинку).
 */
import { UIError } from "./errors.js";

/** @typedef {import("./rows.js").Embed} Embed */

/**
 * @param {string} className
 * @param {string} [text]
 * @returns {HTMLElement}
 */
function node(className, text = "") {
  const element = document.createElement("span");
  element.className = className;
  if (text) element.textContent = text;
  return element;
}

/**
 * Протоколы, которые допустимы в `href` embed-ссылки (P0 REVIEW-W39h).
 * Всё остальное (`javascript:`, `data:`, `vbscript:`…) не становится ссылкой.
 */
const SAFE_PROTOCOLS = Object.freeze(["http:", "https:"]);

/**
 * Безопасный `href` или `null`: только абсолютный http(s)-URL.
 * @param {string | undefined} value
 * @returns {string | null}
 */
function safeHref(value) {
  if (!value) return null;
  try {
    const url = new URL(value);
    return SAFE_PROTOCOLS.includes(url.protocol) ? value : null;
  } catch {
    // Относительный/неразборчивый адрес в спайке всё равно не навигируется:
    // честно считаем его недопустимым, а не подставляем как есть.
    return null;
  }
}

/**
 * @param {Embed} embed
 * @returns {HTMLElement}
 */
function linkEmbed(embed) {
  const card = document.createElement("div");
  card.className = "embed embed--link";
  if (embed.provider) card.append(node("embed-provider", embed.provider));
  const href = safeHref(embed.url);
  /** @type {HTMLElement} */
  let title;
  if (href !== null) {
    const link = document.createElement("a");
    link.className = "embed-title";
    // В спайке навигации нет, но адрес проверен: только http/https.
    link.setAttribute("href", href);
    link.setAttribute("rel", "noreferrer noopener");
    title = link;
  } else {
    // Небезопасный/отсутствующий адрес: не ссылка, а заглушка с честным
    // disabled-состоянием — `javascript:` не попадает в href никогда.
    title = node("embed-title");
    title.setAttribute("aria-disabled", "true");
    title.setAttribute("title", "Ссылка недоступна: недопустимый адрес");
  }
  title.textContent = embed.title ?? embed.url ?? "Ссылка";
  card.append(title);
  if (embed.description) card.append(node("embed-description", embed.description));
  return card;
}

/**
 * @param {Embed} embed
 * @returns {HTMLElement}
 */
function imageEmbed(embed) {
  const card = document.createElement("div");
  card.className = "embed embed--image";
  const image = document.createElement("span");
  image.className = "embed-image";
  // Картинка не грузится (нет сети): честная заглушка, а не битый <img>.
  image.setAttribute("data-state", "placeholder");
  image.setAttribute("aria-hidden", "true");
  image.append(node("embed-image-icon", "🖼"));
  card.append(image);
  if (embed.caption) card.append(node("embed-caption", embed.caption));
  return card;
}

/**
 * Собирает DOM карточки embed.
 * @param {Embed} embed
 * @returns {HTMLElement}
 */
export function createEmbed(embed) {
  if (!embed || (embed.kind !== "link" && embed.kind !== "image")) {
    throw new UIError("ACTION_FAILED", `createEmbed: неизвестный тип «${String(embed?.kind)}»`);
  }
  return embed.kind === "link" ? linkEmbed(embed) : imageEmbed(embed);
}

/**
 * Все embed'ы строки в одном контейнере (сетка 2 колонки, EMB-11).
 * @param {Embed[]} embeds
 * @returns {HTMLElement | null}
 */
export function createEmbeds(embeds) {
  if (embeds.length === 0) return null;
  const list = document.createElement("div");
  list.className = "embeds";
  list.setAttribute("data-count", String(embeds.length));
  for (const embed of embeds) list.append(createEmbed(embed));
  return list;
}
