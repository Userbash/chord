// @ts-check
/**
 * Unit-тесты модального окна событий сервера и контроллера создания события (task-247 / FE-P0-11).
 */
import { describe, it } from "node:test";
import assert from "node:assert/strict";

import {
  EVENT_FILTER_TABS_RU,
  filterEventsByTab,
  createEventsModalController,
} from "../../rust/gui-spike/ui/features/events-modal.js";

/* ------------------------------------------------------------------ *
 * Минимальный DOM-шим для изолированных unit-тестов
 * ------------------------------------------------------------------ */

class FakeElement {
  /**
   * @param {string} tagName
   * @param {any} [ownerDocument]
   */
  constructor(tagName, ownerDocument = null) {
    this.tagName = String(tagName).toUpperCase();
    this.ownerDocument = ownerDocument;
    /** @type {Map<string, string>} */
    this._attrs = new Map();
    /** @type {FakeElement[]} */
    this.children = [];
    /** @type {FakeElement | null} */
    this.parentNode = null;
    this._text = "";
    this._value = "";
    this._disabled = false;
    this.selected = false;
    /** @type {Map<string, Array<(event: any) => void>>} */
    this._listeners = new Map();
  }

  /** @returns {string} */
  get value() {
    if (this._value !== "") return this._value;
    if (this._attrs.has("value")) return this._attrs.get("value") ?? "";
    if (this.tagName === "SELECT") {
      const selectedOpt = this.children.find((c) => c.tagName === "OPTION" && c.selected);
      if (selectedOpt) return selectedOpt.value;
      const firstOpt = this.children.find((c) => c.tagName === "OPTION");
      if (firstOpt) return firstOpt.value;
    }
    return "";
  }

  /** @param {string | number} val */
  set value(val) {
    this._value = String(val ?? "");
  }

  /** @returns {boolean} */
  get disabled() {
    return this._disabled || this._attrs.has("disabled");
  }

  /** @param {boolean} val */
  set disabled(val) {
    this._disabled = Boolean(val);
    if (this._disabled) {
      this._attrs.set("disabled", "");
    } else {
      this._attrs.delete("disabled");
    }
  }

  /** @returns {string} */
  get textContent() {
    if (this.children.length > 0) {
      return this.children.map((c) => c.textContent).join("");
    }
    return this._text;
  }

  /** @param {string} val */
  set textContent(val) {
    this.children = [];
    this._text = String(val ?? "");
  }

  /**
   * @param {string} name
   * @param {string} value
   */
  setAttribute(name, value) {
    const str = String(value);
    this._attrs.set(name, str);
    if (name === "disabled") this._disabled = true;
    if (name === "value" && this._value === "") this._value = str;
  }

  /**
   * @param {string} name
   * @returns {string | null}
   */
  getAttribute(name) {
    return this._attrs.has(name) ? (this._attrs.get(name) ?? null) : null;
  }

  /**
   * @param {string} name
   * @returns {boolean}
   */
  hasAttribute(name) {
    return this._attrs.has(name);
  }

  /** @param {string} name */
  removeAttribute(name) {
    this._attrs.delete(name);
    if (name === "disabled") this._disabled = false;
  }

  /** @param {FakeElement} child */
  appendChild(child) {
    if (child.parentNode) {
      child.parentNode.removeChild(child);
    }
    child.parentNode = this;
    if (!child.ownerDocument && this.ownerDocument) {
      child.ownerDocument = this.ownerDocument;
    }
    this.children.push(child);
    return child;
  }

  /** @param {FakeElement} child */
  removeChild(child) {
    this.children = this.children.filter((c) => c !== child);
    child.parentNode = null;
    return child;
  }

  remove() {
    if (this.parentNode) {
      this.parentNode.removeChild(this);
    }
  }

  /** @param {...FakeElement} nodes */
  replaceChildren(...nodes) {
    for (const child of this.children) {
      child.parentNode = null;
    }
    this.children = [];
    this._text = "";
    for (const node of nodes) {
      this.appendChild(node);
    }
  }

  /**
   * @param {string} type
   * @param {(event: any) => void} listener
   */
  addEventListener(type, listener) {
    if (!this._listeners.has(type)) {
      this._listeners.set(type, []);
    }
    this._listeners.get(type)?.push(listener);
  }

  /**
   * @param {string} type
   * @param {Record<string, any>} [extra]
   */
  dispatchEvent(type, extra = {}) {
    const event = {
      type,
      target: this,
      currentTarget: this,
      defaultPrevented: false,
      preventDefault() {
        this.defaultPrevented = true;
      },
      stopPropagation() {},
      ...extra,
    };
    for (const listener of this._listeners.get(type) ?? []) {
      listener(event);
    }
    return !event.defaultPrevented;
  }

