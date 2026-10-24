// @ts-check
/**
 * Действия `data-action` (§8.1), глобальные клавиши (§8.2), focus-trap панелей и
 * клавиатура виртуализованных списков.
 *
 * В DOM-шиме события не всплывают: каждый `[data-action]` получает прямой
 * слушатель, а глобальные клавиши висят на `body` (keydown всплывает до body и в
 * браузере, и при `body.dispatchEvent` в тестах).
 */
import { byId, make, toggleClass } from "./dom.js";
import { asUIError, reportError, UIError } from "./errors.js";

/**
 * @typedef {{
 *   router: ReturnType<typeof import("./screens.js").createScreenRouter>,
 *   chat: ReturnType<typeof import("./chat.js").createChatScreen>,
 *   members: ReturnType<typeof import("./members.js").createMemberScreen>,
 *   emoji: ReturnType<typeof import("./emoji.js").createEmojiConsole>,
 *   settings: ReturnType<typeof import("./settings.js").createSettingsConsole>,
 *   composer?: import("./composer.js").Composer | null,
 *   search?: import("./search-overlay.js").SearchOverlay | null,
 *   net?: { retry: () => Promise<void> } | null,
 *   join?: ReturnType<typeof import("./join-server.js").createJoinServer> | null,
 *   invoke?: ((cmd: string, args: any) => Promise<any>) | null,
 *   onError?: (error: UIError) => void,
 * }} ActionsContext
 * @typedef {import("./virtual-list.js").VirtualList} VirtualList
 */

/**
 * Пункт поповера сервера. Тексты и порядок — дословно из
 * `waves/RW-40/DESIGN/server-menu.json` (`menu_items`), группы разделителей —
 * §4.2 SERVER-MENU.md. Пункты без `action` — честные `aria-disabled`-заглушки
 * до C2; у них нет `data-action`, поэтому клик не уходит в ACTION_FAILED.
 * @typedef {{
 *   label: string,
 *   subtitle?: string,
 *   action?: string,
 *   role?: "menuitem" | "menuitemcheckbox",
 *   hasPopup?: "menu" | "dialog",
 *   danger?: boolean,
 *   disabled?: boolean,
 *   separatorAfter?: boolean,
 * }} ServerMenuItemSpec
 */

/** @type {ServerMenuItemSpec[]} */
const SERVER_MENU_SPEC = [
  { label: "Пометить как прочитанное", disabled: true, separatorAfter: true },
  { label: "Пригласить на сервер", action: "guild-invite", hasPopup: "dialog", separatorAfter: true },
  { label: "Заглушить сервер", hasPopup: "menu", disabled: true },
  { label: "Параметры уведомлений", subtitle: "Только @упоминания", hasPopup: "menu", disabled: true },
  { label: "Скрыть заглушённые каналы", role: "menuitemcheckbox", disabled: true, separatorAfter: true },
  { label: "Настройки конфиденциальности", disabled: true },
  { label: "Редактировать личный профиль сервера", disabled: true, separatorAfter: true },
  { label: "Переместить...", hasPopup: "menu", disabled: true, separatorAfter: true },
  { label: "Покинуть сервер", action: "guild-leave", danger: true, separatorAfter: true },
  { label: "Копировать ID сервера", action: "guild-copy-id" },
];

/**
 * Поповер `#server-menu`: в браузере это пустой контейнер из `index.html`, в
 * DOM-шиме его нет — поэтому содержимое собирается здесь одним код-путём
 * (как содержимое `#auth-gate` в `features/auth.js`).
 * @returns {HTMLElement}
 */
function ensureServerMenu() {
  const existing = byId("server-menu");
  const menu = existing ?? make("div", "menu server-menu");
  menu.id = "server-menu";
  menu.className = "menu server-menu";
  menu.setAttribute("role", "menu");
  menu.setAttribute("aria-label", "Меню сервера");
  menu.setAttribute("hidden", "");
  // Приглашения (C2) в спайке не реализованы: неизвестная capability = false,
  // поэтому пункт 2 честно disabled, а не «молчащий» клик.
  if (menu.getAttribute("data-capability-invite") === null) {
    menu.setAttribute("data-capability-invite", "false");
  }
  if (menu.childElementCount > 0) menu.replaceChildren();
  for (const spec of SERVER_MENU_SPEC) {
    const item = make("button", "menu-item");
    item.setAttribute("type", "button");
    item.setAttribute("role", spec.role ?? "menuitem");
    if (spec.action) item.setAttribute("data-action", spec.action);
    if (spec.hasPopup) item.setAttribute("aria-haspopup", spec.hasPopup);
    if (spec.role === "menuitemcheckbox") item.setAttribute("aria-checked", "false");
    if (spec.danger) item.setAttribute("data-variant", "danger");
    // Базовое состояние храним отдельно: capability/owner могут снять/вернуть
    // disabled при каждом открытии, не теряя «статичные» заглушки C2.
    if (spec.disabled) item.setAttribute("data-static-disabled", "true");
    item.setAttribute("aria-disabled", "true");
    item.setAttribute("tabindex", "-1");
    item.append(make("span", "menu-item-label", spec.label));
    if (spec.subtitle) item.append(make("span", "menu-item-subtitle", spec.subtitle));
    menu.append(item);
    if (spec.separatorAfter) {
      const separator = make("div", "menu-sep");
      separator.setAttribute("role", "separator");
      menu.append(separator);
    }
  }
  if (menu.parentNode === null) document.body.append(menu);
  return menu;
}

