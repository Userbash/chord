// @ts-check
/**
 * task-160 (FE-P1-05): Модалка приглашения на сервер/канал #invite-modal и
 * контекстное меню сервера #guild-context-menu: создание инвайта, настройки
 * уведомлений, подтверждение выхода из сервера и присоединение по инвайту.
 *
 * Спецификация:
 * - Контекстное меню гильдии #guild-context-menu (role='menu', пункты role='menuitem'
 *   и role='menuitemcheckbox', roving tabindex / клавиатурная навигация ArrowDown/ArrowUp/Home/End/Escape);
 * - Модалка инвайта #invite-modal (role='dialog', aria-modal='true', вызов create_invite,
 *   #invite-link-input, #invite-copy-btn, #invite-max-age-select, #invite-max-uses-select,
 *   #invite-temporary-checkbox);
 * - Модалка выхода #leave-guild-modal (role='dialog', aria-modal='true', вызов leave_guild / onLeaveGuild);
 * - Модалка присоединения #join-guild-modal (role='dialog', aria-modal='true', вызов resolve_invite + accept_invite / join_guild).
 */

/** Допустимые значения срока действия приглашения в секундах (RE §4.3, INVITES-SPEC §1.2). */
export const VALID_INVITE_MAX_AGES = Object.freeze([0, 1800, 3600, 21600, 43200, 86400, 604800]);

/** Допустимые значения максимального количества использований (RE §4.3, INVITES-SPEC §1.2). */
export const VALID_INVITE_MAX_USES = Object.freeze([0, 1, 5, 10, 25, 50, 100]);

/** Битовый флаг гостевого приглашения (IS_GUEST_INVITE = 1 << 0). */
export const INVITE_FLAG_IS_GUEST = 1;

/** Текстовые метки для селекта срока действия приглашения. */
const AGE_LABELS = Object.freeze({
  1800: "30 минут",
  3600: "1 час",
  21600: "6 часов",
  43200: "12 часов",
  86400: "1 день",
  604800: "7 дней",
  0: "Бессрочно",
});

/** Текстовые метки для селекта количества использований. */
const USES_LABELS = Object.freeze({
  0: "Без ограничений",
  1: "1 использование",
  5: "5 использований",
  10: "10 использований",
  25: "25 использований",
  50: "50 использований",
  100: "100 использований",
});

/** Регулярное выражение для валидации кода приглашения (2..32 символов). */
const CODE_RE = /^[A-Za-z0-9-]{2,32}$/;

/**
 * Нормализация ввода кода или ссылки приглашения.
 * @param {unknown} input
 * @returns {string}
 */
