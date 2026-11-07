// @ts-check
/**
 * Hover-панель сообщения (SPEC-P3-02 §1 MSG-27…MSG-30, R-16).
 *
 * Состав: 4 быстрые реакции + 11 действий = 15 пунктов (R-16). Правило волны
 * «нет мёртвых кнопок»: пункт либо выполняет действие (`data-command`), либо
 * честно `aria-disabled="true"` + `title`-подсказка о Phase 3.
 *
 * Панель одна на весь чат и переиспользуется (переезжает по `top`), поэтому
 * виртуализация не получает +15 узлов на строку: 40 строк × 15 кнопок на
 * каждый кадр скролла уронили бы p95. В DOM строка по-прежнему с двумя детьми
 * (§7.1) — панель живёт в `.chat-main`.
 */
import { UIError, asUIError, reportError } from "./errors.js";
import { MESSAGE_ACTIONS, QUICK_REACTIONS, UNAVAILABLE_HINT, extrasFor } from "./rows.js";
import { reactionsFor, requestReaction } from "./reactions.js";

/**
 * @typedef {{
 *   list: HTMLElement,
 *   container: HTMLElement,
 *   renderRow: (index: number) => void,
 *   indexLabel: (index: number) => string,
 *   authorOf: (index: number) => string,
 *   getComposer: () => import("./composer.js").Composer | null,
 *   textOf?: (index: number) => string,
 *   messageIdOf?: (index: number) => string,
 *   reactionTargetOf?: (index: number) => {
 *     base: import("./rows.js").Reaction[],
 *     key: number | string,
 *     channelId?: string,
 *     messageId?: string,
 *   },
 *   onEdit?: (index: number, content: string) => Promise<void>,
 *   onDelete?: (index: number) => Promise<void>,
 *   canEdit?: (index: number, row: HTMLElement) => boolean,
 *   canDelete?: (index: number, row: HTMLElement) => boolean,
 *   onError?: (error: UIError) => void,
 * }} ToolbarOptions
 */

/**
 * Поднимаемся к строке без `closest()` (его нет в DOM-шиме §9.4).
 * @param {Node | null} node
 * @returns {HTMLElement | null}
 */
function closestRow(node) {
  let current = /** @type {any} */ (node);
  while (current) {
    if (current instanceof Element && current.classList?.contains("row")) {
      return /** @type {HTMLElement} */ (current);
    }
    current = current.parentNode ?? null;
  }
  return null;
}

/**
 * @param {ToolbarOptions} options
 */