/**
 * Confirm выхода `#leave-server-dialog` (SERVER-MENU.md §4.4, §6). Числа — в
 * `components/server-menu.css`; здесь только структура и безопасный фокус.
 * @returns {HTMLElement}
 */
function ensureLeaveDialog() {
  const existing = byId("leave-server-dialog");
  const backdrop = existing ?? make("div", "dialog-backdrop");
  backdrop.id = "leave-server-dialog";
  backdrop.className = "dialog-backdrop";
  backdrop.setAttribute("hidden", "");
  if (backdrop.childElementCount > 0) backdrop.replaceChildren();

  const card = make("div", "dialog");
  card.setAttribute("role", "alertdialog");
  card.setAttribute("aria-modal", "true");
  card.setAttribute("aria-labelledby", "leave-server-title");
  card.setAttribute("aria-describedby", "leave-server-text");
  card.setAttribute("data-state", "idle");

  const title = make("h2", "dialog-title", "Вы уверены, что хотите покинуть этот сервер?");
  title.id = "leave-server-title";
  const text = make("p", "dialog-text", "Вы покинете сервер и вернётесь к личным сообщениям.");
  text.id = "leave-server-text";
  const error = make("p", "dialog-error");
  error.id = "leave-error";
  error.setAttribute("role", "alert");
  error.setAttribute("aria-live", "assertive");
  error.setAttribute("hidden", "");

  const actions = make("div", "dialog-actions");
  const cancel = make("button", "btn btn-secondary", "Отмена");
  cancel.setAttribute("type", "button");
  cancel.setAttribute("data-action", "leave-cancel");
  cancel.setAttribute("data-autofocus", "true");
  const confirm = make("button", "btn btn-danger", "Покинуть сервер");
  confirm.id = "leave-confirm";
  confirm.setAttribute("type", "button");
  confirm.setAttribute("data-action", "leave-confirm");
  actions.append(cancel, confirm);

  card.append(title, text, error, actions);
  backdrop.append(card);
  if (backdrop.parentNode === null) document.body.append(backdrop);
  return backdrop;
}

/**
 * Общий live-region успеха/статуса (`#server-status`, `role=status`).
 * @returns {HTMLElement}
 */
function ensureServerStatus() {
  const existing = byId("server-status");
  const status = existing ?? make("p", "visually-hidden");
  status.id = "server-status";
  status.className = "visually-hidden";
  status.setAttribute("role", "status");
  status.setAttribute("aria-live", "polite");
  if (status.parentNode === null) document.body.append(status);
  return status;
}

/**
 * `kind` типизированной ошибки Tauri (`LeaveGuildError` сериализуется как
 * `{kind: "forbidden"}`); неизвестное — пустая строка.
 * @param {unknown} error
 * @returns {string}
 */
function errorKind(error) {
  if (typeof error !== "object" || error === null || !("kind" in error)) return "";
  const kind = /** @type {{ kind?: unknown }} */ (error).kind;
  return typeof kind === "string" ? kind : "";
}

/**
 * Задержка 429 в целых секундах (сервер отдаёт миллисекунды); 0 — неизвестна.
 * @param {unknown} error
 * @returns {number}
 */
function retryAfterSeconds(error) {
  if (typeof error !== "object" || error === null) return 0;
  const bag = /** @type {Record<string, unknown>} */ (error);
  const raw = bag.retry_after_ms ?? bag.retryAfterMs;
  return typeof raw === "number" && Number.isFinite(raw) && raw > 0 ? Math.ceil(raw / 1000) : 0;
}

/**
 * Видимый текст отказа выхода — по статусу/машинному `kind`, без разбора тела
 * и без токена (SERVER-MENU.md §3.2; коды 403/404 помечены `[A]`).
 * @param {unknown} error
 * @returns {string}
 */
function leaveFailureText(error) {
  switch (errorKind(error)) {
    case "unauthorized":
      return "Сессия истекла. Войдите снова.";
    case "forbidden":
      return "Недостаточно прав, чтобы покинуть сервер.";
    case "notFound":
      return "Сервер не найден. Возможно, вы уже вышли.";
    case "rateLimited": {
      const seconds = retryAfterSeconds(error);
      return seconds > 0
        ? `Слишком много попыток. Повторите через ${seconds} с.`
        : "Слишком много попыток. Повторите позже.";
    }
    case "server":
      return "Сервер не ответил. Попробуйте ещё раз.";
    case "network":
      return "Сетевое подключение ограничено или недоступно.";
    default:
      // Таймаут/неизвестный исход: успехом не считается, плитка остаётся.
      return "Результат неизвестен. Проверьте список серверов.";
  }
}

/**
 * @param {EventTarget | null} target
 * @returns {boolean}
 */
function isEditable(target) {
  if (!(target instanceof Element)) return false;
  const tag = target.tagName;
  return tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT" || target.hasAttribute("contenteditable");
}

