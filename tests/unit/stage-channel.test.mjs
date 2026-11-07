// @ts-check
/**
 * Unit-тесты контроллера трибуны (Stage Channel) и очереди спикеров (task-238 / FE-P1-26).
 */
import { describe, it } from "node:test";
import assert from "node:assert/strict";

import {
  STAGE_ROLES,
  partitionStageParticipants,
  createStageChannelController,
} from "../../rust/gui-spike/ui/features/stage-channel.js";

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
    this._disabled = false;
    /** @type {Map<string, Array<(event: any) => void>>} */
    this._listeners = new Map();
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

describe("STAGE_ROLES", () => {
  it("содержит роли speaker, audience, requesting и заморожен", () => {
    assert.deepEqual(
      { ...STAGE_ROLES },
      {
        SPEAKER: "speaker",
        AUDIENCE: "audience",
        REQUESTING: "requesting",
      },
    );
    assert.equal(Object.isFrozen(STAGE_ROLES), true);
  });
});

describe("partitionStageParticipants", () => {
  it("разбивает участников на speakers, requesting и audience с сортировкой по русскому алфавиту", () => {
    const raw = [
      { userId: "u-1", displayName: "Яков", role: "speaker", speaking: true },
      { userId: "u-2", displayName: "Анна", role: "speaker", speaking: false },
      { userId: "u-3", displayName: "Дмитрий", role: "requesting", raisedHandAt: "2026-10-31T01:00:00Z" },
      { userId: "u-4", displayName: "Борис", role: "requesting", raisedHandAt: "2026-10-31T01:02:00Z" },
      { userId: "u-5", displayName: "Сергей", role: "audience" },
      { userId: "u-6", displayName: "Виктор", role: "audience" },
    ];

    const grouped = partitionStageParticipants(raw);

    assert.deepEqual(
      grouped.speakers.map((p) => p.userId),
      ["u-2", "u-1"],
    );
    assert.deepEqual(
      grouped.requesting.map((p) => p.userId),
      ["u-4", "u-3"],
    );
    assert.deepEqual(
      grouped.audience.map((p) => p.userId),
      ["u-6", "u-5"],
    );
  });

  it("неизвестные роли по умолчанию попадают в audience и входной массив не мутируется", () => {
    const raw = [
      { userId: "u-z", displayName: "Зоя", role: "unknown-role" },
      { userId: "u-a", displayName: "Алексей", role: "" },
      { userId: "u-s", displayName: "Мария", role: "speaker" },
    ];
    const snapshot = raw.map((item) => ({ ...item }));

    const grouped = partitionStageParticipants(raw);

    assert.deepEqual(
      grouped.speakers.map((p) => p.userId),
      ["u-s"],
    );
    assert.deepEqual(grouped.requesting, []);
    assert.deepEqual(
      grouped.audience.map((p) => p.userId),
      ["u-a", "u-z"],
    );
    assert.deepEqual(raw, snapshot);
  });
});