export function createMessageToolbar(options) {
  const { list, container, renderRow, indexLabel, authorOf } = options;
  const onError = options.onError ?? (() => {});

  const bar = document.createElement("div");
  bar.className = "msg-toolbar";
  bar.setAttribute("role", "toolbar");
  bar.setAttribute("aria-label", "Действия с сообщением");
  bar.setAttribute("data-open", "false");
  bar.style.position = "absolute";
  bar.style.top = "4px";
  bar.style.right = "16px";

  /** @param {string} label */
  const button = (label) => {
    const element = document.createElement("button");
    element.className = "icon-btn msg-toolbar-btn";
    element.setAttribute("type", "button");
    element.setAttribute("aria-label", label);
    return element;
  };

  /** @type {Map<string, HTMLElement>} */
  const reactionButtons = new Map();
  for (const emoji of QUICK_REACTIONS) {
    const element = button(`Реакция ${emoji}`);
    element.textContent = emoji;
    element.setAttribute("data-command", "react");
    element.setAttribute("data-emoji", emoji);
    element.setAttribute("aria-pressed", "false");
    reactionButtons.set(emoji, element);
    bar.append(element);
  }

  /** @type {Map<string, HTMLElement>} */
  const actionButtons = new Map();
  for (const action of MESSAGE_ACTIONS) {
    const element = button(action.label);
    element.textContent = action.label;
    if (action.command) {
      element.setAttribute("data-command", action.command);
    } else {
      element.setAttribute("aria-disabled", "true");
      element.setAttribute("title", UNAVAILABLE_HINT);
    }
    actionButtons.set(action.label, element);
    bar.append(element);
  }

  container.append(bar);

  /** @type {number | null} */
  let current = null;
  /** @type {HTMLElement | null} */
  let currentRow = null;
  /** @type {Set<number>} */
  const deletePending = new Set();
  /** Строки с delete_message в полёте: второй клик до ответа не взводит confirm заново. */
  const deleteInFlight = new Set();
  const DELETE_LABEL = "Удалить сообщение";
  const DELETE_CONFIRM_LABEL = "Подтвердить удаление";

  /**
   * Текст/атрибут destructive-кнопки — производная от `deletePending`/`deleteInFlight`
   * и текущей строки, а не «вечное» состояние панели (G2-11).
   * @param {number | null} index
   */
  function syncDeleteLabel(index) {
    const remove = actionButtons.get(DELETE_LABEL);
    if (!remove) return;
    const armed = index !== null && (deletePending.has(index) || deleteInFlight.has(index));
    remove.textContent = armed ? DELETE_CONFIRM_LABEL : DELETE_LABEL;
    if (armed) remove.setAttribute("data-confirm", "true");
    else remove.removeAttribute("data-confirm");
  }

  /** Снять подтверждение: панель ушла со строки или IPC завершился. */
  function resetDelete() {
    deletePending.clear();
    deleteInFlight.clear();
    syncDeleteLabel(null);
  }

  /** @param {number} index */
  function syncPressed(index) {
    const target = options.reactionTargetOf?.(index)
      ?? { base: extrasFor(index).reactions, key: index };
    const active = new Map(
      reactionsFor(target.key, target.base).map((reaction) => [reaction.emoji, reaction.me]),
    );
    for (const [emoji, element] of reactionButtons) {
      element.setAttribute("aria-pressed", active.get(emoji) === true ? "true" : "false");
    }
  }

  /**
   * @param {HTMLElement} row
   */
  function isSystemMessageRow(row) {
    const rawType = row?.getAttribute?.("data-message-type");
    if (rawType !== null && rawType !== undefined && rawType !== "") {
      const msgType = Number(rawType);
      if (Number.isFinite(msgType) && msgType !== 0 && msgType !== 19) {
        return true;
      }
    }
    const body = row?.querySelector?.(".text-body");
    if (body?.getAttribute?.("data-system-call") === "true") return true;
    const txt = String(body?.textContent ?? "").trim();
    if (txt.startsWith("📞")) return true;
    return false;
  }

  /**
   * @param {number} index
   * @param {HTMLElement} row
   */
  function syncActionPermissions(index, row) {
    const isSystem = isSystemMessageRow(row);
    const allowEdit = !isSystem && (options.canEdit ? options.canEdit(index, row) : true);
    const allowDelete = !isSystem && (options.canDelete ? options.canDelete(index, row) : true);
    const editBtn = actionButtons.get("Изменить сообщение");
    if (editBtn) {
      editBtn.style.display = allowEdit ? "" : "none";
      if (allowEdit) editBtn.removeAttribute("hidden");
      else editBtn.setAttribute("hidden", "");
    }
    const deleteBtn = actionButtons.get(DELETE_LABEL);
    if (deleteBtn) {
      deleteBtn.style.display = allowDelete ? "" : "none";
      if (allowDelete) deleteBtn.removeAttribute("hidden");
      else deleteBtn.setAttribute("hidden", "");
    }
    return { allowEdit, allowDelete };
  }

  /**
   * @param {number} index
   * @param {HTMLElement} row
   */
  function open(index, row) {
    // Подтверждение принадлежит строке: переход на другую строку его снимает.
    if (current !== index) resetDelete();
    current = index;
    currentRow = row;
    const top = Number.parseFloat(row.style.top || "0");
    const scroll = Number.isFinite(list.scrollTop) ? list.scrollTop : 0;
    bar.style.top = `${Math.max(0, top - scroll + 4)}px`;
    bar.setAttribute("data-open", "true");
    bar.setAttribute("data-for-index", String(index));
    bar.setAttribute("aria-label", `Действия с сообщением ${indexLabel(index)}`);
    syncActionPermissions(index, row);
    syncPressed(index);
    syncDeleteLabel(index);
  }

  function close() {
    current = null;
    currentRow = null;
    resetDelete();
    bar.setAttribute("data-open", "false");
    bar.removeAttribute("data-for-index");
  }

  /** @param {Event} event */
  function onPointer(event) {
    if (
      typeof document !== "undefined" &&
      document.body?.dataset?.liveSession === "true" &&
      !(/** @type {any} */ (globalThis).__QA__)
    ) {
      close();
      return;
    }
    const target = /** @type {Node | null} */ (/** @type {any} */ (event).target ?? null);
    const row = closestRow(target);
    if (!row) return;
    const index = Number(row.getAttribute("data-index"));
    if (!Number.isInteger(index)) return;
    open(index, row);
  }

  list.addEventListener("mouseover", onPointer);
  list.addEventListener("focusin", onPointer);
  // В браузере уход курсора из списка закрывает панель; в шиме mouseleave не эмитится.
  list.addEventListener("mouseleave", () => close());
  list.addEventListener("scroll", () => close());

  /**
   * @param {string} command
   * @param {string} [emoji]
   */
  function run(command, emoji = "") {
    if (current === null) return;
    const index = current;
    if (command === "react" || command === "add-reaction") {
      const target = options.reactionTargetOf?.(index)
        ?? { base: extrasFor(index).reactions, key: index };
      const emojiTarget = emoji || "👍";
      // Один IPC на пару строка/emoji: `requestReaction` держит pending по ключу,
      // поэтому двойной клик до ответа не отправит второй `toggle_reaction`.
      void requestReaction(target.key, emojiTarget, target.base, {
        channelId: target.channelId,
        messageId: target.messageId,
        element: reactionButtons.get(emojiTarget) ?? null,
        onSettled: () => {
          // Панель могла закрыться/переехать: чужую строку не перерисовываем.
          if (current !== index) return;
          // Перерисовать строку и панель после ответа/отката: optimistic-состояние
          // живёт в reactions.js, а не в DOM кнопки.
          renderRow(index);
          syncPressed(index);
        },
      });
      // Optimistic-кадр: toggleReaction внутри requestReaction уже применён.
      renderRow(index);
      syncPressed(index);
      return;
    }
    if (command === "reply") {
      const composer = options.getComposer();
      if (!composer) {
        throw new UIError("ELEMENT_MISSING", "композер не инициализирован — reply недоступен");
      }
      composer.setReply({ author: authorOf(index), text: `#${index}`, id: options.messageIdOf?.(index) ?? String(index) });
      composer.focus();
      close();
      return;
    }
    if (command === "edit") {
      if (currentRow && !syncActionPermissions(index, currentRow).allowEdit) return;
      const text = currentRow?.querySelector(".text");
      if (!text) return;
      const input = document.createElement("textarea");
      input.setAttribute("data-action", "edit-input");
      input.value = options.textOf?.(index) ?? text.querySelector(".text-body")?.textContent ?? "";
      input.maxLength = 2000;
      const save = button("Сохранить изменение");
      save.textContent = "Сохранить";
      save.setAttribute("data-action", "edit-save");
      const cancel = button("Отменить изменение");
      cancel.textContent = "Отмена";
      cancel.setAttribute("data-action", "edit-cancel");
      const saveEdit = () => {
        if (save.getAttribute("data-pending") === "true") return;
        const content = input.value.trim();
        if (content === "" || content.length > 2000) {
          save.setAttribute("aria-disabled", "true");
          return;
        }
        // Enter + клик до ответа IPC — один запрос: повторный submit заблокирован.
        save.setAttribute("data-pending", "true");
        save.setAttribute("aria-disabled", "true");
        void (options.onEdit?.(index, content) ?? Promise.resolve())
          .catch((error) => onError(reportError("ACTION_FAILED", asUIError("ACTION_FAILED", error))))
          .finally(() => {
            save.removeAttribute("data-pending");
            // Строка уже перерисована ответом; detached-кнопку не трогаем.
            if (save.parentNode) save.removeAttribute("aria-disabled");
          });
      };
      input.addEventListener("keydown", (event) => {
        if (event.key === "Enter" && !event.shiftKey) { event.preventDefault(); saveEdit(); }
        if (event.key === "Escape") renderRow(index);
      });
      save.addEventListener("click", saveEdit);
      cancel.addEventListener("click", () => renderRow(index));
      text.replaceChildren(input, save, cancel);
      return;
    }
    if (command === "delete") {
      if (currentRow && !syncActionPermissions(index, currentRow).allowDelete) return;
      // Повторный клик до ответа IPC — не новый confirm.
      if (deleteInFlight.has(index)) return;
      if (!deletePending.has(index)) {
        deletePending.add(index);
        syncDeleteLabel(index);
        return;
      }
      deletePending.delete(index);
      deleteInFlight.add(index);
      syncDeleteLabel(index);
      void (options.onDelete?.(index) ?? Promise.resolve())
        .catch((error) => onError(reportError("ACTION_FAILED", asUIError("ACTION_FAILED", error))))
        .finally(() => {
          // Успех/ошибка одинаково снимают подтверждение: destructive-клик
          // не должен оставаться «взведённым» для следующей строки.
          deleteInFlight.delete(index);
          if (current !== index) return;
          syncDeleteLabel(index);
        });
      return;
    }
    throw new UIError("ACTION_FAILED", `неизвестная команда панели «${command}»`);
  }

  for (const [emoji, element] of reactionButtons) {
    element.addEventListener("click", () => {
      try {
        run("react", emoji);
      } catch (error) {
        onError(reportError("ACTION_FAILED", asUIError("ACTION_FAILED", error)));
      }
    });
  }
  for (const [label, element] of actionButtons) {
    element.addEventListener("click", () => {
      const command = element.getAttribute("data-command");
      if (!command) return; // aria-disabled: честная заглушка с подсказкой
      try {
        run(command);
      } catch (error) {
        onError(reportError("ACTION_FAILED", asUIError("ACTION_FAILED", error)));
      }
    });
  }

  return {
    element: bar,
    open,
    close,
    /** @returns {number | null} */
    get index() {
      return current;
    },
    get itemCount() {
      return bar.children.length;
    },
    get enabledCount() {
      return [...bar.children].filter(
        (child) => child.getAttribute("aria-disabled") !== "true",
      ).length;
    },
    /** @param {string} label */
    button(label) {
      return actionButtons.get(label) ?? null;
    },
  };
}