/**
 * @param {HTMLElement | null} node
 */
function focusNode(node) {
  if (!node) return;
  // В DOM-шиме у узлов нет focus() — в браузере есть; best-effort без throw.
  if (typeof node.focus === "function") node.focus();
}

/**
 * @param {string} screen
 */
function focusRail(screen) {
  const button = [...document.querySelectorAll(".rail-btn")].find(
    (node) => /** @type {HTMLElement} */ (node).dataset.screen === screen,
  );
  focusNode(/** @type {HTMLElement | null} */ (button ?? null));
}

/**
 * Узел скрыт от пользователя/AT атрибутом `hidden` или inline `display:none`
 * (в шиме CSS не вычисляется, поэтому проверяем оба явных признака).
 * @param {HTMLElement} node
 * @returns {boolean}
 */
function isHidden(node) {
  /** @type {any} */
  let current = node;
  while (current) {
    if (typeof current.hasAttribute === "function" && current.hasAttribute("hidden")) return true;
    if (current.style && current.style.display === "none") return true;
    current = current.parentNode ?? null;
  }
  return false;
}

/**
 * @param {HTMLElement | null} container
 * @returns {HTMLElement[]}
 */
function focusables(container) {
  if (!container) return [];
  return /** @type {HTMLElement[]} */ ([...container.querySelectorAll("button, input, textarea")])
    .filter((node) => node.getAttribute("disabled") === null && node.getAttribute("tabindex") !== "-1")
    .filter((node) => !isHidden(node));
}

/**
 * Лежит ли узел внутри контейнера (в шиме нет `Node.contains`).
 * @param {HTMLElement | null} container
 * @param {Node | null} node
 * @returns {boolean}
 */
function isInside(container, node) {
  /** @type {any} */
  let current = node;
  while (current) {
    if (current === container) return true;
    current = current.parentNode ?? null;
  }
  return false;
}

/**
 * @param {ActionsContext} context
 */
