// @ts-check
/**
 * FE-P2-05 (task-183): горячие клавиши Discord и модалка «Горячие клавиши».
 *
 * Оракулы контрактов:
 * - `waves/RW-40/PM/BACKLOG-RE-FRONTEND.md` §3, строка FE-P2-05 — 8 шорткатов
 *   (`Ctrl+K`, `Ctrl+Shift+M/D`, `Ctrl+/`) и модалка справки;
 * - `re-deep/frontend-d03.md` §5 (Modal) — `role="dialog"`, `aria-modal="true"`,
 *   `aria-label`, возврат фокуса прежнему элементу при закрытии;
 * - `re-deep/decoded/` — справка клиента Discord («Keyboard Shortcuts»).
 *
 * Модуль не зависит от глобалов: документ передаётся в
 * `createShortcutsController(doc, handlers)`, поэтому его проверяет DOM-шим
 * `tests/support/gui-dom-harness.mjs` без живого WebView.
 */

/** @typedef {{ id: string, combo: string, key: string, ctrlKey: boolean, shiftKey: boolean, altKey: boolean, labelRu: string, category: string }} DiscordShortcut */

/**
 * Минимум события клавиатуры, который читает `matchShortcut`. В браузере это
 * `KeyboardEvent`, в DOM-шиме — `ShimEvent`.
 * @typedef {{
 *   key?: string,
 *   ctrlKey?: boolean,
 *   shiftKey?: boolean,
 *   altKey?: boolean,
 *   metaKey?: boolean,
 *   target?: any,
 * }} ShortcutEventLike
 */

/** @typedef {Record<string, (event: any) => void>} ShortcutHandlers */

/**
 * Каноническая таблица сочетаний Discord: порядок совпадает со справкой клиента.
 * @type {DiscordShortcut[]}
 */
const SHORTCUT_TABLE = [
  {
    id: "quick_switcher",
    combo: "Ctrl+K",
    key: "k",
    ctrlKey: true,
    shiftKey: false,
    altKey: false,
    labelRu: "Быстрый переход",
    category: "Навигация",
  },
  {
    id: "toggle_mute",
    combo: "Ctrl+Shift+M",
    key: "m",
    ctrlKey: true,
    shiftKey: true,
    altKey: false,
    labelRu: "Вкл./выкл. микрофон",
    category: "Голос",
  },
  {
    id: "toggle_deafen",
    combo: "Ctrl+Shift+D",
    key: "d",
    ctrlKey: true,
    shiftKey: true,
    altKey: false,
    labelRu: "Вкл./выкл. звук",
    category: "Голос",
  },
  {
    id: "shortcuts_help",
    combo: "Ctrl+/",
    key: "/",
    ctrlKey: true,
    shiftKey: false,
    altKey: false,
    labelRu: "Горячие клавиши",
    category: "Справка",
  },
  {
    id: "search_channel",
    combo: "Ctrl+F",
    key: "f",
    ctrlKey: true,
    shiftKey: false,
    altKey: false,
    labelRu: "Поиск в канале",
    category: "Навигация",
  },
  {
    id: "pins_popout",
    combo: "Ctrl+P",
    key: "p",
    ctrlKey: true,
    shiftKey: false,
    altKey: false,
    labelRu: "Закреплённые сообщения",
    category: "Сообщения",
  },
  {
    id: "mark_channel_read",
    combo: "Escape",
    key: "Escape",
    ctrlKey: false,
    shiftKey: false,
    altKey: false,
    labelRu: "Отметить канал прочитанным / Закрыть модалку",
    category: "Навигация",
  },
  {
    id: "navigate_unread_prev",
    combo: "Alt+Shift+ArrowUp",
    key: "ArrowUp",
    ctrlKey: false,
    shiftKey: true,
    altKey: true,
    labelRu: "Предыдущий непрочитанный канал",
    category: "Навигация",
  },
];

/** Замороженная таблица: потребители не должны мутировать контракт. */
export const DISCORD_SHORTCUTS = Object.freeze(
  SHORTCUT_TABLE.map((shortcut) => Object.freeze(shortcut)),
);

