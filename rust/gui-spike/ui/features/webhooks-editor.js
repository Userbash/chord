// @ts-check
/**
 * Контроллер настроек вебхуков и подписок на каналы (task-252 / FE-P0-14).
 *
 * Отвечает за:
 * - русские метки типов вебхуков (`WEBHOOK_TYPE_LABELS_RU`);
 * - валидацию имени вебхука (`isValidWebhookName`: длина 1..=80 символов после trim,
 *   запрет имени `"clyde"` и подстроки `"discord"` без учёта регистра);
 * - маскирование секретного токена в URL вебхука (`maskWebhookTokenUrl`);
 * - рендер и управление регионом `[data-webhooks-editor]` (`role="region"`,
 *   `aria-label="Вебхуки"`), списком вебхуков и формой создания.
 *
 * Оракулы: `/home/sanya/re-deep/rest-static.md` (`CHANNEL_WEBHOOKS`, `GUILD_WEBHOOKS`,
 * `WEBHOOK`, `WEBHOOK_INTEGRATION`), `rust/gui-spike/src/webhooks.rs` (типы `1..=3`,
 * границы `1..=80` символов).
 *
 * @module ui/features/webhooks-editor
 */

/**
 * Русские метки типов вебхуков Discord (`WebhookTypes`).
 * @type {Readonly<Record<number, string>>}
 */
export const WEBHOOK_TYPE_LABELS_RU = Object.freeze({
  1: "Входящий вебхук",
  2: "Подписка на канал",
  3: "Вебхук приложения",
});

/** Минимальная длина имени вебхука после `trim()`. */
const MIN_WEBHOOK_NAME_CHARS = 1;

/** Максимальная длина имени вебхука после `trim()`. */
const MAX_WEBHOOK_NAME_CHARS = 80;

/** Сообщение об ошибке валидации имени вебхука. */
const INVALID_WEBHOOK_NAME_ERROR_RU =
  "Недопустимое имя вебхука: укажите от 1 до 80 символов без «clyde» и «discord».";

/**
 * @typedef {object} WebhookItemInput
 * @property {string} id
 * @property {string} name
 * @property {number} type
 * @property {string} channelId
 * @property {string} [url]
 */

/**
 * @typedef {object} WebhookItemNormalized
 * @property {string} id
 * @property {string} name
 * @property {number} type
 * @property {string} channelId
 * @property {string} url
 */

/**
 * @typedef {object} WebhooksEditorStatePatch
 * @property {ReadonlyArray<WebhookItemInput>} [webhooks]
 * @property {string} [error]
 */

/**
 * @typedef {object} WebhooksEditorStateSnapshot
 * @property {WebhookItemNormalized[]} webhooks
 * @property {string} error
 */

/**
 * @typedef {object} WebhooksEditorControllerOptions
 * @property {any} container
 * @property {ReadonlyArray<WebhookItemInput>} [webhooks]
 * @property {(name: string) => void} [onCreateWebhook]
 * @property {(webhookId: string) => void} [onDeleteWebhook]
 * @property {(webhookId: string, url: string) => void} [onCopyWebhookUrl]
 */

/**
 * Проверяет допустимость имени вебхука Discord:
 * - непустая строка длиной `1..=80` символов после `trim()`;
 * - в нижнем регистре не равна `"clyde"` и не содержит подстроку `"discord"`.
 *
 * @param {unknown} name
 * @returns {boolean}
 */
export function isValidWebhookName(name) {
  if (typeof name !== "string") return false;
  const trimmed = name.trim();
  const charCount = [...trimmed].length;
  if (charCount < MIN_WEBHOOK_NAME_CHARS || charCount > MAX_WEBHOOK_NAME_CHARS) {
    return false;
  }
  const lower = trimmed.toLowerCase();
  if (lower === "clyde" || lower.includes("discord")) {
    return false;
  }
  return true;
}

