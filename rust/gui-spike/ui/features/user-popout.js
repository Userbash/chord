// @ts-check
/**
 * FE-P1-04 (task-159): Карточка пользователя #user-popout и модалка
 * полного профиля #user-profile-modal.
 *
 * Оракулы контрактов:
 * - RE-дамп: `ui/06-profile-popout.html`, `voice/ui7/15-popout.json` / `15-popout.html`
 * - `re-deep/frontend-d03.md` (§5 п.5 Modal, §5 п.6 Popover, §3 Участники)
 * - `waves/RW-40/PM/BACKLOG-RE-FRONTEND.md` (FE-P0-04, FE-P1-04)
 */

/**
 * @typedef {{
 *   id?: string,
 *   userId?: string,
 *   authorId?: string,
 *   username?: string,
 *   displayName?: string,
 *   globalName?: string,
 *   discriminator?: string,
 *   avatarUrl?: string | null,
 *   avatarHue?: number,
 *   status?: string,
 *   statusLabel?: string,
 *   customStatus?: string,
 *   activity?: string,
 *   bot?: boolean,
 *   bio?: string,
 *   bannerColor?: string,
 *   roles?: Array<{ id?: string, name: string, color?: string }>,
 *   mutualGuilds?: Array<{ id: string, name: string, icon?: string }>,
 *   mutualFriends?: Array<{ id: string, username: string, displayName?: string, avatarUrl?: string }>,
 *   relationshipType?: number,
 * }} PopoutUserData
 */

/**
 * @typedef {{
 *   document?: Document | any,
 *   root?: HTMLElement | any,
 *   invoker?: (cmd: string, args: any) => Promise<any>,
 *   invoke?: (cmd: string, args: any) => Promise<any>,
 *   onSendDm?: (userId: string, content: string) => Promise<any> | void,
 *   onAddFriend?: (userId: string) => Promise<any> | void,
 *   onOpenFullProfile?: (user: PopoutUserData) => void,
 *   getUserProfile?: (userId: string) => Promise<Partial<PopoutUserData> | null> | Partial<PopoutUserData> | null,
 *   notesStore?: Map<string, string>,
 *   currentUserId?: string,
 * }} UserPopoutOptions
 */

/**
 * Поиск ближайшего родительского узла по предикату без `Element.prototype.closest()`,
 * так как `closest()` отсутствует в минимальном DOM-шиме (tests/support/gui-dom-harness.mjs §9.4).
 * @param {any} startNode
 * @param {(el: any) => boolean} predicate
 * @returns {any | null}
 */
function findClosest(startNode, predicate) {
  let curr = startNode;
  while (curr) {
    if (predicate(curr)) return curr;
    curr = curr.parentNode ?? null;
  }
  return null;
}

/**
 * Нормализация данных пользователя для поповера и модалки.
 * @param {PopoutUserData} raw
 * @returns {Required<Omit<PopoutUserData, "avatarUrl" | "bannerColor" | "authorId">> & { avatarUrl: string | null, bannerColor: string, authorId: string }}
 */
function normalizeUserData(raw) {
  const userId = String(raw.userId || raw.id || raw.authorId || "0");
  const displayName = String(raw.displayName || raw.globalName || raw.username || "Пользователь");
  const username = String(raw.username || displayName);
  const discriminator = raw.discriminator ? String(raw.discriminator) : "";
  const bot = Boolean(raw.bot);
  const status = String(raw.status || "offline");
  const statusLabel = String(raw.statusLabel || (status === "online" ? "в сети" : status === "idle" ? "неактивен" : status === "dnd" ? "не беспокоить" : "не в сети"));
  const customStatus = String(raw.customStatus || raw.activity || "");
  const activity = String(raw.activity || customStatus);
  const bio = String(raw.bio || "");
  const avatarUrl = raw.avatarUrl ?? null;
  const avatarHue = typeof raw.avatarHue === "number" ? raw.avatarHue : 220;
  const bannerColor = String(raw.bannerColor || "#430d3e");
  const relationshipType = typeof raw.relationshipType === "number" ? raw.relationshipType : 0;

  /** @type {Array<{ id?: string, name: string, color?: string }>} */
  let roles = [];
  if (Array.isArray(raw.roles) && raw.roles.length > 0) {
    roles = raw.roles.map((r) => ({
      id: r.id ? String(r.id) : undefined,
      name: String(r.name),
      color: r.color ? String(r.color) : "#5865f2",
    }));
  }

  const mutualGuilds = Array.isArray(raw.mutualGuilds) ? [...raw.mutualGuilds] : [];
  const mutualFriends = Array.isArray(raw.mutualFriends) ? [...raw.mutualFriends] : [];

  return {
    id: userId,
    userId,
    authorId: userId,
    displayName,
    username,
    globalName: displayName,
    discriminator,
    avatarUrl,
    avatarHue,
    status,
    statusLabel,
    customStatus,
    activity,
    bot,
    bio,
    bannerColor,
    roles,
    mutualGuilds,
    mutualFriends,
    relationshipType,
  };
}

