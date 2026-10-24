// @ts-check
/**
 * Содержимое строки сообщения, которого нет в модели высот (D-04): day divider,
 * reply-quote, reactions, embeds, метка «изменено».
 *
 * Почему отдельный модуль: `ui/model.js` заморожен (высота строки 26/48/+22 —
 * оракул RW-37), а extras — данные демо-канала, а не геометрия. Правила
 * детерминированы по индексу, поэтому строка восстанавливается после любого
 * скролла и переиспользования узла.
 *
 * Раскладка: extras живут **внутри** `.text`, чтобы у `.row` осталось ровно два
 * ребёнка (`.author` + `.text`, инвариант DOM-CONTRACT §7.1). Поэтому `.text-body`
 * всегда первый ребёнок: `textContent` начинается с `#<index>` (оракул тестов).
 */

export const AUTHORS = ["sasha", "mira", "kot", "dev_null", "lena", "arty", "north", "zoe"];

/** Day divider: каждые 100 сообщений — новые сутки (правило демо-данных). */
export const DAY_DIVIDER_EVERY = 100;
/** Reply-quote: индекс ≡ 12 (mod 40) — это начало группы с одной строкой (48px). */
export const REPLY_MOD = 40;
export const REPLY_AT = 12;
/** Reactions: индекс ≡ 9 (mod 17) — строка высотой ≥48, кроме embed-индексов. */
export const REACTIONS_MOD = 17;
export const REACTIONS_AT = 9;
/** Link embed: индекс ≡ 9 (mod 391 = 17·23) — три строки текста (70px). */
export const EMBED_LINK_MOD = 391;
export const EMBED_LINK_AT = 9;
/** Image embed — каждый второй embed-индекс (400 % 4 === 0: начало группы). */
export const EMBED_IMAGE_EVERY = 2;
/** Метка «изменено»: inline-суффикс, высоту не меняет. */
export const EDITED_MOD = 11;
export const EDITED_AT = 1;

/** 9 опций секции «Содержит» (R-10: inputs=5, options=9; тексты — `[A]` G-05). */
export const HAS_OPTIONS = Object.freeze([
  "ссылка",
  "картинка",
  "файл",
  "звук",
  "видео",
  "опрос",
  "приглашение",
  "вложение",
  "эмодзи",
]);

/** Быстрые реакции hover-панели (4 из 15 пунктов R-16). */
export const QUICK_REACTIONS = Object.freeze(["👍", "❤️", "😂", "😮"]);

/**
 * @typedef {{ emoji: string, count: number, me: boolean }} Reaction
 * @typedef {{ author: string, text: string, hue: number }} Reply
 * @typedef {{ kind: "link" | "image", provider?: string, title?: string, url?: string, description?: string, caption?: string }} Embed
 * @typedef {{
 *   day: { iso: string, label: string } | null,
 *   reply: Reply | null,
 *   reactions: Reaction[],
 *   embeds: Embed[],
 *   edited: boolean,
 * }} Extras
 */

/**
 * @param {number} index
 * @returns {{ iso: string, label: string }}
 */
export function dayDivider(index) {
  if (!Number.isInteger(index) || index < 0) {
    throw new RangeError(`dayDivider: индекс должен быть целым ≥ 0, получено ${String(index)}`);
  }
  const day = Math.floor(index / DAY_DIVIDER_EVERY);
  const date = new Date(Date.UTC(2026, 9, 23) - day * 86_400_000);
  // Формат фиксирован локалью: тест/AT-SPI сверяют подпись, а не TZ машины.
  const label = new Intl.DateTimeFormat("ru-RU", {
    day: "numeric",
    month: "long",
    year: "numeric",
    timeZone: "UTC",
  }).format(date);
  return { iso: date.toISOString(), label };
}

/**
 * @param {number} index
 * @returns {Reply}
 */
function replyFor(index) {
  const author = AUTHORS[(index * 3) % AUTHORS.length] ?? "sasha";
  return {
    author,
    text: `исходное сообщение #${index - 3} — на него отвечаем`,
    hue: (index * 59) % 360,
  };
}

/**
 * @param {number} index
 * @returns {Reaction[]}
 */
export function baseReactions(index) {
  return [
    { emoji: "🔥", count: 1 + (index % 4), me: index % 2 === 0 },
    { emoji: "✅", count: 1, me: false },
  ];
}

/**
 * @param {number} index
 * @returns {Embed[]}
 */
function embedsFor(index) {
  if (index % EMBED_LINK_MOD === EMBED_LINK_AT) {
    const ordinal = (index - EMBED_LINK_AT) / EMBED_LINK_MOD;
    if (ordinal % EMBED_IMAGE_EVERY !== 0) {
      // Каждый второй embed — картинка (индекс 400: начало группы, 3 строки).
      return [{ kind: "image", caption: "Макет чата — замер layout.json (без сети, заглушка)" }];
    }
    return [{
      kind: "link",
      provider: "docs.rust-lang.org",
      title: "Собственность и заимствование — Rust Book",
      url: "https://docs.rust-lang.org/book/ch04-00-understanding-ownership.html",
      // 3 строки максимум у описания (EMB-05), дальше ellipsis делает CSS.
      description: "Владение — набор правил, определяющих, как Rust управляет памятью: "
        + "у каждого значения один владелец, владелец уходит из области — значение освобождается.",
    }];
  }
  return [];
}

/**
 * Extras строки по индексу. Детерминированно, без состояния и сети.
 * @param {number} index
 * @returns {Extras}
 */
export function extrasFor(index) {
  if (!Number.isInteger(index) || index < 0) {
    throw new RangeError(`extrasFor: индекс должен быть целым ≥ 0, получено ${String(index)}`);
  }
  const embedded = index % EMBED_LINK_MOD === EMBED_LINK_AT;
  return {
    day: index % DAY_DIVIDER_EVERY === 0 ? dayDivider(index) : null,
    reply: index % REPLY_MOD === REPLY_AT ? replyFor(index) : null,
    // Две группы блоков в одной строке не помещаются в замороженную высоту:
    // приоритет у embed, реакции — на остальных индексах правила.
    reactions: !embedded && index % REACTIONS_MOD === REACTIONS_AT ? baseReactions(index) : [],
    embeds: embedsFor(index),
    edited: index % EDITED_MOD === EDITED_AT,
  };
}

/**
 * Пункты hover-панели (R-16: 4 реакции + 11 действий = 15).
 * `command` — реальный обработчик, `null` — честный `aria-disabled` + подсказка.
 * @type {ReadonlyArray<{ command: string | null, label: string, key?: string }>}
 */
export const MESSAGE_ACTIONS = Object.freeze([
  { command: "add-reaction", label: "Добавить реакцию" },
  { command: "reply", label: "Ответить" },
  { command: "edit", label: "Изменить сообщение" },
  { command: null, label: "Закрепить сообщение" },
  { command: null, label: "Переслать" },
  { command: null, label: "Создать ветку" },
  { command: "delete", label: "Удалить сообщение" },
  // Буфер обмена в webview без разрешения — честно недоступен (не «мёртвая» кнопка).
  { command: null, label: "Копировать текст" },
  { command: null, label: "Копировать ссылку" },
  { command: null, label: "Пожаловаться" },
  { command: null, label: "Ещё" },
]);

/** Подсказка недоступного пункта (без сети/бэкенда в спайке). */
export const UNAVAILABLE_HINT = "Недоступно в спайке (Phase 3)";
