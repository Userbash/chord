// @ts-check
/**
 * Контроллер управления ролями сервера и матрицы прав (task-248 / FE-P0-12).
 *
 * Отвечает за:
 * - каталог прав роли `ROLE_PERMISSIONS_CATALOG_RU` с русскими подписями и битовыми масками;
 * - сортировку ролей `sortRolesByPositionDesc(roles)` по убыванию `position` и возрастанию `String(id)` без мутации входа;
 * - рендер региона `[data-role-editor]` (`role="region"`, `aria-label="Редактор ролей"`),
 *   списка ролей в сайдбаре `[data-role-item-id="<id>"]`, полей редактирования выбранной роли
 *   (`[data-role-name-input]`, `[data-role-color-input]`, `[data-role-hoist-toggle]`),
 *   переключателей прав `[data-role-perm-toggle="<key>"]` (`role="switch"`, `aria-checked="true"|"false"`)
 *   и кнопки сохранения `[data-role-save-btn]` («Сохранить изменения»).
 *
 * @module ui/features/role-editor
 */

/**
 * @typedef {object} RolePermissionCatalogItem
 * @property {string} key
 * @property {number} bit
 * @property {string} label
 */

/**
 * @typedef {object} GuildRoleItem
 * @property {string} id
 * @property {string} name
 * @property {number} [position]
 * @property {string} [color]
 * @property {boolean} [hoist]
 * @property {number} [permissions]
 */

/**
 * @typedef {object} SavedRolePayload
 * @property {string} id
 * @property {string} name
 * @property {string} color
 * @property {boolean} hoist
 * @property {number} permissions
 */

/**
 * @typedef {object} RoleEditorStatePatch
 * @property {ReadonlyArray<GuildRoleItem>} [roles]
 * @property {string} [selectedRoleId]
 */

/**
 * @typedef {object} RoleEditorStateSnapshot
 * @property {GuildRoleItem[]} roles
 * @property {string | null} selectedRoleId
 * @property {SavedRolePayload | null} draft
 */

/**
 * @typedef {object} RoleEditorControllerOptions
 * @property {any} container
 * @property {ReadonlyArray<GuildRoleItem>} [roles]
 * @property {string} [selectedRoleId]
 * @property {(role: SavedRolePayload) => void} [onSaveRole]
 */

/** Цвет роли по умолчанию, если не задан. */
const DEFAULT_ROLE_COLOR_HEX = "#99aab5";

/**
 * Замороженный каталог прав ролей сервера с русскими подписями и битовыми масками.
 * @type {ReadonlyArray<Readonly<RolePermissionCatalogItem>>}
 */
export const ROLE_PERMISSIONS_CATALOG_RU = Object.freeze([
  Object.freeze({ key: "ADMINISTRATOR", bit: 8, label: "Администратор" }),
  Object.freeze({ key: "MANAGE_GUILD", bit: 32, label: "Управление сервером" }),
  Object.freeze({ key: "MANAGE_ROLES", bit: 268435456, label: "Управление ролями" }),
  Object.freeze({ key: "MANAGE_CHANNELS", bit: 16, label: "Управление каналами" }),
  Object.freeze({ key: "KICK_MEMBERS", bit: 2, label: "Выгонять участников" }),
  Object.freeze({ key: "BAN_MEMBERS", bit: 4, label: "Банить участников" }),
]);

/**
 * Сортирует массив ролей по убыванию `position`, а при равенстве — по `String(id)` по возрастанию.
 * Входной массив и его элементы не мутируются.
 *
 * @param {ReadonlyArray<GuildRoleItem> | null | undefined} roles
 * @returns {GuildRoleItem[]}
 */
