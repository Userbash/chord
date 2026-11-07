// @ts-check
/**
 * Контроллер панели модерации сервера, управления банами и журнала аудита (task-257 / FE-P0-16).
 *
 * RE-оракулы и контракты BFF:
 * - `/home/sanya/re-deep/rest-static.json:871-877` (`Endpoints.GUILD_BAN: "/guilds/{arg}/bans/{arg}"`);
 * - `/home/sanya/re-deep/rest-static.json:1240-1246` (`Endpoints.GUILD_AUDIT_LOG: "/guilds/{arg}/audit-logs"`);
 * - `rust/gui-spike/src/moderation.rs` (`MAX_AUDIT_REASON_CHARS = 512`, `MAX_BAN_DELETE_MESSAGE_SECONDS = 604_800`,
 *   положительный числовой Snowflake `1..20` цифр).
 *
 * Отвечает за:
 * - замороженный справочник периодов удаления сообщений при бане `BAN_DELETE_OPTIONS_RU`;
 * - чистую функцию фильтрации журнала аудита `filterAuditEntries(entries, options)` по `actionType` (> 0)
 *   и регистронезависимой подстроке `query` (`actorName`, `targetName`, `reason`) без мутации входа;
 * - рендер региона `[data-moderation-panel]` (`role="region"`, `aria-label="Модерация сервера"`),
 *   формы бана `[data-mod-ban-form]`, списка активных банов `[data-mod-ban-id="<userId>"]` с кнопками
 *   разбана `[data-mod-unban-btn="<userId>"]` и списка журнала аудита `[data-mod-audit-entry-id="<id>"]`,
 *   фильтруемого через поле поиска `[data-mod-audit-search]`.
 *
 * @module ui/features/moderation-panel
 */

/**
 * @typedef {object} BanDeleteOptionItem
 * @property {number} seconds
 * @property {string} label
 */

/**
 * @typedef {object} AuditLogEntryItem
 * @property {string} id
 * @property {number} actionType
 * @property {string} actorName
 * @property {string} [targetName]
 * @property {string} [reason]
 */

/**
 * @typedef {object} AuditFilterOptions
 * @property {number} [actionType]
 * @property {string} [query]
 */

/**
 * @typedef {object} GuildBanItem
 * @property {string} userId
 * @property {string} username
 * @property {string} [reason]
 */

/**
 * @typedef {object} BanMemberPayload
 * @property {string} targetUserId
 * @property {number} deleteMessageSeconds
 * @property {string} reason
 */

/**
 * @typedef {object} ModerationPanelStatePatch
 * @property {ReadonlyArray<GuildBanItem>} [bans]
 * @property {ReadonlyArray<AuditLogEntryItem>} [auditEntries]
 * @property {string} [auditQuery]
 * @property {number} [auditActionType]
 */

/**
 * @typedef {object} ModerationPanelStateSnapshot
 * @property {GuildBanItem[]} bans
 * @property {AuditLogEntryItem[]} auditEntries
 * @property {AuditLogEntryItem[]} filteredAuditEntries
 * @property {string} auditQuery
 * @property {number} auditActionType
 */

/**
 * @typedef {object} ModerationPanelControllerOptions
 * @property {any} container
 * @property {ReadonlyArray<AuditLogEntryItem>} [auditEntries]
 * @property {ReadonlyArray<GuildBanItem>} [bans]
 * @property {(payload: BanMemberPayload) => void} [onBanMember]
 * @property {(userId: string) => void} [onUnbanMember]
 */

/** Максимальная длина причины бана / записи аудита (`X-Audit-Log-Reason`). */
const MAX_BAN_REASON_LENGTH = 512;

/** Регулярное выражение для положительного числового Snowflake ID (`1..20` цифр, > 0). */
const POSITIVE_SNOWFLAKE_RE = /^[1-9]\d{0,19}$/;

/**
 * Замороженный список опций удаления сообщений при бане участника на русском языке.
 * @type {ReadonlyArray<Readonly<BanDeleteOptionItem>>}
 */
