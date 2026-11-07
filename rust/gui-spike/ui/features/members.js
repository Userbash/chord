// @ts-check
/**
 * Участники: экран (виртуализация, высота 42 по D-04) и панель чата
 * (`#member-panel-list`, простой цикл ≤50 строк, тот же шаблон §7.2).
 *
 * W39h/P3-02: 6 групп (MEM-02/04, D-13 — блок заголовка 42 `--row-member-group`):
 * 3 роли (по высшей роли) + 3 статуса. Группы живут в панели и в чипах экрана:
 * виртуализованный `#member-list` групп не содержит (§7.2/MEM-17, frozen), а
 * заголовки внутри sizer сломали бы инвариант `sizer.children.length === rows`.
 * Счётчики групп считаются по загруженному окну и честно помечены (<50 строк).
 */
import { createMemberHeights, memberHeight, PageStore } from "../model.js";
import { byId, formatNumber } from "./dom.js";
import { UIError } from "./errors.js";
import { bindListSearch } from "./search.js";
import { VirtualList } from "./virtual-list.js";

export const MEMBER_ROWS = 24_229;
export const MEMBER_PAGE_SIZE = 100;
export const MEMBER_MAX_PAGES = 5;
export const MEMBER_PANEL_ROWS = 50;
/** Сколько строк группы показываем в панели чата (6 групп × 8 = 48 ≤ 50). */
export const MEMBER_GROUP_ROWS = 8;

const MARKS = ["α", "β", "γ", "δ"];
const STATUSES = ["online", "idle", "dnd", "offline"];
/** Роли по позиции (MEM-04): 3 группы ролей, порядок высшая → низшая. */
export const ROLES = ["ADMIN", "MOD", "GUEST"];
/** @type {Record<string, string>} */
const ROLE_LABELS = { ADMIN: "ADMIN", MOD: "MOD", GUEST: "GUEST" };
/**
 * Подписи статусов — один источник для bench-строк и live-проекций dispatch
 * (`dispatch-model.js` берёт статус-лейбл отсюда, а не из второй таблицы).
 * @type {Record<string, string>}
 */
export const STATUS_LABELS = Object.freeze({
  online: "в сети",
  idle: "неактивен",
  dnd: "не беспокоить",
  offline: "не в сети",
});
const ACTIVITIES = ["играет в Dota 2", "читает канал", "слушает музыку", "пишет код"];

/**
 * 6 групп в порядке MEM-04: сначала роли (по позиции), затем статусы.
 * @type {ReadonlyArray<{ key: string, label: string, kind: "role" | "status", value: string | string[] }>}
 */
export const MEMBER_GROUPS = Object.freeze([
  { key: "role-admin", label: "АДМИНИСТРАЦИЯ", kind: "role", value: "ADMIN" },
  { key: "role-mod", label: "МОДЕРАТОРЫ", kind: "role", value: "MOD" },
  { key: "role-guest", label: "УЧАСТНИКИ", kind: "role", value: "GUEST" },
  { key: "status-online", label: "В СЕТИ", kind: "status", value: ["online"] },
  { key: "status-idle", label: "НЕАКТИВНЫ", kind: "status", value: ["idle", "dnd"] },
  { key: "status-offline", label: "НЕ В СЕТИ", kind: "status", value: ["offline"] },
]);

/**
 * @typedef {{
 *   name: string,
 *   status: string,
 *   statusLabel: string,
 *   role: string,
 *   badge: string,
 *   activity: string,
 *   hue: number,
 * }} Member
 */

/**
 * @param {number} index
 * @returns {Member}
 */
export function makeMember(index) {
  const status = STATUSES[index % STATUSES.length] ?? "offline";
  const role = ROLES[index % ROLES.length] ?? "GUEST";
  const online = status === "online" || status === "idle" || status === "dnd";
  return {
    name: `member_${index}_${MARKS[index % MARKS.length] ?? ""}`,
    status,
    statusLabel: STATUS_LABELS[status] ?? status,
    role,
    // MEM-11: в строке максимум один чип — высшая роль.
    badge: ROLE_LABELS[role] ?? role,
    activity: online ? (ACTIVITIES[index % ACTIVITIES.length] ?? "") : (STATUS_LABELS[status] ?? ""),
    hue: (index * 53) % 360,
  };
}

/**
 * @param {Member} member
 * @param {{ key: string, kind: "role" | "status", value: string | string[] }} group
 * @returns {boolean}
 */
export function inGroup(member, group) {
  if (group.kind === "role") return member.role === group.value;
  const statuses = Array.isArray(group.value) ? group.value : [group.value];
  return statuses.includes(member.status);
}

/**
 * @param {HTMLElement} row
 * @param {Member | null} item
 * @param {number} index
 * @param {{ total?: number, userId?: string, label?: string, role?: string }} [options]
 *   L-02-UI: аддитивные live-поля. Без аргумента bench-строка не меняется:
 *   `aria-setsize` остаётся `MEMBER_ROWS`, `data-user-id` не появляется.
 */
export function renderMemberRow(row, item, index, options = {}) {
  // Тот же контракт, что у renderChatRow: элемент переиспользуется, чистим детей/классы.
  row.className = "member";
  row.replaceChildren();
  if (item === null) {
    row.classList.add("placeholder");
    row.removeAttribute("role");
    row.removeAttribute("aria-label");
    return;
  }
  const rowRole = options.role ?? "listitem";
  row.setAttribute("role", rowRole);
  row.setAttribute("data-index", String(index));
  if (options.userId) row.setAttribute("data-user-id", options.userId);
  if (item.role) row.setAttribute("data-role", item.role);
  row.setAttribute("data-status", item.status);
  row.setAttribute("aria-setsize", String(options.total ?? MEMBER_ROWS));
  row.setAttribute("aria-posinset", String(index + 1));
  row.setAttribute("tabindex", "0");
  row.setAttribute("aria-haspopup", "dialog");
  row.setAttribute("aria-controls", "user-profile-popover");
  const popover = byId("user-profile-popover");
  const activeTrigger = popover ? /** @type {HTMLElement | null} */ (/** @type {any} */ (popover)._activeTrigger ?? null) : null;
  const isOpenOnThisRow = Boolean(
    popover && !popover.hasAttribute("hidden") && activeTrigger && (activeTrigger === row || isInside(row, activeTrigger)),
  );
  row.setAttribute("aria-expanded", isOpenOnThisRow ? "true" : "false");
  // MEM-18: статус не только цветом — он в доступном имени строки. Live-адаптер
  // передаёт готовый label, чтобы не получить хвостовую запятую и не дублировать
  // статус в activity (PLAN §4.3).
  row.setAttribute(
    "aria-label",
    options.label ?? `${item.name}, ${item.statusLabel}, ${item.badge || item.activity}`,
  );

  const avatar = document.createElement("span");
  avatar.className = "avatar avatar--md";
  avatar.setAttribute("aria-hidden", "true");
  avatar.style.setProperty("--avatar-hue", String(item.hue));
  avatar.textContent = item.name.slice(0, 1).toUpperCase();
  const dot = document.createElement("span");
  dot.className = "status-dot";
  dot.setAttribute("data-status", item.status);
  avatar.append(dot);

  const name = document.createElement("span");
  name.className = "member-name";
  const label = document.createElement("span");
  label.className = "member-label";
  label.setAttribute("role", "button");
  label.setAttribute("tabindex", "0");
  label.setAttribute("aria-haspopup", "dialog");
  label.setAttribute("aria-controls", "user-profile-popover");
  label.setAttribute("aria-expanded", isOpenOnThisRow ? "true" : "false");
  label.textContent = item.name;
  label.addEventListener("click", (event) => {
    if (/** @type {any} */ (event)._profileHandled) return;
    /** @type {any} */ (event)._profileHandled = true;
    const p = ensureMemberProfilePopover();
    const currentMember = /** @type {any} */ (row)._member ?? item;
    openMemberProfilePopover(p, currentMember, row);
  });
  label.addEventListener("keydown", (event) => {
    const key = "key" in event ? String(event.key) : "";
    if (key === "Enter" || key === " ") {
      event.preventDefault?.();
      if (/** @type {any} */ (event)._profileHandled) return;
      /** @type {any} */ (event)._profileHandled = true;
      const p = ensureMemberProfilePopover();
      const currentMember = /** @type {any} */ (row)._member ?? item;
      openMemberProfilePopover(p, currentMember, row);
    }
  });
  name.append(label);
  if (item.badge) {
    const badge = document.createElement("span");
    badge.className = "badge badge--role";
    badge.textContent = item.badge;
    name.append(badge);
  }

  const role = document.createElement("span");
  role.className = "member-role";
  role.textContent = item.activity;

  row.append(avatar, name, role);

  /** @type {any} */ (row)._member = item;
  if (!/** @type {any} */ (row)._hasProfileTrigger) {
    /** @type {any} */ (row)._hasProfileTrigger = true;
    row.addEventListener("click", (event) => {
      if (/** @type {any} */ (event)._profileHandled) return;
      /** @type {any} */ (event)._profileHandled = true;
      const p = ensureMemberProfilePopover();
      const currentMember = /** @type {any} */ (row)._member ?? item;
      openMemberProfilePopover(p, currentMember, row);
    });
    row.addEventListener("keydown", (event) => {
      const key = "key" in event ? String(event.key) : "";
      if (key === "Enter" || key === " ") {
        event.preventDefault?.();
        const p = ensureMemberProfilePopover();
        const currentMember = /** @type {any} */ (row)._member ?? item;
        openMemberProfilePopover(p, currentMember, row);
      }
    });
  }
}

