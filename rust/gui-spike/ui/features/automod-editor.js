// @ts-check
/**
 * Контроллер редактора правил автомодерации и фильтра ключевых слов (task-251 / FE-P0-13).
 *
 * Отвечает за:
 * - экспорт замороженных русских словарей меток триггеров (`AUTOMOD_TRIGGER_LABELS_RU`)
 *   и действий (`AUTOMOD_ACTION_LABELS_RU`) автомодерации Discord;
 * - чистую функцию `parseKeywordListInput(rawText)` для разбора списка ключевых слов
 *   из строки (разделители — запятые и переводы строк, обрезка пробелов, отбрасывание
 *   пустых токенов и токенов длиннее 60 символов, регистронезависимая дедупликация с
 *   сохранением первого встреченного регистра, лимит до 1000 элементов);
 * - контроллер `createAutoModEditorController(options)`, рендерящий регион
 *   `[data-automod-editor]` (`role="region"`, `aria-label="Автомодерация"`), карточки
 *   правил `[data-automod-rule-id="<id>"]` с переключателем `[data-automod-rule-toggle="<id>"]`
 *   (`role="switch"`, `aria-checked="true"|"false"`) и форму создания/редактирования
 *   правила `[data-automod-form]`.
 *
 * @module ui/features/automod-editor
 */

/** Максимальная допустимая длина одного ключевого слова автомодерации в символах. */
const MAX_KEYWORD_LENGTH = 60;

/** Максимальное допустимое количество ключевых слов в одном правиле автомодерации. */
const MAX_KEYWORDS_COUNT = 1000;

/**
 * Русские метки типов триггеров автомодерации Discord (`AutoModTriggerTypes`).
 * @type {Readonly<Record<number, string>>}
 */
export const AUTOMOD_TRIGGER_LABELS_RU = Object.freeze({
  1: "Фильтр ключевых слов",
  3: "Защита от спама",
  4: "Часто упоминаемые слова",
  5: "Спам упоминаниями",
});

/**
 * Русские метки типов действий автомодерации Discord (`AutoModActionTypes`).
 * @type {Readonly<Record<number, string>>}
 */
export const AUTOMOD_ACTION_LABELS_RU = Object.freeze({
  1: "Блокировать сообщение",
  2: "Отправить предупреждение",
  3: "Тайм-аут участника",
});

/**
 * @typedef {object} AutoModRuleItem
 * @property {string} id
 * @property {string} name
 * @property {number} triggerType
 * @property {number} actionType
 * @property {string[]} [keywordFilter]
 * @property {boolean} enabled
 */

/**
 * @typedef {object} AutoModSaveRulePayload
 * @property {string} name
 * @property {number} triggerType
 * @property {number} actionType
 * @property {string[]} keywordFilter
 */

/**
 * @typedef {object} AutoModEditorStatePatch
 * @property {ReadonlyArray<AutoModRuleItem>} [rules]
 */

/**
 * @typedef {object} AutoModEditorStateSnapshot
 * @property {Array<Required<AutoModRuleItem>>} rules
 */

/**
 * @typedef {object} AutoModEditorControllerOptions
 * @property {any} container
 * @property {ReadonlyArray<AutoModRuleItem>} [rules]
 * @property {(ruleId: string, enabled: boolean) => void} [onToggleRule]
 * @property {(payload: AutoModSaveRulePayload) => void} [onSaveRule]
 */

/**
 * Разбирает пользовательский ввод ключевых слов (через запятую или перевод строки):
 * - обрезает пробелы по краям каждого токена;
 * - отбрасывает пустые токены и токены длиной более 60 символов;
 * - дедуплицирует без учёта регистра, сохраняя регистр первого вхождения;
 * - возвращает не более 1000 элементов.
 *
 * @param {string} rawText
 * @returns {string[]}
 */