  click() {
    if (this.disabled) return false;
    return this.dispatchEvent("click");
  }

  /**
   * @param {string} selector
   * @returns {FakeElement | null}
   */
  querySelector(selector) {
    return this.querySelectorAll(selector)[0] ?? null;
  }

  /**
   * @param {string} selector
   * @returns {FakeElement[]}
   */
  querySelectorAll(selector) {
    /** @type {FakeElement[]} */
    const out = [];
    /** @param {FakeElement} el */
    const walk = (el) => {
      for (const child of el.children) {
        if (matchesSelector(child, selector)) out.push(child);
        walk(child);
      }
    };
    walk(this);
    return out;
  }
}

/**
 * @param {FakeElement} el
 * @param {string} selector
 * @returns {boolean}
 */
function matchesSelector(el, selector) {
  const tokens = selector.match(/\[[^\]]+\]|[#.][\w-]+|[a-zA-Z][\w-]*/g) ?? [];
  if (tokens.length === 0) return false;
  return tokens.every((token) => {
    if (token.startsWith("#")) return el.getAttribute("id") === token.slice(1);
    if (token.startsWith(".")) {
      const classes = (el.getAttribute("class") ?? "").split(/\s+/).filter(Boolean);
      return classes.includes(token.slice(1));
    }
    if (token.startsWith("[")) {
      const m = token.slice(1, -1).match(/^([\w-]+)(?:=(?:"([^"]*)"|'([^']*)'|([^\]]*)))?$/);
      if (!m || !m[1]) return false;
      const attrName = m[1];
      const expected = m[2] ?? m[3] ?? m[4];
      if (expected === undefined) return el.hasAttribute(attrName);
      return el.getAttribute(attrName) === expected;
    }
    return el.tagName === token.toUpperCase();
  });
}

function makeContainer() {
  const doc = {
    /** @param {string} tag */
    createElement(tag) {
      return new FakeElement(tag, doc);
    },
  };
  return new FakeElement("div", doc);
}

describe("EVENT_FILTER_TABS_RU", () => {
  it("содержит замороженный список вкладок upcoming, active, all с русскими подписями", () => {
    assert.equal(Object.isFrozen(EVENT_FILTER_TABS_RU), true);
    assert.deepEqual(EVENT_FILTER_TABS_RU, [
      { id: "upcoming", label: "Предстоящие" },
      { id: "active", label: "Активные" },
      { id: "all", label: "Все события" },
    ]);
  });
});

describe("filterEventsByTab", () => {
  const SAMPLE_EVENTS = [
    {
      id: "ev-30",
      name: "Вечерний стрим",
      status: 1,
      entityType: 2,
      scheduledStartTime: "2026-11-02T19:00:00Z",
      interested: false,
    },
    {
      id: "ev-20",
      name: "Утренний стендап",
      status: 1,
      entityType: 1,
      scheduledStartTime: "2026-11-01T09:00:00Z",
      interested: true,
    },
    {
      id: "ev-10",
      name: "Параллельный созвон",
      status: 1,
      entityType: 2,
      scheduledStartTime: "2026-11-01T09:00:00Z",
      interested: false,
    },
    {
      id: "ev-40",
      name: "Релизный эфир",
      status: 2,
      entityType: 1,
      scheduledStartTime: "2026-10-31T10:00:00Z",
      interested: true,
    },
    {
      id: "ev-50",
      name: "Завершённый митап",
      status: 3,
      entityType: 3,
      scheduledStartTime: "2026-10-30T15:00:00Z",
      interested: false,
    },
    {
      id: "ev-60",
      name: "Отменённый турнир",
      status: 4,
      entityType: 3,
      scheduledStartTime: "2026-11-05T12:00:00Z",
      interested: false,
    },
  ];

  it("фильтрует по вкладке upcoming (status === 1) и сортирует по scheduledStartTime, затем по id по возрастанию", () => {
    const result = filterEventsByTab(SAMPLE_EVENTS, "upcoming");
    assert.deepEqual(
      result.map((e) => e.id),
      ["ev-10", "ev-20", "ev-30"],
    );

    // При неизвестном или опущенном tabId по умолчанию используется upcoming
    assert.deepEqual(
      filterEventsByTab(SAMPLE_EVENTS).map((e) => e.id),
      ["ev-10", "ev-20", "ev-30"],
    );
    assert.deepEqual(
      filterEventsByTab(SAMPLE_EVENTS, "unknown-tab").map((e) => e.id),
      ["ev-10", "ev-20", "ev-30"],
    );
  });

  it("фильтрует по вкладке active (status === 2) и вкладке all (исключая status === 4), не мутируя вход", () => {
    const snapshot = SAMPLE_EVENTS.map((e) => ({ ...e }));

    const active = filterEventsByTab(SAMPLE_EVENTS, "active");
    assert.deepEqual(
      active.map((e) => e.id),
      ["ev-40"],
    );

    const all = filterEventsByTab(SAMPLE_EVENTS, "all");
    assert.deepEqual(
      all.map((e) => e.id),
      ["ev-50", "ev-40", "ev-10", "ev-20", "ev-30"],
    );

    assert.deepEqual(SAMPLE_EVENTS, snapshot);
    assert.deepEqual(filterEventsByTab(null, "all"), []);
  });
});

describe("createEventsModalController", () => {
  const INITIAL_EVENTS = [
    {
      id: "ev-2",
      name: "Код-ревью ядра",
      status: 1,
      entityType: 2,
      location: "Голосовой #dev",
      scheduledStartTime: "2026-11-03T16:00:00Z",
      interested: false,
    },
    {
      id: "ev-1",
      name: "Демо релиза",
      status: 1,
      entityType: 1,
      location: "Трибуна #main",
      scheduledStartTime: "2026-11-01T12:00:00Z",
      interested: true,
    },
    {
      id: "ev-3",
      name: "Живой разбор инцидента",
      status: 2,
      entityType: 1,
      location: "Трибуна #ops",
      scheduledStartTime: "2026-10-31T10:30:00Z",
      interested: false,
    },
    {
      id: "ev-4",
      name: "Отменённая встреча",
      status: 4,
      entityType: 3,
      location: "Zoom",
      scheduledStartTime: "2026-11-04T10:00:00Z",
      interested: false,
    },
  ];

  it("рендерит диалог События сервера, вкладки и карточки предстоящих событий с переключением вкладок", () => {
    const container = makeContainer();
    createEventsModalController({
      container: /** @type {any} */ (container),
      events: INITIAL_EVENTS,
    });

    const modal = container.querySelector("[data-events-modal]");
    assert.ok(modal, "должен рендериться [data-events-modal]");
    assert.equal(modal.getAttribute("role"), "dialog");
    assert.equal(modal.getAttribute("aria-modal"), "true");
    assert.equal(modal.getAttribute("aria-label"), "События сервера");

    const tabUpcoming = container.querySelector('[data-event-tab="upcoming"]');
    const tabActive = container.querySelector('[data-event-tab="active"]');
    const tabAll = container.querySelector('[data-event-tab="all"]');
    assert.ok(tabUpcoming);
    assert.ok(tabActive);
    assert.ok(tabAll);
    assert.equal(tabUpcoming.getAttribute("role"), "tab");
    assert.equal(tabUpcoming.getAttribute("aria-selected"), "true");
    assert.equal(tabActive.getAttribute("aria-selected"), "false");
    assert.equal(tabAll.getAttribute("aria-selected"), "false");

    const upcomingCards = container.querySelectorAll("[data-event-card-id]");
    assert.deepEqual(
      upcomingCards.map((c) => c.getAttribute("data-event-card-id")),
      ["ev-1", "ev-2"],
    );
    assert.ok(upcomingCards[0]?.textContent.includes("Демо релиза"));
    assert.ok(upcomingCards[1]?.textContent.includes("Код-ревью ядра"));

    // Без прав canManageEvents форма создания не рендерится
    assert.equal(container.querySelector("[data-event-create-form]"), null);

    // Переключение на вкладку active
    tabActive.click();
    assert.equal(
      container.querySelector('[data-event-tab="active"]')?.getAttribute("aria-selected"),
      "true",
    );
    assert.equal(
      container.querySelector('[data-event-tab="upcoming"]')?.getAttribute("aria-selected"),
      "false",
    );
    assert.deepEqual(
      container.querySelectorAll("[data-event-card-id]").map((c) => c.getAttribute("data-event-card-id")),
      ["ev-3"],
    );

    // Переключение на вкладку all (без отменённого ev-4)
    container.querySelector('[data-event-tab="all"]')?.click();
    assert.deepEqual(
      container.querySelectorAll("[data-event-card-id]").map((c) => c.getAttribute("data-event-card-id")),
      ["ev-3", "ev-1", "ev-2"],
    );
  });

  it("переключает RSVP на карточке события (aria-pressed и вызов onToggleRsvp)", () => {
    const container = makeContainer();
    /** @type {Array<{ eventId: string, nextInterested: boolean }>} */
    const rsvpCalls = [];

    createEventsModalController({
      container: /** @type {any} */ (container),
      events: INITIAL_EVENTS,
      activeTab: "upcoming",
      onToggleRsvp: (eventId, nextInterested) => {
        rsvpCalls.push({ eventId, nextInterested });
      },
    });

    const btnEv1 = container.querySelector('[data-event-rsvp-btn="ev-1"]');
    const btnEv2 = container.querySelector('[data-event-rsvp-btn="ev-2"]');
    assert.ok(btnEv1);
    assert.ok(btnEv2);
    assert.equal(btnEv1.getAttribute("aria-pressed"), "true");
    assert.equal(btnEv2.getAttribute("aria-pressed"), "false");

    btnEv2.click();
    assert.deepEqual(rsvpCalls, [{ eventId: "ev-2", nextInterested: true }]);
    assert.equal(
      container.querySelector('[data-event-rsvp-btn="ev-2"]')?.getAttribute("aria-pressed"),
      "true",
    );

    container.querySelector('[data-event-rsvp-btn="ev-1"]')?.click();
    assert.deepEqual(rsvpCalls, [
      { eventId: "ev-2", nextInterested: true },
      { eventId: "ev-1", nextInterested: false },
    ]);
    assert.equal(
      container.querySelector('[data-event-rsvp-btn="ev-1"]')?.getAttribute("aria-pressed"),
      "false",
    );
  });

  it("рендерит форму создания события при canManageEvents === true и валидирует обязательные поля", () => {
    const container = makeContainer();
    /** @type {Array<{ name: string, entityType: number, location: string, scheduledStartTime: string }>} */
    const createdPayloads = [];

    createEventsModalController({
      container: /** @type {any} */ (container),
      events: INITIAL_EVENTS,
      canManageEvents: true,
      onCreateEvent: (payload) => {
        createdPayloads.push(payload);
      },
    });

    const form = container.querySelector("[data-event-create-form]");
    const nameInput = container.querySelector("[data-event-name-input]");
    const entitySelect = container.querySelector("[data-event-entity-select]");
    const locationInput = container.querySelector("[data-event-location-input]");
    const startInput = container.querySelector("[data-event-start-input]");
    const submitBtn = container.querySelector("[data-event-create-submit]");

    assert.ok(form);
    assert.ok(nameInput);
    assert.ok(entitySelect);
    assert.ok(locationInput);
    assert.ok(startInput);
    assert.ok(submitBtn);

    // Пустые поля — onCreateEvent не вызывается
    submitBtn.click();
    assert.equal(createdPayloads.length, 0);

    // Только название без времени начала — не вызывается
    nameInput.value = "  Хакатон Rust  ";
    submitBtn.click();
    assert.equal(createdPayloads.length, 0);

    // Заполняем все поля и отправляем
    entitySelect.value = "3";
    locationInput.value = "  Конференц-зал А  ";
    startInput.value = "  2026-11-10T18:00:00Z  ";
    submitBtn.click();

    assert.deepEqual(createdPayloads, [
      {
        name: "Хакатон Rust",
        entityType: 3,
        location: "Конференц-зал А",
        scheduledStartTime: "2026-11-10T18:00:00Z",
      },
    ]);
  });

  it("поддерживает кнопку закрытия [data-events-close-btn], setState, getState, close и destroy", () => {
    const container = makeContainer();
    let closeCalls = 0;

    const ctrl = createEventsModalController({
      container: /** @type {any} */ (container),
      events: INITIAL_EVENTS,
      activeTab: "upcoming",
      canManageEvents: false,
      onClose: () => {
        closeCalls += 1;
      },
    });

    assert.equal(ctrl.getState().activeTab, "upcoming");
    assert.equal(ctrl.getState().canManageEvents, false);
    assert.equal(ctrl.getState().visibleEvents.length, 2);

    const closeBtn = container.querySelector("[data-events-close-btn]");
    assert.ok(closeBtn, "должна рендериться кнопка закрытия [data-events-close-btn]");
    closeBtn.click();
    assert.equal(closeCalls, 1);

    ctrl.setState({
      activeTab: "active",
      canManageEvents: true,
    });
    assert.equal(ctrl.getState().activeTab, "active");
    assert.equal(ctrl.getState().canManageEvents, true);
    assert.equal(ctrl.getState().visibleEvents.length, 1);
    assert.ok(container.querySelector("[data-event-create-form]"));

    ctrl.close();
    assert.equal(closeCalls, 2);

    ctrl.destroy();
    assert.equal(container.querySelector("[data-events-modal]"), null);
  });
});