/**
 * @param {{ fetchPage?: (page: number) => Member[] }} [options]
 * @returns {PageStore}
 */
export function createMemberStore(options = {}) {
  return new PageStore({
    totalRows: MEMBER_ROWS,
    pageSize: MEMBER_PAGE_SIZE,
    maxPages: MEMBER_MAX_PAGES,
    fetchPage: options.fetchPage ?? ((page) => {
      const start = page * MEMBER_PAGE_SIZE;
      const end = Math.min(MEMBER_ROWS, start + MEMBER_PAGE_SIZE);
      return Array.from({ length: Math.max(0, end - start) }, (_, offset) => makeMember(start + offset));
    }),
  });
}

/**
 * Счётчики 6 групп по загруженному окну (честно: не «по всему серверу»).
 * @param {PageStore} store
 * @returns {Map<string, number>}
 */
export function countGroups(store) {
  /** @type {Map<string, number>} */
  const counts = new Map(MEMBER_GROUPS.map((group) => [group.key, 0]));
  for (const page of store.pages.values()) {
    for (const row of page.rows) {
      const member = /** @type {Member} */ (row);
      for (const group of MEMBER_GROUPS) {
        if (inGroup(member, group)) counts.set(group.key, (counts.get(group.key) ?? 0) + 1);
      }
    }
  }
  return counts;
}

/**
 * Первый индекс загруженного окна, попадающий в группу.
 * @param {PageStore} store
 * @param {{ key: string, kind: "role" | "status", value: string | string[] }} group
 * @returns {number | null}
 */
export function firstInGroup(store, group) {
  /** @type {number | null} */
  let first = null;
  for (const [pageIndex, page] of store.pages) {
    for (let offset = 0; offset < page.rows.length; offset++) {
      const member = /** @type {Member} */ (page.rows[offset]);
      if (!inGroup(member, group)) continue;
      const index = pageIndex * store.pageSize + offset;
      if (first === null || index < first) first = index;
    }
  }
  return first;
}

/**
 * @param {{
 *   list: HTMLElement,
 *   search?: HTMLInputElement | null,
 *   countLabel?: HTMLElement | null,
 *   onError?: (error: UIError) => void,
 * }} options
 */
export function createMemberScreen(options) {
  const store = createMemberStore();
  const heights = createMemberHeights(MEMBER_ROWS);
  const list = new VirtualList({
    container: options.list,
    source: store,
    heights,
    render: renderMemberRow,
    onError: options.onError,
  });
  options.list.setAttribute("data-row-height", String(memberHeight()));
  const onError = options.onError ?? (() => {});

  options.list.addEventListener("click", (event) => {
    if (/** @type {any} */ (event)._profileHandled) return;
    let node = /** @type {any} */ (/** @type {any} */ (event).target ?? null);
    while (node && !(node instanceof Element && node.classList?.contains("member"))) {
      node = node.parentNode ?? null;
    }
    if (!node || !isInside(options.list, node)) return;
    const index = Number(node.getAttribute("data-index"));
    const member = store.getRow(index) ?? /** @type {any} */ (node)._member ?? null;
    const p = ensureMemberProfilePopover();
    openMemberProfilePopover(p, member, node);
  });

  /* ---------- чипы 6 групп (MEM-02/04, D-13) ---------- */
  const toolbar = /** @type {HTMLElement | null} */ (
    /** @type {any} */ (options.list).parentNode?.querySelector?.(".members-toolbar") ?? null
  );
  /** @type {HTMLElement[]} */
  const chips = [];
  const chipsBar = document.createElement("div");
  chipsBar.className = "member-groups";
  chipsBar.setAttribute("role", "toolbar");
  chipsBar.setAttribute("aria-label", "Группы участников");

  for (const [position, group] of MEMBER_GROUPS.entries()) {
    const chip = document.createElement("button");
    chip.className = "member-group-chip";
    chip.setAttribute("type", "button");
    chip.setAttribute("data-group-key", group.key);
    chip.setAttribute("aria-pressed", "false");
    // Roving tabindex: в табе одна кнопка, стрелки двигают фокус.
    chip.setAttribute("tabindex", position === 0 ? "0" : "-1");
    const label = document.createElement("span");
    label.className = "member-group-chip-label";
    label.textContent = group.label;
    const count = document.createElement("span");
    count.className = "member-group-chip-count";
    count.textContent = "0";
    chip.setAttribute("aria-label", `${group.label}: 0 в загруженном окне`);
    chip.append(label, count);
    chip.addEventListener("click", () => selectGroup(group.key));
    chips.push(chip);
    chipsBar.append(chip);
  }
  toolbar?.append(chipsBar);

  // Roving tabindex в тулбаре групп (стрелки/Home/End).
  chipsBar.addEventListener("keydown", (event) => {
    const key = "key" in event ? String(event.key) : "";
    const current = chips.findIndex((chip) => chip.getAttribute("tabindex") === "0");
    /** @type {number | null} */
    let next = null;
    if (key === "ArrowRight" || key === "ArrowDown") next = (current + 1) % chips.length;
    else if (key === "ArrowLeft" || key === "ArrowUp") next = (current - 1 + chips.length) % chips.length;
    else if (key === "Home") next = 0;
    else if (key === "End") next = chips.length - 1;
    if (next === null) return;
    const target = chips[next];
    if (!target) return;
    event.preventDefault?.();
    for (const chip of chips) chip.setAttribute("tabindex", chip === target ? "0" : "-1");
    /** @type {any} */ (target)?.focus?.();
  });

  /** Счётчики групп: пересчитываются по загруженному окну. */
  function refreshGroupChips() {
    const counts = countGroups(store);
    for (const chip of chips) {
      const key = chip.getAttribute("data-group-key") ?? "";
      const group = MEMBER_GROUPS.find((item) => item.key === key);
      const count = counts.get(key) ?? 0;
      const counter = chip.children[1];
      if (counter) counter.textContent = String(count);
      if (group) {
        chip.setAttribute("aria-label", `${group.label}: ${count} в загруженном окне`);
        chip.setAttribute("data-count", String(count));
      }
    }
  }

  /** @param {string} key */
  function selectGroup(key) {
    const group = MEMBER_GROUPS.find((item) => item.key === key);
    if (!group) {
      onError(new UIError("ACTION_FAILED", `неизвестная группа участников «${key}»`));
      return;
    }
    for (const chip of chips) {
      const pressed = chip.getAttribute("data-group-key") === key;
      chip.setAttribute("aria-pressed", pressed ? "true" : "false");
    }
    const index = firstInGroup(store, group);
    if (index === null) return;
    list.scrollToIndex(index);
    for (const node of list.rows.values()) {
      const match = Number(node.getAttribute("data-index")) === index;
      node.classList.toggle("is-selected", match);
      if (match) node.setAttribute("aria-current", "true");
      else node.removeAttribute("aria-current");
    }
  }

  if (options.search) {
    bindListSearch({
      input: options.search,
      list,
      store,
      pageSize: MEMBER_PAGE_SIZE,
      countLabel: options.countLabel ?? null,
      totalLabel: formatNumber(MEMBER_ROWS),
      predicate: (item, index, query) =>
        String(item?.name ?? "").toLowerCase().includes(query) ||
        String(item?.activity ?? "").toLowerCase().includes(query) ||
        String(index).includes(query),
      onResult: () => refreshGroupChips(),
      onError,
    });
  }
  return { store, list, chips, refreshGroupChips, selectGroup };
}

