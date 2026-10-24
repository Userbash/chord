// @ts-check
/**
 * L-02-UI (RW-40): DOM-адаптер dispatch-проекций.
 *
 * Единственный вход — `Change[]` из `dispatch-model.js`; здесь только селекторы
 * контракта (PLAN §4): `#guild-rail .guild-item[data-guild-id]`, `.tree-guild`,
 * `#member-panel-list .member[data-user-id]`, `#member-online`,
 * `#chat-list .row[data-message-id]`. Новых id/разметки/CSS нет:
 * `index.html` заморожен, поэтому live-состояние выражается атрибутами
 * `data-source` / `data-dispatch-source` / `data-live-order`.
 *
 * Инварианты §4.4: ничего не перезапрашиваем (никакого IPC), не пересобираем
 * `#chat-list`, не двигаем фокус/экран/скролл, переиспользуем узлы строк.
 */
import { memberHeight } from "../model.js";
import { formatNumber } from "./dom.js";
import { renderMemberRow } from "./members.js";

/**
 * Стабильный оттенок аватара по id (не зависит от индекса окна).
 * @param {string} key
 * @returns {number}
 */
function hueOf(key) {
  let hash = 0;
  for (const char of key) hash = (hash * 31 + (char.codePointAt(0) ?? 0)) % 360;
  return hash;
}

/**
 * Одна live-строка участника: тот же шаблон §7.2, что у bench, но высота 42,
 * адресация `data-user-id` и `aria-setsize` по размеру окна (PLAN §4.3).
 * @param {HTMLElement} node
 * @param {import("./dispatch-model.js").MemberRow} row
 * @param {number} index
 * @param {number} total
 */
function paintLiveRow(node, row, index, total) {
  const activity = row.activity !== "" ? row.activity : row.statusLabel;
  /** @type {import("./members.js").Member} */
  const item = {
    name: row.name,
    status: row.status,
    statusLabel: row.statusLabel,
    role: "",
    badge: "",
    activity,
    hue: hueOf(row.userId),
  };
  const label = `${row.name}, ${row.statusLabel}${row.activity !== "" ? `, ${row.activity}` : ""}`;
  renderMemberRow(node, item, index, { total, userId: row.userId, label });
  node.style.height = `${memberHeight()}px`;
  // Строка больше не принадлежит bench-группам (группировка — MB-01).
  node.removeAttribute("data-group");
  node.removeAttribute("data-window");
}

/**
 * @param {{
 *   body?: HTMLElement | null,
 *   memberPanelList?: HTMLElement | null,
 *   memberOnline?: HTMLElement | null,
 *   treeHeading?: HTMLElement | null,
 *   rail?: { applyGuilds?: (items: any[]) => void, selectedId?: string } | null,
 *   chat?: { applyReaction?: (change: any) => string } | null,
 *   netStatus?: HTMLElement | null,
 *   selectedGuildId?: string,
 * }} options
 */