export function initActions(context) {
  const { router, chat, members } = context;
  const onError = context.onError ?? (() => {});
  const composer = context.composer ?? null;
  const search = context.search ?? null;
  // L-01: ручной retry из баннера сети (ровно один `net_connect` на клик).
  const net = context.net ?? null;
  // G-03: модалка join-by-invite (resolve → accept).
  const join = context.join ?? null;

  /* ---------- G-05/G-04: поповер сервера и подтверждение выхода ---------- */

  const invoke = context.invoke ?? null;
  const menu = ensureServerMenu();
  const leaveDialog = ensureLeaveDialog();
  const statusRegion = ensureServerStatus();
  const leaveCard = /** @type {HTMLElement} */ (leaveDialog.querySelector(".dialog") ?? leaveDialog);
  const leaveError = byId("leave-error");
  const leaveText = byId("leave-server-text");

  /** @type {HTMLElement | null} */
  let menuOpener = null;
  let menuOpen = false;
  /** @type {HTMLElement | null} */
  let leaveOpener = null;
  let leaveOpen = false;
  let leavePending = false;
  /** @type {{ id: string, name: string, lurking: boolean, owner: boolean }} */
  let leaveTarget = { id: "", name: "", lurking: false, owner: false };

  /** @returns {HTMLElement[]} */
  function menuEntries() {
    return /** @type {HTMLElement[]} */ ([
      ...menu.querySelectorAll('[role="menuitem"], [role="menuitemcheckbox"]'),
    ]);
  }

  /** @param {HTMLElement} node */
  function isEntryDisabled(node) {
    return node.getAttribute("aria-disabled") === "true";
  }

  /** @returns {HTMLElement[]} */
  function enabledEntries() {
    return menuEntries().filter((node) => !isEntryDisabled(node));
  }

  /** @returns {HTMLElement | null} */
  function focusedGuildTile() {
    return /** @type {HTMLElement | null} */ (
      document.querySelector(".guild-item.is-selected") ?? document.querySelector(".guild-item")
    );
  }

  /**
   * Явное состояние плитки. `lurking` не вычисляется из ролей: без
   * `data-lurking` это `false` (lurker/guest-семантику решает backend-контракт
   * `DELETE /users/@me/guilds/{id}`; UI не выдумывает членство).
   * @param {HTMLElement | null} tile
   * @returns {{ id: string, name: string, lurking: boolean, owner: boolean }}
   */
  function readGuildState(tile) {
    if (!tile) return { id: "", name: "", lurking: false, owner: false };
    const label = tile.getAttribute("aria-label") ?? "";
    return {
      // G-01: контракт рейла — `data-guild-id`; `data-guild` — legacy-заглушки
      // шима (index.html их больше не содержит).
      id:
        tile.getAttribute("data-guild-id")
        || tile.dataset.guild
        || tile.getAttribute("data-guild")
        || "",
      name: tile.getAttribute("data-name") || label.replace(/^Сервер\s+/, ""),
      lurking: tile.getAttribute("data-lurking") === "true",
      owner: tile.getAttribute("data-owner") === "true",
    };
  }

  /**
   * Пересчёт disabled по capabilities/правам: статичные заглушки C2 + invite
   * (нет capability) + leave (владелец). Ровинг-tabindex — только доступные.
   */
  function refreshMenuState() {
    const tile = focusedGuildTile();
    const canInvite = menu.getAttribute("data-capability-invite") === "true";
    const isOwner = readGuildState(tile).owner;
    for (const entry of menuEntries()) {
      const action = entry.getAttribute("data-action");
      let disabled = entry.getAttribute("data-static-disabled") === "true";
      if (action === "guild-invite") disabled = !canInvite;
      if (action === "guild-leave") disabled = isOwner;
      if (disabled) entry.setAttribute("aria-disabled", "true");
      else entry.removeAttribute("aria-disabled");
      entry.setAttribute("tabindex", "-1");
    }
    const first = enabledEntries()[0];
    if (first) first.setAttribute("tabindex", "0");
  }

  /**
   * Позиция поповера: правый край плитки +8, верх по центру, кламп в viewport
   * (SERVER-MENU.md §4.2). DOM-шим не считает layout — там работает CSS.
   * @param {HTMLElement} opener
   */
  function placeMenu(opener) {
    const rect = typeof opener.getBoundingClientRect === "function"
      ? opener.getBoundingClientRect()
      : null;
    if (!rect) return;
    const width = 288;
    const viewport = window.innerWidth || 0;
    if (viewport > 0) {
      menu.style.left = `${Math.max(8, Math.min(rect.left + rect.width + 8, viewport - width - 8))}px`;
    }
    menu.style.top = `${Math.max(8, rect.top + rect.height / 2 - 24)}px`;
  }

  /** @param {HTMLElement} opener */
  function openMenu(opener) {
    menuOpener = opener;
    menuOpen = true;
    refreshMenuState();
    menu.removeAttribute("hidden");
    opener.setAttribute("aria-expanded", "true");
    placeMenu(opener);
    focusNode(enabledEntries()[0] ?? null);
  }

  /** @param {boolean} returnFocus */
  function closeMenu(returnFocus) {
    menuOpen = false;
    menu.setAttribute("hidden", "");
    const opener = menuOpener;
    menuOpener = null;
    if (opener) {
      opener.setAttribute("aria-expanded", "false");
      if (returnFocus) focusNode(opener);
    }
  }

  /**
   * @param {boolean} disabled
   */
  function setLeaveButtonsDisabled(disabled) {
    for (const button of leaveDialog.querySelectorAll("button")) {
      if (disabled) button.setAttribute("disabled", "");
      else button.removeAttribute("disabled");
    }
  }

  function openLeaveDialog() {
    const tile = focusedGuildTile();
    leaveTarget = readGuildState(tile);
    if (!leaveTarget.id) return;
    if (leaveError) {
      leaveError.textContent = "";
      leaveError.setAttribute("hidden", "");
    }
    leaveCard.setAttribute("data-state", "idle");
    leaveCard.removeAttribute("aria-busy");
    setLeaveButtonsDisabled(false);
    if (leaveText) {
      leaveText.textContent = leaveTarget.name
        ? `Вы покинете сервер «${leaveTarget.name}».`
        : "Вы покинете сервер и вернётесь к личным сообщениям.";
    }
    leaveOpener = menuOpener;
    closeMenu(false);
    leaveOpen = true;
    leaveDialog.removeAttribute("hidden");
    // Безопасный дефолт: фокус на «Отмена», не на destructive.
    focusNode(/** @type {HTMLElement | null} */ (leaveDialog.querySelector('[data-autofocus="true"]')));
  }

  /** @param {boolean} returnFocus */
  function closeLeave(returnFocus) {
    if (leavePending) return;
    leaveOpen = false;
    leaveDialog.setAttribute("hidden", "");
    leaveCard.setAttribute("data-state", "idle");
    leaveCard.removeAttribute("aria-busy");
    if (leaveError) {
      leaveError.textContent = "";
      leaveError.setAttribute("hidden", "");
    }
    setLeaveButtonsDisabled(false);
    if (returnFocus) focusNode(leaveOpener);
    leaveOpener = null;
  }

  /**
   * Плитка гильдии по id. В DOM-шиме `data-*`, заданные через `dataset`, не
   * читаются селектором `[data-guild="…"]`, поэтому ищем по коллекции.
   * @param {string} id
   * @returns {HTMLElement | null}
   */
  function findGuildTile(id) {
    const tiles = /** @type {HTMLElement[]} */ ([...document.querySelectorAll(".guild-item")]);
    return tiles.find(
      (tile) =>
        tile.getAttribute("data-guild-id") === id
        || (tile.dataset.guild || tile.getAttribute("data-guild")) === id,
    ) ?? null;
  }

  /**
   * @param {HTMLElement} tile
   */
  function removeGuildTile(tile) {
    const wasSelected = tile.classList.contains("is-selected");
    /** @type {HTMLElement | null} */
    let previous = null;
    let sibling = tile.previousElementSibling;
    while (sibling) {
      if (
        sibling.classList.contains("guild-item") &&
        !sibling.classList.contains("guild-add")
      ) {
        previous = /** @type {HTMLElement} */ (sibling);
        break;
      }
      sibling = sibling.previousElementSibling;
    }
    tile.remove();
    if (!wasSelected) return;
    const remaining = /** @type {HTMLElement[]} */ ([
      ...document.querySelectorAll(".guild-item"),
    ]).filter((node) => !node.classList.contains("guild-add"));
    const next = previous ?? remaining[0] ?? null;
    if (!next) {
      const rail = byId("guild-rail");
      if (rail) rail.setAttribute("data-state", "empty");
      return;
    }
    for (const guild of remaining) {
      guild.classList.remove("is-selected");
      guild.removeAttribute("aria-current");
    }
    next.classList.add("is-selected");
    next.setAttribute("aria-current", "true");
    // G-01: заголовок дерева следует за выбранным сервером (после выхода — сосед).
    const heading = document.querySelector(".tree-guild");
    if (heading) heading.textContent = readGuildState(next).name;
  }

  function completeLeave() {
    const tile = leaveTarget.id ? findGuildTile(leaveTarget.id) : null;
    leaveOpen = false;
    leavePending = false;
    leaveDialog.setAttribute("hidden", "");
    leaveCard.setAttribute("data-state", "left");
    leaveCard.removeAttribute("aria-busy");
    if (leaveError) {
      leaveError.textContent = "";
      leaveError.setAttribute("hidden", "");
    }
    for (const button of leaveDialog.querySelectorAll("button")) button.removeAttribute("disabled");
    if (tile) removeGuildTile(tile);
    statusRegion.textContent = "Вы покинули сервер";
    closeMenu(false);
    focusNode(focusedGuildTile());
    leaveOpener = null;
  }

  /** @param {string} message */
  function showLeaveError(message) {
    leaveCard.setAttribute("data-state", "error");
    leaveCard.removeAttribute("aria-busy");
    setLeaveButtonsDisabled(false);
    if (leaveError) {
      leaveError.textContent = message;
      leaveError.removeAttribute("hidden");
    }
  }

  async function confirmLeave() {
    if (leavePending) return;
    leavePending = true;
    leaveCard.setAttribute("data-state", "pending");
    leaveCard.setAttribute("aria-busy", "true");
    setLeaveButtonsDisabled(true);
    try {
      if (!invoke) {
        // IPC-путь не передан: не выдаём «успех» и не трогаем рейл.
        throw new UIError("ACTION_FAILED", "leave_guild: нет IPC-пути");
      }
      await invoke("leave_guild", { guildId: leaveTarget.id, lurking: leaveTarget.lurking });
      completeLeave();
    } catch (error) {
      leavePending = false;
      showLeaveError(leaveFailureText(error));
    }
  }

  /**
   * Видимость панели участников: общий путь для `data-action=members` (VW-01) и
   * Ctrl+U (KEY-03), чтобы aria-expanded и display не расходились.
   * @param {HTMLElement | null} node
   */
  function toggleMemberPanel(node) {
    const panel = byId("member-panel");
    const expanded = (node?.getAttribute("aria-expanded") ?? "true") !== "false";
    node?.setAttribute("aria-expanded", expanded ? "false" : "true");
    if (panel) panel.style.display = expanded ? "none" : "";
  }

  /** @type {Record<string, (node: HTMLElement) => void>} */
  const handlers = {
    // G-03: «+» в рейле. Слушатель плитки прямой (создаётся после init),
    // здесь — защита от «неизвестного data-action», если разметка статична.
    "add-server": (node) => {
      join?.open(node);
    },
    "join-submit": () => {
      void join?.submit();
    },
    "enter-guild": () => {
      // G-03a: lurker-просмотр из preview; прямой слушатель кнопки ставит сам
      // join-server (узел создаётся после init), запись нужна для реестра §8.1.
      void join?.enterGuild();
    },
    "join-cancel": () => {
      join?.cancel(true);
    },
    "dialog-close": () => {
      // Закрытие «×» join-модалки; других модалок с этим action пока нет.
      join?.cancel(true);
    },
    "guild-menu": (node) => {
      // G-05: поповер с 10 пунктами, ролями и возвратом фокуса.
      if (menuOpen) closeMenu(true);
      else openMenu(node);
    },
    "guild-invite": () => {
      // C2: share-поток инвайтов ещё не реализован; по умолчанию пункт
      // aria-disabled (capability=false), поэтому сюда можно попасть только с
      // явной capability. Endpoint/превью не выдумываем — честное сообщение.
      statusRegion.textContent = "Приглашения на сервер недоступны в спайке";
      closeMenu(true);
    },
    "guild-copy-id": () => {
      const id = readGuildState(focusedGuildTile()).id;
      const clipboard = typeof navigator === "undefined"
        ? null
        : /** @type {any} */ (navigator).clipboard;
      // Clipboard — best-effort: в DOM-шиме API нет, токен не логируем.
      if (id && clipboard && typeof clipboard.writeText === "function") {
        void Promise.resolve(clipboard.writeText(id)).catch(() => {});
      }
      statusRegion.textContent = id ? "ID сервера скопирован" : "ID сервера неизвестен";
      closeMenu(true);
    },
    "guild-leave": (node) => {
      if (isEntryDisabled(node)) return;
      openLeaveDialog();
    },
    "leave-cancel": () => {
      if (!leavePending) closeLeave(true);
    },
    "leave-confirm": () => {
      void confirmLeave();
    },
    "toggle-tree": (node) => {
      const tree = byId("channel-tree");
      const expanded = node.getAttribute("aria-expanded") !== "false";
      node.setAttribute("aria-expanded", expanded ? "false" : "true");
      if (tree) tree.style.display = expanded ? "none" : "";
    },
    threads: (node) => {
      // Панель веток — Phase 3; aria-expanded показывает состояние-заглушку.
      node.setAttribute("aria-expanded", node.getAttribute("aria-expanded") === "true" ? "false" : "true");
    },
    inbox: () => {
      // Приёмник уведомлений — Phase 3: кнопка помечена aria-disabled (VW-01),
      // обработчик нужен, чтобы клик не уходил в ошибку «неизвестный data-action».
    },
    pins: (node) => {
      node.setAttribute("aria-expanded", node.getAttribute("aria-expanded") === "true" ? "false" : "true");
    },
    members: (node) => {
      // Панель участников уже в DOM чата; поповер-версия — Phase 3. Пока
      // переключаем видимость панели и aria-expanded (а не экран).
      toggleMemberPanel(node);
    },
    attach: () => {
      // Файл-диалог — Phase 3; кнопка видимо недоступна (aria-disabled + подсказка).
    },
    gift: () => {
      // Подарок — Phase 3: кнопка честно помечена aria-disabled + подпись (см. ниже),
      // обработчик нужен, чтобы клик не уходил в ошибку «неизвестный data-action».
    },
    emoji: (node) => {
      router.switchScreen("emoji");
      node.setAttribute("aria-expanded", "true");
    },
    "emoji-close": () => router.switchScreen("chat"),
    "all-members": () => router.switchScreen("members"),
    profile: (node) => {
      // Поповер профиля — Phase 3.
      node.setAttribute("aria-expanded", node.getAttribute("aria-expanded") === "true" ? "false" : "true");
    },
    mute: (node) => {
      node.setAttribute("aria-pressed", node.getAttribute("aria-pressed") === "true" ? "false" : "true");
    },
    deafen: (node) => {
      node.setAttribute("aria-pressed", node.getAttribute("aria-pressed") === "true" ? "false" : "true");
    },
    settings: () => router.switchScreen("settings"),
    "settings-close": () => router.switchScreen("chat"),
    "net-retry": () => {
      // L-01: только ручной retry; ошибка показывается баннером, без авто-цикла.
      void net?.retry();
    },
  };

  for (const actionNode of [...document.querySelectorAll("[data-action]")]) {
    const node = /** @type {HTMLElement} */ (actionNode);
    const action = node.getAttribute("data-action") ?? "";
    node.addEventListener("click", () => {
      const handler = handlers[action];
      if (!handler) {
        onError(reportError("ACTION_FAILED", new UIError("ACTION_FAILED", `неизвестный data-action="${action}"`)));
        return;
      }
      try {
        handler(node);
      } catch (error) {
        onError(reportError("ACTION_FAILED", asUIError("ACTION_FAILED", error)));
      }
    });
  }

  /* ---------- клавиатура/внешний клик поповера (G-05) ---------- */

  menu.addEventListener("keydown", (event) => {
    if (!menuOpen) return;
    const key = "key" in event ? String(event.key) : "";
    if (key === "Escape") {
      event.preventDefault();
      closeMenu(true);
      return;
    }
    if (key === "Enter" || key === " ") {
      const active = /** @type {HTMLElement | null} */ (/** @type {any} */ (document).activeElement);
      if (active && enabledEntries().includes(active)) {
        event.preventDefault();
        active.click();
      }
      return;
    }
    if (!["ArrowDown", "ArrowUp", "Home", "End"].includes(key)) return;
    const entries = enabledEntries();
    if (entries.length === 0) return;
    event.preventDefault();
    const active = /** @type {HTMLElement | null} */ (/** @type {any} */ (document).activeElement);
    const current = active ? entries.indexOf(active) : -1;
    /** @type {number} */
    let next = 0;
    if (key === "Home") next = 0;
    else if (key === "End") next = entries.length - 1;
    else if (key === "ArrowUp") next = current < 0 ? entries.length - 1 : (current - 1 + entries.length) % entries.length;
    else next = current < 0 ? 0 : (current + 1) % entries.length;
    focusNode(entries[next] ?? null);
  });

  document.body.addEventListener("click", (event) => {
    const target = event.target instanceof Element ? event.target : null;
    // Клик по пункту меню/опенеру обрабатывает сам пункт: не закрываем
    // диалог, который этот же клик открыл (событие всплывает до body).
    if ((target && isInside(menu, target)) || (menuOpener && target === menuOpener)) return;
    if (menuOpen) closeMenu(true);
    if (
      join?.isOpen
      && !join.busy
      && target !== join.opener
      && (target === null || !isInside(join.card, target))
    ) {
      join.cancel(true);
    }
    if (leaveOpen && !leavePending && (target === null || !isInside(leaveDialog, target))) {
      closeLeave(true);
    }
  });
  // Контракт «сейчас aria-disabled» для attach: не мёртвая кнопка, а честная
  // заглушка с подсказкой (CMP-11).
  const attach = [...document.querySelectorAll('[data-action="attach"]')][0];
  if (attach) {
    attach.setAttribute("aria-disabled", "true");
    attach.setAttribute("title", "Недоступно в спайке (Phase 3)");
  }
  // P1 (REVIEW-W39h): подарок — Phase 3, но клик не должен быть «мёртвым»:
  // видимый disabled-affordance с подписью, а не непонятная ошибка в логе.
  for (const gift of document.querySelectorAll('[data-action="gift"]')) {
    gift.setAttribute("aria-disabled", "true");
    gift.setAttribute("title", "Подарок (Phase 3): недоступно в спайке");
  }
  // CMP-12: эмодзи открывает диалог-пикер — контракт affordance, а не только клик.
  for (const emoji of document.querySelectorAll('[data-action="emoji"]')) {
    emoji.setAttribute("aria-haspopup", "dialog");
  }
  // G-05: кнопка «…» открывает поповер — affordance виден и без разметки html
  // (DOM-шим собирает дерево из buildDom, атрибуты живут здесь).
  for (const opener of document.querySelectorAll('[data-action="guild-menu"]')) {
    opener.setAttribute("aria-haspopup", "menu");
    if (opener.getAttribute("aria-expanded") === null) opener.setAttribute("aria-expanded", "false");
  }

  /* ---------- клавиатура списков ---------- */

  /**
   * @param {HTMLElement} container
   * @param {VirtualList} list
   */
  function bindListKeys(container, list) {
    container.addEventListener("keydown", (event) => {
      const key = "key" in event ? String(event.key) : "";
      const current = list.firstVisibleIndex();
      /** @type {number | null} */
      let target = null;
      if (key === "ArrowDown") target = current + 1;
      else if (key === "ArrowUp") target = current - 1;
      else if (key === "PageDown") target = current + 10;
      else if (key === "PageUp") target = current - 10;
      else if (key === "Home") target = 0;
      else if (key === "End") target = list.source.totalRows - 1;
      if (target === null) return;
      event.preventDefault();
      list.scrollToIndex(target);
    });
  }
  bindListKeys(chat.list.container, chat.list);
  bindListKeys(members.list.container, members.list);

  /* ---------- глобальные клавиши ---------- */

  /**
   * Видимый модальный auth-гейт (G2-04): живёт вне `.shell`, пока вход не
   * завершён. Пока он открыт, `.shell` помечен `aria-hidden`, поэтому фокус
   * нельзя отдавать наружу. Открытые оверлеи (поиск/switcher) владеют фокусом
   * сами — тогда гейт не перехватывает Tab.
   * @returns {HTMLElement | null}
   */
  const authGateLayer = () => {
    const gate = byId("auth-gate");
    if (!gate) return null;
    if (gate.getAttribute("data-state") === "ready") return null;
    if (gate.getAttribute("hidden") !== null) return null;
    if (gate.style.display === "none") return null;
    if (search?.open) return null;
    const switcher = byId("quick-switcher");
    if (switcher && switcher.getAttribute("hidden") === null) return null;
    return gate;
  };

  /** @returns {HTMLElement | null} */
  const topLayer = () => {
    // G-03: модалка join — верхний слой, Tab не уходит в рабочую область.
    if (join?.isOpen) return join.card;
    // G-04: confirm выхода модален — Tab не уходит в рабочую область.
    if (leaveOpen) return leaveCard;
    // Явно открытый экран остаётся верхним слоем (settings/emoji не ломаем).
    if (router.current === "settings") {
      return /** @type {HTMLElement | null} */ (document.querySelector(".settings-shell")) ?? authGateLayer();
    }
    if (router.current === "emoji") {
      return /** @type {HTMLElement | null} */ (document.querySelector(".emoji-card")) ?? authGateLayer();
    }
    // chat/members: модальный вход перекрывает рабочую область.
    return authGateLayer();
  };

  /**
   * @param {"chat" | "emoji" | "members" | "settings"} screen
   */
  function openScreen(screen) {
    router.switchScreen(screen);
    const layer = topLayer();
    if (!layer) return;
    const items = focusables(layer);
    focusNode(items[0] ?? null);
  }

  function closeTopLayer() {
    const current = router.current;
    router.switchScreen("chat");
    focusRail(current === "chat" ? "chat" : current);
  }

  /**
   * Один Tab-цикл внутри верхнего слоя (emoji/settings).
   * @param {Event} event
   */
  function trapTab(event) {
    const layer = topLayer();
    if (!layer) return;
    const items = focusables(layer);
    if (items.length === 0) return;
    const active = /** @type {HTMLElement | null} */ (
      /** @type {any} */ (document).activeElement ?? null
    );
    const index = active ? items.indexOf(active) : -1;
    const shift = "shiftKey" in event && Boolean(event.shiftKey);
    const next = index < 0
      ? (shift ? items.length - 1 : 0)
      : (index + (shift ? -1 : 1) + items.length) % items.length;
    event.preventDefault();
    focusNode(items[next] ?? null);
  }

  document.body.addEventListener("keydown", (event) => {
    const key = "key" in event ? String(event.key) : "";
    const withModifier = "ctrlKey" in event && "metaKey" in event
      ? Boolean(event.ctrlKey) || Boolean(event.metaKey)
      : false;
    const withAlt = "altKey" in event && Boolean(event.altKey);
    const withShift = "shiftKey" in event && Boolean(event.shiftKey);

    if (key === "Escape") {
      // KEY-09: Shift+Esc — сбросить плашку непрочитанного (иначе — обычный Esc).
      if (withShift) {
        const band = document.querySelector(".chat-unread-divider");
        // M-07: полоса реального канала (`read-state`) гаснет только успешным
        // ack — клавиша не подменяет read-state выдуманным «прочитано».
        const readStateBand = band?.getAttribute("data-source") === "read-state";
        if (band && !readStateBand && band.getAttribute("data-cleared") !== "true") {
          event.preventDefault();
          band.setAttribute("data-cleared", "true");
          band.setAttribute("aria-hidden", "true");
          /** @type {HTMLElement} */ (band).style.display = "none";
          return;
        }
      }
      // Порядок слоёв: меню сервера → confirm выхода → поиск → ответ в композере → экран.
      if (menuOpen) {
        event.preventDefault();
        closeMenu(true);
        return;
      }
      if (join?.isOpen) {
        event.preventDefault();
        // В pending/entering диалог не закрывается (§3.4): запрос уже в полёте.
        if (!join.busy) join.cancel(true);
        return;
      }
      if (leaveOpen) {
        event.preventDefault();
        // В pending диалог не закрывается (двойной submit/обрыв не теряет запрос).
        if (!leavePending) closeLeave(true);
        return;
      }
      if (search?.open) {
        event.preventDefault();
        search.close(true);
        return;
      }
      // В поле композера Esc обрабатывает сам композер (отмена reply/выход).
      if (composer && event.target === byId("composer-input")) {
        return;
      }
      // G2-04: у модального входа Esc не переключает экран и не отдаёт фокус
      // в `.shell[aria-hidden=true]`: остаёмся в гейте.
      if (topLayer() === authGateLayer()) {
        event.preventDefault();
        const gate = authGateLayer();
        if (gate && !isInside(gate, /** @type {any} */ (document).activeElement)) {
          focusNode(focusables(gate)[0] ?? null);
        }
        return;
      }
      event.preventDefault();
      closeTopLayer();
      return;
    }
    // В input/textarea глобальные шорткаты не перехватываем (кроме Escape выше).
    if (isEditable(event.target)) return;

    if (key === "Tab") {
      trapTab(event);
      return;
    }
    if (withAlt && ["1", "2", "3", "4"].includes(key)) {
      event.preventDefault();
      /** @type {Array<"chat" | "emoji" | "members" | "settings">} */
      const order = ["chat", "emoji", "members", "settings"];
      openScreen(order[Number(key) - 1] ?? "chat");
      return;
    }
    if (withModifier && (key === "e" || key === "E" || key === "у" || key === "У")) {
      event.preventDefault();
      if (router.current === "emoji") closeTopLayer();
      else openScreen("emoji");
      return;
    }
    if (withModifier && key === ",") {
      event.preventDefault();
      openScreen("settings");
      return;
    }
    if (withModifier && (key === "u" || key === "U" || key === "г" || key === "Г")) {
      // KEY-03: Ctrl+U — список участников (toggle), общий путь с data-action.
      event.preventDefault();
      const headerButton = /** @type {HTMLElement | null} */ (
        document.querySelector('[data-action="members"]')
      );
      toggleMemberPanel(headerButton);
      return;
    }
    if (withModifier && (key === "f" || key === "F" || key === "а" || key === "А")) {
      // KEY-03: Ctrl+F — поиск с префиллом канала, Ctrl+Shift+F — без префилла.
      event.preventDefault();
      if (search) {
        search.show(true);
        const inField = /** @type {any} */ (
          document.querySelector('.search-section[data-key="in"] .search-control')
        );
        const channel = document.querySelector(".chat-title-text");
        if (inField) inField.value = withShift ? "" : (channel?.textContent ?? "").trim();
        search.refresh();
      } else {
        focusNode(/** @type {HTMLElement | null} */ (byId("chat-search")));
      }
      return;
    }
    if (key === "/") {
      event.preventDefault();
      focusNode(/** @type {HTMLElement | null} */ (byId("chat-search")));
    }
  });

  /* ---------- подсветка выбора в списках (клик по строке) ---------- */

  /**
   * @param {VirtualList} list
   */
  function bindListSelection(list) {
    list.container.addEventListener("click", (event) => {
      const target = /** @type {HTMLElement | null} */ (
        event.target instanceof Element ? event.target : null
      );
      if (!target) return;
      // closest()/contains() есть в браузере, но не в DOM-шиме — без них клик
      // по строке просто не подсвечивается (в шиме события не всплывают).
      if (typeof target.closest !== "function" || typeof list.container.contains !== "function") return;
      const row = target.closest(".row, .member");
      if (!row || !list.container.contains(row)) return;
      for (const node of list.rows.values()) toggleClass(node, "is-selected", node === row);
    });
  }
  bindListSelection(chat.list);
  bindListSelection(members.list);
}
