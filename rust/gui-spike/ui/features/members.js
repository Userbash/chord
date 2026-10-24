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
 * @param {{ total?: number, userId?: string, label?: string }} [options]
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
  row.setAttribute("role", "listitem");
  row.setAttribute("data-index", String(index));
  if (options.userId) row.setAttribute("data-user-id", options.userId);
  if (item.role) row.setAttribute("data-role", item.role);
  row.setAttribute("data-status", item.status);
  row.setAttribute("aria-setsize", String(options.total ?? MEMBER_ROWS));
  row.setAttribute("aria-posinset", String(index + 1));
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
  label.textContent = item.name;
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
        renderMemberRow(row, item, index);
        // Шаблон §7.2 для панели: тот же `.member`, высота 42, пометка окна.
        row.setAttribute("data-window", "preview");
        row.setAttribute("data-group", group.key);
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
