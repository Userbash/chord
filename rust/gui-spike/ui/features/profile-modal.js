// @ts-check
/**
 * T-203 (RW-40 / G2): контроллер компактного поповера и полного модального окна
 * профиля пользователя (`profile-modal.js`).
 *
 * Оракулы контрактов:
 * - RE-дамп `/home/sanya/re-deep/ui/06-profile-popout.html`,
 *   `/home/sanya/re-deep/voice/ui7/15-popout.json`,
 *   `/home/sanya/re-deep/frontend-d03.md` (§5 Modal / Popover).
 *
 * Отвечает за:
 * - `STATUS_LABELS_RU`: замороженный словарь русских подписей статусов присутствия;
 * - `openPopout(profileData)`: рендер компактной карточки (`data-profile-mode="popout"`)
 *   с `displayName`, `username`, бейджем местоимений (`pronouns`), русским статусом
 *   из `STATUS_LABELS_RU`, био (`bio`/`about`), пилюлями ролей (`data-role-id` и цвет),
 *   полем личной заметки (`onSaveNote(userId, noteText)` на `change`) и полем быстрого
 *   сообщения (`placeholder="Написать @" + username`, `onSendQuickMessage(userId, text)`
 *   при непустом тексте);
 * - `openFullModal(profileData)`: рендер полного диалога профиля (`role="dialog"`,
 *   `aria-modal="true"`, `data-profile-mode="modal"`) с 3 вкладками:
 *   «Обо мне» (`about`), «Общие серверы» (`mutual_guilds` со счётчиком) и
 *   «Общие друзья» (`mutual_friends` со счётчиком);
 * - `setTab(tabId)`: переключение активной вкладки и рендер соответствующего списка
 *   или русского пустого состояния («Нет общих серверов» / «Нет общих друзей»);
 * - `close()`: удаление открытого поповера или модалки из контейнера.
 *
 * @module ui/features/profile-modal
 */

/**
 * Русские подписи статусов присутствия пользователя.
 * @type {Readonly<{ online: string, idle: string, dnd: string, offline: string }>}
 */
export const STATUS_LABELS_RU = Object.freeze({
  online: "В сети",
  idle: "Не активен",
  dnd: "Не беспокоить",
  offline: "Не в сети",
});

/**
 * Идентификаторы вкладок полного модального окна профиля.
 * @type {ReadonlyArray<"about" | "mutual_guilds" | "mutual_friends">}
 */
export const PROFILE_MODAL_TABS = Object.freeze([
  "about",
  "mutual_guilds",
  "mutual_friends",
]);

/**
 * Русские заголовки вкладок полного модального окна профиля.
 * @type {Readonly<{ about: string, mutual_guilds: string, mutual_friends: string }>}
 */
export const PROFILE_TAB_LABELS_RU = Object.freeze({
  about: "Обо мне",
  mutual_guilds: "Общие серверы",
  mutual_friends: "Общие друзья",
});

/**
 * Русские тексты пустых состояний списков общих серверов и общих друзей.
 * @type {Readonly<{ mutual_guilds: string, mutual_friends: string }>}
 */
export const PROFILE_EMPTY_STATES_RU = Object.freeze({
  mutual_guilds: "Нет общих серверов",
  mutual_friends: "Нет общих друзей",
});

/**
 * @typedef {object} ProfileRole
 * @property {string} id
 * @property {string} name
 * @property {string} [color]
 */

/**
 * @typedef {object} MutualGuildItem
 * @property {string} id
 * @property {string} name
 */

/**
 * @typedef {object} MutualFriendItem
 * @property {string} id
 * @property {string} username
 * @property {string} [displayName]
 */

/**
 * @typedef {object} ProfileData
 * @property {string} [userId]
 * @property {string} [id]
 * @property {string} [username]
 * @property {string} [displayName]
 * @property {string} [globalName]
 * @property {string} [pronouns]
 * @property {string} [pronoun]
 * @property {string} [status]
 * @property {string} [bio]
 * @property {string} [about]
 * @property {string} [note]
 * @property {Array<{ id?: string, roleId?: string, name?: string, label?: string, color?: string }>} [roles]
 * @property {Array<{ id?: string, name?: string } | string>} [mutualGuilds]
 * @property {Array<{ id?: string, username?: string, displayName?: string, name?: string } | string>} [mutualFriends]
 */

