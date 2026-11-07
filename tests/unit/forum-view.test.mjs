// @ts-check
/**
 * Unit-тесты контроллера форум-канала и формы создания публикации (task-240 / FE-P0-10)
 * (`rust/gui-spike/ui/features/forum-view.js`).
 */
import { describe, it } from "node:test";
import assert from "node:assert/strict";

import {
  FORUM_SORT_ORDERS,
  FORUM_LAYOUT_VIEWS,
  MAX_FORUM_POST_TAGS,
  sortAndFilterForumPosts,
  createForumViewController,
} from "../../rust/gui-spike/ui/features/forum-view.js";

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
    this._checked = false;
    this._disabled = false;
    /** @type {Map<string, Array<(event: any) => void>>} */
    this._listeners = new Map();
  }

  /** @returns {string} */
  get value() {
    return this._value;
  }

  /** @param {string} val */
  set value(val) {
    this._value = String(val ?? "");
    this._attrs.set("value", this._value);
  }

  /** @returns {boolean} */
  get checked() {
    return this._checked;
  }

  /** @param {boolean} val */
  set checked(val) {
    this._checked = Boolean(val);
    if (this._checked) {
      this._attrs.set("checked", "");
    } else {
      this._attrs.delete("checked");
    }
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
    for (const child of this.children) {
      child.parentNode = null;
    }
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
    if (name === "value") this._value = str;
    if (name === "disabled") this._disabled = true;
    if (name === "checked") this._checked = true;
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
    if (name === "checked") this._checked = false;
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

const SAMPLE_TAGS = [
  { id: "tag-1", name: "Баги" },
  { id: "tag-2", name: "Идеи" },
  { id: "tag-3", name: "Помощь" },
  { id: "tag-4", name: "Релизы" },
  { id: "tag-5", name: "Голос" },
  { id: "tag-6", name: "Дизайн" },
];

const SAMPLE_POSTS = [
  {
    id: "post-10",
    title: "Падает звук при демонстрации экрана",
    appliedTags: ["tag-1", "tag-5"],
    pinned: false,
    createdAtTimestamp: 1_000,
    lastActivityTimestamp: 5_000,
    messageCount: 14,
  },
  {
    id: "post-20",
    title: "Правила публикации в форуме",
    appliedTags: ["tag-3"],
    pinned: true,
    createdAtTimestamp: 500,
    lastActivityTimestamp: 2_000,
    messageCount: 2,
  },
  {
    id: "post-30",
    title: "Новые карточки для галереи и звук",
    appliedTags: ["tag-1", "tag-2", "tag-5"],
    pinned: false,
    createdAtTimestamp: 4_000,
    lastActivityTimestamp: 3_500,
    messageCount: 9,
  },
  {
    id: "post-40",
    title: "Обновление клиента 1.2",
    appliedTags: ["tag-4"],
    pinned: true,
    createdAtTimestamp: 3_000,
    lastActivityTimestamp: 4_000,
    messageCount: 6,
  },
];

describe("FORUM_SORT_ORDERS, FORUM_LAYOUT_VIEWS, MAX_FORUM_POST_TAGS и sortAndFilterForumPosts", () => {
  it("экспортирует замороженные константы FORUM_SORT_ORDERS, FORUM_LAYOUT_VIEWS и MAX_FORUM_POST_TAGS = 5", () => {
    assert.equal(Object.isFrozen(FORUM_SORT_ORDERS), true);
    assert.deepEqual(FORUM_SORT_ORDERS, {
      LATEST_ACTIVITY: 0,
      CREATION_DATE: 1,
    });

    assert.equal(Object.isFrozen(FORUM_LAYOUT_VIEWS), true);
    assert.deepEqual(FORUM_LAYOUT_VIEWS, {
      LIST: 1,
      GALLERY: 2,
    });

    assert.equal(MAX_FORUM_POST_TAGS, 5);
  });

  it("сортирует закреплённые публикации первыми, затем по lastActivityTimestamp (sortOrder=0/omitted) или createdAtTimestamp (sortOrder=1) по убыванию с tie-break по String(id) desc и не мутирует вход", () => {
    const input = [
      {
        id: "post-a",
        title: "Alpha",
        appliedTags: ["tag-1"],
        pinned: false,
        createdAtTimestamp: 2_000,
        lastActivityTimestamp: 3_000,
        messageCount: 1,
      },
      {
        id: "post-c",
        title: "Charlie",
        appliedTags: ["tag-1"],
        pinned: false,
        createdAtTimestamp: 2_000,
        lastActivityTimestamp: 3_000,
        messageCount: 5,
      },
      {
        id: "post-b",
        title: "Bravo",
        appliedTags: ["tag-2"],
        pinned: true,
        createdAtTimestamp: 100,
        lastActivityTimestamp: 500,
        messageCount: 2,
      },
      {
        id: "post-d",
        title: "Delta",
        appliedTags: ["tag-2"],
        pinned: false,
        createdAtTimestamp: 9_000,
        lastActivityTimestamp: 1_500,
        messageCount: 4,
      },
    ];
    const snapshot = structuredClone(input);

    // По умолчанию (sortOrder не передан -> LATEST_ACTIVITY = 0):
    // 1) post-b (pinned=true)
    // 2) post-c и post-a имеют одинаковый lastActivityTimestamp=3000 -> tie-break String(id) desc: post-c перед post-a
    // 3) post-d (lastActivityTimestamp=1500)
    const byActivity = sortAndFilterForumPosts(input);
    assert.deepEqual(
      byActivity.map((p) => p.id),
      ["post-b", "post-c", "post-a", "post-d"],
    );

    // При sortOrder === CREATION_DATE (1):
    // 1) post-b (pinned=true)
    // 2) post-d (createdAtTimestamp=9000)
    // 3) post-c и post-a (createdAtTimestamp=2000, tie-break desc -> post-c, post-a)
    const byCreation = sortAndFilterForumPosts(input, {
      sortOrder: FORUM_SORT_ORDERS.CREATION_DATE,
    });
    assert.deepEqual(
      byCreation.map((p) => p.id),
      ["post-b", "post-d", "post-c", "post-a"],
    );

    // Проверка отсутствия мутации входного массива и вложенных объектов
    assert.deepEqual(input, snapshot);
    if (byActivity[0]) {
      byActivity[0].title = "Mutated";
      byActivity[0].appliedTags?.push("tag-999");
    }
    assert.deepEqual(input, snapshot);
  });

  it("фильтрует публикации по всем выбранным тегам (ALL selectedTagIds) и регистронезависимому подстрочному поиску по title", () => {
    // Фильтр по одному тегу tag-1 -> post-10 и post-30
    const withTag1 = sortAndFilterForumPosts(SAMPLE_POSTS, {
      selectedTagIds: ["tag-1"],
    });
    assert.deepEqual(
      withTag1.map((p) => p.id),
      ["post-10", "post-30"],
    );

    // Фильтр по двум тегам ["tag-1", "tag-2"] -> post.appliedTags должен содержать ОБА тега (только post-30)
    const withTag1And2 = sortAndFilterForumPosts(SAMPLE_POSTS, {
      selectedTagIds: ["tag-1", "tag-2"],
    });
    assert.deepEqual(
      withTag1And2.map((p) => p.id),
      ["post-30"],
    );

    // Поиск по подстроке в разном регистре + фильтр по тегам
    const bySearchAndTag = sortAndFilterForumPosts(SAMPLE_POSTS, {
      selectedTagIds: ["tag-1", "tag-5"],
      searchQuery: "  ГАЛЕРЕИ ",
    });
    assert.deepEqual(
      bySearchAndTag.map((p) => p.id),
      ["post-30"],
    );

    // Невалидный вход -> пустой массив
    assert.deepEqual(sortAndFilterForumPosts(null), []);
  });
});

describe("createForumViewController — рендер региона, фильтра тегов, карточек и формы создания публикации", () => {
  it("рендерит регион [data-forum-view] (role='region', aria-label='Форум'), фильтр тегов и отсортированные карточки публикаций", () => {
    const container = makeContainer();
    /** @type {string[]} */
    const selectedPosts = [];

    createForumViewController({
      container: /** @type {any} */ (container),
      posts: SAMPLE_POSTS,
      availableTags: SAMPLE_TAGS,
      onSelectPost: (postId) => selectedPosts.push(postId),
    });

    const viewEl = container.querySelector("[data-forum-view]");
    assert.ok(viewEl, "должен рендериться корень [data-forum-view]");
    assert.equal(viewEl.getAttribute("role"), "region");
    assert.equal(viewEl.getAttribute("aria-label"), "Форум");

    const tagButtons = container.querySelectorAll("[data-forum-tag-filter]");
    assert.equal(tagButtons.length, SAMPLE_TAGS.length);
    assert.equal(tagButtons[0]?.getAttribute("data-forum-tag-filter"), "tag-1");
    assert.equal(tagButtons[0]?.getAttribute("aria-pressed"), "false");
    assert.ok(tagButtons[0]?.textContent.includes("Баги"));

    // Pinned первыми: post-40 (lastActivity=4000), post-20 (lastActivity=2000), затем post-10 (5000), post-30 (3500)
    const cards = container.querySelectorAll("[data-forum-post-id]");
    assert.deepEqual(
      cards.map((c) => c.getAttribute("data-forum-post-id")),
      ["post-40", "post-20", "post-10", "post-30"],
    );

    const firstCard = cards[0];
    assert.ok(firstCard);
    assert.ok(firstCard.textContent.includes("Обновление клиента 1.2"));
    assert.ok(firstCard.textContent.includes("6"));
    assert.ok(
      firstCard.getAttribute("data-pinned") === "true" ||
        firstCard.querySelector('[data-pinned="true"]') !== null,
      "закреплённая карточка должна иметь индикатор [data-pinned='true']",
    );

    const unpinnedCard = cards[2];
    assert.ok(unpinnedCard);
    assert.ok(unpinnedCard.textContent.includes("Падает звук при демонстрации экрана"));
    assert.ok(unpinnedCard.textContent.includes("14"));
    assert.equal(unpinnedCard.getAttribute("data-pinned"), null);
    assert.equal(unpinnedCard.querySelector('[data-pinned="true"]'), null);

    unpinnedCard.click();
    firstCard.click();
    assert.deepEqual(selectedPosts, ["post-10", "post-40"]);
  });

  it("клик по кнопке [data-forum-tag-filter='<tagId>'] переключает aria-pressed='true'|'false' и фильтрует карточки по всем активным тегам", () => {
    const container = makeContainer();
    createForumViewController({
      container: /** @type {any} */ (container),
      posts: SAMPLE_POSTS,
      availableTags: SAMPLE_TAGS,
    });

    const btnTag1 = container.querySelector('[data-forum-tag-filter="tag-1"]');
    assert.ok(btnTag1);
    assert.equal(btnTag1.getAttribute("aria-pressed"), "false");

    // Включаем фильтр по tag-1
    btnTag1.click();
    const btnTag1After = container.querySelector('[data-forum-tag-filter="tag-1"]');
    assert.equal(btnTag1After?.getAttribute("aria-pressed"), "true");
    assert.deepEqual(
      container.querySelectorAll("[data-forum-post-id]").map((c) => c.getAttribute("data-forum-post-id")),
      ["post-10", "post-30"],
    );

    // Включаем дополнительно tag-2 -> должен остаться только post-30 (содержит и tag-1, и tag-2)
    const btnTag2 = container.querySelector('[data-forum-tag-filter="tag-2"]');
    btnTag2?.click();
    assert.equal(
      container.querySelector('[data-forum-tag-filter="tag-2"]')?.getAttribute("aria-pressed"),
      "true",
    );
    assert.deepEqual(
      container.querySelectorAll("[data-forum-post-id]").map((c) => c.getAttribute("data-forum-post-id")),
      ["post-30"],
    );

    // Снимаем оба фильтра -> все 4 карточки возвращаются
    container.querySelector('[data-forum-tag-filter="tag-2"]')?.click();
    container.querySelector('[data-forum-tag-filter="tag-1"]')?.click();
    assert.equal(
      container.querySelector('[data-forum-tag-filter="tag-1"]')?.getAttribute("aria-pressed"),
      "false",
    );
    assert.equal(container.querySelectorAll("[data-forum-post-id]").length, 4);
  });

  it("форма создания публикации [data-forum-new-post-form] запрещает выбор более MAX_FORUM_POST_TAGS (5) тегов и отправляет обрезанные title/content/appliedTags только при непустых полях", () => {
    const container = makeContainer();
    /** @type {Array<{ title: string, content: string, appliedTags: string[] }>} */
    const createdPosts = [];

    createForumViewController({
      container: /** @type {any} */ (container),
      posts: SAMPLE_POSTS,
      availableTags: SAMPLE_TAGS,
      onCreatePost: (payload) => createdPosts.push(payload),
    });

    const form = container.querySelector("[data-forum-new-post-form]");
    assert.ok(form, "должна рендериться форма [data-forum-new-post-form]");

    const titleInput = form.querySelector("[data-forum-title-input]");
    const bodyInput = form.querySelector("[data-forum-body-input]");
    const submitBtn = form.querySelector("[data-forum-submit-btn]");
    assert.ok(titleInput, "должно быть поле [data-forum-title-input]");
    assert.ok(bodyInput, "должно быть поле [data-forum-body-input]");
    assert.ok(submitBtn, "должна быть кнопка [data-forum-submit-btn]");

    const checkboxes = form.querySelectorAll('input[type="checkbox"]');
    assert.equal(checkboxes.length, 6);

    // Пустые или пробельные поля не вызывают onCreatePost
    titleInput.value = "   ";
    bodyInput.value = "Текст сообщения";
    submitBtn.click();
    assert.equal(createdPosts.length, 0);

    titleInput.value = "Заголовок";
    bodyInput.value = "   \n ";
    submitBtn.click();
    assert.equal(createdPosts.length, 0);

    // Выбираем первые 5 тегов (MAX_FORUM_POST_TAGS = 5)
    for (let i = 0; i < MAX_FORUM_POST_TAGS; i += 1) {
      checkboxes[i]?.click();
      assert.equal(checkboxes[i]?.checked, true);
    }

    // 6-й чекбокс должен быть заблокирован и не должен выбираться ни кликом, ни событием change
    const sixthCheckbox = checkboxes[5];
    assert.ok(sixthCheckbox);
    assert.equal(sixthCheckbox.disabled, true);
    sixthCheckbox.click();
    sixthCheckbox.dispatchEvent("change", { target: { checked: true } });
    assert.equal(sixthCheckbox.checked, false);

    // Снимаем 2-й тег -> 6-й разблокируется и его можно выбрать
    checkboxes[1]?.click();
    assert.equal(checkboxes[1]?.checked, false);
    assert.equal(sixthCheckbox.disabled, false);
    sixthCheckbox.click();
    assert.equal(sixthCheckbox.checked, true);

    // Заполняем поля с пробелами по краям и отправляем
    titleInput.value = "  Новая тема форума  ";
    bodyInput.value = "  Подробное описание первого сообщения  ";
    submitBtn.click();

    assert.equal(createdPosts.length, 1);
    assert.deepEqual(createdPosts[0], {
      title: "Новая тема форума",
      content: "Подробное описание первого сообщения",
      appliedTags: ["tag-1", "tag-3", "tag-4", "tag-5", "tag-6"],
    });
  });

  it("поддерживает setState, getState и destroy", () => {
    const container = makeContainer();
    const ctrl = createForumViewController({
      container: /** @type {any} */ (container),
      posts: SAMPLE_POSTS,
      availableTags: SAMPLE_TAGS,
      sortOrder: FORUM_SORT_ORDERS.LATEST_ACTIVITY,
      layoutView: FORUM_LAYOUT_VIEWS.LIST,
    });

    const initialState = ctrl.getState();
    assert.equal(initialState.sortOrder, FORUM_SORT_ORDERS.LATEST_ACTIVITY);
    assert.equal(initialState.layoutView, FORUM_LAYOUT_VIEWS.LIST);

    // Меняем сортировку на CREATION_DATE и задаём поисковый запрос через setState
    const updatedState = ctrl.setState({
      sortOrder: FORUM_SORT_ORDERS.CREATION_DATE,
      layoutView: FORUM_LAYOUT_VIEWS.GALLERY,
      searchQuery: "звук",
    });
    assert.equal(updatedState.sortOrder, FORUM_SORT_ORDERS.CREATION_DATE);
    assert.equal(updatedState.layoutView, FORUM_LAYOUT_VIEWS.GALLERY);
    assert.equal(updatedState.searchQuery, "звук");

    // Среди постов со словом "звук" (post-10 created=1000, post-30 created=4000) при CREATION_DATE первым идёт post-30
    assert.deepEqual(
      container.querySelectorAll("[data-forum-post-id]").map((c) => c.getAttribute("data-forum-post-id")),
      ["post-30", "post-10"],
    );

    ctrl.destroy();
    assert.equal(container.querySelector("[data-forum-view]"), null);
  });
});