/** @typedef {ReturnType<typeof createMemberScreen>} MemberScreen */

/** Количество «в сети» в синтетических данных: 3 статуса из 4, кроме offline. */
export const MEMBER_ONLINE = MEMBER_ROWS - Math.floor(MEMBER_ROWS / 4);

/**
 * Панель участников в чате: 6 групп по ≤8 строк из загруженного окна (≤50 строк).
 * Отдельный список без PageStore-виртуализации (§6.4).
 * @param {{
 *   container: HTMLElement,
 *   store: PageStore,
 *   onlineCounter?: HTMLElement | null,
 *   onError?: (error: UIError) => void,
 * }} options
 * @returns {Promise<void>}
 */
export async function renderMemberPanel(options) {
  const { container, store } = options;
  const onError = options.onError ?? (() => {});
  try {
    if (options.onlineCounter) {
      options.onlineCounter.textContent = formatNumber(MEMBER_ONLINE);
      options.onlineCounter.setAttribute("aria-live", "polite");
    }
    const rows = Math.min(MEMBER_PANEL_ROWS, MEMBER_ROWS);
    await store.ensureRange(0, rows - 1);

    /** @type {HTMLElement[]} */
    const nodes = [];
    for (const group of MEMBER_GROUPS) {
      /** @type {HTMLElement[]} */
      const rowsNodes = [];
      let count = 0;
      for (let index = 0; index < rows; index++) {
        const item = /** @type {Member | null} */ (store.getRow(index) ?? null);
        if (!item || !inGroup(item, group)) continue;
        count += 1;
        if (rowsNodes.length >= MEMBER_GROUP_ROWS) continue;
        const row = document.createElement("div");
        row.className = "member";
        renderMemberRow(row, item, index, { role: "listitem" });
        // Шаблон §7.2 для панели: тот же `.member`, высота 42, пометка окна.
        row.setAttribute("data-window", "preview");
        row.setAttribute("data-group", group.key);
        row.setAttribute("role", "listitem");
        row.style.height = `${memberHeight()}px`;
        rowsNodes.push(row);
      }
      if (rowsNodes.length === 0) continue;
      const wrapper = document.createElement("div");
      wrapper.className = "member-group";
      wrapper.setAttribute("role", "none");
      wrapper.setAttribute("data-group-key", group.key);
      const title = document.createElement("h3");
      title.className = "member-group-title";
      // D-13: блок заголовка группы 42 (`--row-member-group`); число дублируем
      // атрибутом — CSS-каскад в DOM-шиме не считается.
      title.setAttribute("data-height", "42");
      title.setAttribute("aria-label", `${group.label}: ${count} в загруженном окне`);
      title.textContent = `${group.label} — ${count}`;
      wrapper.append(title, ...rowsNodes);
      nodes.push(wrapper);
    }
    container.replaceChildren(...nodes);

    // Выбор строки: aria-current дублирует визуальный selected (MEM-15).
    container.addEventListener("click", (event) => {
      let node = /** @type {any} */ (/** @type {any} */ (event).target ?? null);
      while (node && !(node instanceof Element && node.classList?.contains("member"))) {
        node = node.parentNode ?? null;
      }
      if (!node || !container.contains?.(node)) return;
      for (const row of container.querySelectorAll(".member")) {
        if (row === node) row.setAttribute("aria-current", "true");
        else row.removeAttribute("aria-current");
      }
    });
  } catch (error) {
    onError(error instanceof UIError ? error : new UIError("WINDOW_EMPTY", "панель участников", error));
  }
}

/**
 * @param {(error: UIError) => void} onError
 * @returns {MemberScreen}
 */
export function initMemberScreen(onError) {
  const list = byId("member-list");
  if (!list) throw new UIError("ELEMENT_MISSING", "в DOM нет #member-list");
  const search = /** @type {HTMLInputElement | null} */ (byId("member-search"));
  const count = byId("member-count");
  // Счётчик — единственный live-region экрана (FPS/p95 не озвучиваются).
  if (count) count.setAttribute("aria-live", "polite");
  const screen = createMemberScreen({ list, search, countLabel: count, onError });
  // Счётчики групп зависят от загруженного окна: первый расчёт — после paint.
  refreshWhenLoaded(screen);
  return screen;
}

/**
 * @param {MemberScreen} screen
 */
function refreshWhenLoaded(screen) {
  if (screen.store.pages.size > 0) {
    screen.refreshGroupChips();
    return;
  }
  // Стор пуст до первого ensureRange: пробуем на кадрах, не блокируя старт.
  let attempts = 0;
  const tick = () => {
    attempts += 1;
    if (screen.store.pages.size > 0 || attempts > 60) {
      screen.refreshGroupChips();
      return;
    }
    requestAnimationFrame(tick);
  };
  requestAnimationFrame(tick);
}

/* =============================================================================
 * MB-03 (STUB-16): поповер профиля участника (#user-profile-popover)
 * SPEC: waves/RW-40/DESIGN/PINS-THREADS-PROFILE-SPEC.md §4, §5, §6
 * ========================================================================== */

/**
 * Лежит ли узел внутри контейнера (в шиме нет Node.contains).
 * @param {HTMLElement | null} container
 * @param {Element | null} node
 * @returns {boolean}
 */