/**
 * @typedef {object} NormalizedProfileData
 * @property {string} userId
 * @property {string} username
 * @property {string} displayName
 * @property {string} pronouns
 * @property {"online" | "idle" | "dnd" | "offline"} status
 * @property {string} statusLabelRu
 * @property {string} bio
 * @property {string} note
 * @property {Array<{ id: string, name: string, color: string }>} roles
 * @property {MutualGuildItem[]} mutualGuilds
 * @property {MutualFriendItem[]} mutualFriends
 */

/**
 * @typedef {object} ProfileModalOptions
 * @property {(userId: string, text: string) => void} [onSendQuickMessage]
 * @property {(userId: string, noteText: string) => void} [onSaveNote]
 */

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
  throw new TypeError("containerEl must provide ownerDocument.createElement");
}

/**
 * Нормализует ключ статуса присутствия к одному из ключей `STATUS_LABELS_RU`.
 *
 * @param {unknown} rawStatus
 * @returns {"online" | "idle" | "dnd" | "offline"}
 */
function normalizeStatus(rawStatus) {
  if (
    rawStatus === "online" ||
    rawStatus === "idle" ||
    rawStatus === "dnd" ||
    rawStatus === "offline"
  ) {
    return rawStatus;
  }
  return "offline";
}

/**
 * Нормализует входные данные профиля пользователя.
 *
 * @param {ProfileData | null | undefined} raw
 * @returns {NormalizedProfileData}
 */
function normalizeProfileData(raw) {
  const source = raw && typeof raw === "object" ? raw : {};
  const userId = String(source.userId ?? source.id ?? "");
  const username = String(source.username ?? "");
  const displayName = String(
    source.displayName ?? source.globalName ?? (username || "Пользователь"),
  );
  const pronouns = String(source.pronouns ?? source.pronoun ?? "");
  const status = normalizeStatus(source.status);
  const statusLabelRu = STATUS_LABELS_RU[status];
  const bio = String(source.bio ?? source.about ?? "");
  const note = String(source.note ?? "");

  /** @type {Array<{ id: string, name: string, color: string }>} */
  const roles = [];
  if (Array.isArray(source.roles)) {
    for (let i = 0; i < source.roles.length; i += 1) {
      const role = source.roles[i];
      if (!role || typeof role !== "object") continue;
      const id = String(role.id ?? role.roleId ?? `role-${i}`);
      const name = String(role.name ?? role.label ?? id);
      const color = String(role.color ?? "");
      roles.push({ id, name, color });
    }
  }

  /** @type {MutualGuildItem[]} */
  const mutualGuilds = [];
  if (Array.isArray(source.mutualGuilds)) {
    for (let i = 0; i < source.mutualGuilds.length; i += 1) {
      const entry = source.mutualGuilds[i];
      if (entry === null || entry === undefined) continue;
      if (typeof entry === "string") {
        mutualGuilds.push({ id: entry, name: entry });
      } else if (typeof entry === "object") {
        const id = String(entry.id ?? `guild-${i}`);
        const name = String(entry.name ?? id);
        mutualGuilds.push({ id, name });
      }
    }
  }

  /** @type {MutualFriendItem[]} */
  const mutualFriends = [];
  if (Array.isArray(source.mutualFriends)) {
    for (let i = 0; i < source.mutualFriends.length; i += 1) {
      const entry = source.mutualFriends[i];
      if (entry === null || entry === undefined) continue;
      if (typeof entry === "string") {
        mutualFriends.push({ id: entry, username: entry, displayName: entry });
      } else if (typeof entry === "object") {
        const id = String(entry.id ?? `friend-${i}`);
        const friendUsername = String(entry.username ?? entry.name ?? id);
        const friendDisplayName = String(
          entry.displayName ?? entry.name ?? friendUsername,
        );
        mutualFriends.push({
          id,
          username: friendUsername,
          displayName: friendDisplayName,
        });
      }
    }
  }

  return {
    userId,
    username,
    displayName,
    pronouns,
    status,
    statusLabelRu,
    bio,
    note,
    roles,
    mutualGuilds,
    mutualFriends,
  };
}

/**
 * Создаёт DOM-блок пилюль ролей пользователя.
 *
 * @param {{ createElement: (tag: string) => any }} doc
 * @param {Array<{ id: string, name: string, color: string }>} roles
 * @returns {any}
 */