export function createDispatchView(options = {}) {
  const body = options.body ?? null;
  const memberPanelList = options.memberPanelList ?? null;
  const memberOnline = options.memberOnline ?? null;
  const treeHeading = options.treeHeading ?? null;
  const rail = options.rail ?? null;
  const chat = options.chat ?? null;
  const netStatus = options.netStatus ?? null;
  let selectedGuildId = String(options.selectedGuildId ?? "").trim();
  /** READY не перебивает точный `online_count` из LIST_UPDATE (PLAN §3.1). */
  let onlineCountKnown = false;

  /** @returns {string} */
  function currentSelection() {
    if (selectedGuildId !== "") return selectedGuildId;
    return String(rail?.selectedId ?? "");
  }

  /**
   * Честный источник dispatch без нового текста/баннера (`CONTRACTS/dispatch.md`
   * §8): live | absent | unavailable.
   * @param {string} source
   */
  function setSource(source) {
    const value = source === "live" || source === "unavailable" ? source : "absent";
    body?.setAttribute("data-dispatch-source", value);
    const node = netStatus
      ?? (typeof document !== "undefined" ? document.getElementById("net-status") : null);
    node?.setAttribute("data-dispatch-source", value);
  }

  /** @param {string} guildId */
  function selectGuild(guildId) {
    selectedGuildId = String(guildId ?? "").trim();
  }

  /** @param {import("./dispatch-model.js").GuildsChange} change */
  function applyGuilds(change) {
    const items = Array.isArray(change.guilds) ? change.guilds : [];
    if (rail && typeof rail.applyGuilds === "function") rail.applyGuilds(items);
    const selected = currentSelection();
    const chosen = items.find((guild) => guild.id === selected)
      ?? items.find((guild) => guild.id === String(rail?.selectedId ?? ""));
    if (treeHeading && chosen) treeHeading.textContent = chosen.name;
    if (memberOnline && !onlineCountKnown && chosen && chosen.memberCount !== null) {
      memberOnline.textContent = formatNumber(chosen.memberCount);
    }
  }

  /**
   * @param {import("./dispatch-model.js").MembersChange} change
   * @returns {number}
   */
  function applyMembers(change) {
    // Гильдия не выбрана — только store (DOM другой гильдии не трогаем).
    if (change.guildId !== currentSelection()) return 1;
    if (!memberPanelList) return 1;
    memberPanelList.setAttribute("data-source", "live");
    // В live-списке есть и offline: нейтральное имя вместо bench «в сети».
    memberPanelList.setAttribute("aria-label", "Участники сервера");
    const order = Array.isArray(change.order) ? change.order : [];
    const rows = Array.isArray(change.rows) ? change.rows : [];
    /** @type {Map<string, import("./dispatch-model.js").MemberRow>} */
    const byId = new Map(rows.map((row) => [row.userId, row]));
    /** @type {Map<string, HTMLElement>} */
    const existing = new Map();
    for (const child of /** @type {HTMLElement[]} */ ([...memberPanelList.children])) {
      const id = child.getAttribute("data-user-id") ?? "";
      if (id !== "") existing.set(id, child);
    }
    const nodes = order.map((userId, index) => {
      const node = existing.get(userId) ?? document.createElement("div");
      const row = byId.get(userId) ?? {
        userId,
        name: userId,
        status: "offline",
        statusLabel: "не в сети",
        activity: "",
      };
      paintLiveRow(node, row, index, order.length);
      return node;
    });
    // Переиспользуем узлы (идентичность строк важна для C3/C11), меняя порядок.
    memberPanelList.replaceChildren(...nodes);
    memberPanelList.setAttribute("data-live-order", String(order.length));
    if (memberOnline) {
      // Счётчик уже live-регион (PLAN §4.5): повторная установка безвредна и
      // держит контракт, даже если bench-панель не успела отрисоваться.
      memberOnline.setAttribute("aria-live", "polite");
      if (typeof change.onlineCount === "number") {
        memberOnline.textContent = formatNumber(change.onlineCount);
        onlineCountKnown = true;
      } else if (!onlineCountKnown && typeof change.memberCount === "number") {
        memberOnline.textContent = formatNumber(change.memberCount);
      }
    }
    return 1;
  }

  /**
   * @param {import("./dispatch-model.js").PresenceChange} change
   * @returns {number}
   */
  function applyPresence(change) {
    if (!memberPanelList) return 1;
    // Presence адресован гильдии панели: чужой scope не трогает выбранные строки.
    if (change.guildId !== currentSelection()) return 1;
    const row = /** @type {HTMLElement | null} */ (
      [...memberPanelList.children].find(
        (node) => node.getAttribute("data-user-id") === change.userId,
      ) ?? null
    );
    // Строки нет в окне: store уже обновлён моделью — не «тихо потеряли».
    if (!row) return 1;
    row.setAttribute("data-status", change.status);
    const dot = row.querySelector(".status-dot");
    if (dot) dot.setAttribute("data-status", change.status);
    const nameNode = row.querySelector(".member-label");
    const name = nameNode?.textContent ?? change.userId;
    const role = row.querySelector(".member-role");
    if (role) role.textContent = change.activity !== "" ? change.activity : change.statusLabel;
    row.setAttribute(
      "aria-label",
      `${name}, ${change.statusLabel}${change.activity !== "" ? `, ${change.activity}` : ""}`,
    );
    return 1;
  }

  /**
   * @param {import("./dispatch-model.js").ReactionChange} change
   * @returns {{ applied: number, notOpen: number, notLoaded: number }}
   */
  function applyReaction(change) {
    if (!chat || typeof chat.applyReaction !== "function") return { applied: 0, notOpen: 0, notLoaded: 0 };
    const result = chat.applyReaction(change);
    if (result === "not-open") return { applied: 0, notOpen: 1, notLoaded: 0 };
    if (result === "not-loaded") return { applied: 0, notOpen: 0, notLoaded: 1 };
    return { applied: 1, notOpen: 0, notLoaded: 0 };
  }

  /**
   * Применить пачку изменений. Ошибка одного change не роняет остальные
   * (N13); мост считает `failed` и продолжает.
   * @param {import("./dispatch-model.js").DispatchChange[]} changes
   * @returns {{ applied: number, ignored: number, failed: number, notOpen: number, notLoaded: number }}
   */
  function apply(changes) {
    let applied = 0;
    let ignored = 0;
    let failed = 0;
    let notOpen = 0;
    let notLoaded = 0;
    for (const change of Array.isArray(changes) ? changes : []) {
      try {
        switch (change.type) {
          case "guilds":
            applyGuilds(change);
            applied += 1;
            break;
          case "members":
            applied += applyMembers(change);
            break;
          case "presence":
            applied += applyPresence(change);
            break;
          case "reaction": {
            const result = applyReaction(change);
            applied += result.applied;
            notOpen += result.notOpen;
            notLoaded += result.notLoaded;
            break;
          }
          default:
            ignored += 1;
            break;
        }
      } catch {
        failed += 1;
      }
    }
    return { applied, ignored, failed, notOpen, notLoaded };
  }

  return {
    apply,
    selectGuild,
    setSource,
    /** @returns {string} */
    get selectedGuildId() {
      return currentSelection();
    },
  };
}

/** @typedef {ReturnType<typeof createDispatchView>} DispatchView */