export function isInside(container, node) {
  if (!container || !node) return false;
  /** @type {any} */
  let current = node;
  while (current) {
    if (current === container) return true;
    current = current.parentNode ?? null;
  }
  return false;
}

/**
 * Создаёт кнопку закрытия поповера.
 * @returns {HTMLElement}
 */
function createProfileCloseButton() {
  const btn = document.createElement("button");
  btn.className = "icon-btn profile-close";
  btn.setAttribute("type", "button");
  btn.setAttribute("data-action", "close-profile");
  btn.setAttribute("aria-label", "Закрыть");
  btn.textContent = "✕";
  btn.addEventListener("click", (event) => {
    event.preventDefault?.();
    closeMemberProfilePopover();
  });
  return btn;
}

/**
 * Позиционирует поповер относительно строки участника.
 * @param {HTMLElement} popover
 * @param {HTMLElement} trigger
 */
function positionProfilePopover(popover, trigger) {
  if (typeof trigger.getBoundingClientRect === "function") {
    const rect = trigger.getBoundingClientRect();
    if (rect && (rect.top !== 0 || rect.left !== 0 || rect.right !== 0)) {
      const popoverWidth = 300;
      let left = rect.left - popoverWidth - 8;
      if (left < 8) {
        left = rect.right + 8;
      }
      let top = rect.top;
      const windowHeight = typeof window !== "undefined" ? window.innerHeight : 600;
      if (top + 400 > windowHeight) {
        top = Math.max(8, windowHeight - 408);
      }
      popover.style.position = "fixed";
      popover.style.left = `${Math.round(left)}px`;
      popover.style.top = `${Math.round(top)}px`;
    }
  }
}

/**
 * Подписывается на клики по каналам и экранам для закрытия поповера.
 */
function bindExternalDismissHandlers() {
  if (typeof document === "undefined") return;

  for (const item of /** @type {HTMLElement[]} */ ([...document.querySelectorAll(".channel-item")])) {
    if (!/** @type {any} */ (item)._profileDismissBound) {
      /** @type {any} */ (item)._profileDismissBound = true;
      item.addEventListener("click", () => {
        closeMemberProfilePopover({ restoreFocus: false });
      });
    }
  }

  for (const btn of /** @type {HTMLElement[]} */ ([...document.querySelectorAll(".rail-btn")])) {
    if (!/** @type {any} */ (btn)._profileDismissBound) {
      /** @type {any} */ (btn)._profileDismissBound = true;
      btn.addEventListener("click", () => {
        closeMemberProfilePopover({ restoreFocus: false });
      });
    }
  }

  const tree = document.getElementById("channel-tree") || document.querySelector(".tree");
  if (tree && !/** @type {any} */ (tree)._profileDismissBound) {
    /** @type {any} */ (tree)._profileDismissBound = true;
    tree.addEventListener("click", (event) => {
      let target = /** @type {any} */ (event.target ?? null);
      while (target && target !== tree) {
        if (target.classList?.contains("channel-item")) {
          closeMemberProfilePopover({ restoreFocus: false });
          break;
        }
        target = target.parentNode ?? null;
      }
    });
  }

  const chatHeader = document.getElementById("chat-header");
  if (chatHeader && !/** @type {any} */ (chatHeader)._profileDismissBound) {
    /** @type {any} */ (chatHeader)._profileDismissBound = true;
    const origAttr = chatHeader.setAttribute.bind(chatHeader);
    chatHeader.setAttribute = function(name, value) {
      origAttr(name, value);
      if (name === "data-channel-id") {
        closeMemberProfilePopover({ restoreFocus: false });
      }
    };
  }
}

/**
 * Инициализирует обработчики событий для поповера (Escape, outside click, close btn).
 * @param {HTMLElement} popover
 */
function setupPopoverEvents(popover) {
  popover.addEventListener("keydown", (event) => {
    const key = "key" in event ? String(event.key) : "";
    if (key === "Escape") {
      event.preventDefault?.();
      event.stopPropagation?.();
      closeMemberProfilePopover();
    }
  });

  popover.addEventListener("click", (event) => {
    let node = /** @type {any} */ (/** @type {any} */ (event).target ?? null);
    while (node && node !== popover) {
      const action = typeof node.getAttribute === "function" ? node.getAttribute("data-action") : "";
      if (action === "close-profile" || (node.classList?.contains && node.classList.contains("profile-close"))) {
        event.preventDefault?.();
        closeMemberProfilePopover();
        return;
      }
      node = node.parentNode ?? null;
    }
  });

  if (document.body && !/** @type {any} */ (document.body)._profileGlobalBound) {
    /** @type {any} */ (document.body)._profileGlobalBound = true;

    if (typeof MutationObserver !== "undefined") {
      const observer = new MutationObserver((mutations) => {
        for (const m of mutations) {
          if (m.attributeName === "data-active-screen") {
            closeMemberProfilePopover({ restoreFocus: false });
          }
        }
      });
      observer.observe(document.body, { attributes: true, attributeFilter: ["data-active-screen"] });
    }

    const origBodySetAttribute = document.body.setAttribute.bind(document.body);
    document.body.setAttribute = function(name, value) {
      origBodySetAttribute(name, value);
      if (name === "data-active-screen") {
        closeMemberProfilePopover({ restoreFocus: false });
      }
    };

    document.body.addEventListener("keydown", (event) => {
      const key = "key" in event ? String(event.key) : "";
      if (key === "Escape") {
        const p = byId("user-profile-popover");
        if (p && !p.hasAttribute("hidden")) {
          closeMemberProfilePopover();
        }
      }
    });

    document.body.addEventListener("click", (event) => {
      const p = byId("user-profile-popover");
      if (!p || p.hasAttribute("hidden")) return;
      const target = /** @type {Element | null} */ (/** @type {any} */ (event).target ?? null);
      if (!target) return;
      if (isInside(p, target)) return;
      const activeTrigger = /** @type {HTMLElement | null} */ (/** @type {any} */ (p)._activeTrigger ?? null);
      if (activeTrigger && (activeTrigger === target || isInside(activeTrigger, target))) return;
      closeMemberProfilePopover();
    });
  }

  bindExternalDismissHandlers();
}

/**
 * Прогрессивное создание DOM-узла `#user-profile-popover`.
 * @returns {HTMLElement}
 */
export function ensureMemberProfilePopover() {
  const existing = byId("user-profile-popover");
  if (existing) return existing;

  const popover = document.createElement("aside");
  popover.id = "user-profile-popover";
  popover.className = "user-profile-popover";
  popover.setAttribute("role", "dialog");
  popover.setAttribute("aria-label", "Профиль пользователя");
  popover.setAttribute("aria-modal", "false");
  popover.setAttribute("data-state", "empty");
  popover.setAttribute("tabindex", "-1");
  popover.setAttribute("hidden", "");

  if (document.body) {
    document.body.append(popover);
  }
  setupPopoverEvents(popover);
  return popover;
}

/**
 * Отрисовывает состояние поповера: loading | empty | error | populated.
 * @param {HTMLElement} popover
 * @param {"empty" | "loading" | "error" | "populated"} state
 * @param {any} [data]
 */