function buildRolesList(doc, roles) {
  const rolesWrap = doc.createElement("div");
  rolesWrap.setAttribute("data-profile-roles", "");
  rolesWrap.setAttribute("role", "list");

  for (const role of roles) {
    const pill = doc.createElement("span");
    pill.setAttribute("role", "listitem");
    pill.setAttribute("data-role-id", role.id);
    if (role.color) {
      pill.setAttribute("data-role-color", role.color);
      if (pill.style) {
        if (typeof pill.style.setProperty === "function") {
          pill.style.setProperty("color", role.color);
        }
        pill.style.color = role.color;
      }
    }
    pill.textContent = role.name;
    rolesWrap.appendChild(pill);
  }

  return rolesWrap;
}

/**
 * Создаёт поле личной заметки с вызовом `onSaveNote(userId, noteText)` на `change`.
 *
 * @param {{ createElement: (tag: string) => any }} doc
 * @param {NormalizedProfileData} profile
 * @param {((userId: string, noteText: string) => void) | undefined} onSaveNote
 * @returns {any}
 */
function buildNoteInput(doc, profile, onSaveNote) {
  const noteInput = doc.createElement("textarea");
  noteInput.setAttribute("data-profile-note-input", "");
  noteInput.setAttribute("aria-label", "Заметка");
  noteInput.setAttribute("placeholder", "Нажмите, чтобы добавить заметку");
  noteInput.placeholder = "Нажмите, чтобы добавить заметку";
  noteInput.value = profile.note;

  noteInput.addEventListener("change", (/** @type {any} */ event) => {
    const nextNote = String(event?.target?.value ?? noteInput.value ?? "");
    profile.note = nextNote;
    if (typeof onSaveNote === "function") {
      onSaveNote(profile.userId, nextNote);
    }
  });

  return noteInput;
}

/**
 * Создаёт шапку профиля с отображаемым именем, юзернеймом, местоимениями и статусом.
 *
 * @param {{ createElement: (tag: string) => any }} doc
 * @param {NormalizedProfileData} profile
 * @returns {any}
 */
function buildProfileHeader(doc, profile) {
  const header = doc.createElement("header");
  header.setAttribute("data-profile-header", "");

  const displayNameEl = doc.createElement("h3");
  displayNameEl.setAttribute("data-profile-display-name", "");
  displayNameEl.textContent = profile.displayName;
  header.appendChild(displayNameEl);

  const usernameEl = doc.createElement("span");
  usernameEl.setAttribute("data-profile-username", "");
  usernameEl.textContent = profile.username;
  header.appendChild(usernameEl);

  const pronounsEl = doc.createElement("span");
  pronounsEl.setAttribute("data-profile-pronouns", "");
  pronounsEl.textContent = profile.pronouns;
  if (!profile.pronouns) {
    pronounsEl.hidden = true;
  }
  header.appendChild(pronounsEl);

  const statusBadge = doc.createElement("span");
  statusBadge.setAttribute("data-profile-status", profile.status);
  statusBadge.textContent = profile.statusLabelRu;
  header.appendChild(statusBadge);

  return header;
}

/**
 * Создаёт контроллер компактного поповера и полного модального окна профиля.
 *
 * @param {any} containerEl
 * @param {ProfileModalOptions} [options]
 */
