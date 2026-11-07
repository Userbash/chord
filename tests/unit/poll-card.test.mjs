// @ts-check
/**
 * Unit-тесты карточки опроса и контроллера голосования в реальном времени (task-241 / FE-P1-27).
 */
import { describe, it } from "node:test";
import assert from "node:assert/strict";

import {
  MAX_POLL_ANSWERS,
  MAX_POLL_QUESTION_CHARS,
  computePollPercentages,
  createPollCardController,
} from "../../rust/gui-spike/ui/features/poll-card.js";

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

describe("Константы лимитов опроса", () => {
  it("экспортирует MAX_POLL_ANSWERS = 10 и MAX_POLL_QUESTION_CHARS = 300", () => {
    assert.equal(MAX_POLL_ANSWERS, 10);
    assert.equal(MAX_POLL_QUESTION_CHARS, 300);
  });
});

describe("computePollPercentages", () => {
  it("вычисляет округлённые проценты, отмечает лидирующий вариант и не мутирует вход", () => {
    const rawAnswers = [
      { answerId: 1, text: "Rust", votes: 7 },
      { answerId: "2", text: "TypeScript", votes: 2 },
      { answerId: 3, text: "C++", votes: 1 },
    ];
    const snapshot = rawAnswers.map((a) => ({ ...a }));

    const result = computePollPercentages(rawAnswers, 10);

    assert.deepEqual(result, [
      { answerId: "1", text: "Rust", votes: 7, percentage: 70, isLeading: true },
      { answerId: "2", text: "TypeScript", votes: 2, percentage: 20, isLeading: false },
      { answerId: "3", text: "C++", votes: 1, percentage: 10, isLeading: false },
    ]);
    assert.deepEqual(rawAnswers, snapshot);
  });

  it("возвращает 0% и isLeading=false при totalVotes = 0 или отсутствии голосов", () => {
    const rawAnswers = [
      { answerId: "a", text: "Вариант А", votes: 0 },
      { answerId: "b", text: "Вариант Б" },
    ];

    const result = computePollPercentages(rawAnswers, 0);

    assert.deepEqual(result, [
      { answerId: "a", text: "Вариант А", votes: 0, percentage: 0, isLeading: false },
      { answerId: "b", text: "Вариант Б", votes: 0, percentage: 0, isLeading: false },
    ]);
  });

  it("отмечает несколько вариантов как isLeading при равенстве максимальных голосов > 0", () => {
    const rawAnswers = [
      { answerId: "1", text: "Да", votes: 4 },
      { answerId: "2", text: "Нет", votes: 4 },
      { answerId: "3", text: "Воздержусь", votes: 1 },
    ];

    const result = computePollPercentages(rawAnswers, 9);

    assert.deepEqual(result, [
      { answerId: "1", text: "Да", votes: 4, percentage: 44, isLeading: true },
      { answerId: "2", text: "Нет", votes: 4, percentage: 44, isLeading: true },
      { answerId: "3", text: "Воздержусь", votes: 1, percentage: 11, isLeading: false },
    ]);
  });
});

