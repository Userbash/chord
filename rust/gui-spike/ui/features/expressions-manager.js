// @ts-check
/**
 * Контроллер управления кастомными эмодзи и стикерами сервера (task-256 / FE-P0-15).
 *
 * Отвечает за:
 * - замороженный справочник вкладок `EXPRESSION_MANAGER_TABS_RU` (`emojis`, `stickers`);
 * - валидацию имени кастомного эмодзи `isValidCustomEmojiName(name)` (`2..=32` символов `[A-Za-z0-9_]` после `trim()`);
 * - фильтрацию и сортировку выражений `filterCustomExpressions(items, query)` по регистронезависимой подстроке `name`,
 *   с сортировкой `name.localeCompare(..., "ru")` по возрастанию и tie-break по `id` по возрастанию без мутации входа;
 * - рендер региона `[data-expressions-manager]` (`role="region"`, `aria-label="Управление эмодзи и стикерами"`),
 *   вкладок `[data-expr-tab="emojis"|"stickers"]` (`role="tab"`, `aria-selected="true"|"false"`),
 *   счётчика слотов `[data-expr-slots]` (`"Слоты: N / M"`), списка выражений `[data-expr-item-id="<id>"]`
 *   с кнопкой удаления `[data-expr-delete-btn="<id>"]` и формы загрузки эмодзи
 *   (`[data-expr-name-input]`, `[data-expr-datauri-input]`, `[data-expr-upload-btn]`).
 *
 * Оракулы: `/home/sanya/re-deep/rest-static.md` (`GUILD_EMOJIS`, `GUILD_EMOJI`, `GUILD_STICKER_PACKS`, `GUILD_STICKER`).
 *
 * @module ui/features/expressions-manager
 */

/**
 * @typedef {"emojis" | "stickers"} ExpressionManagerTabId
 */

/**
 * @typedef {object} ExpressionManagerTabEntry
 * @property {ExpressionManagerTabId} id
 * @property {string} label
 */

/**
 * @typedef {object} CustomExpressionItem
 * @property {string} id
 * @property {string} name
 * @property {boolean} [animated]
 */

/**
 * @typedef {object} UploadEmojiPayload
 * @property {string} name
 * @property {string} imageDataUri
 */

/**
 * @typedef {object} ExpressionsManagerStatePatch
 * @property {ReadonlyArray<CustomExpressionItem>} [emojis]
 * @property {ReadonlyArray<CustomExpressionItem>} [stickers]
 * @property {string} [activeTab]
 * @property {number} [maxSlots]
 */

/**
 * @typedef {object} ExpressionsManagerStateSnapshot
 * @property {CustomExpressionItem[]} emojis
 * @property {CustomExpressionItem[]} stickers
 * @property {ExpressionManagerTabId} activeTab
 * @property {number} maxSlots
 */

/**
 * @typedef {object} ExpressionsManagerControllerOptions
 * @property {any} container
 * @property {ReadonlyArray<CustomExpressionItem>} [emojis]
 * @property {ReadonlyArray<CustomExpressionItem>} [stickers]
 * @property {string} [activeTab]
 * @property {number} [maxSlots]
 * @property {(payload: UploadEmojiPayload) => void} [onUploadEmoji]
 * @property {(kind: string, id: string) => void} [onDeleteExpression]
 */

const MIN_EMOJI_NAME_LENGTH = 2;
const MAX_EMOJI_NAME_LENGTH = 32;
const EMOJI_NAME_PATTERN = /^[A-Za-z0-9_]+$/;
const DATA_IMAGE_URI_PREFIX = "data:image/";
const DEFAULT_MAX_SLOTS = 50;

/**
 * Замороженный список вкладок менеджера эмодзи и стикеров сервера с русскими подписями.
 * @type {ReadonlyArray<Readonly<ExpressionManagerTabEntry>>}
 */