/**
 * Шорткаты, которые остаются рабочими внутри поля ввода: Ctrl-комбинации с
 * явным модификатором из эталонной справки. Escape обрабатывается отдельно.
 * @type {ReadonlySet<string>}
 */
const EDITABLE_TARGET_ALLOWED = new Set([
  "quick_switcher",
  "toggle_mute",
  "toggle_deafen",
  "shortcuts_help",
]);

/**
 * Поле ввода/редактируемый контейнер, где голые буквы и Escape принадлежат
 * набору текста, а не глобальным шорткатам.
 * @param {any} target
 * @returns {boolean}
 */
function isEditableTarget(target) {
  if (!target || typeof target !== "object") return false;
  const tag = typeof target.tagName === "string" ? target.tagName.toUpperCase() : "";
  if (tag === "INPUT" || tag === "TEXTAREA") return true;
  if (target.isContentEditable === true) return true;
  if (typeof target.getAttribute === "function") {
    const attr = target.getAttribute("contenteditable");
    if (attr === "" || attr === "true") return true;
  }
  return false;
}

/**
 * Сравнение клавиши: буквы — без учёта регистра (Shift меняет `event.key`),
 * служебные клавиши (`Escape`, `ArrowUp`) — точно.
 * @param {string} expected
 * @param {string} actual
 * @returns {boolean}
 */
function keyMatches(expected, actual) {
  if (expected.length === 1 && actual.length === 1) return expected.toLowerCase() === actual.toLowerCase();
  return expected === actual;
}

/**
 * Находит шорткат для события клавиатуры.
 * @param {ShortcutEventLike | null | undefined} event
 * @returns {DiscordShortcut | null}
 */
export function matchShortcut(event) {
  if (!event) return null;
  const key = typeof event.key === "string" ? event.key : "";
  if (!key) return null;
  const ctrlKey = Boolean(event.ctrlKey);
  const shiftKey = Boolean(event.shiftKey);
  const altKey = Boolean(event.altKey);
  const editable = isEditableTarget(event.target);

  for (const shortcut of DISCORD_SHORTCUTS) {
    if (shortcut.ctrlKey !== ctrlKey) continue;
    if (shortcut.shiftKey !== shiftKey) continue;
    if (shortcut.altKey !== altKey) continue;
    if (!keyMatches(shortcut.key, key)) continue;
    // В поле ввода Escape и явные Ctrl-хоткеи из справки не перехватываем
    // только там, где это ломает набор текста (Ctrl+F/Ctrl+P, Alt+Shift+ArrowUp).
    if (editable && !EDITABLE_TARGET_ALLOWED.has(shortcut.id) && shortcut.key !== "Escape") continue;
    return shortcut;
  }
  return null;
}

/**
 * Возвращает (создавая при необходимости) модалку справки в `doc.body`.
 * @param {any} doc
 * @returns {any}
 */
function ensureHelpModal(doc) {
  const existing = typeof doc.getElementById === "function" ? doc.getElementById("shortcuts-help-modal") : null;
  if (existing) return existing;

  const modal = doc.createElement("div");
  modal.id = "shortcuts-help-modal";
  modal.className = "shortcuts-help-modal";
  modal.setAttribute("role", "dialog");
  modal.setAttribute("aria-modal", "true");
  modal.setAttribute("aria-label", "Горячие клавиши");
  modal.setAttribute("hidden", "");
  modal.setAttribute("aria-hidden", "true");

  const panel = doc.createElement("div");
  panel.className = "shortcuts-help-panel";

  const header = doc.createElement("header");
  header.className = "shortcuts-help-header";
  const title = doc.createElement("h2");
  title.className = "shortcuts-help-title";
  title.textContent = "Горячие клавиши";
  const close = doc.createElement("button");
  close.type = "button";
  close.className = "icon-btn shortcuts-help-close";
  close.setAttribute("data-action", "close-shortcuts-help");
  close.setAttribute("aria-label", "Закрыть справку по горячим клавишам");
  close.textContent = "×";
  header.append(title, close);

  const list = doc.createElement("ul");
  list.className = "shortcuts-help-list";
  list.setAttribute("role", "list");
  for (const shortcut of DISCORD_SHORTCUTS) {
    const row = doc.createElement("li");
    row.className = "shortcuts-help-row";
    row.setAttribute("data-shortcut-id", shortcut.id);
    row.setAttribute("data-category", shortcut.category);

    const keys = doc.createElement("span");
    keys.className = "shortcuts-help-keys";
    const kbd = doc.createElement("kbd");
    kbd.className = "shortcuts-help-kbd";
    kbd.textContent = shortcut.combo;
    keys.append(kbd);

    const label = doc.createElement("span");
    label.className = "shortcuts-help-label";
    label.textContent = shortcut.labelRu;

    row.append(keys, label);
    list.append(row);
  }

  panel.append(header, list);
  modal.append(panel);
  doc.body.append(modal);
  return modal;
}