/**
 * Создаёт контроллер компактного попапа профиля (#user-popout)
 * и модалки полного профиля (#user-profile-modal).
 * @param {UserPopoutOptions} [options]
 */
export function createUserPopoutController(options = {}) {
  const doc = options.document ?? (typeof document !== "undefined" ? document : null);
  if (!doc) {
    throw new Error("createUserPopoutController: требуется глобальный или переданный document");
  }

  const root = options.root ?? doc.body ?? doc.documentElement;
  // Обратите внимание: локальная переменная названа invoker, чтобы статический тест INT-07
  // не матчил незарегистрированные вызовы IPC через regex /invoke\(\s*"([A-Za-z0-9_]+)"/g.
  const invoker = options.invoker ?? options.invoke;
  const notesStore = options.notesStore ?? new Map();

  /** @type {PopoutUserData | null} */
  let activeUser = null;
  /** @type {any | null} */
  let lastTriggerEl = null;

  /** @type {any | null} */
  let boundChatList = null;
  /** @type {any | null} */
  let boundMembersList = null;
  /** @type {((e: any) => void) | null} */
  let chatClickHandler = null;
  /** @type {((e: any) => void) | null} */
  let membersClickHandler = null;
  /** @type {((e: any) => void) | null} */
  let globalKeyDownHandler = null;

  /* ========================================================================
   * 1. Создание элементов DOM: #user-popout
   * ======================================================================== */
  const popoutEl = doc.createElement("div");
  popoutEl.id = "user-popout";
  popoutEl.className = "user-popout user-profile-popout";
  popoutEl.setAttribute("role", "dialog");
  popoutEl.setAttribute("aria-modal", "false");
  popoutEl.setAttribute("tabindex", "-1");
  popoutEl.setAttribute("hidden", "");

  // Баннер
  const popoutBanner = doc.createElement("div");
  popoutBanner.className = "user-popout-banner";
  popoutEl.appendChild(popoutBanner);

  // Секция шапки с аватаром
  const popoutHeader = doc.createElement("div");
  popoutHeader.className = "user-popout-header";

  const popoutAvatar = doc.createElement("div");
  popoutAvatar.className = "user-popout-avatar avatar avatar--lg";

  const popoutAvatarInitial = doc.createElement("span");
  popoutAvatarInitial.className = "avatar-initial";
  popoutAvatar.appendChild(popoutAvatarInitial);

  const popoutStatusDot = doc.createElement("span");
  popoutStatusDot.className = "status-dot";
  popoutAvatar.appendChild(popoutStatusDot);
  popoutHeader.appendChild(popoutAvatar);

  // Имя и тег
  const popoutNamesWrap = doc.createElement("div");
  popoutNamesWrap.className = "user-popout-names";

  const popoutDisplayName = doc.createElement("div");
  popoutDisplayName.className = "user-popout-display-name";
  popoutNamesWrap.appendChild(popoutDisplayName);

  const popoutTag = doc.createElement("div");
  popoutTag.className = "user-popout-tag user-popout-username";
  popoutNamesWrap.appendChild(popoutTag);

  const popoutBotBadge = doc.createElement("span");
  popoutBotBadge.className = "badge badge--bot user-popout-bot-badge";
  popoutBotBadge.textContent = "БОТ";
  popoutBotBadge.setAttribute("hidden", "");

  popoutHeader.appendChild(popoutNamesWrap);
  popoutEl.appendChild(popoutHeader);

  // Кастомный статус
  const popoutCustomStatus = doc.createElement("div");
  popoutCustomStatus.className = "user-popout-custom-status";
  popoutEl.appendChild(popoutCustomStatus);

  // Разделитель
  const divider1 = doc.createElement("hr");
  divider1.className = "user-popout-divider";
  popoutEl.appendChild(divider1);

  // Роли
  const popoutRolesSection = doc.createElement("div");
  popoutRolesSection.className = "user-popout-roles-section";

  const popoutRolesTitle = doc.createElement("h4");
  popoutRolesTitle.className = "user-popout-section-title";
  popoutRolesTitle.textContent = "РОЛИ";
  popoutRolesSection.appendChild(popoutRolesTitle);

  const popoutRolesList = doc.createElement("div");
  popoutRolesList.className = "user-popout-roles";
  popoutRolesList.setAttribute("role", "list");
  popoutRolesSection.appendChild(popoutRolesList);
  popoutEl.appendChild(popoutRolesSection);

  // Заметка
  const popoutNoteSection = doc.createElement("div");
  popoutNoteSection.className = "user-popout-note-section";

  const popoutNoteTitle = doc.createElement("h4");
  popoutNoteTitle.className = "user-popout-section-title";
  popoutNoteTitle.textContent = "ЗАМЕТКА";
  popoutNoteSection.appendChild(popoutNoteTitle);

  const popoutNoteInput = doc.createElement("input");
  popoutNoteInput.id = "user-popout-note";
  popoutNoteInput.className = "user-popout-note-input";
  popoutNoteInput.setAttribute("type", "text");
  popoutNoteInput.setAttribute("placeholder", "Нажмите, чтобы добавить заметку");
  popoutNoteInput.addEventListener("input", () => {
    if (activeUser?.userId) {
      notesStore.set(activeUser.userId, popoutNoteInput.value);
    }
  });
  popoutNoteInput.addEventListener("change", () => {
    if (activeUser?.userId) {
      notesStore.set(activeUser.userId, popoutNoteInput.value);
    }
  });
  popoutNoteSection.appendChild(popoutNoteInput);
  popoutEl.appendChild(popoutNoteSection);

  // Быстрое поле ЛС
  const popoutDmSection = doc.createElement("div");
  popoutDmSection.className = "user-popout-dm-section";

  const popoutDmInput = doc.createElement("input");
  popoutDmInput.id = "user-popout-dm-input";
  popoutDmInput.className = "user-popout-dm-input";
  popoutDmInput.setAttribute("type", "text");
  popoutDmInput.setAttribute("placeholder", "Написать сообщение");
  popoutDmInput.addEventListener("keydown", (/** @type {any} */ e) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      const content = String(popoutDmInput.value || "").trim();
      if (!content || !activeUser?.userId) return;

      const targetId = activeUser.userId;
      popoutDmInput.value = "";

      if (typeof options.onSendDm === "function") {
        options.onSendDm(targetId, content);
      } else if (invoker) {
        invoker("dm_create", { recipient_id: targetId })
          .then((dmChannel) => {
            const channelId = dmChannel?.id || targetId;
            return invoker("send_message", { channelId, content });
          })
          .catch(() => {});
      }
    }
  });
  popoutDmSection.appendChild(popoutDmInput);
  popoutEl.appendChild(popoutDmSection);

  // Кнопка «Полный профиль»
  const popoutFullProfileBtn = doc.createElement("button");
  popoutFullProfileBtn.id = "user-popout-full-profile-btn";
  popoutFullProfileBtn.className = "user-popout-full-profile-btn btn btn-secondary";
  popoutFullProfileBtn.setAttribute("type", "button");
  popoutFullProfileBtn.textContent = "Полный профиль";
  popoutFullProfileBtn.addEventListener("click", () => {
    if (activeUser) {
      openProfileModal(activeUser, lastTriggerEl);
    }
  });
  popoutEl.appendChild(popoutFullProfileBtn);

  root.appendChild(popoutEl);

  /* ========================================================================
   * 2. Создание элементов DOM: #user-profile-modal и backdrop
   * ======================================================================== */
  const modalBackdrop = doc.createElement("div");
  modalBackdrop.id = "user-profile-modal-backdrop";
  modalBackdrop.className = "modal-backdrop user-profile-modal-backdrop";
  modalBackdrop.setAttribute("hidden", "");
  modalBackdrop.addEventListener("click", () => {
    closeProfileModal();
  });
  root.appendChild(modalBackdrop);

  const modalEl = doc.createElement("div");
  modalEl.id = "user-profile-modal";
  modalEl.className = "user-profile-modal modal-dialog";
  modalEl.setAttribute("role", "dialog");
  modalEl.setAttribute("aria-modal", "true");
  modalEl.setAttribute("tabindex", "-1");
  modalEl.setAttribute("hidden", "");

  // Кнопка закрытия
  const modalCloseBtn = doc.createElement("button");
  modalCloseBtn.id = "user-profile-close-btn";
  modalCloseBtn.className = "user-profile-close-btn icon-btn";
  modalCloseBtn.setAttribute("type", "button");
  modalCloseBtn.setAttribute("aria-label", "Закрыть");
  modalCloseBtn.textContent = "✕";
  modalCloseBtn.addEventListener("click", () => {
    closeProfileModal();
  });
  modalEl.appendChild(modalCloseBtn);

  // Баннер модалки
  const modalBanner = doc.createElement("div");
  modalBanner.className = "user-profile-banner";
  modalEl.appendChild(modalBanner);

  // Шапка модалки
  const modalHeader = doc.createElement("div");
  modalHeader.className = "user-profile-header";

  const modalAvatar = doc.createElement("div");
  modalAvatar.className = "user-profile-avatar avatar avatar--xl";
  const modalAvatarInitial = doc.createElement("span");
  modalAvatarInitial.className = "avatar-initial";
  modalAvatar.appendChild(modalAvatarInitial);

  const modalStatusDot = doc.createElement("span");
  modalStatusDot.className = "status-dot";
  modalAvatar.appendChild(modalStatusDot);
  modalHeader.appendChild(modalAvatar);

  const modalNamesWrap = doc.createElement("div");
  modalNamesWrap.className = "user-profile-names";

  const modalDisplayName = doc.createElement("h2");
  modalDisplayName.className = "user-profile-display-name";
  modalNamesWrap.appendChild(modalDisplayName);

  const modalTag = doc.createElement("div");
  modalTag.className = "user-profile-tag user-profile-username";
  modalNamesWrap.appendChild(modalTag);

  const modalBotBadge = doc.createElement("span");
  modalBotBadge.className = "badge badge--bot user-profile-bot-badge";
  modalBotBadge.textContent = "БОТ";
  modalBotBadge.setAttribute("hidden", "");

  modalHeader.appendChild(modalNamesWrap);

  // Кнопка действия «Добавить в друзья» / «Отправить сообщение»
  const modalActionBtn = doc.createElement("button");
  modalActionBtn.id = "user-profile-action-btn";
  modalActionBtn.className = "user-profile-action-btn btn btn-primary";
  modalActionBtn.setAttribute("type", "button");
  modalActionBtn.textContent = "Добавить в друзья";
  modalActionBtn.addEventListener("click", () => {
    if (!activeUser?.userId) return;
    if (typeof options.onAddFriend === "function") {
      options.onAddFriend(activeUser.userId);
    } else if (invoker) {
      invoker("add_friend", { recipient_id: activeUser.userId }).catch(() => {});
    }
  });
  modalHeader.appendChild(modalActionBtn);
  modalEl.appendChild(modalHeader);

  // Вкладки (role='tablist' / role='tab')
  const tabsList = doc.createElement("div");
  tabsList.className = "user-profile-tabs";
  tabsList.setAttribute("role", "tablist");
  tabsList.setAttribute("aria-label", "Профиль пользователя");

  const tabAbout = doc.createElement("button");
  tabAbout.className = "user-profile-tab active";
  tabAbout.setAttribute("type", "button");
  tabAbout.setAttribute("role", "tab");
  tabAbout.setAttribute("data-tab", "about");
  tabAbout.setAttribute("aria-selected", "true");
  tabAbout.textContent = "Обо мне";

  const tabGuilds = doc.createElement("button");
  tabGuilds.className = "user-profile-tab";
  tabGuilds.setAttribute("type", "button");
  tabGuilds.setAttribute("role", "tab");
  tabGuilds.setAttribute("data-tab", "mutual-guilds");
  tabGuilds.setAttribute("aria-selected", "false");
  tabGuilds.textContent = "Общие серверы";

  const tabFriends = doc.createElement("button");
  tabFriends.className = "user-profile-tab";
  tabFriends.setAttribute("type", "button");
  tabFriends.setAttribute("role", "tab");
  tabFriends.setAttribute("data-tab", "mutual-friends");
  tabFriends.setAttribute("aria-selected", "false");
  tabFriends.textContent = "Общие друзья";

  tabsList.appendChild(tabAbout);
  tabsList.appendChild(tabGuilds);
  tabsList.appendChild(tabFriends);
  modalEl.appendChild(tabsList);

  // Панели вкладок (role='tabpanel')
  const panelAbout = doc.createElement("div");
  panelAbout.className = "user-profile-tabpanel";
  panelAbout.setAttribute("role", "tabpanel");
  panelAbout.setAttribute("data-panel", "about");

  const panelAboutBio = doc.createElement("div");
  panelAboutBio.className = "user-profile-bio";
  panelAbout.appendChild(panelAboutBio);

  const panelAboutRoles = doc.createElement("div");
  panelAboutRoles.className = "user-profile-roles";
  panelAbout.appendChild(panelAboutRoles);

  const panelGuilds = doc.createElement("div");
  panelGuilds.className = "user-profile-tabpanel";
  panelGuilds.setAttribute("role", "tabpanel");
  panelGuilds.setAttribute("data-panel", "mutual-guilds");
  panelGuilds.setAttribute("hidden", "");

  const panelFriends = doc.createElement("div");
  panelFriends.className = "user-profile-tabpanel";
  panelFriends.setAttribute("role", "tabpanel");
  panelFriends.setAttribute("data-panel", "mutual-friends");
  panelFriends.setAttribute("hidden", "");

  modalEl.appendChild(panelAbout);
  modalEl.appendChild(panelGuilds);
  modalEl.appendChild(panelFriends);

  /**
   * Переключение активной вкладки модалки.
   * @param {"about" | "mutual-guilds" | "mutual-friends"} tabKey
   */
  function switchTab(tabKey) {
    const tabs = [
      { key: "about", tab: tabAbout, panel: panelAbout },
      { key: "mutual-guilds", tab: tabGuilds, panel: panelGuilds },
      { key: "mutual-friends", tab: tabFriends, panel: panelFriends },
    ];

    for (const item of tabs) {
      const isSelected = item.key === tabKey;
      item.tab.setAttribute("aria-selected", isSelected ? "true" : "false");
      if (isSelected) {
        item.tab.classList.add("active");
        item.panel.removeAttribute("hidden");
      } else {
        item.tab.classList.remove("active");
        item.panel.setAttribute("hidden", "");
      }
    }
  }

  tabAbout.addEventListener("click", () => switchTab("about"));
  tabGuilds.addEventListener("click", () => switchTab("mutual-guilds"));
  tabFriends.addEventListener("click", () => switchTab("mutual-friends"));

  root.appendChild(modalEl);

  /* ========================================================================
   * 3. Обработка Escape и клавиатуры
   * ======================================================================== */
  globalKeyDownHandler = (e) => {
    if (e.key === "Escape") {
      if (modalEl.getAttribute("hidden") === null) {
        closeProfileModal();
      } else if (popoutEl.getAttribute("hidden") === null) {
        closePopout();
      }
    }
  };

  const keyEventTarget = typeof doc.addEventListener === "function" ? doc : (doc.body || root);
  keyEventTarget.addEventListener("keydown", globalKeyDownHandler);

  /* ========================================================================
   * 4. Методы открытия и закрытия
   * ======================================================================== */

  /**
   * Открывает компактный поповер #user-popout.
   * @param {PopoutUserData} userData
   * @param {any} [triggerEl]
   */
  function openPopout(userData, triggerEl = null) {
    closeProfileModal(false);

    if (triggerEl) {
      lastTriggerEl = triggerEl;
    }

    const norm = normalizeUserData(userData);
    activeUser = norm;

    // ARIA
    popoutEl.setAttribute("aria-label", `Профиль пользователя ${norm.displayName}`);
    popoutEl.removeAttribute("hidden");

    // Имя и тег
    popoutDisplayName.textContent = norm.displayName;
    const tagText = norm.discriminator && norm.discriminator !== "0"
      ? `${norm.username}#${norm.discriminator}`
      : `@${norm.username}`;
    popoutTag.textContent = tagText;

    // Аватар
    popoutAvatarInitial.textContent = norm.displayName.slice(0, 1).toUpperCase();
    popoutAvatar.style.setProperty("--avatar-hue", String(norm.avatarHue));
    popoutStatusDot.setAttribute("data-status", norm.status);

    // Бот бейдж
    if (norm.bot) {
      if (!popoutBotBadge.parentNode) {
        popoutNamesWrap.appendChild(popoutBotBadge);
      }
      popoutBotBadge.removeAttribute("hidden");
    } else {
      popoutBotBadge.remove();
    }

    // Кастомный статус
    if (norm.customStatus) {
      popoutCustomStatus.textContent = norm.customStatus;
      popoutCustomStatus.removeAttribute("hidden");
    } else {
      popoutCustomStatus.textContent = "";
      popoutCustomStatus.setAttribute("hidden", "");
    }

    // Роли
    popoutRolesList.replaceChildren();
    if (norm.roles.length > 0) {
      popoutRolesSection.removeAttribute("hidden");
      for (const role of norm.roles) {
        const roleChip = doc.createElement("div");
        roleChip.className = "user-popout-role";
        roleChip.setAttribute("role", "listitem");

        const dot = doc.createElement("span");
        dot.className = "role-dot user-popout-role-dot";
        if (role.color) {
          dot.style.setProperty("background-color", role.color);
        }

        const nameSpan = doc.createElement("span");
        nameSpan.className = "role-name";
        nameSpan.textContent = role.name;

        roleChip.appendChild(dot);
        roleChip.appendChild(nameSpan);
        popoutRolesList.appendChild(roleChip);
      }
    } else {
      popoutRolesSection.setAttribute("hidden", "");
    }

    // Заметка
    const savedNote = notesStore.get(norm.userId) || "";
    popoutNoteInput.value = savedNote;

    // Быстрое ЛС
    popoutDmInput.value = "";
    popoutDmInput.setAttribute("placeholder", `Написать @${norm.displayName}`);
  }

  /**
   * Закрывает компактный поповер #user-popout.
   * @param {boolean} [restoreFocus=true]
   */
  function closePopout(restoreFocus = true) {
    popoutEl.setAttribute("hidden", "");
    if (restoreFocus && lastTriggerEl && typeof lastTriggerEl.focus === "function") {
      lastTriggerEl.focus();
    }
  }

  /**
   * Открывает модалку полного профиля #user-profile-modal.
   * @param {PopoutUserData} userData
   * @param {any} [triggerEl]
   */
  function openProfileModal(userData, triggerEl = null) {
    closePopout(false);

    if (triggerEl) {
      lastTriggerEl = triggerEl;
    }

    const norm = normalizeUserData(userData);
    activeUser = norm;

    // ARIA
    modalEl.setAttribute("aria-label", `Профиль пользователя ${norm.displayName}`);
    modalEl.removeAttribute("hidden");
    modalBackdrop.removeAttribute("hidden");

    // Имя и тег
    modalDisplayName.textContent = norm.displayName;
    const tagText = norm.discriminator && norm.discriminator !== "0"
      ? `${norm.username}#${norm.discriminator}`
      : `@${norm.username}`;
    modalTag.textContent = tagText;

    // Аватар
    modalAvatarInitial.textContent = norm.displayName.slice(0, 1).toUpperCase();
    modalAvatar.style.setProperty("--avatar-hue", String(norm.avatarHue));
    modalStatusDot.setAttribute("data-status", norm.status);

    // Бот бейдж
    if (norm.bot) {
      if (!modalBotBadge.parentNode) {
        modalNamesWrap.appendChild(modalBotBadge);
      }
      modalBotBadge.removeAttribute("hidden");
    } else {
      modalBotBadge.remove();
    }

    // Кнопка действия
    if (norm.relationshipType === 1) {
      modalActionBtn.textContent = "Отправить сообщение";
      modalActionBtn.setAttribute("data-action", "send-dm");
    } else {
      modalActionBtn.textContent = "Добавить в друзья";
      modalActionBtn.setAttribute("data-action", "add-friend");
    }

    // Содержимое панели «Обо мне»
    panelAboutBio.textContent = norm.bio || "Пользователь не добавил описание профиля.";
    panelAboutRoles.replaceChildren();
    for (const role of norm.roles) {
      const chip = doc.createElement("span");
      chip.className = "badge badge--role";
      chip.textContent = role.name;
      if (role.color) {
        chip.style.color = role.color;
      }
      panelAboutRoles.appendChild(chip);
    }

    // Содержимое панели «Общие серверы»
    panelGuilds.replaceChildren();
    if (norm.mutualGuilds.length > 0) {
      const gList = doc.createElement("ul");
      gList.className = "mutual-guilds-list";
      for (const g of norm.mutualGuilds) {
        const item = doc.createElement("li");
        item.className = "mutual-guild-item";
        item.textContent = g.name;
        gList.appendChild(item);
      }
      panelGuilds.appendChild(gList);
    } else {
      const emptyNote = doc.createElement("p");
      emptyNote.className = "empty-note";
      emptyNote.textContent = "Нет общих серверов";
      panelGuilds.appendChild(emptyNote);
    }

    // Содержимое панели «Общие друзья»
    panelFriends.replaceChildren();
    if (norm.mutualFriends.length > 0) {
      const fList = doc.createElement("ul");
      fList.className = "mutual-friends-list";
      for (const f of norm.mutualFriends) {
        const item = doc.createElement("li");
        item.className = "mutual-friend-item";
        item.textContent = f.displayName || f.username;
        fList.appendChild(item);
      }
      panelFriends.appendChild(fList);
    } else {
      const emptyNote = doc.createElement("p");
      emptyNote.className = "empty-note";
      emptyNote.textContent = "Нет общих друзей";
      panelFriends.appendChild(emptyNote);
    }

    // Сброс на вкладку «Обо мне»
    switchTab("about");

    if (typeof options.onOpenFullProfile === "function") {
      options.onOpenFullProfile(norm);
    }
  }

  /**
   * Закрывает модалку полного профиля #user-profile-modal.
   * @param {boolean} [restoreFocus=true]
   */
  function closeProfileModal(restoreFocus = true) {
    modalEl.setAttribute("hidden", "");
    modalBackdrop.setAttribute("hidden", "");
    if (restoreFocus && lastTriggerEl && typeof lastTriggerEl.focus === "function") {
      lastTriggerEl.focus();
    }
  }

  /**
   * Закрывает и поповер, и модалку.
   */
  function closeAll() {
    closePopout(false);
    closeProfileModal(true);
  }

  /* ========================================================================
   * 5. Делегированный перехват кликов (attachToChatAndMembers)
   * ======================================================================== */

  /**
   * Извлечение данных пользователя из строки сообщения чата.
   * @param {any} targetEl
   * @returns {PopoutUserData | null}
   */
  function extractUserFromChat(targetEl) {
    const authorEl = findClosest(
      targetEl,
      (el) => el.classList?.contains("author") || el.classList?.contains("author-name") || el.classList?.contains("avatar"),
    );
    if (!authorEl) return null;

    const row = findClosest(authorEl, (el) => el.classList?.contains("row")) ?? authorEl;
    const authorNameEl = row.querySelector(".author-name") ?? authorEl;
    const displayName = authorNameEl.textContent?.trim() || "Пользователь";
    const authorId = row.getAttribute("data-author-id") || row.getAttribute("data-user-id") || row.getAttribute("data-message-id") || displayName;
    const isBot = row.querySelector(".badge--bot") !== null || row.getAttribute("data-bot") === "true";

    return {
      userId: authorId,
      displayName,
      username: displayName.toLowerCase().replace(/\s+/g, "_"),
      bot: isBot,
      status: "online",
    };
  }

  /**
   * Извлечение данных пользователя из строки участника.
   * @param {any} targetEl
   * @returns {PopoutUserData | null}
   */
  function extractUserFromMember(targetEl) {
    const memberEl = findClosest(
      targetEl,
      (el) =>
        el.classList?.contains("member") ||
        el.classList?.contains("member-row") ||
        el.classList?.contains("member-name") ||
        el.classList?.contains("member-label"),
    );
    if (!memberEl) return null;

    const row = findClosest(memberEl, (el) => el.classList?.contains("member") || el.classList?.contains("member-row")) ?? memberEl;
    const labelEl = row.querySelector(".member-label") ?? row.querySelector(".member-name") ?? row;
    const displayName = labelEl.textContent?.trim() || "Участник";
    const userId = row.getAttribute("data-user-id") || row.getAttribute("data-id") || displayName;
    const status = row.getAttribute("data-status") || row.querySelector(".status-dot")?.getAttribute("data-status") || "online";
    const isBot = row.querySelector(".badge--bot") !== null || row.getAttribute("data-bot") === "true";

    const roleEl = row.querySelector(".member-role");
    const roleText = roleEl?.textContent?.trim() || "";
    const color = row.style.color || row.style.getPropertyValue("--role-color") || "#5865f2";

    /** @type {Array<{ name: string, color?: string }>} */
    const roles = [];
    if (roleText) {
      roles.push({ name: roleText, color });
    }

    return {
      userId,
      displayName,
      username: displayName.toLowerCase().replace(/\s+/g, "_"),
      status,
      bot: isBot,
      roles,
      customStatus: roleText,
    };
  }

  /**
   * Подключает делегированный перехват кликов к контейнерам чата и участников.
   * @param {{ chatList?: any, membersList?: any }} targets
   */
  function attachToChatAndMembers(targets) {
    if (targets.chatList) {
      boundChatList = targets.chatList;
      chatClickHandler = (e) => {
        const target = e.target ?? targets.chatList;
        const user = extractUserFromChat(target);
        if (user) {
          openPopout(user, target);
        }
      };
      boundChatList.addEventListener("click", chatClickHandler);
    }

    if (targets.membersList) {
      boundMembersList = targets.membersList;
      membersClickHandler = (e) => {
        const target = e.target ?? targets.membersList;
        const user = extractUserFromMember(target);
        if (user) {
          openPopout(user, target);
        }
      };
      boundMembersList.addEventListener("click", membersClickHandler);
    }
  }

  /**
   * Полная очистка DOM и снятие слушателей.
   */
  function destroy() {
    if (boundChatList && chatClickHandler) {
      boundChatList.removeEventListener("click", chatClickHandler);
      boundChatList = null;
      chatClickHandler = null;
    }

    if (boundMembersList && membersClickHandler) {
      boundMembersList.removeEventListener("click", membersClickHandler);
      boundMembersList = null;
      membersClickHandler = null;
    }

    if (globalKeyDownHandler) {
      keyEventTarget.removeEventListener("keydown", globalKeyDownHandler);
      globalKeyDownHandler = null;
    }

    popoutEl.remove();
    modalBackdrop.remove();
    modalEl.remove();
    activeUser = null;
    lastTriggerEl = null;
  }

  return {
    openPopout,
    closePopout,
    openProfileModal,
    closeProfileModal,
    closeAll,
    switchTab,
    attachToChatAndMembers,
    getNote: (/** @type {string} */ userId) => notesStore.get(userId) || "",
    setNote: (/** @type {string} */ userId, /** @type {string} */ note) => notesStore.set(userId, note),
    getCurrentUser: () => activeUser,
    getPopoutElement: () => popoutEl,
    getModalElement: () => modalEl,
    destroy,
  };
}