export const EXPRESSION_MANAGER_TABS_RU = Object.freeze([
  Object.freeze({ id: /** @type {const} */ ("emojis"), label: "Эмодзи" }),
  Object.freeze({ id: /** @type {const} */ ("stickers"), label: "Стикеры" }),
]);

/**
 * Проверяет корректность имени кастомного эмодзи:
 * строка после `trim()` должна иметь длину `2..=32` символов и соответствовать `/^[A-Za-z0-9_]+$/`.
 *
 * @param {unknown} name
 * @returns {boolean}
 */
export function isValidCustomEmojiName(name) {
  if (typeof name !== "string") return false;
  const trimmed = name.trim();
  if (trimmed.length < MIN_EMOJI_NAME_LENGTH || trimmed.length > MAX_EMOJI_NAME_LENGTH) {
    return false;
  }
  return EMOJI_NAME_PATTERN.test(trimmed);
}

/**
 * Сравнивает два выражения по `name` через `localeCompare("ru")` по возрастанию,
 * а при равенстве — по `String(id)` по возрастанию.
 *
 * @param {CustomExpressionItem} a
 * @param {CustomExpressionItem} b
 * @returns {number}
 */
function compareCustomExpressionsAsc(a, b) {
  const nameA = String(a?.name ?? "");
  const nameB = String(b?.name ?? "");
  const byName = nameA.localeCompare(nameB, "ru");
  if (byName !== 0) return byName;
  const idA = String(a?.id ?? "");
  const idB = String(b?.id ?? "");
  if (idA < idB) return -1;
  if (idA > idB) return 1;
  return 0;
}

/**
 * Фильтрует массив кастомных выражений `{ id, name, animated? }` по регистронезависимой подстроке `name`
 * и сортирует результат по `name.localeCompare(..., "ru")` по возрастанию с tie-break по `id` по возрастанию.
 * Входной массив и его элементы не мутируются.
 *
 * @param {ReadonlyArray<CustomExpressionItem> | null | undefined} items
 * @param {string} [query]
 * @returns {CustomExpressionItem[]}
 */
export function filterCustomExpressions(items, query) {
  if (!Array.isArray(items)) return [];
  const needle = typeof query === "string" ? query.trim().toLowerCase() : "";
  /** @type {CustomExpressionItem[]} */
  const matched = [];
  for (const item of items) {
    if (!item || typeof item !== "object") continue;
    const itemName = String(item.name ?? "");
    if (needle.length > 0 && !itemName.toLowerCase().includes(needle)) {
      continue;
    }
    matched.push({ ...item });
  }
  matched.sort(compareCustomExpressionsAsc);
  return matched;
}

/**
 * Нормализует идентификатор активной вкладки.
 *
 * @param {unknown} rawTab
 * @returns {ExpressionManagerTabId}
 */
function normalizeExpressionTabId(rawTab) {
  return rawTab === "stickers" ? "stickers" : "emojis";
}

/**
 * Нормализует максимальное число слотов.
 *
 * @param {unknown} rawMaxSlots
 * @returns {number}
 */
function normalizeMaxSlots(rawMaxSlots) {
  if (typeof rawMaxSlots === "number" && Number.isFinite(rawMaxSlots)) {
    return Math.max(0, Math.trunc(rawMaxSlots));
  }
  return DEFAULT_MAX_SLOTS;
}

/**
 * Клонирует массив выражений без мутации исходных элементов.
 *
 * @param {ReadonlyArray<CustomExpressionItem> | null | undefined} list
 * @returns {CustomExpressionItem[]}
 */
function cloneExpressionList(list) {
  if (!Array.isArray(list)) return [];
  /** @type {CustomExpressionItem[]} */
  const out = [];
  for (const item of list) {
    if (!item || typeof item !== "object") continue;
    out.push({ ...item });
  }
  return out;
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
  throw new TypeError("Container element must provide ownerDocument.createElement");
}