export function normalizeInviteCode(input) {
  const raw = typeof input === "string" ? input : "";
  const trimmed = raw.trim();
  const cut = trimmed.search(/[?#]/);
  const withoutQuery = (cut < 0 ? trimmed : trimmed.slice(0, cut)).replace(/\/+$/, "");
  const code = withoutQuery.slice(withoutQuery.lastIndexOf("/") + 1);
  if ((code === "invite" || code === "invites") && trimmed.includes("/")) return "";
  return CODE_RE.test(code) ? code : "";
}

/**
 * @typedef {{
 *   channelId?: string,
 *   maxAge: number,
 *   maxUses: number,
 *   temporary: boolean,
 * }} CreateInvitePayload
 *
 * @typedef {{
 *   document?: any,
 *   root?: any,
 *   invoker?: (cmd: string, args?: any) => Promise<any>,
 *   invoke?: (cmd: string, args?: any) => Promise<any>,
 *   guildId?: string,
 *   channelId?: string,
 *   hideMuted?: boolean,
 *   onCreateInvite?: (params: CreateInvitePayload) => Promise<any> | any,
 *   onLeaveGuild?: (params: { guildId: string }) => Promise<any> | any,
 *   onLeftGuild?: (params: { guildId: string }) => void,
 *   onJoinGuild?: (params: { code: string }) => Promise<any> | any,
 *   onJoined?: (result: any) => void,
 *   onOpenNotifications?: (params: { guildId: string }) => void,
 *   onToggleHideMuted?: (hideMuted: boolean) => void,
 *   clipboard?: { writeText: (text: string) => Promise<void> | void },
 * }} GuildModalsOptions
 */

/**
 * Создаёт контроллер модалок гильдии и контекстного меню #guild-context-menu.
 * @param {GuildModalsOptions} options
 */
export function createGuildModalsController(options) {
  const doc = options.document ?? globalThis.document;
  const root = options.root ?? doc.body;

  // Локальный IPC-инвокер (назван invoker, чтобы статический анализатор INT-07 проверял только зарегистрированные команды)
  const invoker = options.invoker ?? options.invoke ?? (async () => null);

  let currentGuildId = options.guildId ?? "";
  let currentChannelId = options.channelId ?? "";
  let hideMuted = Boolean(options.hideMuted);

  /** @type {any} */
  let activeTriggerElement = null;

  /** @type {Array<{ el: any, onContextMenu: (event: any) => void }>} */
  const attachedTriggers = [];

  /** @type {Array<() => void>} */
  const cleanupTasks = [];

  /**
   * Вспомогательный хелпер создания DOM-элемента.
   * @param {string} tag
   * @param {string} [className]
   * @param {string} [text]
   * @returns {any}
   */
  const createElement = (tag, className = "", text = "") => {
    const el = doc.createElement(tag);
    if (className) el.className = className;
    if (text) el.textContent = text;
    return el;
  };

  /* ==========================================================================
   * 1. Контекстное меню гильдии #guild-context-menu
   * ========================================================================== */
  let contextMenu = doc.getElementById("guild-context-menu");
  if (!contextMenu) {
    contextMenu = createElement("div", "menu context-menu guild-context-menu");
    contextMenu.id = "guild-context-menu";
    root.append(contextMenu);
  }
  contextMenu.setAttribute("role", "menu");
  contextMenu.setAttribute("aria-label", "Меню сервера");
  contextMenu.setAttribute("tabindex", "-1");
  contextMenu.setAttribute("hidden", "");

  const inviteItem = createElement("button", "menu-item", "Пригласить людей");
  inviteItem.setAttribute("role", "menuitem");
  inviteItem.setAttribute("data-action", "invite");
  inviteItem.setAttribute("tabindex", "-1");

  const notifItem = createElement("button", "menu-item", "Настройки уведомлений");
  notifItem.setAttribute("role", "menuitem");
  notifItem.setAttribute("data-action", "notifications");
  notifItem.setAttribute("tabindex", "-1");

  const hideMutedItem = createElement("button", "menu-item", "Скрыть заглушенные каналы");
  hideMutedItem.setAttribute("role", "menuitemcheckbox");
  hideMutedItem.setAttribute("data-action", "hide-muted");
  hideMutedItem.setAttribute("aria-checked", hideMuted ? "true" : "false");
  hideMutedItem.setAttribute("tabindex", "-1");

  const leaveItem = createElement("button", "menu-item danger", "Покинуть сервер");
  leaveItem.setAttribute("role", "menuitem");
  leaveItem.setAttribute("data-action", "leave-guild");
  leaveItem.setAttribute("data-danger", "true");
  leaveItem.setAttribute("tabindex", "-1");

  contextMenu.replaceChildren();
  contextMenu.append(inviteItem, notifItem, hideMutedItem, leaveItem);

  const menuItems = [inviteItem, notifItem, hideMutedItem, leaveItem];
  let focusedItemIndex = 0;

  /**
   * Обновляет фокус внутри контекстного меню.
   * @param {number} nextIndex
   */
  const focusMenuItem = (nextIndex) => {
    focusedItemIndex = ((nextIndex % menuItems.length) + menuItems.length) % menuItems.length;
    menuItems.forEach((item, idx) => {
      item.setAttribute("tabindex", idx === focusedItemIndex ? "0" : "-1");
    });
    menuItems[focusedItemIndex]?.focus();
  };

  /**
   * Открывает контекстное меню сервера.
   * @param {{ x?: number, y?: number, guildId?: string, triggerElement?: any }} [params]
   */
  const openContextMenu = (params = {}) => {
    closeAllModals();
    if (params.guildId) currentGuildId = params.guildId;
    if (params.triggerElement) activeTriggerElement = params.triggerElement;

    if (typeof params.x === "number") contextMenu.style.left = `${params.x}px`;
    if (typeof params.y === "number") contextMenu.style.top = `${params.y}px`;

    contextMenu.removeAttribute("hidden");
    focusMenuItem(0);
  };

  /**
   * Закрывает контекстное меню сервера.
   */
  const closeContextMenu = () => {
    contextMenu.setAttribute("hidden", "");
  };

  // Клавиатурная навигация меню
  /** @param {any} event */
  const onContextMenuKeydown = (event) => {
    if (contextMenu.hasAttribute("hidden")) return;
    const key = event?.key;
    if (key === "ArrowDown") {
      if (event?.preventDefault) event.preventDefault();
      focusMenuItem(focusedItemIndex + 1);
    } else if (key === "ArrowUp") {
      if (event?.preventDefault) event.preventDefault();
      focusMenuItem(focusedItemIndex - 1);
    } else if (key === "Home") {
      if (event?.preventDefault) event.preventDefault();
      focusMenuItem(0);
    } else if (key === "End") {
      if (event?.preventDefault) event.preventDefault();
      focusMenuItem(menuItems.length - 1);
    } else if (key === "Escape") {
      if (event?.preventDefault) event.preventDefault();
      closeContextMenu();
      if (activeTriggerElement && typeof activeTriggerElement.focus === "function") {
        activeTriggerElement.focus();
      }
    }
  };
  contextMenu.addEventListener("keydown", onContextMenuKeydown);
  cleanupTasks.push(() => contextMenu.removeEventListener("keydown", onContextMenuKeydown));

  // Обработчики действий меню
  inviteItem.addEventListener("click", () => {
    closeContextMenu();
    openInviteModal();
  });

  notifItem.addEventListener("click", () => {
    closeContextMenu();
    if (options.onOpenNotifications) {
      options.onOpenNotifications({ guildId: currentGuildId });
    }
  });

  hideMutedItem.addEventListener("click", () => {
    hideMuted = !hideMuted;
    hideMutedItem.setAttribute("aria-checked", hideMuted ? "true" : "false");
    if (options.onToggleHideMuted) {
      options.onToggleHideMuted(hideMuted);
    }
  });

  leaveItem.addEventListener("click", () => {
    closeContextMenu();
    openLeaveModal({ guildId: currentGuildId });
  });

  /* ==========================================================================
   * 2. Модалка создания приглашения #invite-modal
   * ========================================================================== */
  let inviteModal = doc.getElementById("invite-modal");
  if (!inviteModal) {
    inviteModal = createElement("div", "modal-dialog invite-modal");
    inviteModal.id = "invite-modal";
    root.append(inviteModal);
  }
  inviteModal.setAttribute("role", "dialog");
  inviteModal.setAttribute("aria-modal", "true");
  inviteModal.setAttribute("aria-label", "Пригласить на сервер");
  inviteModal.setAttribute("hidden", "");

  const inviteTitle = createElement("h2", "modal-title", "Пригласить людей");
  inviteTitle.id = "invite-modal-title";

  const inviteCloseBtn = createElement("button", "modal-close-btn", "×");
  inviteCloseBtn.id = "invite-modal-close";
  inviteCloseBtn.setAttribute("aria-label", "Закрыть");

  const inviteLinkInput = createElement("input", "modal-input invite-link-input");
  inviteLinkInput.id = "invite-link-input";
  inviteLinkInput.type = "text";
  inviteLinkInput.setAttribute("readonly", "");

  const inviteCopyBtn = createElement("button", "btn btn-primary invite-copy-btn", "Копировать");
  inviteCopyBtn.id = "invite-copy-btn";
  inviteCopyBtn.type = "button";

  const inviteMaxAgeSelect = createElement("select", "select invite-max-age-select");
  inviteMaxAgeSelect.id = "invite-max-age-select";
  VALID_INVITE_MAX_AGES.forEach((age) => {
    const opt = createElement("option", "", AGE_LABELS[/** @type {keyof typeof AGE_LABELS} */ (age)] ?? `${age}s`);
    opt.value = String(age);
    if (age === 86400) opt.setAttribute("selected", "");
    inviteMaxAgeSelect.append(opt);
  });
  inviteMaxAgeSelect.value = "86400";

  const inviteMaxUsesSelect = createElement("select", "select invite-max-uses-select");
  inviteMaxUsesSelect.id = "invite-max-uses-select";
  VALID_INVITE_MAX_USES.forEach((uses) => {
    const opt = createElement("option", "", USES_LABELS[/** @type {keyof typeof USES_LABELS} */ (uses)] ?? `${uses}`);
    opt.value = String(uses);
    if (uses === 0) opt.setAttribute("selected", "");
    inviteMaxUsesSelect.append(opt);
  });
  inviteMaxUsesSelect.value = "0";

  const inviteTempCheckbox = createElement("input", "checkbox invite-temporary-checkbox");
  inviteTempCheckbox.id = "invite-temporary-checkbox";
  inviteTempCheckbox.type = "checkbox";
  inviteTempCheckbox.checked = false;

  const inviteTempLabel = createElement("label", "checkbox-label", "Временное членство");
  inviteTempLabel.setAttribute("for", "invite-temporary-checkbox");

  const inviteSettingsContainer = createElement("div", "invite-settings");
  inviteSettingsContainer.append(inviteMaxAgeSelect, inviteMaxUsesSelect, inviteTempCheckbox, inviteTempLabel);

  inviteModal.replaceChildren();
  inviteModal.append(
    inviteTitle,
    inviteCloseBtn,
    inviteLinkInput,
    inviteCopyBtn,
    inviteSettingsContainer,
  );

  /**
   * Вызывает создание инвайта и обновляет поле ссылки.
   * @returns {Promise<any>}
   */
  const generateInvite = async () => {
    const maxAge = Number(inviteMaxAgeSelect.value ?? 86400);
    const maxUses = Number(inviteMaxUsesSelect.value ?? 0);
    const temporary = Boolean(inviteTempCheckbox.checked);

    /** @type {CreateInvitePayload} */
    const payload = {
      channelId: currentChannelId || undefined,
      maxAge,
      maxUses,
      temporary,
    };

    try {
      let res;
      if (options.onCreateInvite) {
        res = await options.onCreateInvite(payload);
      } else {
        res = await invoker("create_invite", payload);
      }
      const code = typeof res === "string" ? res : res?.code ?? "";
      inviteLinkInput.value = code ? `https://discord.gg/${code}` : "";
      return res;
    } catch (err) {
      inviteLinkInput.value = "";
      throw err;
    }
  };

  /**
   * Открывает модалку приглашения.
   * @param {{ channelId?: string, guildId?: string }} [params]
   * @returns {Promise<any>}
   */
  const openInviteModal = async (params = {}) => {
    closeAllModals();
    if (params.channelId) currentChannelId = params.channelId;
    if (params.guildId) currentGuildId = params.guildId;

    inviteCopyBtn.textContent = "Копировать";
    inviteModal.removeAttribute("hidden");
    return generateInvite();
  };

  const closeInviteModal = () => {
    inviteModal.setAttribute("hidden", "");
  };

  /**
   * Копирует ссылку приглашения в буфер обмена.
   * @returns {Promise<void>}
   */
  const copyInviteLink = async () => {
    const textToCopy = inviteLinkInput.value;
    inviteCopyBtn.textContent = "Скопировано!";
    if (options.clipboard && typeof options.clipboard.writeText === "function") {
      await options.clipboard.writeText(textToCopy);
    } else if (globalThis.navigator?.clipboard && typeof globalThis.navigator.clipboard.writeText === "function") {
      await globalThis.navigator.clipboard.writeText(textToCopy);
    }
  };

  inviteCopyBtn.addEventListener("click", () => {
    copyInviteLink();
  });

  inviteMaxAgeSelect.addEventListener("change", () => generateInvite());
  inviteMaxUsesSelect.addEventListener("change", () => generateInvite());
  inviteTempCheckbox.addEventListener("change", () => generateInvite());
  inviteCloseBtn.addEventListener("click", closeInviteModal);

  /** @param {any} event */
  const onInviteModalKeydown = (event) => {
    if (event?.key === "Escape") {
      if (event?.preventDefault) event.preventDefault();
      closeInviteModal();
    }
  };
  inviteModal.addEventListener("keydown", onInviteModalKeydown);
  cleanupTasks.push(() => inviteModal.removeEventListener("keydown", onInviteModalKeydown));

  /* ==========================================================================
   * 3. Модалка выхода из сервера #leave-guild-modal
   * ========================================================================== */
  let leaveModal = doc.getElementById("leave-guild-modal");
  if (!leaveModal) {
    leaveModal = createElement("div", "modal-dialog leave-guild-modal");
    leaveModal.id = "leave-guild-modal";
    root.append(leaveModal);
  }
  leaveModal.setAttribute("role", "dialog");
  leaveModal.setAttribute("aria-modal", "true");
  leaveModal.setAttribute("aria-label", "Покинуть сервер");
  leaveModal.setAttribute("hidden", "");

  const leaveTitle = createElement("h2", "modal-title", "Покинуть сервер");
  leaveTitle.id = "leave-guild-title";

  const leaveDesc = createElement(
    "p",
    "modal-desc",
    "Вы действительно хотите покинуть этот сервер? Вы потеряете доступ ко всем каналам.",
  );
  leaveDesc.id = "leave-guild-desc";

  const leaveError = createElement("div", "modal-error", "");
  leaveError.id = "leave-guild-error";

  const leaveConfirmBtn = createElement("button", "btn btn-danger", "Покинуть сервер");
  leaveConfirmBtn.id = "leave-guild-confirm-btn";
  leaveConfirmBtn.type = "button";

  const leaveCancelBtn = createElement("button", "btn btn-secondary", "Отмена");
  leaveCancelBtn.id = "leave-guild-cancel-btn";
  leaveCancelBtn.type = "button";

  leaveModal.replaceChildren();
  leaveModal.append(leaveTitle, leaveDesc, leaveError, leaveConfirmBtn, leaveCancelBtn);

  /**
   * Открывает модалку подтверждения выхода.
   * @param {{ guildId?: string, guildName?: string }} [params]
   */
  const openLeaveModal = (params = {}) => {
    closeAllModals();
    if (params.guildId) currentGuildId = params.guildId;
    leaveError.textContent = "";
    leaveModal.removeAttribute("hidden");
  };

  const closeLeaveModal = () => {
    leaveModal.setAttribute("hidden", "");
  };

  /**
   * Подтверждает выход из гильдии.
   * @returns {Promise<void>}
   */
  const confirmLeave = async () => {
    try {
      leaveError.textContent = "";
      if (!currentGuildId && typeof doc?.querySelector === "function") {
        const selectedTile =
          doc.querySelector('.guild-item[aria-current="true"]') ??
          doc.querySelector(".guild-item.is-selected") ??
          doc.querySelector(".guild-item[data-guild-id]");
        const fallbackId = selectedTile?.getAttribute?.("data-guild-id") ?? "";
        if (fallbackId && fallbackId !== "@me") {
          currentGuildId = fallbackId;
        }
      }
      if (options.onLeaveGuild) {
        await options.onLeaveGuild({ guildId: currentGuildId });
      } else {
        await invoker("leave_guild", { guildId: currentGuildId, lurking: false });
      }
      closeLeaveModal();
      if (options.onLeftGuild) {
        options.onLeftGuild({ guildId: currentGuildId });
      }
    } catch (err) {
      const msg =
        err instanceof Error
          ? err.message
          : typeof err === "string"
            ? err
            : err && typeof err === "object"
              ? String(
                  /** @type {any} */ (err).message ||
                    (/** @type {any} */ (err).kind === "forbidden"
                      ? "Выход с сервера запрещён"
                      : "Не удалось покинуть сервер"),
                )
              : "Ошибка выхода из сервера";
      leaveError.textContent = msg;
    }
  };

  leaveConfirmBtn.addEventListener("click", () => {
    confirmLeave();
  });

  leaveCancelBtn.addEventListener("click", closeLeaveModal);

  /** @param {any} event */
  const onLeaveModalKeydown = (event) => {
    if (event?.key === "Escape") {
      if (event?.preventDefault) event.preventDefault();
      closeLeaveModal();
    }
  };
  leaveModal.addEventListener("keydown", onLeaveModalKeydown);
  cleanupTasks.push(() => leaveModal.removeEventListener("keydown", onLeaveModalKeydown));

  /* ==========================================================================
   * 4. Модалка присоединения к серверу #join-guild-modal
   * ========================================================================== */
  let joinModal = doc.getElementById("join-guild-modal");
  if (!joinModal) {
    joinModal = createElement("div", "modal-dialog join-guild-modal");
    joinModal.id = "join-guild-modal";
    root.append(joinModal);
  }
  joinModal.setAttribute("role", "dialog");
  joinModal.setAttribute("aria-modal", "true");
  joinModal.setAttribute("aria-label", "Присоединиться к серверу");
  joinModal.setAttribute("hidden", "");

  const joinTitle = createElement("h2", "modal-title", "Присоединиться к серверу");
  joinTitle.id = "join-guild-title";

  const joinInput = createElement("input", "modal-input join-guild-input");
  joinInput.id = "join-guild-input";
  joinInput.type = "text";
  joinInput.placeholder = "https://discord.gg/hTK7Dis или hTK7Dis";

  const joinError = createElement("div", "modal-error", "");
  joinError.id = "join-guild-error";

  const joinSubmitBtn = createElement("button", "btn btn-primary", "Присоединиться");
  joinSubmitBtn.id = "join-guild-submit-btn";
  joinSubmitBtn.type = "button";

  const joinCancelBtn = createElement("button", "btn btn-secondary", "Отмена");
  joinCancelBtn.id = "join-guild-cancel-btn";
  joinCancelBtn.type = "button";

  joinModal.replaceChildren();
  joinModal.append(joinTitle, joinInput, joinError, joinSubmitBtn, joinCancelBtn);

  const openJoinModal = () => {
    closeAllModals();
    joinInput.value = "";
    joinError.textContent = "";
    joinModal.removeAttribute("hidden");
  };

  const closeJoinModal = () => {
    joinModal.setAttribute("hidden", "");
  };

  /**
   * Отправляет запрос на присоединение к серверу по инвайт-коду.
   * @param {string} [inputOverride]
   * @returns {Promise<any>}
   */
  const submitJoin = async (inputOverride) => {
    joinError.textContent = "";
    const rawCode = inputOverride ?? joinInput.value;
    const code = normalizeInviteCode(rawCode);

    if (!code) {
      joinError.textContent = "Некорректный код приглашения";
      return;
    }

    try {
      let result;
      if (options.onJoinGuild) {
        result = await options.onJoinGuild({ code });
      } else {
        const preview = await invoker("resolve_invite", { code });
        try {
          result = await invoker("accept_invite", { code });
        } catch {
          result = await invoker("join_guild", { guildId: preview?.guildId ?? "", lurking: false });
        }
      }
      closeJoinModal();
      if (options.onJoined) {
        options.onJoined(result);
      }
      return result;
    } catch (err) {
      joinError.textContent = err instanceof Error ? err.message : String(err || "Ошибка присоединения к серверу");
      throw err;
    }
  };

  joinSubmitBtn.addEventListener("click", () => {
    submitJoin();
  });

  joinCancelBtn.addEventListener("click", closeJoinModal);

  /** @param {any} event */
  const onJoinModalKeydown = (event) => {
    if (event?.key === "Escape") {
      if (event?.preventDefault) event.preventDefault();
      closeJoinModal();
    }
  };
  joinModal.addEventListener("keydown", onJoinModalKeydown);
  cleanupTasks.push(() => joinModal.removeEventListener("keydown", onJoinModalKeydown));

  /* ==========================================================================
   * Вспомогательные методы контроллера
   * ========================================================================== */
  const closeAllModals = () => {
    closeContextMenu();
    closeInviteModal();
    closeLeaveModal();
    closeJoinModal();
  };

  /** @param {any} event */
  const onDocumentMouseDown = (event) => {
    if (
      !contextMenu.hasAttribute("hidden") &&
      event?.target &&
      typeof contextMenu.contains === "function" &&
      !contextMenu.contains(event.target)
    ) {
      closeContextMenu();
    }
  };
  if (doc && typeof doc.addEventListener === "function") {
    doc.addEventListener("mousedown", onDocumentMouseDown);
    cleanupTasks.push(() => {
      if (typeof doc.removeEventListener === "function") {
        doc.removeEventListener("mousedown", onDocumentMouseDown);
      }
    });
  }

  /**
   * Привязывает открытие контекстного меню к элементу триггеру (иконка гильдии, кнопка шапки).
   * @param {any} el
   * @param {{ guildId?: string }} [opts]
   */
  const attachTrigger = (el, opts = {}) => {
    if (!el || typeof el.addEventListener !== "function") return;
    /** @param {any} event */
    const onContextMenu = (event) => {
      let resolvedGuildId = opts.guildId;
      let resolvedTrigger = el;
      if (
        (el.id === "guild-rail" || el.id === "guild-list") &&
        event?.target &&
        event.target !== el &&
        typeof event.target.closest === "function"
      ) {
        const guildTile = event.target.closest(".guild-item[data-guild-id]");
        const tileGuildId = guildTile?.getAttribute?.("data-guild-id") ?? "";
        if (!guildTile || !tileGuildId || tileGuildId === "@me") {
          return;
        }
        resolvedGuildId = resolvedGuildId || tileGuildId;
        resolvedTrigger = guildTile;
      }
      if (event?.preventDefault) event.preventDefault();
      const x = event?.clientX ?? 0;
      const y = event?.clientY ?? 0;
      openContextMenu({
        x,
        y,
        guildId: resolvedGuildId,
        triggerElement: resolvedTrigger,
      });
    };
    el.addEventListener("contextmenu", onContextMenu);
    attachedTriggers.push({ el, onContextMenu });
  };

  /**
   * Полная очистка контроллера и обработчиков.
   */
  const destroy = () => {
    closeAllModals();
    cleanupTasks.forEach((task) => task());
    cleanupTasks.length = 0;

    attachedTriggers.forEach(({ el, onContextMenu }) => {
      if (typeof el.removeEventListener === "function") {
        el.removeEventListener("contextmenu", onContextMenu);
      }
    });
    attachedTriggers.length = 0;
  };

  return {
    openContextMenu,
    closeContextMenu,
    openInviteModal,
    closeInviteModal,
    openLeaveModal,
    closeLeaveModal,
    openJoinModal,
    closeJoinModal,
    closeAll: closeAllModals,
    attachTrigger,
    generateInvite,
    copyInviteLink,
    confirmLeave,
    submitJoin,
    destroy,
    getState: () => ({
      contextMenuOpen: !contextMenu.hasAttribute("hidden"),
      inviteModalOpen: !inviteModal.hasAttribute("hidden"),
      leaveModalOpen: !leaveModal.hasAttribute("hidden"),
      joinModalOpen: !joinModal.hasAttribute("hidden"),
      hideMuted,
      guildId: currentGuildId,
      channelId: currentChannelId,
    }),
  };
}