export function renderMemberProfileState(popover, state, data) {
  popover.setAttribute("data-state", state);

  if (state === "loading") {
    popover.setAttribute("aria-busy", "true");
    popover.replaceChildren();

    const banner = document.createElement("div");
    banner.className = "profile-banner";

    const closeBtn = createProfileCloseButton();

    const avatarWrap = document.createElement("div");
    avatarWrap.className = "profile-avatar-wrap";

    const avatarSkeleton = document.createElement("div");
    avatarSkeleton.className = "avatar avatar--xl profile-avatar profile-avatar-skeleton";
    avatarSkeleton.setAttribute("aria-hidden", "true");
    avatarWrap.append(avatarSkeleton);

    const body = document.createElement("div");
    body.className = "profile-body";

    const skeletonName1 = document.createElement("div");
    skeletonName1.className = "profile-skeleton-name";
    skeletonName1.setAttribute("aria-hidden", "true");

    const skeletonName2 = document.createElement("div");
    skeletonName2.className = "profile-skeleton-name profile-skeleton-name--sub";
    skeletonName2.setAttribute("aria-hidden", "true");

    body.append(skeletonName1, skeletonName2);

    const loadingWrap = document.createElement("div");
    loadingWrap.className = "profile-loading";
    loadingWrap.setAttribute("role", "status");
    loadingWrap.setAttribute("aria-live", "polite");

    const srText = document.createElement("span");
    srText.className = "sr-only";
    srText.textContent = "Загрузка данных профиля…";
    loadingWrap.append(srText);

    popover.append(banner, closeBtn, avatarWrap, body, loadingWrap);
    return;
  }

  if (state === "empty") {
    popover.setAttribute("aria-busy", "false");
    popover.replaceChildren();

    const banner = document.createElement("div");
    banner.className = "profile-banner";

    const closeBtn = createProfileCloseButton();

    const emptyWrap = document.createElement("div");
    emptyWrap.className = "dropdown-empty-state profile-empty";
    emptyWrap.setAttribute("role", "status");

    const icon = document.createElement("span");
    icon.className = "empty-icon";
    icon.setAttribute("aria-hidden", "true");
    icon.textContent = "👤";

    const title = document.createElement("p");
    title.className = "empty-title";
    title.textContent = "Пользователь не найден или покинул сервер";

    emptyWrap.append(icon, title);
    popover.append(banner, closeBtn, emptyWrap);
    return;
  }

  if (state === "error") {
    popover.setAttribute("aria-busy", "false");
    popover.replaceChildren();

    const banner = document.createElement("div");
    banner.className = "profile-banner";

    const closeBtn = createProfileCloseButton();

    const errorWrap = document.createElement("div");
    errorWrap.className = "dropdown-error-state profile-error";
    errorWrap.setAttribute("role", "alert");

    const errorText = document.createElement("p");
    errorText.className = "error-text";
    errorText.textContent = (data && data.message) ? String(data.message) : "Не удалось загрузить данные профиля";

    const retryBtn = document.createElement("button");
    retryBtn.className = "btn btn-secondary btn-retry";
    retryBtn.setAttribute("type", "button");
    retryBtn.setAttribute("data-action", "retry-profile");
    retryBtn.textContent = "Повторить попытку";
    retryBtn.addEventListener("click", () => {
      const trigger = /** @type {HTMLElement | null} */ (/** @type {any} */ (popover)._activeTrigger ?? null);
      if (trigger) {
        const member = /** @type {any} */ (trigger)._member ?? null;
        openMemberProfilePopover(popover, member, trigger);
      }
    });

    errorWrap.append(errorText, retryBtn);
    popover.append(banner, closeBtn, errorWrap);
    return;
  }

  if (state === "populated") {
    popover.setAttribute("aria-busy", "false");
    popover.replaceChildren();

    const member = /** @type {Member | null} */ (data ?? null);
    const hue = member?.hue ?? 212;

    const banner = document.createElement("div");
    banner.className = "profile-banner";
    banner.style.setProperty("background-color", `hsl(${hue} 45% 25%)`);

    const closeBtn = createProfileCloseButton();

    const avatarWrap = document.createElement("div");
    avatarWrap.className = "profile-avatar-wrap";

    const avatar = document.createElement("div");
    avatar.className = "avatar avatar--xl profile-avatar";
    avatar.style.setProperty("--avatar-hue", String(hue));

    const monogram = document.createElement("span");
    monogram.className = "avatar-monogram";
    const rawName = String(member?.name ?? "U");
    monogram.textContent = rawName.slice(0, 2).toUpperCase();

    const statusDot = document.createElement("span");
    statusDot.className = "status-dot status-dot--lg";
    const status = member?.status ?? "offline";
    statusDot.setAttribute("data-status", status);
    statusDot.setAttribute("aria-label", member?.statusLabel ?? (STATUS_LABELS[status] ?? status));

    avatar.append(monogram, statusDot);
    avatarWrap.append(avatar);

    const body = document.createElement("div");
    body.className = "profile-body";

    const headerInfo = document.createElement("div");
    headerInfo.className = "profile-header-info";

    const nameEl = document.createElement("h3");
    nameEl.className = "profile-display-name";
    nameEl.textContent = member?.name ?? "Участник";

    const tagEl = document.createElement("div");
    tagEl.className = "profile-tag";
    tagEl.textContent = `${member?.name ?? "user"}#0`;

    const statusEl = document.createElement("div");
    statusEl.className = "profile-custom-status";
    const activityText = member?.activity || member?.statusLabel || "";
    statusEl.setAttribute("data-has-status", activityText ? "true" : "false");
    statusEl.textContent = activityText;

    headerInfo.append(nameEl, tagEl, statusEl);

    const divider1 = document.createElement("div");
    divider1.className = "profile-divider";

    const rolesSection = document.createElement("div");
    rolesSection.className = "profile-section profile-section-roles";

    const rolesTitle = document.createElement("h4");
    rolesTitle.className = "profile-section-title";
    rolesTitle.textContent = "РОЛИ";

    const badgesWrap = document.createElement("div");
    badgesWrap.className = "profile-badges";
    badgesWrap.setAttribute("role", "list");

    const roleName = member?.badge || member?.role || "GUEST";
    const badge = document.createElement("span");
    badge.className = "badge badge--role";
    badge.setAttribute("role", "listitem");
    badge.textContent = roleName;
    badgesWrap.append(badge);

    rolesSection.append(rolesTitle, badgesWrap);

    const divider2 = document.createElement("div");
    divider2.className = "profile-divider";

    const actionsWrap = document.createElement("div");
    actionsWrap.className = "profile-actions";

    const dmBtn = document.createElement("button");
    dmBtn.className = "btn btn-secondary btn-sm";
    dmBtn.setAttribute("type", "button");
    dmBtn.setAttribute("data-action", "dm-user");
    dmBtn.textContent = "Написать сообщение";

    const mentionBtn = document.createElement("button");
    mentionBtn.className = "btn btn-ghost btn-sm";
    mentionBtn.setAttribute("type", "button");
    mentionBtn.setAttribute("data-action", "mention-user");
    mentionBtn.textContent = "Упомянуть";

    actionsWrap.append(dmBtn, mentionBtn);

    body.append(headerInfo, divider1, rolesSection, divider2, actionsWrap);
    popover.append(banner, closeBtn, avatarWrap, body);
  }
}

/**
 * Синхронизирует aria-expanded и атрибуты триггера на строке .member и кнопке внутри неё.
 * @param {HTMLElement} trigger
 * @param {boolean} expanded
 */
