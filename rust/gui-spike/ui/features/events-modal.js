// @ts-check
/**
 * Контроллер модального окна запланированных событий сервера и формы создания события (task-247 / FE-P0-11).
 *
 * Отвечает за:
 * - замороженный справочник вкладок фильтрации `EVENT_FILTER_TABS_RU` (`upcoming`, `active`, `all`);
 * - чистую функцию фильтрации и сортировки `filterEventsByTab(events, tabId)` без мутации входного массива;
 * - рендер диалога `[data-events-modal]` (`role="dialog"`, `aria-modal="true"`, `aria-label="События сервера"`),
 *   вкладок `[data-event-tab="<id>"]`, карточек событий `[data-event-card-id="<id>"]` с кнопкой RSVP
 *   `[data-event-rsvp-btn="<id>"]`, формы создания события `[data-event-create-form]` (при `canManageEvents === true`)
 *   и кнопки закрытия `[data-events-close-btn]`.
 *
 * @module ui/features/events-modal
 */

/**
 * @typedef {"upcoming" | "active" | "all"} EventFilterTabId
 */

/**
 * @typedef {object} EventFilterTabEntry
 * @property {EventFilterTabId} id
 * @property {string} label
 */

/**
 * Замороженный список вкладок фильтрации событий сервера с русскими подписями.
 * @type {ReadonlyArray<Readonly<EventFilterTabEntry>>}
 */
export const EVENT_FILTER_TABS_RU = Object.freeze([
  Object.freeze({ id: /** @type {const} */ ("upcoming"), label: "Предстоящие" }),
  Object.freeze({ id: /** @type {const} */ ("active"), label: "Активные" }),
  Object.freeze({ id: /** @type {const} */ ("all"), label: "Все события" }),
]);

/**
 * @typedef {object} GuildScheduledEventItem
 * @property {string | number} id
 * @property {string} [name]
 * @property {string} [title]
 * @property {string} [description]
 * @property {number} [status]
 * @property {number} [entityType]
 * @property {string} [location]
 * @property {string} [scheduledStartTime]
 * @property {boolean} [interested]
 * @property {number} [userCount]
 */

/**
 * @typedef {object} CreateEventPayload
 * @property {string} name
 * @property {number} entityType
 * @property {string} location
 * @property {string} scheduledStartTime
 */

/**
 * @typedef {object} EventsModalStatePatch
 * @property {ReadonlyArray<GuildScheduledEventItem>} [events]
 * @property {string} [activeTab]
 * @property {boolean} [canManageEvents]
 */

/**
 * @typedef {object} EventsModalStateSnapshot
 * @property {GuildScheduledEventItem[]} events
 * @property {GuildScheduledEventItem[]} visibleEvents
 * @property {EventFilterTabId} activeTab
 * @property {boolean} canManageEvents
 */

/**
 * @typedef {object} EventsModalControllerOptions
 * @property {any} container
 * @property {ReadonlyArray<GuildScheduledEventItem>} [events]
 * @property {string} [activeTab]
 * @property {boolean} [canManageEvents]
 * @property {(eventId: string, nextInterested: boolean) => void} [onToggleRsvp]
 * @property {(payload: CreateEventPayload) => void} [onCreateEvent]
 * @property {() => void} [onClose]
 */

/**
 * Нормализует идентификатор вкладки фильтра событий.
 * При неизвестном или опущенном значении возвращает `"upcoming"`.
 *
 * @param {unknown} rawTabId
 * @returns {EventFilterTabId}
 */
function normalizeEventTabId(rawTabId) {
  if (rawTabId === "active") return "active";
  if (rawTabId === "all") return "all";
  return "upcoming";
}

/**
 * Сравнивает два события по `String(scheduledStartTime || "")` по возрастанию,
 * а при равенстве — по `String(id)` по возрастанию.
 *
 * @param {GuildScheduledEventItem} a
 * @param {GuildScheduledEventItem} b
 * @returns {number}
 */
function compareEventsAsc(a, b) {
  const startA = String(a?.scheduledStartTime || "");
  const startB = String(b?.scheduledStartTime || "");
  if (startA < startB) return -1;
  if (startA > startB) return 1;
  const idA = String(a?.id ?? "");
  const idB = String(b?.id ?? "");
  if (idA < idB) return -1;
  if (idA > idB) return 1;
  return 0;
}

/**
 * Фильтрует и сортирует список событий по выбранной вкладке:
 * - `"active"`: только события со `status === 2`;
 * - `"upcoming"`: только события со `status === 1` (используется по умолчанию для неизвестного/опущенного `tabId`);
 * - `"all"`: все события кроме отменённых (`status !== 4`).
 * Результат сортируется по `String(scheduledStartTime || "")` по возрастанию с tie-break по `String(id)` по возрастанию.
 * Входной массив и его элементы не мутируются.
 *
 * @param {ReadonlyArray<GuildScheduledEventItem> | null | undefined} events
 * @param {string} [tabId]
 * @returns {GuildScheduledEventItem[]}
 */