export function parseKeywordListInput(rawText) {
  if (typeof rawText !== "string" || rawText.length === 0) {
    return [];
  }

  const tokens = rawText.split(/[,\r\n]+/);
  /** @type {Set<string>} */
  const seenLower = new Set();
  /** @type {string[]} */
  const result = [];

  for (const rawToken of tokens) {
    const trimmed = rawToken.trim();
    if (trimmed.length === 0 || trimmed.length > MAX_KEYWORD_LENGTH) {
      continue;
    }
    const lowerKey = trimmed.toLowerCase();
    if (seenLower.has(lowerKey)) {
      continue;
    }
    seenLower.add(lowerKey);
    result.push(trimmed);
    if (result.length >= MAX_KEYWORDS_COUNT) {
      break;
    }
  }

  return result;
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
  throw new TypeError("options.container must provide ownerDocument.createElement");
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
 * Нормализует и клонирует объект правила автомодерации без мутации исходного объекта.
 *
 * @param {AutoModRuleItem} rule
 * @returns {Required<AutoModRuleItem>}
 */
function cloneAutoModRule(rule) {
  return {
    id: rule.id !== undefined && rule.id !== null ? String(rule.id) : "",
    name: typeof rule.name === "string" ? rule.name : "",
    triggerType:
      typeof rule.triggerType === "number" && Number.isFinite(rule.triggerType)
        ? rule.triggerType
        : 1,
    actionType:
      typeof rule.actionType === "number" && Number.isFinite(rule.actionType)
        ? rule.actionType
        : 1,
    keywordFilter: Array.isArray(rule.keywordFilter)
      ? rule.keywordFilter.map((kw) => String(kw))
      : [],
    enabled: Boolean(rule.enabled),
  };
}

/**
 * Нормализует входной массив правил автомодерации.
 *
 * @param {ReadonlyArray<AutoModRuleItem> | null | undefined} rawRules
 * @returns {Array<Required<AutoModRuleItem>>}
 */
function normalizeAutoModRules(rawRules) {
  if (!Array.isArray(rawRules)) return [];
  /** @type {Array<Required<AutoModRuleItem>>} */
  const list = [];
  for (const item of rawRules) {
    if (!item || typeof item !== "object") continue;
    list.push(cloneAutoModRule(item));
  }
  return list;
}

/**
 * Возвращает русскую метку триггера по коду.
 *
 * @param {number} triggerType
 * @returns {string}
 */
function getTriggerLabelRu(triggerType) {
  return AUTOMOD_TRIGGER_LABELS_RU[triggerType] ?? `Триггер ${String(triggerType)}`;
}

/**
 * Возвращает русскую метку действия по коду.
 *
 * @param {number} actionType
 * @returns {string}
 */
function getActionLabelRu(actionType) {
  return AUTOMOD_ACTION_LABELS_RU[actionType] ?? `Действие ${String(actionType)}`;
}

/**
 * Создаёт контроллер редактора правил автомодерации внутри `options.container`.
 *
 * @param {AutoModEditorControllerOptions} options
 */
export function createAutoModEditorController(options) {
  if (!options || !options.container) {
    throw new TypeError("options.container is required");
  }

  const container = options.container;
  const doc = resolveDocument(container);

  /** @type {Array<Required<AutoModRuleItem>>} */
  let rules = normalizeAutoModRules(options.rules);
  let destroyed = false;

  const root = doc.createElement("section");
  root.setAttribute("data-automod-editor", "");
  root.setAttribute("role", "region");
  root.setAttribute("aria-label", "Автомодерация");

  const rulesListEl = doc.createElement("div");
  rulesListEl.setAttribute("data-automod-rules-list", "");

  const formEl = doc.createElement("form");
  formEl.setAttribute("data-automod-form", "");

  const nameInput = doc.createElement("input");
  nameInput.setAttribute("type", "text");
  nameInput.setAttribute("data-automod-name-input", "");
  nameInput.setAttribute("placeholder", "Название правила");
  nameInput.value = "";

  const triggerSelect = doc.createElement("select");
  triggerSelect.setAttribute("data-automod-trigger-select", "");
  for (const [code, label] of Object.entries(AUTOMOD_TRIGGER_LABELS_RU)) {
    const opt = doc.createElement("option");
    opt.setAttribute("value", code);
    opt.value = code;
    opt.textContent = label;
    triggerSelect.appendChild(opt);
  }
  triggerSelect.value = "1";

  const actionSelect = doc.createElement("select");
  actionSelect.setAttribute("data-automod-action-select", "");
  for (const [code, label] of Object.entries(AUTOMOD_ACTION_LABELS_RU)) {
    const opt = doc.createElement("option");
    opt.setAttribute("value", code);
    opt.value = code;
    opt.textContent = label;
    actionSelect.appendChild(opt);
  }
  actionSelect.value = "1";

  const keywordsInput = doc.createElement("textarea");
  keywordsInput.setAttribute("data-automod-keywords-input", "");
  keywordsInput.setAttribute("placeholder", "Ключевые слова через запятую или с новой строки");
  keywordsInput.value = "";

  const saveBtn = doc.createElement("button");
  saveBtn.setAttribute("type", "button");
  saveBtn.setAttribute("data-automod-save-btn", "");
  saveBtn.textContent = "Сохранить правило";

  function handleSaveRule() {
    if (destroyed) return;
    const trimmedName = String(nameInput.value ?? "").trim();
    if (trimmedName.length === 0) {
      return;
    }

    const rawTrigger = Number(triggerSelect.value);
    const triggerType = Number.isFinite(rawTrigger) && rawTrigger > 0 ? rawTrigger : 1;

    const rawAction = Number(actionSelect.value);
    const actionType = Number.isFinite(rawAction) && rawAction > 0 ? rawAction : 1;

    const keywordFilter = parseKeywordListInput(String(keywordsInput.value ?? ""));

    if (typeof options.onSaveRule === "function") {
      options.onSaveRule({
        name: trimmedName,
        triggerType,
        actionType,
        keywordFilter,
      });
    }
  }

  saveBtn.addEventListener("click", () => {
    handleSaveRule();
  });

  formEl.addEventListener("submit", (/** @type {any} */ event) => {
    if (event && typeof event.preventDefault === "function") {
      event.preventDefault();
    }
    handleSaveRule();
  });

  replaceElementChildren(formEl, [
    nameInput,
    triggerSelect,
    actionSelect,
    keywordsInput,
    saveBtn,
  ]);

  /**
   * Перерисовывает список карточек правил автомодерации.
   */
  function renderRulesList() {
    if (destroyed) return;

    /** @type {any[]} */
    const cards = [];

    for (const rule of rules) {
      const cardEl = doc.createElement("article");
      cardEl.setAttribute("data-automod-rule-id", rule.id);

      const nameEl = doc.createElement("h3");
      nameEl.setAttribute("data-automod-rule-name", "");
      nameEl.textContent = rule.name;
      cardEl.appendChild(nameEl);

      const triggerEl = doc.createElement("span");
      triggerEl.setAttribute("data-automod-rule-trigger", "");
      triggerEl.textContent = getTriggerLabelRu(rule.triggerType);
      cardEl.appendChild(triggerEl);

      const actionEl = doc.createElement("span");
      actionEl.setAttribute("data-automod-rule-action", "");
      actionEl.textContent = getActionLabelRu(rule.actionType);
      cardEl.appendChild(actionEl);

      const toggleBtn = doc.createElement("button");
      toggleBtn.setAttribute("type", "button");
      toggleBtn.setAttribute("data-automod-rule-toggle", rule.id);
      toggleBtn.setAttribute("role", "switch");
      toggleBtn.setAttribute("aria-checked", rule.enabled ? "true" : "false");
      toggleBtn.textContent = rule.enabled ? "Включено" : "Выключено";

      toggleBtn.addEventListener("click", () => {
        if (destroyed) return;
        const targetRule = rules.find((item) => item.id === rule.id);
        if (!targetRule) return;
        const nextEnabled = !targetRule.enabled;
        rules = rules.map((item) =>
          item.id === rule.id ? { ...item, enabled: nextEnabled } : item,
        );
        renderRulesList();
        if (typeof options.onToggleRule === "function") {
          options.onToggleRule(rule.id, nextEnabled);
        }
      });

      cardEl.appendChild(toggleBtn);
      cards.push(cardEl);
    }

    replaceElementChildren(rulesListEl, cards);
  }

  /**
   * Возвращает снимок текущего состояния контроллера автомодерации.
   *
   * @returns {AutoModEditorStateSnapshot}
   */
  function getState() {
    return {
      rules: rules.map(cloneAutoModRule),
    };
  }

  /**
   * Частично обновляет состояние контроллера и перерисовывает список правил.
   *
   * @param {AutoModEditorStatePatch} [patch]
   * @returns {AutoModEditorStateSnapshot}
   */
  function setState(patch) {
    if (patch && typeof patch === "object") {
      if (Array.isArray(patch.rules)) {
        rules = normalizeAutoModRules(patch.rules);
      }
    }
    renderRulesList();
    return getState();
  }

  /**
   * Размонтирует контроллер и очищает контейнер.
   */
  function destroy() {
    destroyed = true;
    if (root.parentNode === container && typeof container.removeChild === "function") {
      container.removeChild(root);
    } else if (typeof root.remove === "function" && root.parentNode === container) {
      root.remove();
    } else {
      replaceElementChildren(container, []);
    }
  }

  renderRulesList();
  replaceElementChildren(root, [rulesListEl, formEl]);
  replaceElementChildren(container, [root]);

  return {
    setState,
    getState,
    destroy,
  };
}
