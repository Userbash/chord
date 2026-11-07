// @ts-check
/**
 * T-222 (FE-P1-21, RW-40 / G2): UI-контроллер всплывающего автодополнения композера
 * для слэш-команд `/`, упоминаний `@user/@role` и текстовых каналов `#channel`
 * (`composer-autocomplete.js`).
 *
 * Контрактные оракулы:
 * - `/home/sanya/re-deep/frontend-d03.md` (композер, всплывающий список подсказок `role="listbox"`,
 *   элементы `role="option"` с `aria-selected`);
 * - `/home/sanya/re-deep/ru-catalog.json` (`X2x0MF`: «Доступные команды», секции КОМАНДЫ /
 *   УЧАСТНИКИ И РОЛИ / ТЕКСТОВЫЕ КАНАЛЫ).
 *
 * @module ui/features/composer-autocomplete
 */

/**
 * Максимальное число подсказок в выпадающем списке автодополнения.
 */
export const AUTOCOMPLETE_MAX_SUGGESTIONS = 10;

/**
 * Префиксы триггеров автодополнения композера.
 * @type {Readonly<{ USER: "@", CHANNEL: "#", SLASH: "/" }>}
 */
export const AUTOCOMPLETE_KINDS = Object.freeze({
  USER: "@",
  CHANNEL: "#",
  SLASH: "/",
});

/**
 * Русские заголовки секций автодополнения по типу триггера.
 * @type {Readonly<{ slash: string, user: string, channel: string }>}
 */
export const AUTOCOMPLETE_HEADERS_RU = Object.freeze({
  slash: "КОМАНДЫ",
  user: "УЧАСТНИКИ И РОЛИ",
  channel: "ТЕКСТОВЫЕ КАНАЛЫ",
});

/**
 * @typedef {"slash" | "user" | "channel"} AutocompleteTriggerKind
 */

/**
 * @typedef {object} AutocompleteTrigger
 * @property {AutocompleteTriggerKind} kind
 * @property {string} query
 * @property {number} startIndex
 */

/**
 * @typedef {object} AutocompleteUserItem
 * @property {string} id
 * @property {string} [username]
 * @property {string} [displayName]
 * @property {string} [globalName]
 * @property {string} [name]
 */

/**
 * @typedef {object} AutocompleteChannelItem
 * @property {string} id
 * @property {string} name
 * @property {string} [topic]
 */

/**
 * @typedef {object} AutocompleteCommandItem
 * @property {string} [id]
 * @property {string} name
 * @property {string} [description]
 */

/**
 * @typedef {AutocompleteUserItem | AutocompleteChannelItem | AutocompleteCommandItem} AutocompleteItem
 */

/**
 * @typedef {object} InsertReplacementPayload
 * @property {string} newText
 * @property {number} newCursorIndex
 * @property {any} item
 */

/**
 * @typedef {object} UpdateFromInputParams
 * @property {string} text
 * @property {number} [cursorIndex]
 * @property {ReadonlyArray<any>} [users]
 * @property {ReadonlyArray<any>} [channels]
 * @property {ReadonlyArray<any>} [commands]
 */

/**
 * @typedef {object} ComposerAutocompleteControllerOptions
 * @property {any} container
 * @property {(payload: InsertReplacementPayload) => void} [onInsertReplacement]
 */

/**
 * Распознаёт активный триггер автодополнения (`/`, `@`, `#`) перед позицией курсора.
 *
 * Правила:
 * - Инспектирует подстроку `text.slice(0, cursorIndex)`.
 * - Если с начала строки до курсора текст соответствует `/^\/([a-z0-9_-]*)$/i`,
 *   возвращает `{ kind: "slash", query: match[1], startIndex: 0 }`.
 * - Если токен непосредственно перед курсором начинается с `@` или `#` и предваряется
 *   началом строки или пробельным символом, возвращает
 *   `{ kind: prefix === "@" ? "user" : "channel", query, startIndex }`.
 * - Иначе возвращает `null`.
 *
 * @param {string} text
 * @param {number} [cursorIndex]
 * @returns {AutocompleteTrigger | null}
 */