export function sortRolesByPositionDesc(roles) {
  if (!Array.isArray(roles)) return [];
  /** @type {GuildRoleItem[]} */
  const cloned = [];
  for (const item of roles) {
    if (!item || typeof item !== "object") continue;
    cloned.push({ ...item });
  }
  cloned.sort((a, b) => {
    const posA = typeof a.position === "number" && Number.isFinite(a.position) ? a.position : 0;
    const posB = typeof b.position === "number" && Number.isFinite(b.position) ? b.position : 0;
    if (posA !== posB) return posB - posA;
    const idA = String(a.id ?? "");
    const idB = String(b.id ?? "");
    if (idA < idB) return -1;
    if (idA > idB) return 1;
    return 0;
  });
  return cloned;
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
 * Формирует черновик редактируемой роли из объекта роли.
 *
 * @param {GuildRoleItem | null | undefined} role
 * @returns {SavedRolePayload | null}
 */
function buildDraftFromRole(role) {
  if (!role || typeof role !== "object") return null;
  const id = String(role.id ?? "");
  const name = typeof role.name === "string" ? role.name : String(role.name ?? "");
  const color =
    typeof role.color === "string" && role.color.length > 0
      ? role.color
      : DEFAULT_ROLE_COLOR_HEX;
  const hoist = role.hoist === true;
  const permissions =
    typeof role.permissions === "number" && Number.isFinite(role.permissions)
      ? Math.trunc(role.permissions)
      : 0;
  return {
    id,
    name,
    color,
    hoist,
    permissions,
  };
}

/**
 * Создаёт контроллер редактора ролей сервера и матрицы прав внутри `options.container`.
 *
 * @param {RoleEditorControllerOptions} options
 */
export function createRoleEditorController(options) {
  if (!options || !options.container) {
    throw new TypeError("options.container is required");
  }

  const container = options.container;
  const doc = resolveDocument(container);

  /** @type {GuildRoleItem[]} */
  let roles = sortRolesByPositionDesc(options.roles);

  /** @type {string | null} */
  let selectedRoleId = null;

  /** @type {SavedRolePayload | null} */
  let draft = null;

  /** @type {any | null} */
  let activeNameInput = null;
  /** @type {any | null} */
  let activeColorInput = null;

  let destroyed = false;

  /**
   * Выбирает начальную или указанную роль и создаёт её черновик.
   *
   * @param {string | null | undefined} preferredRoleId
   */
  function syncSelectionAndDraft(preferredRoleId) {
    const sorted = sortRolesByPositionDesc(roles);
    roles = sorted;

    if (sorted.length === 0) {
      selectedRoleId = null;
      draft = null;
      return;
    }

    const candidateId =
      typeof preferredRoleId === "string" && preferredRoleId.length > 0
        ? preferredRoleId
        : selectedRoleId;

    const found =
      (candidateId !== null
        ? sorted.find((r) => String(r.id) === String(candidateId))
        : undefined) ?? sorted[0];

    selectedRoleId = found ? String(found.id) : null;
    draft = buildDraftFromRole(found);
  }

  syncSelectionAndDraft(options.selectedRoleId);

  const root = doc.createElement("section");
  root.setAttribute("data-role-editor", "");
  root.setAttribute("role", "region");
  root.setAttribute("aria-label", "Редактор ролей");

  /**
   * Синхронизирует значения текстовых полей черновика из живых DOM-инпутов,
   * если тест изменил `.value` напрямую без генерации события `input`.
   */
  function syncDraftFromInputs() {
    if (!draft) return;
    if (activeNameInput && typeof activeNameInput.value === "string") {
      draft.name = activeNameInput.value;
    }
    if (activeColorInput && typeof activeColorInput.value === "string") {
      draft.color = activeColorInput.value;
    }
  }

  /**
   * Перерисовывает DOM редактора ролей.
   */
  function renderDom() {
    if (destroyed) return;

    const sorted = sortRolesByPositionDesc(roles);
    roles = sorted;

    // 1. Боковой список ролей
    const sidebarEl = doc.createElement("aside");
    sidebarEl.setAttribute("data-role-sidebar", "");

    const roleListEl = doc.createElement("div");
    roleListEl.setAttribute("data-role-list", "");
    roleListEl.setAttribute("role", "listbox");
    roleListEl.setAttribute("aria-label", "Список ролей");

    for (const roleItem of sorted) {
      const roleId = String(roleItem.id);
      const itemBtn = doc.createElement("button");
      itemBtn.setAttribute("type", "button");
      itemBtn.setAttribute("role", "option");
      itemBtn.setAttribute("data-role-item-id", roleId);
      const isSelected = roleId === selectedRoleId;
      itemBtn.setAttribute("aria-selected", isSelected ? "true" : "false");
      itemBtn.textContent = String(roleItem.name ?? "");
      itemBtn.addEventListener("click", () => {
        selectRole(roleId);
      });
      roleListEl.appendChild(itemBtn);
    }

    sidebarEl.appendChild(roleListEl);

    // 2. Панель редактирования выбранной роли
    const formPaneEl = doc.createElement("div");
    formPaneEl.setAttribute("data-role-form", "");

    activeNameInput = null;
    activeColorInput = null;

    if (draft) {
      const nameInput = doc.createElement("input");
      nameInput.setAttribute("type", "text");
      nameInput.setAttribute("data-role-name-input", "");
      nameInput.setAttribute("aria-label", "Название роли");
      nameInput.value = draft.name;
      nameInput.addEventListener("input", (/** @type {any} */ event) => {
        if (!draft) return;
        draft.name = String(event?.target?.value ?? nameInput.value ?? "");
      });
      nameInput.addEventListener("change", (/** @type {any} */ event) => {
        if (!draft) return;
        draft.name = String(event?.target?.value ?? nameInput.value ?? "");
      });
      activeNameInput = nameInput;
      formPaneEl.appendChild(nameInput);

      const colorInput = doc.createElement("input");
      colorInput.setAttribute("type", "text");
      colorInput.setAttribute("data-role-color-input", "");
      colorInput.setAttribute("aria-label", "Цвет роли");
      colorInput.value = draft.color;
      colorInput.addEventListener("input", (/** @type {any} */ event) => {
        if (!draft) return;
        draft.color = String(event?.target?.value ?? colorInput.value ?? "");
      });
      colorInput.addEventListener("change", (/** @type {any} */ event) => {
        if (!draft) return;
        draft.color = String(event?.target?.value ?? colorInput.value ?? "");
      });
      activeColorInput = colorInput;
      formPaneEl.appendChild(colorInput);

      const hoistToggle = doc.createElement("button");
      hoistToggle.setAttribute("type", "button");
      hoistToggle.setAttribute("role", "switch");
      hoistToggle.setAttribute("data-role-hoist-toggle", "");
      hoistToggle.setAttribute("aria-label", "Отображать участников с ролью отдельно");
      hoistToggle.setAttribute("aria-checked", draft.hoist ? "true" : "false");
      hoistToggle.textContent = "Отображать участников с ролью отдельно";
      hoistToggle.addEventListener("click", () => {
        if (!draft) return;
        syncDraftFromInputs();
        draft.hoist = !draft.hoist;
        renderDom();
      });
      formPaneEl.appendChild(hoistToggle);

      const permsContainer = doc.createElement("div");
      permsContainer.setAttribute("data-role-permissions-matrix", "");

      for (const permEntry of ROLE_PERMISSIONS_CATALOG_RU) {
        const isChecked = (draft.permissions & permEntry.bit) !== 0;
        const permSwitch = doc.createElement("button");
        permSwitch.setAttribute("type", "button");
        permSwitch.setAttribute("role", "switch");
        permSwitch.setAttribute("data-role-perm-toggle", permEntry.key);
        permSwitch.setAttribute("aria-label", permEntry.label);
        permSwitch.setAttribute("aria-checked", isChecked ? "true" : "false");
        permSwitch.textContent = permEntry.label;
        permSwitch.addEventListener("click", () => {
          if (!draft) return;
          syncDraftFromInputs();
          const currentlySet = (draft.permissions & permEntry.bit) !== 0;
          draft.permissions = currentlySet
            ? draft.permissions & ~permEntry.bit
            : draft.permissions | permEntry.bit;
          renderDom();
        });
        permsContainer.appendChild(permSwitch);
      }

      formPaneEl.appendChild(permsContainer);

      const saveBtn = doc.createElement("button");
      saveBtn.setAttribute("type", "button");
      saveBtn.setAttribute("data-role-save-btn", "");
      saveBtn.textContent = "Сохранить изменения";
      saveBtn.addEventListener("click", () => {
        if (!draft || selectedRoleId === null) return;
        syncDraftFromInputs();
        const trimmedName = String(draft.name ?? "").trim();
        if (trimmedName.length < 1 || trimmedName.length > 100) {
          return;
        }
        const savedPayload = {
          id: selectedRoleId,
          name: trimmedName,
          color: String(draft.color ?? DEFAULT_ROLE_COLOR_HEX),
          hoist: Boolean(draft.hoist),
          permissions: draft.permissions,
        };
        draft = { ...savedPayload };
        roles = roles.map((r) =>
          String(r.id) === selectedRoleId
            ? {
                ...r,
                name: savedPayload.name,
                color: savedPayload.color,
                hoist: savedPayload.hoist,
                permissions: savedPayload.permissions,
              }
            : r,
        );
        renderDom();
        if (typeof options.onSaveRole === "function") {
          options.onSaveRole({ ...savedPayload });
        }
      });
      formPaneEl.appendChild(saveBtn);
    }

    replaceElementChildren(root, [sidebarEl, formPaneEl]);
  }

  /**
   * Выбирает роль по `roleId`, сбрасывает черновик к значениям выбранной роли и перерисовывает DOM.
   *
   * @param {string} roleId
   * @returns {boolean}
   */
  function selectRole(roleId) {
    const targetId = String(roleId ?? "");
    const found = roles.find((r) => String(r.id) === targetId);
    if (!found) return false;
    selectedRoleId = targetId;
    draft = buildDraftFromRole(found);
    renderDom();
    return true;
  }

  /**
   * Возвращает снимок текущего состояния редактора ролей.
   *
   * @returns {RoleEditorStateSnapshot}
   */
  function getState() {
    syncDraftFromInputs();
    return {
      roles: sortRolesByPositionDesc(roles),
      selectedRoleId,
      draft: draft ? { ...draft } : null,
    };
  }

  /**
   * Частично обновляет состояние редактора ролей и перерисовывает DOM.
   *
   * @param {RoleEditorStatePatch} [patch]
   * @returns {RoleEditorStateSnapshot}
   */
  function setState(patch) {
    if (patch && typeof patch === "object") {
      if (Array.isArray(patch.roles)) {
        roles = sortRolesByPositionDesc(patch.roles);
      }
      const nextSelected =
        typeof patch.selectedRoleId === "string" ? patch.selectedRoleId : selectedRoleId;
      syncSelectionAndDraft(nextSelected);
    }
    renderDom();
    return getState();
  }

  /**
   * Размонтирует контроллер и очищает контейнер.
   */
  function destroy() {
    destroyed = true;
    activeNameInput = null;
    activeColorInput = null;
    if (root.parentNode === container && typeof container.removeChild === "function") {
      container.removeChild(root);
    } else {
      replaceElementChildren(container, []);
    }
  }

  renderDom();
  replaceElementChildren(container, [root]);

  return {
    selectRole,
    setState,
    getState,
    destroy,
  };
}