export function filterEventsByTab(events, tabId) {
  if (!Array.isArray(events)) {
    return [];
  }
  const normalizedTab = normalizeEventTabId(tabId);
  /** @type {GuildScheduledEventItem[]} */
  const filtered = [];

  for (const item of events) {
    if (!item || typeof item !== "object") continue;
    const status = item.status;
    if (normalizedTab === "active") {
      if (status !== 2) continue;
    } else if (normalizedTab === "all") {
      if (status === 4) continue;
    } else {
      if (status !== 1) continue;
    }
    filtered.push({ ...item });
  }

  filtered.sort(compareEventsAsc);
  return filtered;
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
 * Создаёт контроллер модального окна событий сервера внутри `options.container`.
 *
 * @param {EventsModalControllerOptions} options
 */
export function createEventsModalController(options) {
  if (!options || !options.container) {
    throw new TypeError("options.container is required");
  }

  const container = options.container;
  const doc = resolveDocument(container);

  /** @type {GuildScheduledEventItem[]} */
  let events = Array.isArray(options.events)
    ? options.events.filter((e) => e && typeof e === "object").map((e) => ({ ...e }))
    : [];
  /** @type {EventFilterTabId} */
  let activeTab = normalizeEventTabId(options.activeTab);
  /** @type {boolean} */
  let canManageEvents = options.canManageEvents === true;
  let destroyed = false;

  const root = doc.createElement("div");
  root.setAttribute("data-events-modal", "");
  root.setAttribute("role", "dialog");
  root.setAttribute("aria-modal", "true");
  root.setAttribute("aria-label", "События сервера");

  /**
   * Перерисовывает внутреннее дерево модального окна событий.
   */
  function renderDom() {
    if (destroyed) return;

    const visibleEvents = filterEventsByTab(events, activeTab);

    // 1. Шапка модального окна с заголовком и кнопкой закрытия
    const headerEl = doc.createElement("header");
    headerEl.setAttribute("data-events-header", "");

    const titleEl = doc.createElement("h2");
    titleEl.textContent = "События сервера";
    headerEl.appendChild(titleEl);

    const closeBtn = doc.createElement("button");
    closeBtn.setAttribute("type", "button");
    closeBtn.setAttribute("data-events-close-btn", "");
    closeBtn.setAttribute("aria-label", "Закрыть");
    closeBtn.textContent = "Закрыть";
    closeBtn.addEventListener("click", () => {
      close();
    });
    headerEl.appendChild(closeBtn);

    // 2. Панель вкладок фильтрации
    const tabListEl = doc.createElement("div");
    tabListEl.setAttribute("role", "tablist");
    tabListEl.setAttribute("aria-label", "Фильтр событий");

    for (const tab of EVENT_FILTER_TABS_RU) {
      const isSelected = tab.id === activeTab;
      const tabBtn = doc.createElement("button");
      tabBtn.setAttribute("type", "button");
      tabBtn.setAttribute("role", "tab");
      tabBtn.setAttribute("data-event-tab", tab.id);
      tabBtn.setAttribute("aria-selected", isSelected ? "true" : "false");
      tabBtn.textContent = tab.label;
      tabBtn.addEventListener("click", () => {
        activeTab = tab.id;
        renderDom();
      });
      tabListEl.appendChild(tabBtn);
    }

    // 3. Список карточек событий
    const listEl = doc.createElement("div");
    listEl.setAttribute("data-events-list", "");

    if (visibleEvents.length === 0) {
      const emptyEl = doc.createElement("p");
      emptyEl.setAttribute("data-events-empty", "");
      emptyEl.textContent = "Нет событий";
      listEl.appendChild(emptyEl);
    } else {
      for (const ev of visibleEvents) {
        const eventId = String(ev.id ?? "");
        const isInterested = ev.interested === true;
        const eventTitle =
          String(ev.name ?? ev.title ?? "").trim() || "Событие без названия";

        const cardEl = doc.createElement("article");
        cardEl.setAttribute("data-event-card-id", eventId);

        const cardTitleEl = doc.createElement("h3");
        cardTitleEl.setAttribute("data-event-title", "");
        cardTitleEl.textContent = eventTitle;
        cardEl.appendChild(cardTitleEl);

        if (ev.scheduledStartTime) {
          const timeEl = doc.createElement("time");
          timeEl.setAttribute("datetime", String(ev.scheduledStartTime));
          timeEl.textContent = String(ev.scheduledStartTime);
          cardEl.appendChild(timeEl);
        }

        if (ev.location) {
          const locEl = doc.createElement("span");
          locEl.setAttribute("data-event-location", "");
          locEl.textContent = String(ev.location);
          cardEl.appendChild(locEl);
        }

        const rsvpBtn = doc.createElement("button");
        rsvpBtn.setAttribute("type", "button");
        rsvpBtn.setAttribute("data-event-rsvp-btn", eventId);
        rsvpBtn.setAttribute("aria-pressed", isInterested ? "true" : "false");
        rsvpBtn.textContent = isInterested ? "Интересуюсь ✓" : "Интересуюсь";
        rsvpBtn.addEventListener("click", () => {
          const current = events.find((item) => String(item.id ?? "") === eventId);
          const wasInterested = current ? current.interested === true : isInterested;
          const nextInterested = !wasInterested;
          events = events.map((item) =>
            String(item.id ?? "") === eventId
              ? { ...item, interested: nextInterested }
              : item,
          );
          renderDom();
          if (typeof options.onToggleRsvp === "function") {
            options.onToggleRsvp(eventId, nextInterested);
          }
        });
        cardEl.appendChild(rsvpBtn);

        listEl.appendChild(cardEl);
      }
    }

    /** @type {any[]} */
    const children = [headerEl, tabListEl, listEl];

    // 4. Форма создания события (только при canManageEvents === true)
    if (canManageEvents) {
      const formEl = doc.createElement("form");
      formEl.setAttribute("data-event-create-form", "");

      const nameInput = doc.createElement("input");
      nameInput.setAttribute("type", "text");
      nameInput.setAttribute("data-event-name-input", "");
      nameInput.setAttribute("placeholder", "Название события");
      nameInput.value = "";
      formEl.appendChild(nameInput);

      const entitySelect = doc.createElement("select");
      entitySelect.setAttribute("data-event-entity-select", "");
      const entityOptions = [
        { value: "1", label: "Трибуна" },
        { value: "2", label: "Голосовой канал" },
        { value: "3", label: "Внешнее место" },
      ];
      for (const optMeta of entityOptions) {
        const optEl = doc.createElement("option");
        optEl.setAttribute("value", optMeta.value);
        optEl.value = optMeta.value;
        optEl.textContent = optMeta.label;
        entitySelect.appendChild(optEl);
      }
      entitySelect.value = "3";
      formEl.appendChild(entitySelect);

      const locationInput = doc.createElement("input");
      locationInput.setAttribute("type", "text");
      locationInput.setAttribute("data-event-location-input", "");
      locationInput.setAttribute("placeholder", "Место проведения");
      locationInput.value = "";
      formEl.appendChild(locationInput);

      const startInput = doc.createElement("input");
      startInput.setAttribute("type", "text");
      startInput.setAttribute("data-event-start-input", "");
      startInput.setAttribute("placeholder", "Время начала (ISO-8601)");
      startInput.value = "";
      formEl.appendChild(startInput);

      const submitBtn = doc.createElement("button");
      submitBtn.setAttribute("type", "button");
      submitBtn.setAttribute("data-event-create-submit", "");
      submitBtn.textContent = "Создать событие";
      formEl.appendChild(submitBtn);

      const handleCreate = (/** @type {any} */ event) => {
        if (event && typeof event.preventDefault === "function") {
          event.preventDefault();
        }
        const trimmedName = String(nameInput.value ?? "").trim();
        const trimmedStart = String(startInput.value ?? "").trim();
        if (trimmedName.length === 0 || trimmedStart.length === 0) {
          return;
        }
        const trimmedLocation = String(locationInput.value ?? "").trim();
        const parsedEntity = Number(entitySelect.value);
        const entityType =
          Number.isFinite(parsedEntity) && parsedEntity > 0
            ? Math.trunc(parsedEntity)
            : 3;

        if (typeof options.onCreateEvent === "function") {
          options.onCreateEvent({
            name: trimmedName,
            entityType,
            location: trimmedLocation,
            scheduledStartTime: trimmedStart,
          });
        }
      };

      submitBtn.addEventListener("click", handleCreate);
      formEl.addEventListener("submit", handleCreate);

      children.push(formEl);
    }

    replaceElementChildren(root, children);
  }

  /**
   * Возвращает снимок текущего состояния контроллера.
   *
   * @returns {EventsModalStateSnapshot}
   */
  function getState() {
    return {
      events: events.map((e) => ({ ...e })),
      visibleEvents: filterEventsByTab(events, activeTab),
      activeTab,
      canManageEvents,
    };
  }

  /**
   * Частично обновляет состояние модального окна и перерисовывает DOM.
   *
   * @param {EventsModalStatePatch} [patch]
   * @returns {EventsModalStateSnapshot}
   */
  function setState(patch) {
    if (patch && typeof patch === "object") {
      if (Array.isArray(patch.events)) {
        events = patch.events
          .filter((e) => e && typeof e === "object")
          .map((e) => ({ ...e }));
      }
      if ("activeTab" in patch && patch.activeTab !== undefined) {
        activeTab = normalizeEventTabId(patch.activeTab);
      }
      if (typeof patch.canManageEvents === "boolean") {
        canManageEvents = patch.canManageEvents;
      }
    }
    renderDom();
    return getState();
  }

  /**
   * Закрывает модальное окно и вызывает `options.onClose()`.
   */
  function close() {
    if (destroyed) return;
    if (typeof options.onClose === "function") {
      options.onClose();
    }
  }

  /**
   * Размонтирует модальное окно и очищает контейнер.
   */
  function destroy() {
    destroyed = true;
    if (root.parentNode === container && typeof container.removeChild === "function") {
      container.removeChild(root);
    } else {
      replaceElementChildren(container, []);
    }
  }

  renderDom();
  replaceElementChildren(container, [root]);

  return {
    setState,
    getState,
    close,
    destroy,
  };
}