describe("createStageChannelController", () => {
  const SAMPLE_PARTICIPANTS = [
    { userId: "sp-2", displayName: "Ольга", role: "speaker", speaking: true, muted: false },
    { userId: "sp-1", displayName: "Игорь", role: "speaker", speaking: false, muted: true },
    { userId: "req-1", displayName: "Павел", role: "requesting", raisedHandAt: "2026-10-31T01:00:00Z" },
    { userId: "aud-2", displayName: "Татьяна", role: "audience" },
    { userId: "aud-1", displayName: "Елена", role: "audience" },
  ];

  it("рендерит регион Трибуна, тему, секции со счётчиками и элементы участников", () => {
    const container = makeContainer();
    createStageChannelController({
      container,
      topic: "Архитектурный разбор голосового движка",
      participants: SAMPLE_PARTICIPANTS,
      currentUserId: "aud-1",
      isModerator: false,
    });

    const root = container.querySelector("[data-stage-channel]");
    assert.ok(root, "корневой элемент [data-stage-channel] должен существовать");
    assert.equal(root.getAttribute("role"), "region");
    assert.equal(root.getAttribute("aria-label"), "Трибуна");

    const topicEl = container.querySelector("[data-stage-topic]");
    assert.ok(topicEl);
    assert.equal(topicEl.textContent, "Архитектурный разбор голосового движка");

    const speakersSection = container.querySelector("[data-stage-speakers]");
    assert.ok(speakersSection);
    assert.ok(speakersSection.textContent.includes("Спикеры (2)"));

    const speakerIgor = container.querySelector('[data-stage-speaker-id="sp-1"]');
    const speakerOlga = container.querySelector('[data-stage-speaker-id="sp-2"]');
    assert.ok(speakerIgor);
    assert.ok(speakerOlga);
    assert.equal(speakerIgor.getAttribute("data-speaking"), null);
    assert.equal(speakerOlga.getAttribute("data-speaking"), "true");

    const requestsSection = container.querySelector("[data-stage-requests]");
    assert.ok(requestsSection);
    assert.ok(requestsSection.textContent.includes("Запросы на выступление (1)"));

    const audienceSection = container.querySelector("[data-stage-audience]");
    assert.ok(audienceSection);
    assert.ok(audienceSection.textContent.includes("Слушатели (2)"));
    assert.ok(container.querySelector('[data-stage-listener-id="aud-1"]'));
    assert.ok(container.querySelector('[data-stage-listener-id="aud-2"]'));

    // Без прав модератора кнопки управления очередью не рендерятся
    assert.equal(container.querySelector('[data-move-to-audience="sp-1"]'), null);
    assert.equal(container.querySelector('[data-approve-speaker="req-1"]'), null);
  });

  it("показывает «Трибуна без темы», если тема не передана или пустая", () => {
    const container = makeContainer();
    const ctrl = createStageChannelController({
      container,
      participants: [],
    });

    const topicEl = container.querySelector("[data-stage-topic]");
    assert.ok(topicEl);
    assert.equal(topicEl.textContent, "Трибуна без темы");

    ctrl.setState({ topic: "   " });
    assert.equal(
      container.querySelector("[data-stage-topic]")?.textContent,
      "Трибуна без темы",
    );
  });

  it("рендерит кнопки модератора и вызывает onApproveSpeaker и onMoveToAudience", () => {
    const container = makeContainer();
    /** @type {string[]} */
    const approved = [];
    /** @type {string[]} */
    const moved = [];

    createStageChannelController({
      container,
      topic: "Модерация трибуны",
      participants: SAMPLE_PARTICIPANTS,
      currentUserId: "sp-1",
      isModerator: true,
      onApproveSpeaker: (userId) => approved.push(userId),
      onMoveToAudience: (userId) => moved.push(userId),
    });

    const approveBtn = container.querySelector('[data-approve-speaker="req-1"]');
    assert.ok(approveBtn, "кнопка одобрения спикера должна быть доступна модератору");
    approveBtn.click();
    assert.deepEqual(approved, ["req-1"]);

    const moveBtn = container.querySelector('[data-move-to-audience="sp-2"]');
    assert.ok(moveBtn, "кнопка перевода в слушатели должна быть доступна модератору");
    moveBtn.click();
    assert.deepEqual(moved, ["sp-2"]);
  });

  it("переключает поднятую руку текущего пользователя (aria-pressed и onToggleHand) и обрабатывает выход", () => {
    const container = makeContainer();
    /** @type {boolean[]} */
    const handEvents = [];
    let leaveCount = 0;

    const ctrl = createStageChannelController({
      container,
      participants: SAMPLE_PARTICIPANTS,
      currentUserId: "aud-1",
      onToggleHand: (raised) => handEvents.push(raised),
      onLeaveStage: () => {
        leaveCount += 1;
      },
    });

    const handBtn = container.querySelector("[data-stage-hand-btn]");
    assert.ok(handBtn);
    assert.equal(handBtn.getAttribute("aria-pressed"), "false");

    handBtn.click();
    assert.deepEqual(handEvents, [true]);
    assert.equal(
      container.querySelector("[data-stage-hand-btn]")?.getAttribute("aria-pressed"),
      "true",
    );

    // Если текущий пользователь уже в статусе requesting через setState
    ctrl.setState({ currentUserId: "req-1", participants: SAMPLE_PARTICIPANTS });
    const handBtnReq = container.querySelector("[data-stage-hand-btn]");
    assert.ok(handBtnReq);
    assert.equal(handBtnReq.getAttribute("aria-pressed"), "true");

    handBtnReq.click();
    assert.deepEqual(handEvents, [true, false]);
    assert.equal(
      container.querySelector("[data-stage-hand-btn]")?.getAttribute("aria-pressed"),
      "false",
    );

    const leaveBtn = container.querySelector("[data-stage-leave-btn]");
    assert.ok(leaveBtn);
    assert.ok(leaveBtn.textContent.includes("Покинуть трибуну"));
    leaveBtn.click();
    assert.equal(leaveCount, 1);
  });

  it("поддерживает setState, getState и destroy", () => {
    const container = makeContainer();
    const ctrl = createStageChannelController({
      container,
      topic: "Начальная тема",
      participants: [{ userId: "u-1", displayName: "Алиса", role: "audience" }],
      currentUserId: "u-1",
      isModerator: false,
    });

    assert.equal(ctrl.getState().topic, "Начальная тема");
    assert.equal(ctrl.getState().audience.length, 1);

    ctrl.setState({
      topic: "Обновлённая тема",
      isModerator: true,
      participants: [{ userId: "u-1", displayName: "Алиса", role: "speaker", speaking: true }],
    });

    assert.equal(ctrl.getState().topic, "Обновлённая тема");
    assert.equal(ctrl.getState().isModerator, true);
    assert.equal(ctrl.getState().speakers.length, 1);
    assert.equal(container.querySelector("[data-stage-topic]")?.textContent, "Обновлённая тема");
    assert.ok(container.querySelector('[data-stage-speaker-id="u-1"]'));
    assert.ok(container.querySelector('[data-move-to-audience="u-1"]'));

    ctrl.destroy();
    assert.equal(container.querySelector("[data-stage-channel]"), null);
  });
});
