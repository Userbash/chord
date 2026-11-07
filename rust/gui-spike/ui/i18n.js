// @ts-check
/**
 * Каталог русской локализации Discord UI (`rust/gui-spike/ui/i18n.js`, task-194).
 *
 * Источники-оракулы:
 * - `/home/sanya/re-deep/i18n-deep.md` (§1 6-символьные base64 xxh64 ключи, §2 CLDR плурализация ru, §6 числа)
 * - `/home/sanya/re-deep/ru-catalog.json` (канонические строки Discord 1.0.162)
 * - `./features/plural-format.js` (`selectRussianPluralCategory`, `formatPluralRu`, `RU_PLURAL_PRESETS`)
 */

import {
  RU_PLURAL_PRESETS,
  formatPluralRu,
  formatPresetRu,
  selectRussianPluralCategory,
} from "./features/plural-format.js";

export {
  RU_PLURAL_PRESETS,
  formatPluralRu,
  formatPresetRu,
  selectRussianPluralCategory,
};

/** Локаль интерфейса по умолчанию. */
export const DEFAULT_LOCALE = "ru";

/**
 * Канонические 6-символьные base64(xxh64(id, seed=0))[0..6] ключи Discord 1.0.162
 * из `/home/sanya/re-deep/i18n-deep.md` и `/home/sanya/re-deep/ru-catalog.json`.
 * Покрывают: inbox, search, settings, threads, polls, voice, attachments,
 * tooltips, shortcuts, errors, статусы и плюральные счётчики.
 */
export const HASHED_RU_KEYS = Object.freeze({
  // Inbox & Notifications
  GSmTKJ: "Почта",
  NaZyYC: "Уведомления",
  e6RscS: "Пометить как прочитанное",
  M3tKAP: "Помечено как прочитанное",

  // Search
  "pw+r5b": "Поиск",
  "41014u": "По вашему запросу ничего не найдено",
  "4Y3O+O":
    "{count, plural, one {Найден {count} результат} few {Найдено {count} результата} many {Найдено {count} результатов} other {Найдено {count} результатов}}",

  // Settings & Locales (i18n-deep.md §1)
  "QV4/6p": "Настройки",
  cduTBL: "Настройки пользователя",
  "+irF5O": "Русский",
  "+dXJII": "Английский, США",
  TyCVIq: "Хорошо",

  // Threads & Pins
  B2panI: "Ветки",
  "mp1N/2": "Закреплённые сообщения",

  // Polls
  Flr51u: "Создать опрос",
  RgIi2B: "Опрос",

  // Voice
  daXg45: "Голосовая связь подключена",
  PygWIS: "Переподключение",
  zSqdrS: "Сейчас в голосовом чате никого нет",
  "juL9/L": "Голосовая связь отключена",
  lhPHmz: "Этот микрофон отключён",
  HITUcR: "Заглушить",
  wjcRFX: "Откл. звук",
  "6vrfgt": "Отключиться",
  fRzCbB: "Настройки ввода",
  aA4Vce: "Настройки вывода",

  // Attachments & Composer
  "d3+iYs": "Отправить файл",
  QuPY4s: "Отправить сообщение",
  BDYHSe: "Написать сообщение",
  "3Gc2XP": "Файл скрыт под спойлером",
  DYFPg2:
    "Слишком большой размер вложений. Максимальный общий размер\u00A0— {maxSizeMb}\u00A0МБ.",

  // Tooltips & Message Actions
  lfIHs4: "Добавить реакцию",
  "5IEsGx": "Ответить",
  ZXE1s4: "Редактировать сообщение",
  JrGD7E: "Скопировать текст",
  zBoHlf: "Копировать ID сообщения",
  xwMqD7: "Удалить сообщение",
  eNjAah: "ID скопирован",
  kLQySL: "Показать всех участников",
  ti7FrI: "Закрыть",
  "4gTnU0": "Отмена",

  // Shortcuts
  T9DA2K: "Горячие клавиши",
  "43Qn2r": "Список горячих клавиш",
  c1qUOQ: "Режим рации ({keybind}) активирован",

  // Errors
  S69lJR: "Не удаётся подключить вас к этому каналу. Повторите попытку позже.",
  "7/2/3M": "У вас нет права присоединиться к этому голосовому каналу.",
  "PW7+do": "Недостаточно прав для выполнения запроса.",
  "1BRkQ1": "Сетевое подключение ограничено или недоступно.",
  sAfT1K: "Не удалось загрузить изображение.",
  w3ND8L: "Повторить",

  // Statuses & Canonical Plurals (i18n-deep.md §2, §6)
  "NG43/6": "В сети",
  qWbtVU: "Неактивен",
  gH3Frd: "Не беспокоить",
  BagU2U: "В сети\u00A0— {online}",
  pAQJsJ:
    "{count, plural, one {{count} участник} few {{count} участника} many {{count} участников} other {{count} участников}}",
  "r/EUap":
    "{count, plural, one {{count} игра} few {{count} игры} many {{count} игр} other {{count} игр}}",
  "+sThav":
    "{count, plural, one {{count} час} few {{count} часа} many {{count} часов} other {{count} часа}}",
});