function syncTriggerAria(trigger, expanded) {
  const val = expanded ? "true" : "false";
  /** @type {HTMLElement[]} */
  const targets = [trigger];
  if (typeof trigger.querySelector === "function") {
    const innerBtn = /** @type {HTMLElement | null} */ (trigger.querySelector('[role="button"]'));
    if (innerBtn && !targets.includes(innerBtn)) targets.push(innerBtn);
  }
  /** @type {any} */
  let cur = trigger.parentNode ?? null;
  while (cur) {
    if (cur.classList?.contains?.("member")) {
      if (!targets.includes(cur)) targets.push(cur);
      break;
    }
    cur = cur.parentNode ?? null;
  }
  for (const el of targets) {
    el.setAttribute("aria-haspopup", "dialog");
    el.setAttribute("aria-controls", "user-profile-popover");
    el.setAttribute("aria-expanded", val);
  }
}

/**
 * Открывает поповер профиля участника, привязанный к элементу-триггеру.
 * @param {HTMLElement} popover
 * @param {Member | null} member
 * @param {HTMLElement} trigger
 * @param {{ error?: string }} [options]
 */
export function openMemberProfilePopover(popover, member, trigger, options = {}) {
  const currentTrigger = /** @type {HTMLElement | null} */ (/** @type {any} */ (popover)._activeTrigger ?? null);
  if (currentTrigger === trigger && !popover.hasAttribute("hidden")) {
    closeMemberProfilePopover();
    return;
  }

  if (currentTrigger && currentTrigger !== trigger) {
    syncTriggerAria(currentTrigger, false);
  }

  /** @type {any} */ (popover)._activeTrigger = trigger;
  syncTriggerAria(trigger, true);

  if (options.error) {
    renderMemberProfileState(popover, "error", { message: options.error });
  } else if (!member) {
    renderMemberProfileState(popover, "empty");
  } else {
    renderMemberProfileState(popover, "populated", member);
  }

  bindExternalDismissHandlers();
  popover.removeAttribute("hidden");
  positionProfilePopover(popover, trigger);
  popover.focus();
}

/**
 * Закрывает поповер профиля участника и возвращает фокус на триггер.
 * @param {{ restoreFocus?: boolean }} [options]
 */
export function closeMemberProfilePopover(options = {}) {
  const popover = byId("user-profile-popover");
  if (!popover || popover.hasAttribute("hidden")) return;
  popover.setAttribute("hidden", "");

  const trigger = /** @type {HTMLElement | null} */ (/** @type {any} */ (popover)._activeTrigger ?? null);
  if (trigger) {
    syncTriggerAria(trigger, false);
    /** @type {any} */ (popover)._activeTrigger = null;
    if (options.restoreFocus !== false && typeof trigger.focus === "function") {
      trigger.focus();
    }
  }
}

/* =============================================================================
 * OP 37 Guild Subscriptions & Member Grouping (task-155 / FE-P1-03)
 * ========================================================================== */

/**
 * Вычисляет диапазоны окон подписки OP 37 для канала при вертикальной прокрутке.
 *
 * Инварианты:
 * 1. Всегда включает первый чанк [0, 99].
 * 2. Вычисляет видимые 100-элементные чанки [start, start + 99] (кратные 100).
 * 3. Дедуплицирует совпадающие окна (чанк 0 не дублируется).
 * 4. Максимум 3 чанка на канал (в каноническом возрастающем порядке).
 *
 * @param {number} scrollTop смещение скролла в пикселях
 * @param {number} viewportHeight высота видимой области в пикселях
 * @param {number} [rowHeight=44] высота строки участника в пикселях
 * @returns {Array<[number, number]>}
 */
export function computeSubscriptionRanges(scrollTop, viewportHeight, rowHeight = 44) {
  const safeRowHeight = Math.max(1, rowHeight);
  const safeTop = Math.max(0, scrollTop || 0);
  const safeHeight = Math.max(0, viewportHeight || 0);
  const bottom = safeTop + safeHeight;

  const firstRow = Math.floor(safeTop / safeRowHeight);
  const lastRow = safeHeight > 0
    ? Math.max(firstRow, Math.floor(Math.max(0, bottom - 1) / safeRowHeight))
    : firstRow;

  const firstChunk = Math.floor(firstRow / 100);
  const lastChunk = Math.floor(lastRow / 100);

  /** @type {Set<number>} */
  const chunkIndices = new Set([0]);
  for (let c = firstChunk; c <= lastChunk; c++) {
    if (c >= 0) chunkIndices.add(c);
  }

  const sorted = Array.from(chunkIndices)
    .sort((a, b) => a - b)
    .slice(0, 3);

  return sorted.map((c) => /** @type {[number, number]} */ ([c * 100, c * 100 + 99]));
}

/**
 * @typedef {{
 *   subscriptions: Record<string, {
 *     typing: boolean,
 *     threads: boolean,
 *     activities: boolean,
 *     channels: Record<string, Array<[number, number]>>,
 *   }>
 * }} Op37SubscriptionPayload
 */

/**
 * @param {Array<[number, number]>} a
 * @param {Array<[number, number]>} b
 * @returns {boolean}
 */
function areRangesEqual(a, b) {
  if (a === b) return true;
  if (!a || !b || a.length !== b.length) return false;
  for (let i = 0; i < a.length; i++) {
    const rangeA = a[i];
    const rangeB = b[i];
    if (!rangeA || !rangeB || rangeA[0] !== rangeB[0] || rangeA[1] !== rangeB[1]) return false;
  }
  return true;
}

/**
 * Нормализует цвет роли в #rrggbb или hex строку.
 * @param {string | number | null | undefined} color
 * @returns {string}
 */
function formatRoleColor(color) {
  if (color === null || color === undefined || color === "") return "";
  if (typeof color === "string") {
    if (color.startsWith("#")) return color.toLowerCase();
    const num = Number(color);
    if (!Number.isNaN(num) && num > 0) {
      return `#${num.toString(16).padStart(6, "0").toLowerCase()}`;
    }
    return color;
  }
  if (typeof color === "number" && color > 0) {
    return `#${color.toString(16).padStart(6, "0").toLowerCase()}`;
  }
  return "";
}

/**
 * Трекер подписок гильдии (OP 37).
 *
 * Отслеживает смену канала и прокрутку #members-sidebar, отправляя структуру
 * OP 37 { subscriptions: { [guildId]: { typing: true, threads: true, activities: true, channels: { [channelId]: ranges } } } }
 * и предотвращает спам повторными вызовами при неизменных диапазонах.
 *
 * @param {{
 *   onSubscribe?: (payload: Op37SubscriptionPayload) => void,
 *   debounceMs?: number,
 *   sidebar?: HTMLElement | null,
 *   guildId?: string,
 *   channelId?: string,
 *   rowHeight?: number,
 * }} [options]
 */