/**
 * Заменяет дочерние узлы элемента на переданный массив узлов.
 *
 * @param {any} parentEl
 * @param {any[]} nodes
 */
function replaceElementChildren(parentEl, nodes) {
  if (typeof parentEl.replaceChildren === "function") {
    parentEl.replaceChildren(...nodes);
    return;
  }
  if (Array.isArray(parentEl.children)) {
    parentEl.children.length = 0;
  }
  for (const node of nodes) {
    parentEl.appendChild(node);
  }
}

/**
 * Создаёт контроллер управления кастомными эмодзи и стикерами сервера внутри `options.container`.
 *
 * @param {ExpressionsManagerControllerOptions} options
 */
export function createExpressionsManagerController(options) {
  if (!options || !options.container) {
    throw new TypeError("options.container is required");
  }

  const container = options.container;
  const doc = resolveDocument(container);

  /** @type {CustomExpressionItem[]} */
  let emojis = cloneExpressionList(options.emojis);
  /** @type {CustomExpressionItem[]} */
  let stickers = cloneExpressionList(options.stickers);
  /** @type {ExpressionManagerTabId} */
  let activeTab = normalizeExpressionTabId(options.activeTab);
  /** @type {number} */
  let maxSlots = normalizeMaxSlots(options.maxSlots);
  let destroyed = false;

  const root = doc.createElement("section");
  root.setAttribute("data-expressions-manager", "");
  root.setAttribute("role", "region");
  root.setAttribute("aria-label", "Управление эмодзи и стикерами");

  /**
   * Перерисовывает DOM-дерево менеджера выражений.
   */
  function renderDom() {
    if (destroyed) return;

    const currentItems = activeTab === "stickers" ? stickers : emojis;
    const sortedItems = filterCustomExpressions(currentItems, "");

    // 1. Вкладки (Эмодзи / Стикеры)
    const tabListEl = doc.createElement("div");
    tabListEl.setAttribute("role", "tablist");
    tabListEl.setAttribute("aria-label", "Разделы выражений сервера");

    for (const tabMeta of EXPRESSION_MANAGER_TABS_RU) {
      const isSelected = tabMeta.id === activeTab;
      const tabBtn = doc.createElement("button");
      tabBtn.setAttribute("type", "button");
      tabBtn.setAttribute("role", "tab");
      tabBtn.setAttribute("data-expr-tab", tabMeta.id);
      tabBtn.setAttribute("aria-selected", isSelected ? "true" : "false");
      tabBtn.textContent = tabMeta.label;
      tabBtn.addEventListener("click", () => {
        activeTab = tabMeta.id;
        renderDom();
      });
      tabListEl.appendChild(tabBtn);
    }

    // 2. Счётчик слотов
    const slotsEl = doc.createElement("div");
    slotsEl.setAttribute("data-expr-slots", "");
    slotsEl.textContent = `Слоты: ${currentItems.length} / ${maxSlots}`;

    // 3. Список элементов активной вкладки
    const listEl = doc.createElement("div");
    listEl.setAttribute("data-expr-list", "");

    for (const item of sortedItems) {
      const itemId = String(item.id ?? "");
      const itemName = String(item.name ?? "");

      const rowEl = doc.createElement("div");
      rowEl.setAttribute("data-expr-item-id", itemId);

      const nameEl = doc.createElement("span");
      nameEl.setAttribute("data-expr-item-name", "");
      nameEl.textContent = itemName;
      rowEl.appendChild(nameEl);

      const deleteBtn = doc.createElement("button");
      deleteBtn.setAttribute("type", "button");
      deleteBtn.setAttribute("data-expr-delete-btn", itemId);
      deleteBtn.setAttribute("aria-label", `Удалить ${itemName}`);
      deleteBtn.textContent = "Удалить";
      deleteBtn.addEventListener("click", () => {
        const currentKind = activeTab;
        if (currentKind === "stickers") {
          stickers = stickers.filter((entry) => String(entry.id ?? "") !== itemId);
        } else {
          emojis = emojis.filter((entry) => String(entry.id ?? "") !== itemId);
        }
        renderDom();
        if (typeof options.onDeleteExpression === "function") {
          options.onDeleteExpression(currentKind, itemId);
        }
      });
      rowEl.appendChild(deleteBtn);

      listEl.appendChild(rowEl);
    }

    // 4. Форма загрузки эмодзи
    const formEl = doc.createElement("form");
    formEl.setAttribute("data-expr-upload-form", "");

    const nameInput = doc.createElement("input");
    nameInput.setAttribute("type", "text");
    nameInput.setAttribute("data-expr-name-input", "");
    nameInput.setAttribute("aria-label", "Имя эмодзи");
    nameInput.value = "";
    formEl.appendChild(nameInput);

    const dataUriInput = doc.createElement("input");
    dataUriInput.setAttribute("type", "text");
    dataUriInput.setAttribute("data-expr-datauri-input", "");
    dataUriInput.setAttribute("aria-label", "Data URI изображения");
    dataUriInput.value = "";
    formEl.appendChild(dataUriInput);

    const uploadBtn = doc.createElement("button");
    uploadBtn.setAttribute("type", "button");
    uploadBtn.setAttribute("data-expr-upload-btn", "");
    uploadBtn.textContent = "Загрузить эмодзи";
    formEl.appendChild(uploadBtn);

    const handleUpload = (/** @type {any} */ event) => {
      if (event && typeof event.preventDefault === "function") {
        event.preventDefault();
      }
      const rawName = String(nameInput.value ?? "");
      const trimmedName = rawName.trim();
      const trimmedUri = String(dataUriInput.value ?? "").trim();

      if (!isValidCustomEmojiName(rawName)) {
        return;
      }
      if (!trimmedUri.startsWith(DATA_IMAGE_URI_PREFIX)) {
        return;
      }

      if (typeof options.onUploadEmoji === "function") {
        options.onUploadEmoji({
          name: trimmedName,
          imageDataUri: trimmedUri,
        });
      }
    };

    uploadBtn.addEventListener("click", handleUpload);
    formEl.addEventListener("submit", handleUpload);

    replaceElementChildren(root, [tabListEl, slotsEl, listEl, formEl]);
  }

  /**
   * Возвращает снимок текущего состояния контроллера.
   *
   * @returns {ExpressionsManagerStateSnapshot}
   */
  function getState() {
    return {
      emojis: cloneExpressionList(emojis),
      stickers: cloneExpressionList(stickers),
      activeTab,
      maxSlots,
    };
  }

  /**
   * Частично обновляет состояние контроллера и перерисовывает DOM.
   *
   * @param {ExpressionsManagerStatePatch} [patch]
   * @returns {ExpressionsManagerStateSnapshot}
   */
  function setState(patch) {
    if (patch && typeof patch === "object") {
      if (Array.isArray(patch.emojis)) {
        emojis = cloneExpressionList(patch.emojis);
      }
      if (Array.isArray(patch.stickers)) {
        stickers = cloneExpressionList(patch.stickers);
      }
      if ("activeTab" in patch && patch.activeTab !== undefined) {
        activeTab = normalizeExpressionTabId(patch.activeTab);
      }
      if ("maxSlots" in patch && patch.maxSlots !== undefined) {
        maxSlots = normalizeMaxSlots(patch.maxSlots);
      }
    }
    renderDom();
    return getState();
  }

  /**
   * Размонтирует контроллер и очищает контейнер.
   */
  function destroy() {
    destroyed = true;
    if (root.parentNode === container && typeof container.removeChild === "function") {
      container.removeChild(root);
    } else {
      replaceElementChildren(container, []);
    }
  }

  renderDom();
  replaceElementChildren(container, [root]);

  return {
    setState,
    getState,
    destroy,
  };
}