/**
 * Семантический каталог русских строк UI.
 * @type {Readonly<Record<string, string>>}
 */
export const RU_CATALOG = Object.freeze({
  "inbox.title": HASHED_RU_KEYS["GSmTKJ"],
  "inbox.markRead": HASHED_RU_KEYS["e6RscS"],
  "inbox.markedRead": HASHED_RU_KEYS["M3tKAP"],
  "notifications.title": HASHED_RU_KEYS["NaZyYC"],
  "search.placeholder": HASHED_RU_KEYS["pw+r5b"],
  "search.empty": HASHED_RU_KEYS["41014u"],
  "search.resultsCount": HASHED_RU_KEYS["4Y3O+O"],
  "settings.title": HASHED_RU_KEYS["QV4/6p"],
  "settings.user": HASHED_RU_KEYS["cduTBL"],
  "threads.title": HASHED_RU_KEYS["B2panI"],
  "pins.title": HASHED_RU_KEYS["mp1N/2"],
  "polls.create": HASHED_RU_KEYS["Flr51u"],
  "polls.title": HASHED_RU_KEYS["RgIi2B"],
  "voice.connected": HASHED_RU_KEYS["daXg45"],
  "voice.reconnecting": HASHED_RU_KEYS["PygWIS"],
  "voice.empty": HASHED_RU_KEYS["zSqdrS"],
  "voice.disconnected": HASHED_RU_KEYS["juL9/L"],
  "voice.micMuted": HASHED_RU_KEYS["lhPHmz"],
  "voice.mute": HASHED_RU_KEYS["HITUcR"],
  "voice.deafen": HASHED_RU_KEYS["wjcRFX"],
  "voice.disconnect": HASHED_RU_KEYS["6vrfgt"],
  "attachments.upload": HASHED_RU_KEYS["d3+iYs"],
  "attachments.spoiler": HASHED_RU_KEYS["3Gc2XP"],
  "attachments.tooLarge": HASHED_RU_KEYS["DYFPg2"],
  "composer.send": HASHED_RU_KEYS["QuPY4s"],
  "composer.write": HASHED_RU_KEYS["BDYHSe"],
  "composer.placeholder": "Написать сообщение в #{channel}",
  "tooltips.addReaction": HASHED_RU_KEYS["lfIHs4"],
  "tooltips.reply": HASHED_RU_KEYS["5IEsGx"],
  "tooltips.editMessage": HASHED_RU_KEYS["ZXE1s4"],
  "tooltips.copyText": HASHED_RU_KEYS["JrGD7E"],
  "tooltips.copyId": HASHED_RU_KEYS["zBoHlf"],
  "tooltips.deleteMessage": HASHED_RU_KEYS["xwMqD7"],
  "tooltips.close": HASHED_RU_KEYS["ti7FrI"],
  "tooltips.cancel": HASHED_RU_KEYS["4gTnU0"],
  "shortcuts.title": HASHED_RU_KEYS["T9DA2K"],
  "shortcuts.list": HASHED_RU_KEYS["43Qn2r"],
  "shortcuts.pttActive": HASHED_RU_KEYS["c1qUOQ"],
  "errors.voiceConnect": HASHED_RU_KEYS["S69lJR"],
  "errors.voiceNoPermission": HASHED_RU_KEYS["7/2/3M"],
  "errors.forbidden": HASHED_RU_KEYS["PW7+do"],
  "errors.network": HASHED_RU_KEYS["1BRkQ1"],
  "errors.imageLoad": HASHED_RU_KEYS["sAfT1K"],
  "errors.retry": HASHED_RU_KEYS["w3ND8L"],
  "status.online": HASHED_RU_KEYS["NG43/6"],
  "status.idle": HASHED_RU_KEYS["qWbtVU"],
  "status.dnd": HASHED_RU_KEYS["gH3Frd"],
  "members.onlineCount": HASHED_RU_KEYS["BagU2U"],
  "members.count": HASHED_RU_KEYS["pAQJsJ"],
});