export function detectAutocompleteTrigger(text, cursorIndex) {
  if (typeof text !== "string" || text.length === 0) {
    return null;
  }

  let effectiveCursor = text.length;
  if (cursorIndex !== undefined) {
    if (typeof cursorIndex !== "number" || !Number.isFinite(cursorIndex) || cursorIndex <= 0) {
      return null;
    }
    effectiveCursor = Math.min(Math.trunc(cursorIndex), text.length);
  }

  const beforeCursor = text.slice(0, effectiveCursor);
  if (beforeCursor.length === 0) {
    return null;
  }

  const slashMatch = /^\/([a-z0-9_-]*)$/i.exec(beforeCursor);
  if (slashMatch !== null && slashMatch[1] !== undefined) {
    return {
      kind: "slash",
      query: slashMatch[1],
      startIndex: 0,
    };
  }

  const tokenMatch = /(?:^|\s)([@#])([^\s@#]*)$/.exec(beforeCursor);
  if (tokenMatch !== null && tokenMatch[1] !== undefined && tokenMatch[2] !== undefined) {
    const prefix = tokenMatch[1];
    const query = tokenMatch[2];
    const startIndex = beforeCursor.length - prefix.length - query.length;
    return {
      kind: prefix === AUTOCOMPLETE_KINDS.USER ? "user" : "channel",
      query,
      startIndex,
    };
  }

  return null;
}

/**
 * Возвращает фабрику DOM-элементов из контейнера или глобального документа.
 *
 * @param {any} containerEl
 * @returns {{ createElement: (tag: string) => any }}
 */
function resolveDocument(containerEl) {
  if (
    containerEl &&
    containerEl.ownerDocument &&
    typeof containerEl.ownerDocument.createElement === "function"
  ) {
    return containerEl.ownerDocument;
  }
  if (containerEl && typeof containerEl.createElement === "function") {
    return containerEl;
  }
  const globalDoc = /** @type {any} */ (globalThis).document;
  if (globalDoc && typeof globalDoc.createElement === "function") {
    return globalDoc;
  }
  throw new TypeError("container must provide ownerDocument.createElement");
}

/**
 * Заменяет дочерние узлы элемента без использования `innerHTML`.
 *
 * @param {any} targetEl
 * @param {any[]} nodes
 */
function replaceElementChildren(targetEl, nodes) {
  if (typeof targetEl.replaceChildren === "function") {
    targetEl.replaceChildren(...nodes);
    return;
  }
  if (Array.isArray(targetEl.children)) {
    for (const child of targetEl.children) {
      if (child && typeof child === "object") {
        child.parentNode = null;
      }
    }
    targetEl.children.length = 0;
  }
  for (const node of nodes) {
    targetEl.appendChild(node);
  }
}

/**
 * Фильтрует список кандидатов под активный триггер и ограничивает длину `AUTOCOMPLETE_MAX_SUGGESTIONS`.
 *
 * @param {AutocompleteTrigger} trigger
 * @param {{ users?: ReadonlyArray<any>, channels?: ReadonlyArray<any>, commands?: ReadonlyArray<any> }} sources
 * @returns {any[]}
 */
function filterSuggestions(trigger, sources) {
  const needle = trigger.query.toLowerCase();

  if (trigger.kind === "slash") {
    const list = Array.isArray(sources.commands) ? sources.commands : [];
    return list
      .filter((item) => {
        if (!item || typeof item !== "object") return false;
        const name = String(item.name ?? item.command ?? "")
          .replace(/^\/+/, "")
          .toLowerCase();
        return name.length > 0 && name.includes(needle);
      })
      .slice(0, AUTOCOMPLETE_MAX_SUGGESTIONS);
  }

  if (trigger.kind === "user") {
    const list = Array.isArray(sources.users) ? sources.users : [];
    return list
      .filter((item) => {
        if (!item || typeof item !== "object") return false;
        const fields = [item.username, item.displayName, item.globalName, item.nick, item.name];
        return fields.some(
          (val) => typeof val === "string" && val.toLowerCase().includes(needle),
        );
      })
      .slice(0, AUTOCOMPLETE_MAX_SUGGESTIONS);
  }

  const list = Array.isArray(sources.channels) ? sources.channels : [];
  return list
    .filter((item) => {
      if (!item || typeof item !== "object") return false;
      const name = String(item.name ?? "").toLowerCase();
      return name.length > 0 && name.includes(needle);
    })
    .slice(0, AUTOCOMPLETE_MAX_SUGGESTIONS);
}

/**
 * Формирует подпись опции для кнопки автодополнения.
 *
 * @param {AutocompleteTriggerKind} kind
 * @param {any} item
 * @returns {string}
 */
function formatOptionLabel(kind, item) {
  if (kind === "slash") {
    const cmdName = String(item.name ?? item.command ?? "").replace(/^\/+/, "");
    const desc = typeof item.description === "string" && item.description.trim().length > 0
      ? ` — ${item.description.trim()}`
      : "";
    return `/${cmdName}${desc}`;
  }
  if (kind === "channel") {
    return `#${String(item.name ?? item.id ?? "")}`;
  }
  const primary = String(item.displayName ?? item.globalName ?? item.name ?? item.username ?? item.id ?? "");
  const secondary = typeof item.username === "string" && item.username !== primary ? ` (@${item.username})` : "";
  return `@${primary}${secondary}`;
}

/**
 * Формирует строку замены триггер-токена при подтверждении выбора.
 *
 * @param {AutocompleteTriggerKind} kind
 * @param {any} item
 * @returns {string}
 */
function buildReplacementToken(kind, item) {
  if (kind === "user") {
    return `<@${String(item.id ?? "")}>`;
  }
  if (kind === "channel") {
    return `<#${String(item.id ?? "")}>`;
  }
  const cmdName = String(item.name ?? item.command ?? "").replace(/^\/+/, "");
  return `/${cmdName} `;
}

/**
 * Создаёт контроллер всплывающего окна автодополнения композера.
 *
 * @param {ComposerAutocompleteControllerOptions} options
 */
export function createComposerAutocompleteController(options) {
  if (!options || typeof options !== "object" || !options.container) {
    throw new TypeError("createComposerAutocompleteController requires { container }");
  }

  const { container, onInsertReplacement } = options;
  const doc = resolveDocument(container);

  /** @type {AutocompleteTrigger | null} */
  let currentTrigger = null;
  /** @type {string} */
  let currentText = "";
  /** @type {number} */
  let currentCursorIndex = 0;
  /** @type {any[]} */
  let currentItems = [];
  /** @type {number} */
  let activeIndex = 0;

  /**
   * Закрывает всплывающее окно автодополнения и очищает DOM-контейнер.
   */
  function close() {
    currentTrigger = null;
    currentItems = [];
    activeIndex = 0;
    replaceElementChildren(container, []);
  }

  /**
   * Перерисовывает listbox `[data-composer-autocomplete]` в контейнере.
   */
  function render() {
    if (!currentTrigger || currentItems.length === 0) {
      replaceElementChildren(container, []);
      return;
    }

    const listbox = doc.createElement("div");
    listbox.setAttribute("role", "listbox");
    listbox.setAttribute("data-composer-autocomplete", currentTrigger.kind);

    const header = doc.createElement("div");
    header.setAttribute("data-autocomplete-header", currentTrigger.kind);
    header.textContent = AUTOCOMPLETE_HEADERS_RU[currentTrigger.kind];
    listbox.appendChild(header);

    const kind = currentTrigger.kind;
    currentItems.forEach((item, index) => {
      const optionBtn = doc.createElement("button");
      optionBtn.setAttribute("type", "button");
      optionBtn.setAttribute("role", "option");
      optionBtn.setAttribute("data-autocomplete-option", String(index));
      optionBtn.setAttribute("aria-selected", index === activeIndex ? "true" : "false");
      optionBtn.textContent = formatOptionLabel(kind, item);

      optionBtn.addEventListener("click", () => {
        activeIndex = index;
        confirmSelection();
      });

      listbox.appendChild(optionBtn);
    });

    replaceElementChildren(container, [listbox]);
  }

  /**
   * Обновляет состояние автодополнения по текущему тексту и позиции курсора в композере.
   *
   * @param {UpdateFromInputParams} params
   */
  function updateFromInput(params) {
    if (!params || typeof params.text !== "string") {
      close();
      return;
    }

    const text = params.text;
    const cursorIndex =
      typeof params.cursorIndex === "number" && Number.isFinite(params.cursorIndex)
        ? Math.min(Math.max(0, Math.trunc(params.cursorIndex)), text.length)
        : text.length;

    const trigger = detectAutocompleteTrigger(text, cursorIndex);
    if (!trigger) {
      close();
      return;
    }

    const filtered = filterSuggestions(trigger, {
      users: params.users,
      channels: params.channels,
      commands: params.commands,
    });

    if (filtered.length === 0) {
      close();
      return;
    }

    currentText = text;
    currentCursorIndex = cursorIndex;
    currentTrigger = trigger;
    currentItems = filtered;
    activeIndex = 0;
    render();
  }

  /**
   * Сдвигает активный индекс выбора на `delta` с циклическим переходом по `0..items.length - 1`.
   *
   * @param {number} delta
   * @returns {number} Новый `activeIndex` или `-1`, если список закрыт.
   */
  function moveSelection(delta) {
    if (!currentTrigger || currentItems.length === 0) {
      return -1;
    }
    const step = typeof delta === "number" && Number.isFinite(delta) ? Math.trunc(delta) : 0;
    const len = currentItems.length;
    activeIndex = (((activeIndex + step) % len) + len) % len;
    render();
    return activeIndex;
  }

  /**
   * Подтверждает выбранную подсказку: заменяет триггер-токен на `<@id>`, `<#id>` или `/command `,
   * вызывает `onInsertReplacement({ newText, newCursorIndex, item })` и закрывает список.
   *
   * @returns {InsertReplacementPayload | null}
   */
  function confirmSelection() {
    if (!currentTrigger || currentItems.length === 0) {
      return null;
    }
    const item = currentItems[activeIndex];
    if (!item) {
      return null;
    }

    const replacement = buildReplacementToken(currentTrigger.kind, item);
    const before = currentText.slice(0, currentTrigger.startIndex);
    const after = currentText.slice(currentCursorIndex);
    const newText = `${before}${replacement}${after}`;
    const newCursorIndex = before.length + replacement.length;

    close();

    const payload = { newText, newCursorIndex, item };
    if (typeof onInsertReplacement === "function") {
      onInsertReplacement(payload);
    }
    return payload;
  }

  return {
    updateFromInput,
    moveSelection,
    confirmSelection,
    close,
  };
}