export const BAN_DELETE_OPTIONS_RU = Object.freeze([
  Object.freeze({ seconds: 0, label: "Не удалять сообщения" }),
  Object.freeze({ seconds: 3600, label: "За последний час" }),
  Object.freeze({ seconds: 86400, label: "За последние 24 часа" }),
  Object.freeze({ seconds: 604800, label: "За последние 7 дней" }),
]);

/**
 * Фильтрует массив записей журнала аудита по `options.actionType` (если `> 0`)
 * и `options.query` (регистронезависимый поиск подстроки в `actorName`, `targetName` или `reason`).
 * Входной массив и его элементы никогда не мутируются.
 *
 * @param {ReadonlyArray<AuditLogEntryItem> | null | undefined} entries
 * @param {AuditFilterOptions | null | undefined} [options]
 * @returns {AuditLogEntryItem[]}
 */
export function filterAuditEntries(entries, options) {
  if (!Array.isArray(entries)) {
    return [];
  }

  const opts = options && typeof options === "object" ? options : {};
  const filterActionType =
    typeof opts.actionType === "number" &&
    Number.isFinite(opts.actionType) &&
    opts.actionType > 0
      ? Math.trunc(opts.actionType)
      : 0;
  const query = typeof opts.query === "string" ? opts.query.trim().toLowerCase() : "";

  /** @type {AuditLogEntryItem[]} */
  const result = [];

  for (const item of entries) {
    if (!item || typeof item !== "object") continue;

    if (filterActionType > 0 && item.actionType !== filterActionType) {
      continue;
    }

    if (query.length > 0) {
      const actor = String(item.actorName ?? "").toLowerCase();
      const target = String(item.targetName ?? "").toLowerCase();
      const reason = String(item.reason ?? "").toLowerCase();
      if (!actor.includes(query) && !target.includes(query) && !reason.includes(query)) {
        continue;
      }
    }

    result.push({ ...item });
  }

  return result;
}

/**
 * Клонирует массив записей банов без мутации исходных объектов.
 *
 * @param {ReadonlyArray<GuildBanItem> | null | undefined} bans
 * @returns {GuildBanItem[]}
 */
function cloneBans(bans) {
  if (!Array.isArray(bans)) return [];
  /** @type {GuildBanItem[]} */
  const out = [];
  for (const item of bans) {
    if (!item || typeof item !== "object") continue;
    out.push({ ...item });
  }
  return out;
}

/**
 * Клонирует массив записей журнала аудита без мутации исходных объектов.
 *
 * @param {ReadonlyArray<AuditLogEntryItem> | null | undefined} entries
 * @returns {AuditLogEntryItem[]}
 */