export function createGuildSubscriptionsTracker(options = {}) {
  const onSubscribe = options.onSubscribe ?? (() => {});
  const debounceMs = typeof options.debounceMs === "number" ? Math.max(0, options.debounceMs) : 50;
  const rowHeight = options.rowHeight ?? 44;

  let currentGuildId = options.guildId ? String(options.guildId).trim() : "";
  let currentChannelId = options.channelId ? String(options.channelId).trim() : "";

  /** @type {HTMLElement | null} */
  let sidebarEl = options.sidebar ?? (
    typeof document !== "undefined"
      ? (byId("members-sidebar") || (document.querySelector ? document.querySelector("#members-sidebar") : null))
      : null
  );

  /** @type {Array<[number, number]>} */
  let lastRanges = [];
  let lastSentGuildId = "";
  let lastSentChannelId = "";
  /** @type {Op37SubscriptionPayload | null} */
  let lastPayload = null;

  /** @type {ReturnType<typeof setTimeout> | null} */
  let debounceTimer = null;

  /**
   * Вычисляет текущие диапазоны на основе скролла сайдбара.
   * @returns {Array<[number, number]>}
   */
  function getCurrentRanges() {
    const scrollTop = sidebarEl ? Number(sidebarEl.scrollTop || 0) : 0;
    const clientHeight = sidebarEl ? Number(sidebarEl.clientHeight || 0) : 600;
    return computeSubscriptionRanges(scrollTop, clientHeight, rowHeight);
  }

  /**
   * Выполняет отправку подписки, если параметры изменились.
   * @param {boolean} [force=false]
   */
  function syncSubscription(force = false) {
    if (debounceTimer !== null) {
      clearTimeout(debounceTimer);
      debounceTimer = null;
    }

    if (!currentGuildId || !currentChannelId) return;

    const ranges = getCurrentRanges();

    const isSameChannel = currentGuildId === lastSentGuildId && currentChannelId === lastSentChannelId;
    if (!force && isSameChannel && areRangesEqual(lastRanges, ranges)) {
      return; // Дедупликация: не спамим при неизменных диапазонах
    }

    lastSentGuildId = currentGuildId;
    lastSentChannelId = currentChannelId;
    lastRanges = ranges;

    /** @type {Op37SubscriptionPayload} */
    const payload = {
      subscriptions: {
        [currentGuildId]: {
          typing: true,
          threads: true,
          activities: true,
          channels: {
            [currentChannelId]: ranges,
          },
        },
      },
    };

    lastPayload = payload;
    onSubscribe(payload);
  }

  function handleScroll() {
    if (debounceMs <= 0) {
      syncSubscription(false);
      return;
    }
    if (debounceTimer !== null) clearTimeout(debounceTimer);
    debounceTimer = setTimeout(() => {
      debounceTimer = null;
      syncSubscription(false);
    }, debounceMs);
  }

  /**
   * Привязать DOM-узел сайдбара.
   * @param {HTMLElement | null} element
   */
  function attachSidebar(element) {
    if (sidebarEl && typeof sidebarEl.removeEventListener === "function") {
      sidebarEl.removeEventListener("scroll", handleScroll);
    }
    sidebarEl = element;
    if (sidebarEl && typeof sidebarEl.addEventListener === "function") {
      sidebarEl.addEventListener("scroll", handleScroll);
    }
  }

  // Если сайдбар уже есть, привязываем слушатель
  if (sidebarEl && typeof sidebarEl.addEventListener === "function") {
    sidebarEl.addEventListener("scroll", handleScroll);
  }

  /**
   * Переключить гильдию и канал.
   * @param {string} guildId
   * @param {string} channelId
   */
  function switchChannel(guildId, channelId) {
    currentGuildId = String(guildId ?? "").trim();
    currentChannelId = String(channelId ?? "").trim();
    syncSubscription(false);
  }

  function destroy() {
    if (debounceTimer !== null) {
      clearTimeout(debounceTimer);
      debounceTimer = null;
    }
    if (sidebarEl && typeof sidebarEl.removeEventListener === "function") {
      sidebarEl.removeEventListener("scroll", handleScroll);
    }
  }

  return {
    switchChannel,
    setChannel: switchChannel,
    attachSidebar,
    attach: attachSidebar,
    getCurrentRanges,
    getLastPayload: () => lastPayload,
    flush: () => syncSubscription(false),
    destroy,
    dispose: destroy,
  };
}

/**
 * @typedef {{
 *   userId?: string,
 *   id?: string,
 *   name?: string,
 *   username?: string,
 *   status?: string,
 *   statusLabel?: string,
 *   role?: string,
 *   roles?: string[],
 *   roleColor?: string | number,
 *   color?: string | number,
 *   badge?: string,
 *   activity?: string,
 *   hue?: number,
 *   user?: { id?: string, username?: string, global_name?: string },
 * }} MemberLike
 */

/**
 * @typedef {{
 *   id: string,
 *   key: string,
 *   name: string,
 *   count: number,
 *   roleColor?: string,
 *   color?: string,
 *   members: MemberLike[],
 * }} MemberGroupResult
 */

/**
 * Группирует участников по hoisted-ролям или статусам.
 *
 * Поддерживает:
 * - hoisted-группы из GUILD_MEMBER_LIST_UPDATE (groups: [{ id, name, count }])
 * - цвета ролей (roleColor в числовом или hex формате)
 * - сохранение обратной совместимости со стандартными группами статусов (online/idle/dnd/offline).
 *
 * @param {MemberLike[]} members список участников
 * @param {{
 *   groups?: Array<{ id: string, name?: string, count?: number, roleColor?: string | number, color?: string | number }>,
 *   roles?: Record<string, { id?: string, name?: string, color?: string | number, position?: number }> | Array<{ id: string, name: string, color?: string | number }>,
 *   statusLabels?: Record<string, string>,
 * }} [options]
 * @returns {MemberGroupResult[]}
 */