/**
 * Маскирует секретный токен в URL вебхука Discord:
 * для ссылок, содержащих `"/api/webhooks/<id>/<token>"` или `"/api/v9/webhooks/<id>/<token>"`,
 * заменяет `"<token>"` на `"***"`, сохраняя префикс вплоть до `"<id>/"`.
 * Возвращает `""` для нестроковых и пустых значений.
 *
 * @param {unknown} url
 * @returns {string}
 */
export function maskWebhookTokenUrl(url) {
  if (typeof url !== "string") return "";
  const trimmed = url.trim();
  if (trimmed.length === 0) return "";

  const match = trimmed.match(/^(.*\/api\/(?:v\d+\/)?webhooks\/[^/?#\s]+\/)([^/?#\s]+)(.*)$/);
  if (match && match[1]) {
    return `${match[1]}***`;
  }
  return trimmed;
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
 * Нормализует входной список вебхуков без мутации исходного массива.
 *
 * @param {ReadonlyArray<WebhookItemInput> | null | undefined} rawWebhooks
 * @returns {WebhookItemNormalized[]}
 */
function normalizeWebhooks(rawWebhooks) {
  if (!Array.isArray(rawWebhooks)) return [];
  /** @type {WebhookItemNormalized[]} */
  const out = [];
  for (const item of rawWebhooks) {
    if (!item || typeof item !== "object") continue;
    const id = item.id !== undefined && item.id !== null ? String(item.id) : "";
    if (id.length === 0) continue;
    const name = typeof item.name === "string" ? item.name : "";
    const rawType = Number(item.type);
    const type = Number.isInteger(rawType) && rawType > 0 ? rawType : 1;
    const channelId =
      item.channelId !== undefined && item.channelId !== null ? String(item.channelId) : "";
    const url = typeof item.url === "string" ? item.url : "";
    out.push({ id, name, type, channelId, url });
  }
  return out;
}

/**
 * Возвращает русскую метку типа вебхука по числовому коду.
 *
 * @param {number} type
 * @returns {string}
 */
function getWebhookTypeLabelRu(type) {
  return WEBHOOK_TYPE_LABELS_RU[type] ?? WEBHOOK_TYPE_LABELS_RU[1] ?? "Входящий вебхук";
}

/**
 * Создаёт контроллер редактора вебхуков и подписок на каналы внутри `options.container`.
 *
 * @param {WebhooksEditorControllerOptions} options
 */
export function createWebhooksEditorController(options) {
  if (!options || !options.container) {
    throw new TypeError("options.container is required");
  }

  const container = options.container;
  const doc = resolveDocument(container);

  /** @type {WebhookItemNormalized[]} */
  let webhooks = normalizeWebhooks(options.webhooks);
  /** @type {string} */
  let errorText = "";
  let destroyed = false;

  const root = doc.createElement("section");
  root.setAttribute("data-webhooks-editor", "");
  root.setAttribute("role", "region");
  root.setAttribute("aria-label", "Вебхуки");

  const listEl = doc.createElement("div");
  listEl.setAttribute("data-webhooks-list", "");

  const createFormEl = doc.createElement("div");
  createFormEl.setAttribute("data-webhook-create-form", "");

  const nameInputEl = doc.createElement("input");
  nameInputEl.setAttribute("type", "text");
  nameInputEl.setAttribute("data-webhook-new-name-input", "");
  nameInputEl.value = "";

  const createBtnEl = doc.createElement("button");
  createBtnEl.setAttribute("type", "button");
  createBtnEl.setAttribute("data-webhook-create-btn", "");
  createBtnEl.textContent = "Создать вебхук";

  const errorEl = doc.createElement("div");
  errorEl.setAttribute("data-webhook-error", "");
  errorEl.setAttribute("role", "alert");
  errorEl.textContent = "";

  createBtnEl.addEventListener("click", () => {
    if (destroyed) return;
    const rawName = typeof nameInputEl.value === "string" ? nameInputEl.value : "";
    if (!isValidWebhookName(rawName)) {
      errorText = INVALID_WEBHOOK_NAME_ERROR_RU;
      errorEl.textContent = errorText;
      return;
    }
    errorText = "";
    errorEl.textContent = "";
    const trimmedName = rawName.trim();
    if (typeof options.onCreateWebhook === "function") {
      options.onCreateWebhook(trimmedName);
    }
  });

  replaceElementChildren(createFormEl, [nameInputEl, createBtnEl, errorEl]);

  /**
   * Перерисовывает список элементов вебхуков внутри `listEl`.
   */
  function renderItems() {
    if (destroyed) return;

    /** @type {any[]} */
    const itemNodes = [];

    for (const item of webhooks) {
      const itemEl = doc.createElement("div");
      itemEl.setAttribute("data-webhook-item-id", item.id);

      const nameEl = doc.createElement("span");
      nameEl.setAttribute("data-webhook-name", item.id);
      nameEl.textContent = item.name;
      itemEl.appendChild(nameEl);

      const typeEl = doc.createElement("span");
      typeEl.setAttribute("data-webhook-type-label", item.id);
      typeEl.textContent = ` (${getWebhookTypeLabelRu(item.type)})`;
      itemEl.appendChild(typeEl);

      const maskedUrlEl = doc.createElement("code");
      maskedUrlEl.setAttribute("data-webhook-masked-url", item.id);
      maskedUrlEl.textContent = maskWebhookTokenUrl(item.url);
      itemEl.appendChild(maskedUrlEl);

      const copyBtn = doc.createElement("button");
      copyBtn.setAttribute("type", "button");
      copyBtn.setAttribute("data-webhook-copy-url", item.id);
      copyBtn.textContent = "Копировать URL";
      copyBtn.addEventListener("click", () => {
        if (destroyed) return;
        if (typeof options.onCopyWebhookUrl === "function") {
          options.onCopyWebhookUrl(item.id, item.url);
        }
      });
      itemEl.appendChild(copyBtn);

      const deleteBtn = doc.createElement("button");
      deleteBtn.setAttribute("type", "button");
      deleteBtn.setAttribute("data-webhook-delete-btn", item.id);
      deleteBtn.textContent = "Удалить";
      deleteBtn.addEventListener("click", () => {
        if (destroyed) return;
        if (typeof options.onDeleteWebhook === "function") {
          options.onDeleteWebhook(item.id);
        }
      });
      itemEl.appendChild(deleteBtn);

      itemNodes.push(itemEl);
    }

    replaceElementChildren(listEl, itemNodes);
  }

  /**
   * Возвращает неизменяемый снимок текущего состояния контроллера.
   *
   * @returns {WebhooksEditorStateSnapshot}
   */
  function getState() {
    return {
      webhooks: webhooks.map((item) => ({ ...item })),
      error: errorText,
    };
  }

  /**
   * Частично обновляет состояние редактора вебхуков и перерисовывает список.
   *
   * @param {WebhooksEditorStatePatch} [patch]
   * @returns {WebhooksEditorStateSnapshot}
   */
  function setState(patch) {
    if (patch && typeof patch === "object") {
      if (Array.isArray(patch.webhooks)) {
        webhooks = normalizeWebhooks(patch.webhooks);
      }
      if (typeof patch.error === "string") {
        errorText = patch.error;
        errorEl.textContent = errorText;
      }
    }
    renderItems();
    return getState();
  }

  /**
   * Размонтирует редактор вебхуков и очищает контейнер.
   */
  function destroy() {
    destroyed = true;
    if (root.parentNode === container && typeof container.removeChild === "function") {
      container.removeChild(root);
    } else {
      replaceElementChildren(container, []);
    }
  }

  renderItems();
  replaceElementChildren(root, [listEl, createFormEl]);
  replaceElementChildren(container, [root]);

  return {
    setState,
    getState,
    destroy,
  };
}