describe("createPollCardController", () => {
  const SAMPLE_ANSWERS = [
    { answerId: "ans-1", text: "WebRTC Opus", votes: 6 },
    { answerId: "ans-2", text: "dave-mls", votes: 3 },
    { answerId: "ans-3", text: "xsalsa20_poly1305", votes: 1 },
  ];

  it("рендерит [data-poll-card] с role=group, вопрос, варианты с процентами, лидера и статус голосов", () => {
    const container = makeContainer();
    createPollCardController({
      container,
      question: "Какой кодек и транспорт использовать по умолчанию?",
      answers: SAMPLE_ANSWERS,
      allowMultiselect: false,
      isExpired: false,
    });

    const card = container.querySelector("[data-poll-card]");
    assert.ok(card, "корневой элемент [data-poll-card] должен существовать");
    assert.equal(card.getAttribute("role"), "group");

    const questionEl = container.querySelector("[data-poll-question]");
    assert.ok(questionEl);
    assert.equal(
      questionEl.textContent,
      "Какой кодек и транспорт использовать по умолчанию?",
    );

    const ans1 = container.querySelector('[data-poll-answer-id="ans-1"]');
    const ans2 = container.querySelector('[data-poll-answer-id="ans-2"]');
    const ans3 = container.querySelector('[data-poll-answer-id="ans-3"]');
    assert.ok(ans1 && ans2 && ans3);

    assert.equal(ans1.getAttribute("role"), "radio");
    assert.equal(ans1.getAttribute("aria-checked"), "false");
    assert.equal(ans1.getAttribute("data-poll-percent"), "60%");
    assert.equal(ans1.getAttribute("data-leading"), "true");

    assert.equal(ans2.getAttribute("role"), "radio");
    assert.equal(ans2.getAttribute("aria-checked"), "false");
    assert.equal(ans2.getAttribute("data-poll-percent"), "30%");
    assert.equal(ans2.getAttribute("data-leading"), null);

    assert.equal(ans3.getAttribute("data-poll-percent"), "10%");
    assert.equal(ans3.getAttribute("data-leading"), null);

    const voteBtn = container.querySelector("[data-poll-vote-btn]");
    assert.ok(voteBtn);
    assert.equal(voteBtn.textContent, "Голосовать");
    assert.equal(voteBtn.disabled, true);

    const statusEl = container.querySelector("[data-poll-status]");
    assert.ok(statusEl);
    assert.equal(statusEl.textContent, "Всего голосов: 10");
  });

  it("в режиме single-select заменяет выбор при клике и отправляет голос через onSubmitVote", () => {
    const container = makeContainer();
    /** @type {string[][]} */
    const submitted = [];

    createPollCardController({
      container,
      question: "Выберите один вариант",
      answers: SAMPLE_ANSWERS,
      allowMultiselect: false,
      onSubmitVote: (ids) => submitted.push(ids),
    });

    const ans1 = container.querySelector('[data-poll-answer-id="ans-1"]');
    assert.ok(ans1);
    ans1.click();

    assert.equal(
      container.querySelector('[data-poll-answer-id="ans-1"]')?.getAttribute("aria-checked"),
      "true",
    );
    assert.equal(container.querySelector("[data-poll-vote-btn]")?.disabled, false);

    // Клик по другому варианту в single-select заменяет предыдущий выбор
    const ans2 = container.querySelector('[data-poll-answer-id="ans-2"]');
    assert.ok(ans2);
    ans2.click();

    assert.equal(
      container.querySelector('[data-poll-answer-id="ans-1"]')?.getAttribute("aria-checked"),
      "false",
    );
    assert.equal(
      container.querySelector('[data-poll-answer-id="ans-2"]')?.getAttribute("aria-checked"),
      "true",
    );

    const voteBtn = container.querySelector("[data-poll-vote-btn]");
    assert.ok(voteBtn);
    assert.equal(voteBtn.disabled, false);
    voteBtn.click();

    assert.deepEqual(submitted, [["ans-2"]]);

    // Повторный клик по уже выбранному варианту в single-select снимает выбор
    container.querySelector('[data-poll-answer-id="ans-2"]')?.click();
    assert.equal(
      container.querySelector('[data-poll-answer-id="ans-2"]')?.getAttribute("aria-checked"),
      "false",
    );
    assert.equal(container.querySelector("[data-poll-vote-btn]")?.disabled, true);
  });

  it("в режиме multiselect (role=checkbox) переключает несколько вариантов вплоть до MAX_POLL_ANSWERS", () => {
    const container = makeContainer();
    const manyAnswers = Array.from({ length: 12 }, (_, idx) => ({
      answerId: `opt-${idx + 1}`,
      text: `Опция ${idx + 1}`,
      votes: 1,
    }));
    /** @type {string[][]} */
    const submitted = [];

    const ctrl = createPollCardController({
      container,
      question: "Выберите несколько вариантов",
      answers: manyAnswers,
      allowMultiselect: true,
      selectedAnswerIds: ["opt-1"],
      onSubmitVote: (ids) => submitted.push(ids),
    });

    const firstOpt = container.querySelector('[data-poll-answer-id="opt-1"]');
    assert.ok(firstOpt);
    assert.equal(firstOpt.getAttribute("role"), "checkbox");
    assert.equal(firstOpt.getAttribute("aria-checked"), "true");

    // Добавляем opt-2 и снимаем opt-1
    container.querySelector('[data-poll-answer-id="opt-2"]')?.click();
    assert.deepEqual(ctrl.getState().selectedAnswerIds, ["opt-1", "opt-2"]);

    container.querySelector('[data-poll-answer-id="opt-1"]')?.click();
    assert.deepEqual(ctrl.getState().selectedAnswerIds, ["opt-2"]);

    // Выбираем до MAX_POLL_ANSWERS (10)
    for (let i = 1; i <= 12; i += 1) {
      const id = `opt-${i}`;
      if (!ctrl.getState().selectedAnswerIds.includes(id)) {
        container.querySelector(`[data-poll-answer-id="${id}"]`)?.click();
      }
    }

    assert.equal(ctrl.getState().selectedAnswerIds.length, MAX_POLL_ANSWERS);

    container.querySelector("[data-poll-vote-btn]")?.click();
    assert.equal(submitted.length, 1);
    assert.equal(submitted[0]?.length, MAX_POLL_ANSWERS);
  });

  it("блокирует голосование и выбор при isExpired = true и показывает статус «Опрос завершён»", () => {
    const container = makeContainer();
    /** @type {string[][]} */
    const submitted = [];

    const ctrl = createPollCardController({
      container,
      question: "Архивный опрос",
      answers: SAMPLE_ANSWERS,
      isExpired: true,
      selectedAnswerIds: ["ans-1"],
      onSubmitVote: (ids) => submitted.push(ids),
    });

    const statusEl = container.querySelector("[data-poll-status]");
    assert.ok(statusEl);
    assert.equal(statusEl.textContent, "Опрос завершён");

    const voteBtn = container.querySelector("[data-poll-vote-btn]");
    assert.ok(voteBtn);
    assert.equal(voteBtn.disabled, true);

    // Клик по варианту в завершённом опросе не меняет выбранные варианты
    container.querySelector('[data-poll-answer-id="ans-2"]')?.click();
    assert.deepEqual(ctrl.getState().selectedAnswerIds, ["ans-1"]);

    voteBtn.click();
    assert.equal(submitted.length, 0);
  });

  it("поддерживает setState, getState и destroy", () => {
    const container = makeContainer();
    const ctrl = createPollCardController({
      container,
      question: "Исходный вопрос",
      answers: [
        { answerId: "1", text: "А", votes: 0 },
        { answerId: "2", text: "Б", votes: 0 },
      ],
      allowMultiselect: false,
      isExpired: false,
    });

    assert.equal(ctrl.getState().question, "Исходный вопрос");
    assert.equal(
      container.querySelector("[data-poll-status]")?.textContent,
      "Всего голосов: 0",
    );

    ctrl.setState({
      question: "Обновлённый вопрос",
      answers: [
        { answerId: "1", text: "А", votes: 3 },
        { answerId: "2", text: "Б", votes: 1 },
      ],
      allowMultiselect: true,
      selectedAnswerIds: ["1", "2"],
    });

    assert.equal(ctrl.getState().question, "Обновлённый вопрос");
    assert.equal(ctrl.getState().allowMultiselect, true);
    assert.deepEqual(ctrl.getState().selectedAnswerIds, ["1", "2"]);
    assert.equal(
      container.querySelector("[data-poll-question]")?.textContent,
      "Обновлённый вопрос",
    );
    assert.equal(
      container.querySelector('[data-poll-answer-id="1"]')?.getAttribute("data-poll-percent"),
      "75%",
    );
    assert.equal(
      container.querySelector('[data-poll-answer-id="1"]')?.getAttribute("data-leading"),
      "true",
    );
    assert.equal(
      container.querySelector("[data-poll-status]")?.textContent,
      "Всего голосов: 4",
    );

    ctrl.setState({ isExpired: true });
    assert.equal(
      container.querySelector("[data-poll-status]")?.textContent,
      "Опрос завершён",
    );
    assert.equal(container.querySelector("[data-poll-vote-btn]")?.disabled, true);

    ctrl.destroy();
    assert.equal(container.querySelector("[data-poll-card]"), null);
  });
});