export function createProfileModalController(containerEl, options = {}) {
  if (!containerEl || typeof containerEl.appendChild !== "function") {
    throw new TypeError("containerEl must be a valid DOM container element");
  }
  const doc = resolveDocument(containerEl);

  /** @type {any | null} */
  let activeElement = null;
  /** @type {"popout" | "modal" | null} */
  let currentMode = null;
  /** @type {NormalizedProfileData | null} */
  let currentProfile = null;
  /** @type {"about" | "mutual_guilds" | "mutual_friends"} */
  let activeTab = "about";
  /** @type {Map<string, any>} */
  let tabButtons = new Map();
  /** @type {any | null} */
  let modalTabPanel = null;

  /**
   * Удаляет текущий открытый поповер или модальное окно из контейнера.
   */
  function close() {
    if (activeElement) {
      if (typeof activeElement.remove === "function") {
        activeElement.remove();
      } else if (typeof containerEl.removeChild === "function") {
        try {
          containerEl.removeChild(activeElement);
        } catch {
          // Элемент уже мог быть удалён из контейнера внешним кодом
        }
      }
    }
    activeElement = null;
    currentMode = null;
    currentProfile = null;
    tabButtons = new Map();
    modalTabPanel = null;
  }

  /**
   * Перерисовывает содержимое панели активной вкладки полного модального окна.
   */
  function renderModalTabPanel() {
    if (!modalTabPanel || !currentProfile) return;

    /** @type {any[]} */
    const nodes = [];

    if (activeTab === "about") {
      const bioEl = doc.createElement("p");
      bioEl.setAttribute("data-profile-bio", "");
      bioEl.textContent = currentProfile.bio;
      nodes.push(bioEl);

      nodes.push(buildRolesList(doc, currentProfile.roles));
      nodes.push(buildNoteInput(doc, currentProfile, options.onSaveNote));
    } else if (activeTab === "mutual_guilds") {
      if (currentProfile.mutualGuilds.length === 0) {
        const emptyEl = doc.createElement("p");
        emptyEl.setAttribute("data-profile-empty", "mutual_guilds");
        emptyEl.textContent = PROFILE_EMPTY_STATES_RU.mutual_guilds;
        nodes.push(emptyEl);
      } else {
        const listEl = doc.createElement("ul");
        listEl.setAttribute("data-profile-mutual-guilds-list", "");
        for (const guild of currentProfile.mutualGuilds) {
          const itemEl = doc.createElement("li");
          itemEl.setAttribute("data-mutual-guild-id", guild.id);
          itemEl.textContent = guild.name;
          listEl.appendChild(itemEl);
        }
        nodes.push(listEl);
      }
    } else if (activeTab === "mutual_friends") {
      if (currentProfile.mutualFriends.length === 0) {
        const emptyEl = doc.createElement("p");
        emptyEl.setAttribute("data-profile-empty", "mutual_friends");
        emptyEl.textContent = PROFILE_EMPTY_STATES_RU.mutual_friends;
        nodes.push(emptyEl);
      } else {
        const listEl = doc.createElement("ul");
        listEl.setAttribute("data-profile-mutual-friends-list", "");
        for (const friend of currentProfile.mutualFriends) {
          const itemEl = doc.createElement("li");
          itemEl.setAttribute("data-mutual-friend-id", friend.id);
          itemEl.textContent = friend.displayName || friend.username;
          listEl.appendChild(itemEl);
        }
        nodes.push(listEl);
      }
    }

    modalTabPanel.setAttribute("data-profile-tabpanel", activeTab);
    if (typeof modalTabPanel.replaceChildren === "function") {
      modalTabPanel.replaceChildren(...nodes);
    } else {
      if (Array.isArray(modalTabPanel.children)) {
        modalTabPanel.children.length = 0;
      }
      for (const node of nodes) {
        modalTabPanel.appendChild(node);
      }
    }
  }

  /**
   * Переключает активную вкладку в открытом модальном окне полного профиля.
   *
   * @param {string} tabId
   * @returns {boolean}
   */
  function setTab(tabId) {
    if (
      currentMode !== "modal" ||
      !activeElement ||
      (tabId !== "about" &&
        tabId !== "mutual_guilds" &&
        tabId !== "mutual_friends")
    ) {
      return false;
    }

    activeTab = tabId;
    activeElement.setAttribute("data-active-tab", activeTab);

    for (const [id, btn] of tabButtons.entries()) {
      const selected = id === activeTab;
      btn.setAttribute("aria-selected", selected ? "true" : "false");
      btn.setAttribute("tabindex", selected ? "0" : "-1");
    }

    renderModalTabPanel();
    return true;
  }

  /**
   * Открывает компактную карточку профиля (`data-profile-mode="popout"`).
   *
   * @param {ProfileData} profileData
   * @returns {any}
   */
  function openPopout(profileData) {
    close();
    const profile = normalizeProfileData(profileData);
    currentProfile = profile;
    currentMode = "popout";

    const card = doc.createElement("section");
    card.setAttribute("data-profile-mode", "popout");
    card.setAttribute("data-user-id", profile.userId);

    card.appendChild(buildProfileHeader(doc, profile));

    const bioEl = doc.createElement("p");
    bioEl.setAttribute("data-profile-bio", "");
    bioEl.textContent = profile.bio;
    card.appendChild(bioEl);

    card.appendChild(buildRolesList(doc, profile.roles));
    card.appendChild(buildNoteInput(doc, profile, options.onSaveNote));

    const dmForm = doc.createElement("form");
    dmForm.setAttribute("data-profile-quick-dm-form", "");

    const dmInput = doc.createElement("input");
    dmInput.setAttribute("type", "text");
    dmInput.setAttribute("data-profile-quick-dm-input", "");
    const placeholderText = `Написать @${profile.username}`;
    dmInput.setAttribute("placeholder", placeholderText);
    dmInput.placeholder = placeholderText;
    dmInput.value = "";

    const submitQuickDm = () => {
      const text = String(dmInput.value ?? "").trim();
      if (text.length === 0) return false;
      dmInput.value = "";
      if (typeof options.onSendQuickMessage === "function") {
        options.onSendQuickMessage(profile.userId, text);
      }
      return true;
    };

    dmInput.addEventListener("keydown", (/** @type {any} */ event) => {
      if (!event?.key || event.key === "Enter") {
        if (typeof event?.preventDefault === "function") {
          event.preventDefault();
        }
        submitQuickDm();
      }
    });

    dmInput.addEventListener("change", () => {
      submitQuickDm();
    });

    dmForm.addEventListener("submit", (/** @type {any} */ event) => {
      if (typeof event?.preventDefault === "function") {
        event.preventDefault();
      }
      submitQuickDm();
    });

    dmForm.appendChild(dmInput);
    card.appendChild(dmForm);

    containerEl.appendChild(card);
    activeElement = card;
    return card;
  }

  /**
   * Открывает полное модальное окно профиля (`role="dialog"`, `aria-modal="true"`, `data-profile-mode="modal"`).
   *
   * @param {ProfileData} profileData
   * @returns {any}
   */
  function openFullModal(profileData) {
    close();
    const profile = normalizeProfileData(profileData);
    currentProfile = profile;
    currentMode = "modal";
    activeTab = "about";

    const modal = doc.createElement("div");
    modal.setAttribute("role", "dialog");
    modal.setAttribute("aria-modal", "true");
    modal.setAttribute("data-profile-mode", "modal");
    modal.setAttribute("data-user-id", profile.userId);
    modal.setAttribute("data-active-tab", activeTab);
    modal.setAttribute("aria-label", profile.displayName || profile.username);

    modal.appendChild(buildProfileHeader(doc, profile));

    const tabList = doc.createElement("div");
    tabList.setAttribute("role", "tablist");
    tabList.setAttribute("aria-label", "Разделы профиля");

    /** @type {Array<{ id: "about" | "mutual_guilds" | "mutual_friends", label: string, count?: number }>} */
    const tabsMeta = [
      { id: "about", label: PROFILE_TAB_LABELS_RU.about },
      {
        id: "mutual_guilds",
        label: `${PROFILE_TAB_LABELS_RU.mutual_guilds} (${profile.mutualGuilds.length})`,
        count: profile.mutualGuilds.length,
      },
      {
        id: "mutual_friends",
        label: `${PROFILE_TAB_LABELS_RU.mutual_friends} (${profile.mutualFriends.length})`,
        count: profile.mutualFriends.length,
      },
    ];

    for (const meta of tabsMeta) {
      const btn = doc.createElement("button");
      btn.setAttribute("type", "button");
      btn.setAttribute("role", "tab");
      btn.setAttribute("data-profile-tab", meta.id);
      if (typeof meta.count === "number") {
        btn.setAttribute("data-tab-count", String(meta.count));
      }
      const selected = meta.id === activeTab;
      btn.setAttribute("aria-selected", selected ? "true" : "false");
      btn.setAttribute("tabindex", selected ? "0" : "-1");
      btn.textContent = meta.label;
      btn.addEventListener("click", () => {
        setTab(meta.id);
      });
      tabButtons.set(meta.id, btn);
      tabList.appendChild(btn);
    }

    modal.appendChild(tabList);

    const panel = doc.createElement("div");
    panel.setAttribute("role", "tabpanel");
    modalTabPanel = panel;
    modal.appendChild(panel);

    containerEl.appendChild(modal);
    activeElement = modal;
    renderModalTabPanel();
    return modal;
  }

  return {
    openPopout,
    openFullModal,
    setTab,
    close,
    /** @returns {"popout" | "modal" | null} */
    getMode() {
      return currentMode;
    },
    /** @returns {string} */
    getActiveTab() {
      return activeTab;
    },
  };
}