/**
 * Запрещённые захардкоженные английские UI-подписи, не допускаемые на русских экранах.
 * @type {ReadonlyArray<string>}
 */
export const FORBIDDEN_ENGLISH_LABELS = Object.freeze([
  "Send Message",
  "Send a message",
  "Search",
  "Settings",
  "User Settings",
  "Mute",
  "Unmute",
  "Deafen",
  "Undeafen",
  "Disconnect",
  "Upload File",
  "Upload a File",
  "Inbox",
  "Threads",
  "Create Poll",
  "Pinned Messages",
  "Keyboard Shortcuts",
  "Add Reaction",
  "Reply",
  "Edit Message",
  "Delete Message",
  "Copy Text",
  "Mark As Read",
  "Notifications",
  "Voice Connected",
]);

/**
 * Разбирает ветви внутри тела ICU `{varName, plural, ...}`.
 * @param {string} body
 * @returns {Record<string, string>}
 */
function parsePluralBranches(body) {
  /** @type {Record<string, string>} */
  const branches = {};
  let index = 0;
  const len = body.length;

  while (index < len) {
    while (index < len && /\s/.test(body.charAt(index))) index += 1;
    if (index >= len) break;

    const selectorStart = index;
    while (index < len && !/[\s{]/.test(body.charAt(index))) index += 1;
    const selector = body.slice(selectorStart, index).trim();
    while (index < len && /\s/.test(body.charAt(index))) index += 1;

    if (!selector || body.charAt(index) !== "{") break;

    let depth = 0;
    const contentStart = index + 1;
    while (index < len) {
      const ch = body.charAt(index);
      if (ch === "{") depth += 1;
      else if (ch === "}") {
        depth -= 1;
        if (depth === 0) {
          branches[selector] = body.slice(contentStart, index);
          index += 1;
          break;
        }
      }
      index += 1;
    }
  }

  return branches;
}

/**
 * Вычисляет все блоки `{varName, plural, ...}` в строке шаблона через
 * `selectRussianPluralCategory` и `formatPluralRu`.
 * @param {string} template
 * @param {Record<string, unknown>} params
 * @returns {string}
 */
function evaluateIcuPluralBlocks(template, params) {
  const headerRe = /\{([A-Za-z0-9_]+)\s*,\s*plural\s*,/;
  let result = template;

  while (true) {
    const match = headerRe.exec(result);
    if (!match || match.index === undefined) break;

    const blockStart = match.index;
    const varName = match[1] ?? "count";
    const bodyStart = blockStart + match[0].length;
    let depth = 1;
    let scan = bodyStart;

    while (scan < result.length && depth > 0) {
      const ch = result.charAt(scan);
      if (ch === "{") depth += 1;
      else if (ch === "}") depth -= 1;
      scan += 1;
    }

    if (depth !== 0) break;

    const body = result.slice(bodyStart, scan - 1);
    const branches = parsePluralBranches(body);
    const rawCount = Object.prototype.hasOwnProperty.call(params, varName)
      ? params[varName]
      : params.count;
    const sanitizedCount = Math.max(0, Math.trunc(Number(rawCount) || 0));
    const exactSelector = `=${sanitizedCount}`;

    let replacement = "";
    if (Object.prototype.hasOwnProperty.call(branches, exactSelector)) {
      replacement = (branches[exactSelector] ?? "")
        .split("{count}")
        .join(String(sanitizedCount))
        .split(`{${varName}}`)
        .join(String(sanitizedCount))
        .split("#")
        .join(String(sanitizedCount));
    } else {
      const category = selectRussianPluralCategory(sanitizedCount);
      const fallbackBranch =
        branches[category] ?? branches.other ?? branches.many ?? branches.one ?? "";
      /** @type {Record<string, string>} */
      const normalizedForms = {
        one: (branches.one ?? fallbackBranch)
          .split(`{${varName}}`)
          .join("{count}")
          .split("#")
          .join("{count}"),
        few: (branches.few ?? fallbackBranch)
          .split(`{${varName}}`)
          .join("{count}")
          .split("#")
          .join("{count}"),
        many: (branches.many ?? branches.other ?? fallbackBranch)
          .split(`{${varName}}`)
          .join("{count}")
          .split("#")
          .join("{count}"),
        other: (branches.other ?? branches.many ?? fallbackBranch)
          .split(`{${varName}}`)
          .join("{count}")
          .split("#")
          .join("{count}"),
      };
      replacement = formatPluralRu(sanitizedCount, normalizedForms);
    }

    result = result.slice(0, blockStart) + replacement + result.slice(scan);
  }

  return result;
}

/**
 * Подставляет `{param}` плейсхолдеры из `params`.
 * @param {string} text
 * @param {Record<string, unknown>} params
 * @returns {string}
 */
function substituteParams(text, params) {
  let output = text;
  for (const key of Object.keys(params)) {
    const val = params[key];
    if (val === undefined || val === null || typeof val === "object") continue;
    output = output.split(`{${key}}`).join(String(val));
  }
  return output;
}

/**
 * Разрешает 6-символьный base64 xxh64 хеш-ключ из `HASHED_RU_KEYS`,
 * семантический ключ из `RU_CATALOG`, пресет из `RU_PLURAL_PRESETS` или ICU-шаблон,
 * подставляет `{param}` и форматирует русские плюральные формы.
 *
 * @param {string} hashKey
 * @param {Record<string, unknown>} [params]
 * @returns {string}
 */
export function tHashed(hashKey, params = {}) {
  if (typeof hashKey !== "string" || hashKey.length === 0) return "";
  const safeParams =
    params && typeof params === "object" ? /** @type {Record<string, unknown>} */ (params) : {};

  /** @type {Record<string, string>} */
  const hashedMap = HASHED_RU_KEYS;
  /** @type {Record<string, string>} */
  const semanticMap = RU_CATALOG;

  /** @type {string | null} */
  let template = null;
  if (Object.prototype.hasOwnProperty.call(hashedMap, hashKey)) {
    template = hashedMap[hashKey] ?? null;
  } else if (Object.prototype.hasOwnProperty.call(semanticMap, hashKey)) {
    template = semanticMap[hashKey] ?? null;
  } else if (Object.prototype.hasOwnProperty.call(RU_PLURAL_PRESETS, hashKey)) {
    return formatPresetRu(hashKey, safeParams.count);
  } else if (/\{[A-Za-z0-9_]+\s*,\s*plural\s*,/.test(hashKey)) {
    template = hashKey;
  }

  if (template === null) return "";

  let resolved = template;
  if (/\{[A-Za-z0-9_]+\s*,\s*plural\s*,/.test(resolved)) {
    resolved = evaluateIcuPluralBlocks(resolved, safeParams);
  } else if (
    Object.prototype.hasOwnProperty.call(safeParams, "count") &&
    safeParams.forms &&
    typeof safeParams.forms === "object"
  ) {
    const pluralText = formatPluralRu(
      safeParams.count,
      /** @type {import("./features/plural-format.js").PluralForms} */ (safeParams.forms),
    );
    if (resolved.includes("{plural}")) {
      resolved = resolved.split("{plural}").join(pluralText);
    } else {
      resolved = pluralText;
    }
  }

  return substituteParams(resolved, safeParams);
}

/**
 * Семантический алиас `tHashed` для вызова `t(key, params)`.
 * @param {string} key
 * @param {Record<string, unknown>} [params]
 * @returns {string}
 */
export function t(key, params = {}) {
  return tHashed(key, params);
}

/**
 * Форматирует русские плюральные формы через `formatPluralRu`.
 * @param {unknown} count
 * @param {import("./features/plural-format.js").PluralForms | undefined} forms
 * @returns {string}
 */
export function plural(count, forms) {
  return formatPluralRu(count, forms);
}

/**
 * Форматирует число в локали (`ru-RU` по умолчанию, с неразрывным пробелом `\u00A0`).
 * @param {number | string} value
 * @param {string} [locale]
 * @returns {string}
 */
export function formatNumber(value, locale = "ru-RU") {
  const num = Number(value);
  if (!Number.isFinite(num)) return "0";
  return new Intl.NumberFormat(locale).format(num);
}

/**
 * Форматирует дату/время через `Intl.DateTimeFormat`.
 * @param {Date | number | string} value
 * @param {string} [locale]
 * @param {Intl.DateTimeFormatOptions} [options]
 * @returns {string}
 */
export function formatDate(value, locale = "ru-RU", options = undefined) {
  const date = value instanceof Date ? value : new Date(value);
  if (Number.isNaN(date.getTime())) return "";
  return new Intl.DateTimeFormat(locale, options).format(date);
}

/**
 * Проверяет, совпадает ли строка с одной из запрещённых английских UI-меток.
 * @param {string} candidate
 * @returns {boolean}
 */
function matchesForbiddenEnglishLabel(candidate) {
  const normalized = candidate.trim().toLowerCase();
  if (!normalized) return false;
  for (const label of FORBIDDEN_ENGLISH_LABELS) {
    if (normalized === label.toLowerCase()) return true;
  }
  return false;
}

/**
 * Извлекает значение атрибута из реального или тестового DOM-узла.
 * @param {any} node
 * @param {string} attrName
 * @returns {string | null}
 */
function readNodeAttribute(node, attrName) {
  if (!node || typeof node !== "object") return null;
  if (typeof node.getAttribute === "function") {
    const val = node.getAttribute(attrName);
    if (typeof val === "string") return val;
  }
  if (node.attributes && typeof node.attributes === "object") {
    if (typeof node.attributes.get === "function") {
      const val = node.attributes.get(attrName);
      if (typeof val === "string") return val;
    } else if (Object.prototype.hasOwnProperty.call(node.attributes, attrName)) {
      const raw = node.attributes[attrName];
      if (typeof raw === "string") return raw;
      if (raw && typeof raw.value === "string") return raw.value;
    }
  }
  if (attrName === "aria-label" && typeof node.ariaLabel === "string") return node.ariaLabel;
  if (attrName === "placeholder" && typeof node.placeholder === "string") return node.placeholder;
  if (attrName === "title" && typeof node.title === "string") return node.title;
  return null;
}

/**
 * Сканирует текстовые узлы и атрибуты `aria-label` / `placeholder` / `title`
 * внутри `rootEl` и возвращает список обнаруженных непереведённых английских
 * UI-меток из `FORBIDDEN_ENGLISH_LABELS`.
 *
 * @param {any} rootEl
 * @returns {string[]}
 */
export function auditUntranslatedDomText(rootEl) {
  if (!rootEl || typeof rootEl !== "object") return [];

  /** @type {Set<string>} */
  const found = new Set();
  /** @type {Set<any>} */
  const visited = new Set();
  const auditedAttrs = ["aria-label", "placeholder", "title"];

  /**
   * @param {unknown} raw
   */
  const checkText = (raw) => {
    if (typeof raw !== "string") return;
    const trimmed = raw.trim();
    if (trimmed.length > 0 && matchesForbiddenEnglishLabel(trimmed)) {
      found.add(trimmed);
    }
  };

  /**
   * @param {any} node
   */
  const walk = (node) => {
    if (!node || typeof node !== "object" || visited.has(node)) return;
    visited.add(node);

    const isTextNode =
      node.nodeType === 3 ||
      (!node.tagName && !Array.isArray(node.children) && !Array.isArray(node.childNodes));

    if (isTextNode) {
      checkText(node.nodeValue ?? node.textContent);
      return;
    }

    for (const attr of auditedAttrs) {
      checkText(readNodeAttribute(node, attr));
    }

    const childNodes = Array.isArray(node.childNodes) ? node.childNodes : [];
    const children = Array.isArray(node.children) ? node.children : [];

    if (childNodes.length === 0 && children.length === 0 && typeof node.textContent === "string") {
      checkText(node.textContent);
    }

    for (const child of childNodes) walk(child);
    for (const child of children) walk(child);
  };

  walk(rootEl);
  return [...found];
}