function cloneAuditEntries(entries) {
  if (!Array.isArray(entries)) return [];
  /** @type {AuditLogEntryItem[]} */
  const out = [];
  for (const item of entries) {
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
 * Создаёт контроллер панели модерации сервера (баны + журнал аудита) внутри `options.container`.
 *
 * @param {ModerationPanelControllerOptions} options
 */
export function createModerationPanelController(options) {
  if (!options || !options.container) {
    throw new TypeError("options.container is required");
  }

  const container = options.container;
  const doc = resolveDocument(container);

  /** @type {GuildBanItem[]} */
  let bans = cloneBans(options.bans);
  /** @type {AuditLogEntryItem[]} */
  let auditEntries = cloneAuditEntries(options.auditEntries);
  /** @type {string} */
  let auditQuery = "";
  /** @type {number} */
  let auditActionType = 0;
  let destroyed = false;

  const root = doc.createElement("section");
  root.setAttribute("data-moderation-panel", "");
  root.setAttribute("role", "region");
  root.setAttribute("aria-label", "Модерация сервера");

  // 1. Форма бана участника
  const banFormEl = doc.createElement("form");
  banFormEl.setAttribute("data-mod-ban-form", "");

  const targetInput = doc.createElement("input");
  targetInput.setAttribute("type", "text");
  targetInput.setAttribute("data-mod-target-input", "");
  targetInput.setAttribute("placeholder", "ID пользователя (Snowflake)");
  targetInput.value = "";
  banFormEl.appendChild(targetInput);

  const reasonInput = doc.createElement("input");
  reasonInput.setAttribute("type", "text");
  reasonInput.setAttribute("data-mod-reason-input", "");
  reasonInput.setAttribute("placeholder", "Причина бана");
  reasonInput.value = "";
  banFormEl.appendChild(reasonInput);

  const deleteSelect = doc.createElement("select");
  deleteSelect.setAttribute("data-mod-delete-select", "");
  for (const optItem of BAN_DELETE_OPTIONS_RU) {
    const optEl = doc.createElement("option");
    const valStr = String(optItem.seconds);
    optEl.setAttribute("value", valStr);
    optEl.value = valStr;
    optEl.textContent = optItem.label;
    deleteSelect.appendChild(optEl);
  }
  deleteSelect.value = "0";
  banFormEl.appendChild(deleteSelect);

  const banSubmitBtn = doc.createElement("button");
  banSubmitBtn.setAttribute("type", "button");
  banSubmitBtn.setAttribute("data-mod-ban-submit", "");
  banSubmitBtn.textContent = "Забанить участника";
  banFormEl.appendChild(banSubmitBtn);

  const handleBanSubmit = (/** @type {any} */ event) => {
    if (event && typeof event.preventDefault === "function") {
      event.preventDefault();
    }
    if (destroyed) return;

    const targetUserId = String(targetInput.value ?? "").trim();
    if (!POSITIVE_SNOWFLAKE_RE.test(targetUserId)) {
      return;
    }

    const reason = String(reasonInput.value ?? "").trim();
    if (reason.length > MAX_BAN_REASON_LENGTH) {
      return;
    }

    const parsedSeconds = Number(deleteSelect.value);
    const deleteMessageSeconds =
      Number.isFinite(parsedSeconds) && parsedSeconds >= 0 ? Math.trunc(parsedSeconds) : 0;

    if (typeof options.onBanMember === "function") {
      options.onBanMember({
        targetUserId,
        deleteMessageSeconds,
        reason,
      });
    }
  };

  banSubmitBtn.addEventListener("click", handleBanSubmit);
  banFormEl.addEventListener("submit", handleBanSubmit);

  // 2. Список активных банов
  const bansListEl = doc.createElement("div");
  bansListEl.setAttribute("data-mod-bans-list", "");

  // 3. Поиск и список записей журнала аудита
  const auditSectionEl = doc.createElement("div");
  auditSectionEl.setAttribute("data-mod-audit-section", "");

  const auditSearchInput = doc.createElement("input");
  auditSearchInput.setAttribute("type", "search");
  auditSearchInput.setAttribute("data-mod-audit-search", "");
  auditSearchInput.setAttribute("placeholder", "Поиск по журналу аудита");
  auditSearchInput.value = "";
  auditSearchInput.addEventListener("input", () => {
    if (destroyed) return;
    auditQuery = String(auditSearchInput.value ?? "");
    renderAuditList();
  });
  auditSectionEl.appendChild(auditSearchInput);

  const auditListEl = doc.createElement("div");
  auditListEl.setAttribute("data-mod-audit-list", "");
  auditSectionEl.appendChild(auditListEl);

  /**
   * Перерисовывает список активных банов.
   */
  function renderBansList() {
    if (destroyed) return;
    /** @type {any[]} */
    const nodes = [];

    if (bans.length === 0) {
      const emptyEl = doc.createElement("p");
      emptyEl.setAttribute("data-mod-bans-empty", "");
      emptyEl.textContent = "Нет активных банов";
      nodes.push(emptyEl);
    } else {
      for (const ban of bans) {
        const userId = String(ban.userId ?? "");
        const itemEl = doc.createElement("div");
        itemEl.setAttribute("data-mod-ban-id", userId);

        const nameEl = doc.createElement("span");
        nameEl.setAttribute("data-mod-ban-username", "");
        nameEl.textContent = String(ban.username ?? userId);
        itemEl.appendChild(nameEl);

        if (typeof ban.reason === "string" && ban.reason.trim().length > 0) {
          const reasonEl = doc.createElement("span");
          reasonEl.setAttribute("data-mod-ban-reason", "");
          reasonEl.textContent = ban.reason;
          itemEl.appendChild(reasonEl);
        }

        const unbanBtn = doc.createElement("button");
        unbanBtn.setAttribute("type", "button");
        unbanBtn.setAttribute("data-mod-unban-btn", userId);
        unbanBtn.textContent = "Разбанить";
        unbanBtn.addEventListener("click", () => {
          if (destroyed) return;
          if (typeof options.onUnbanMember === "function") {
            options.onUnbanMember(userId);
          }
        });
        itemEl.appendChild(unbanBtn);

        nodes.push(itemEl);
      }
    }

    replaceElementChildren(bansListEl, nodes);
  }

  /**
   * Перерисовывает список записей журнала аудита с учётом текущего фильтра.
   */
  function renderAuditList() {
    if (destroyed) return;
    const filtered = filterAuditEntries(auditEntries, {
      actionType: auditActionType,
      query: auditQuery,
    });

    /** @type {any[]} */
    const nodes = [];

    if (filtered.length === 0) {
      const emptyEl = doc.createElement("p");
      emptyEl.setAttribute("data-mod-audit-empty", "");
      emptyEl.textContent = "Записи журнала аудита не найдены";
      nodes.push(emptyEl);
    } else {
      for (const entry of filtered) {
        const entryId = String(entry.id ?? "");
        const rowEl = doc.createElement("div");
        rowEl.setAttribute("data-mod-audit-entry-id", entryId);
        rowEl.setAttribute("data-mod-audit-action-type", String(entry.actionType ?? 0));

        const actorEl = doc.createElement("span");
        actorEl.setAttribute("data-mod-audit-actor", "");
        actorEl.textContent = String(entry.actorName ?? "");
        rowEl.appendChild(actorEl);

        if (typeof entry.targetName === "string" && entry.targetName.length > 0) {
          const targetEl = doc.createElement("span");
          targetEl.setAttribute("data-mod-audit-target", "");
          targetEl.textContent = entry.targetName;
          rowEl.appendChild(targetEl);
        }

        if (typeof entry.reason === "string" && entry.reason.length > 0) {
          const reasonEl = doc.createElement("span");
          reasonEl.setAttribute("data-mod-audit-reason", "");
          reasonEl.textContent = entry.reason;
          rowEl.appendChild(reasonEl);
        }

        nodes.push(rowEl);
      }
    }

    replaceElementChildren(auditListEl, nodes);
  }

  /**
   * Возвращает снимок текущего состояния контроллера.
   *
   * @returns {ModerationPanelStateSnapshot}
   */
  function getState() {
    return {
      bans: cloneBans(bans),
      auditEntries: cloneAuditEntries(auditEntries),
      filteredAuditEntries: filterAuditEntries(auditEntries, {
        actionType: auditActionType,
        query: auditQuery,
      }),
      auditQuery,
      auditActionType,
    };
  }

  /**
   * Частично обновляет состояние панели модерации и перерисовывает списки.
   *
   * @param {ModerationPanelStatePatch} [patch]
   * @returns {ModerationPanelStateSnapshot}
   */
  function setState(patch) {
    if (patch && typeof patch === "object") {
      if (Array.isArray(patch.bans)) {
        bans = cloneBans(patch.bans);
      }
      if (Array.isArray(patch.auditEntries)) {
        auditEntries = cloneAuditEntries(patch.auditEntries);
      }
      if (typeof patch.auditQuery === "string") {
        auditQuery = patch.auditQuery;
        auditSearchInput.value = auditQuery;
      }
      if (typeof patch.auditActionType === "number" && Number.isFinite(patch.auditActionType)) {
        auditActionType = patch.auditActionType > 0 ? Math.trunc(patch.auditActionType) : 0;
      }
    }
    renderBansList();
    renderAuditList();
    return getState();
  }

  /**
   * Размонтирует панель модерации и очищает контейнер.
   */
  function destroy() {
    destroyed = true;
    if (root.parentNode === container && typeof container.removeChild === "function") {
      container.removeChild(root);
    } else {
      replaceElementChildren(container, []);
    }
  }

  renderBansList();
  renderAuditList();
  replaceElementChildren(root, [banFormEl, bansListEl, auditSectionEl]);
  replaceElementChildren(container, [root]);

  return {
    setState,
    getState,
    destroy,
  };
}