export function groupMembersByRoleOrStatus(members, options = {}) {
  const membersList = Array.isArray(members) ? members : [];
  const statusLabels = options.statusLabels ?? STATUS_LABELS;

  /** @type {Map<string, { id: string, name?: string, color?: string | number }>} */
  const rolesMap = new Map();
  if (Array.isArray(options.roles)) {
    for (const r of options.roles) {
      if (r && r.id) rolesMap.set(String(r.id), r);
    }
  } else if (options.roles && typeof options.roles === "object") {
    for (const [id, r] of Object.entries(options.roles)) {
      if (r) rolesMap.set(id, { id, name: r.name, color: r.color });
    }
  }

  // 1. Если переданы hoisted-группы из GUILD_MEMBER_LIST_UPDATE
  if (Array.isArray(options.groups) && options.groups.length > 0) {
    /** @type {MemberGroupResult[]} */
    const results = options.groups.map((g) => {
      const id = String(g.id);
      const roleMeta = rolesMap.get(id);
      const roleColor = formatRoleColor(g.roleColor ?? g.color ?? roleMeta?.color);
      const rawCandidate = g.name
        ?? roleMeta?.name
        ?? (statusLabels[id] ? statusLabels[id].toUpperCase() : (id === "online" ? "В СЕТИ" : id === "offline" ? "НЕ В СЕТИ" : id));
      const matchedWithRole = /** @type {any} */ (
        membersList.find(
          (m) =>
            Array.isArray(m?.roles) &&
            m.roles.map(String).includes(id) &&
            Boolean(/** @type {any} */ (m)?.roleName),
        )
      );
      const name = /^\d{15,22}$/.test(String(rawCandidate))
        ? String(matchedWithRole?.roleName || "УЧАСТНИКИ")
        : rawCandidate;
      return {
        id,
        key: id,
        name,
        count: typeof g.count === "number" ? g.count : 0,
        roleColor: roleColor || undefined,
        color: roleColor || undefined,
        members: /** @type {MemberLike[]} */ ([]),
      };
    });

    /** @type {Set<string | MemberLike>} */
    const assigned = new Set();

    // Первый проход: распределяем участников по hoisted-ролям
    for (const group of results) {
      const isStatusGroup = group.id === "online" || group.id === "offline" || group.id === "idle" || group.id === "dnd";
      if (isStatusGroup) continue;

      for (const m of membersList) {
        if (assigned.has(m)) continue;
        const memberRoles = Array.isArray(m.roles) ? m.roles.map(String) : [];
        const memberRole = m.role ? String(m.role) : "";
        if (memberRoles.includes(group.id) || memberRole === group.id) {
          const color = formatRoleColor(m.roleColor ?? m.color ?? group.roleColor);
          const memberWithColor = color ? { ...m, roleColor: color, color } : m;
          group.members.push(memberWithColor);
          assigned.add(m);
        }
      }
    }

    // Второй проход: оставшихся участников распределяем по статусным группам
    for (const group of results) {
      if (group.id === "online") {
        for (const m of membersList) {
          if (assigned.has(m)) continue;
          const status = m.status ?? "offline";
          if (status === "online" || status === "idle" || status === "dnd") {
            const color = formatRoleColor(m.roleColor ?? m.color);
            const memberWithColor = color ? { ...m, roleColor: color, color } : m;
            group.members.push(memberWithColor);
            assigned.add(m);
          }
        }
      } else if (group.id === "offline") {
        for (const m of membersList) {
          if (assigned.has(m)) continue;
          const status = m.status ?? "offline";
          if (status === "offline") {
            const color = formatRoleColor(m.roleColor ?? m.color);
            const memberWithColor = color ? { ...m, roleColor: color, color } : m;
            group.members.push(memberWithColor);
            assigned.add(m);
          }
        }
      } else if (group.id === "idle" || group.id === "dnd") {
        for (const m of membersList) {
          if (assigned.has(m)) continue;
          if (m.status === group.id) {
            const color = formatRoleColor(m.roleColor ?? m.color);
            const memberWithColor = color ? { ...m, roleColor: color, color } : m;
            group.members.push(memberWithColor);
            assigned.add(m);
          }
        }
      }
    }

    // Если в group.count не было числа (или было 0), обновляем по факту членов
    for (const group of results) {
      if (group.count === 0 && group.members.length > 0) {
        group.count = group.members.length;
      }
    }

    return results;
  }

  // 2. Обратная совместимость: стандартные группы статусов
  /** @type {Array<{ id: string, name: string, statuses: string[] }>} */
  const defaultStatusGroups = [
    { id: "online", name: "В СЕТИ", statuses: ["online"] },
    { id: "idle", name: "НЕАКТИВЕН", statuses: ["idle"] },
    { id: "dnd", name: "НЕ БЕСПОКОИТЬ", statuses: ["dnd"] },
    { id: "offline", name: "НЕ В СЕТИ", statuses: ["offline"] },
  ];

  /** @type {MemberGroupResult[]} */
  const results = [];
  for (const sg of defaultStatusGroups) {
    const matched = membersList.filter((m) => sg.statuses.includes(m.status ?? "offline"));
    if (matched.length > 0) {
      results.push({
        id: sg.id,
        key: sg.id,
        name: sg.name,
        count: matched.length,
        members: matched.map((m) => {
          const color = formatRoleColor(m.roleColor ?? m.color);
          return color ? { ...m, roleColor: color, color } : m;
        }),
      });
    }
  }

  // Если список был пуст или никто не подошел
  if (results.length === 0 && membersList.length === 0) {
    return [
      { id: "online", key: "online", name: "В СЕТИ", count: 0, members: [] },
      { id: "offline", key: "offline", name: "НЕ В СЕТИ", count: 0, members: [] },
    ];
  }

  return results;
}

/**
 * Отрисовывает сгруппированный список участников в DOM-контейнер.
 *
 * @param {HTMLElement} container DOM-контейнер для списка
 * @param {MemberLike[]} members список участников
 * @param {{
 *   groups?: Array<{ id: string, name?: string, count?: number, roleColor?: string | number, color?: string | number }>,
 *   roles?: Record<string, { id?: string, name?: string, color?: string | number, position?: number }> | Array<{ id: string, name: string, color?: string | number }>,
 *   onMemberClick?: (member: MemberLike, element: HTMLElement) => void,
 * }} [options]
 */
export function renderMembersList(container, members, options = {}) {
  if (!container) return;

  const doc = container.ownerDocument || (typeof document !== "undefined" ? document : null);
  if (!doc) return;

  const grouped = groupMembersByRoleOrStatus(members, options);
  /** @type {HTMLElement[]} */
  const groupNodes = [];

  for (const group of grouped) {
    if (group.members.length === 0 && group.count === 0 && grouped.length > 1) {
      continue;
    }

    const groupWrap = doc.createElement("div");
    groupWrap.className = "member-group";
    groupWrap.setAttribute("role", "none");
    groupWrap.setAttribute("data-group-id", group.id);
    groupWrap.setAttribute("data-group-key", group.key);

    const title = doc.createElement("h3");
    title.className = "member-group-title";
    title.setAttribute("data-height", "42");
    title.setAttribute("aria-label", `${group.name}: ${group.count} участников`);
    title.textContent = `${group.name} — ${group.count}`;
    if (group.roleColor) {
      title.style.color = group.roleColor;
      title.style.setProperty("--role-color", group.roleColor);
    }
    groupWrap.append(title);

    for (let i = 0; i < group.members.length; i++) {
      const member = group.members[i];
      if (!member) continue;

      const row = doc.createElement("div");
      row.className = "member";
      row.setAttribute("role", "listitem");
      row.setAttribute("data-status", member.status ?? "offline");
      if (member.userId || member.id) {
        row.setAttribute("data-user-id", String(member.userId ?? member.id));
      }
      const memberName = String(member.name ?? member.username ?? member.user?.global_name ?? member.user?.username ?? "Участник");
      row.setAttribute("aria-label", `${memberName}, ${member.statusLabel ?? (STATUS_LABELS[member.status ?? "offline"] ?? member.status ?? "offline")}`);

      const memberColor = formatRoleColor(member.roleColor ?? member.color ?? group.roleColor);
      if (memberColor) {
        row.setAttribute("data-role-color", memberColor);
        row.style.color = memberColor;
        row.style.setProperty("--role-color", memberColor);
      }

      // Аватар
      const avatar = doc.createElement("span");
      avatar.className = "avatar avatar--md";
      avatar.setAttribute("aria-hidden", "true");
      avatar.textContent = memberName.slice(0, 1).toUpperCase();
      const dot = doc.createElement("span");
      dot.className = "status-dot";
      dot.setAttribute("data-status", member.status ?? "offline");
      avatar.append(dot);

      // Имя
      const nameWrap = doc.createElement("span");
      nameWrap.className = "member-name";
      const nameLabel = doc.createElement("span");
      nameLabel.className = "member-label";
      nameLabel.textContent = memberName;
      if (memberColor) {
        nameLabel.style.color = memberColor;
      }
      nameWrap.append(nameLabel);

      if (member.badge || member.role) {
        const badge = doc.createElement("span");
        badge.className = "badge badge--role";
        badge.textContent = String(member.badge || member.role);
        nameWrap.append(badge);
      }

      // Роль / активность
      const activityEl = doc.createElement("span");
      activityEl.className = "member-role";
      activityEl.textContent = member.activity ?? "";

      row.append(avatar, nameWrap, activityEl);

      if (options.onMemberClick) {
        row.addEventListener("click", () => {
          options.onMemberClick?.(member, row);
        });
      }

      groupWrap.append(row);
    }

    groupNodes.push(groupWrap);
  }

  container.replaceChildren(...groupNodes);
}