/**
 * Контроллер горячих клавиш: глобальный слушатель `keydown` на `doc.body`,
 * модалка справки и возврат фокуса.
 *
 * @param {any} doc документ (в браузере `Document`, в тестах `ShimDocument`)
 * @param {ShortcutHandlers} [handlers] обработчики по `shortcut.id`
 */
export function createShortcutsController(doc, handlers = {}) {
  if (!doc || !doc.body) throw new Error("createShortcutsController: требуется документ с body");
  const body = doc.body;
  const registry = handlers ?? {};
  const modal = ensureHelpModal(doc);
  const closeButton = /** @type {any} */ (modal.querySelector('[data-action="close-shortcuts-help"]'));

  let open = false;
  let destroyed = false;
  /** @type {any} */
  let previouslyFocused = null;

  function openHelpModal() {
    if (destroyed || open) return;
    open = true;
    previouslyFocused = doc.activeElement ?? null;
    modal.removeAttribute("hidden");
    modal.setAttribute("aria-hidden", "false");
    if (closeButton && typeof closeButton.focus === "function") closeButton.focus();
  }

  function closeHelpModal() {
    if (destroyed) return;
    if (!open) {
      // Состояние разметки приводим к «закрыто» даже без активной модалки.
      modal.setAttribute("hidden", "");
      modal.setAttribute("aria-hidden", "true");
      return;
    }
    open = false;
    modal.setAttribute("hidden", "");
    modal.setAttribute("aria-hidden", "true");
    const target = previouslyFocused;
    previouslyFocused = null;
    if (target && typeof target.focus === "function") target.focus();
  }

  /** @returns {boolean} */
  function isHelpModalOpen() {
    return open;
  }

  function onCloseClick() {
    closeHelpModal();
  }

  /**
   * @param {any} event
   * @returns {void}
   */
  function onKeydown(event) {
    const shortcut = matchShortcut(event);
    if (!shortcut) return;

    if (shortcut.id === "shortcuts_help") {
      event.preventDefault();
      if (open) closeHelpModal();
      else openHelpModal();
      return;
    }

    // Escape принадлежит открытой модалке: закрыть и вернуть фокус, не трогая
    // обработчик «отметить канал прочитанным».
    if (open && shortcut.key === "Escape") {
      event.preventDefault();
      closeHelpModal();
      return;
    }

    event.preventDefault();
    const handler = registry[shortcut.id];
    if (typeof handler === "function") handler(event);
  }

  function destroy() {
    if (destroyed) return;
    destroyed = true;
    body.removeEventListener("keydown", onKeydown);
    if (closeButton && typeof closeButton.removeEventListener === "function") {
      closeButton.removeEventListener("click", onCloseClick);
    }
    if (typeof modal.remove === "function") modal.remove();
    open = false;
    previouslyFocused = null;
  }

  body.addEventListener("keydown", onKeydown);
  if (closeButton && typeof closeButton.addEventListener === "function") {
    closeButton.addEventListener("click", onCloseClick);
  }

  return { openHelpModal, closeHelpModal, isHelpModalOpen, destroy };
}
