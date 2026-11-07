// @ts-check
/**
 * Контроллер представления форум-канала и формы создания публикации (task-240 / FE-P0-10).
 *
 * Отвечает за:
 * - экспорт констант сортировки `FORUM_SORT_ORDERS`, режимов отображения `FORUM_LAYOUT_VIEWS`
 *   и лимита тегов публикации `MAX_FORUM_POST_TAGS = 5`;
 * - чистую функцию `sortAndFilterForumPosts(posts, options)` для фильтрации по всем
 *   выбранным тегам (`selectedTagIds`), регистронезависимого поиска по `title` и
 *   сортировки (закреплённые `pinned === true` первыми, затем по `lastActivityTimestamp`
 *   или `createdAtTimestamp` по убыванию с tie-break по `String(post.id)` `localeCompare` desc);
 * - контроллер `createForumViewController(options)`, рендерящий регион `[data-forum-view]`
 *   (`role="region"`, `aria-label="Форум"`), панель фильтра тегов `[data-forum-tag-filter="<tagId>"]`,
 *   карточки публикаций `[data-forum-post-id="<id>"]` и форму создания публикации
 *   `[data-forum-new-post-form]` с защитой от выбора более `MAX_FORUM_POST_TAGS` тегов.
 *
 * Оракул: `/home/sanya/re-deep/gateway-schema.md` (`default_sort_order`, `default_forum_layout`)
 * и `rust/gui-spike/src/forum.rs` (`MAX_FORUM_APPLIED_TAGS = 5`, `ForumSortOrder`).
 *
 * @module ui/features/forum-view
 */

/**
 * Коды порядка сортировки публикаций форум-канала.
 * @type {Readonly<{ LATEST_ACTIVITY: 0, CREATION_DATE: 1 }>}
 */
export const FORUM_SORT_ORDERS = Object.freeze({
  LATEST_ACTIVITY: 0,
  CREATION_DATE: 1,
});

/**
 * Режимы раскладки публикаций форум-канала.
 * @type {Readonly<{ LIST: 1, GALLERY: 2 }>}
 */
export const FORUM_LAYOUT_VIEWS = Object.freeze({
  LIST: 1,
  GALLERY: 2,
});

/**
 * Максимальное количество тегов, прикрепляемых к одной публикации форума.
 * @type {5}
 */
export const MAX_FORUM_POST_TAGS = 5;

/**
 * @typedef {object} ForumTagItem
 * @property {string} id
 * @property {string} name
 */

/**
 * @typedef {object} ForumPostItem
 * @property {string} id
 * @property {string} title
 * @property {string[]} [appliedTags]
 * @property {boolean} [pinned]
 * @property {number} [lastActivityTimestamp]
 * @property {number} [createdAtTimestamp]
 * @property {number} [messageCount]
 */

/**
 * @typedef {object} SortAndFilterForumPostsOptions
 * @property {number} [sortOrder]
 * @property {string[]} [selectedTagIds]
 * @property {string} [searchQuery]
 */

/**
 * @typedef {object} CreateForumPostPayload
 * @property {string} title
 * @property {string} content
 * @property {string[]} appliedTags
 */

/**
 * @typedef {object} ForumViewStatePatch
 * @property {ForumPostItem[]} [posts]
 * @property {ForumTagItem[]} [availableTags]
 * @property {string[]} [selectedTagIds]
 * @property {string} [searchQuery]
 * @property {number} [sortOrder]
 * @property {number} [layoutView]
 */

/**
 * @typedef {object} ForumViewStateSnapshot
 * @property {ForumPostItem[]} posts
 * @property {ForumPostItem[]} filteredPosts
 * @property {ForumTagItem[]} availableTags
 * @property {string[]} selectedTagIds
 * @property {string} searchQuery
 * @property {number} sortOrder
 * @property {number} layoutView
 * @property {string[]} composerTagIds
 */

/**
 * @typedef {object} ForumViewControllerOptions
 * @property {any} container
 * @property {ForumPostItem[]} [posts]
 * @property {ForumTagItem[]} [availableTags]
 * @property {string[]} [selectedTagIds]
 * @property {string} [searchQuery]
 * @property {number} [sortOrder]
 * @property {number} [layoutView]
 * @property {(postId: string) => void} [onSelectPost]
 * @property {(payload: CreateForumPostPayload) => void} [onCreatePost]
 */

/**
 * Создаёт поверхностную копию публикации форума без мутации исходного объекта.
 *
 * @param {ForumPostItem} post
 * @returns {ForumPostItem}
 */
function cloneForumPost(post) {
  return {
    ...post,
    appliedTags: Array.isArray(post.appliedTags) ? [...post.appliedTags] : [],
  };
}

/**
 * Нормализует числовую временную метку (при невалидном значении возвращает `0`).
 *
 * @param {unknown} rawValue
 * @returns {number}
 */
function normalizeTimestamp(rawValue) {
  return typeof rawValue === "number" && Number.isFinite(rawValue) ? rawValue : 0;
}

/**
 * Нормализует код сортировки форума (`0` — недавняя активность, `1` — дата создания).
 *
 * @param {unknown} rawSortOrder
 * @returns {number}
 */
function normalizeSortOrder(rawSortOrder) {
  return rawSortOrder === FORUM_SORT_ORDERS.CREATION_DATE
    ? FORUM_SORT_ORDERS.CREATION_DATE
    : FORUM_SORT_ORDERS.LATEST_ACTIVITY;
}

/**
 * Нормализует код раскладки форума (`1` — список, `2` — галерея).
 *
 * @param {unknown} rawLayoutView
 * @returns {number}
 */
function normalizeLayoutView(rawLayoutView) {
  return rawLayoutView === FORUM_LAYOUT_VIEWS.GALLERY
    ? FORUM_LAYOUT_VIEWS.GALLERY
    : FORUM_LAYOUT_VIEWS.LIST;
}

/**
 * Фильтрует и сортирует публикации форум-канала без мутации входного массива.
 *
 * Правила:
 * - если `selectedTagIds` непустой, публикация должна содержать ВСЕ выбранные теги в `post.appliedTags`;
 * - если `searchQuery` непустой (после `trim()`), заголовок `post.title` проверяется на
 *   регистронезависимое вхождение подстроки;
 * - закреплённые публикации (`post.pinned === true`) всегда идут первыми;
 * - внутри закреплённых/незакреплённых сортировка идёт по убыванию `lastActivityTimestamp`
 *   (при `sortOrder === 0` или не указан) либо `createdAtTimestamp` (при `sortOrder === 1`);
 * - при равенстве временных меток применяется tie-break по убыванию `String(post.id)` через `localeCompare`.
 *
 * @param {ReadonlyArray<ForumPostItem> | null | undefined} posts
 * @param {SortAndFilterForumPostsOptions} [options]
 * @returns {ForumPostItem[]}
 */
export function sortAndFilterForumPosts(posts, options = {}) {
  if (!Array.isArray(posts)) {
    return [];
  }

  const sortOrder = normalizeSortOrder(options?.sortOrder);
  const requiredTags = Array.isArray(options?.selectedTagIds)
    ? options.selectedTagIds.filter((tagId) => typeof tagId === "string" && tagId.length > 0)
    : [];
  const normalizedQuery =
    typeof options?.searchQuery === "string" ? options.searchQuery.trim().toLowerCase() : "";

  /** @type {ForumPostItem[]} */
  const filtered = [];

  for (const post of posts) {
    if (!post || typeof post !== "object") continue;

    const postTags = Array.isArray(post.appliedTags) ? post.appliedTags : [];
    if (requiredTags.length > 0) {
      const hasAllSelectedTags = requiredTags.every((tagId) => postTags.includes(tagId));
      if (!hasAllSelectedTags) continue;
    }

    if (normalizedQuery.length > 0) {
      const titleLower = typeof post.title === "string" ? post.title.toLowerCase() : "";
      if (!titleLower.includes(normalizedQuery)) continue;
    }

    filtered.push(cloneForumPost(post));
  }

  filtered.sort((a, b) => {
    const pinnedA = a.pinned === true ? 1 : 0;
    const pinnedB = b.pinned === true ? 1 : 0;
    if (pinnedA !== pinnedB) {
      return pinnedB - pinnedA;
    }

    const timeA =
      sortOrder === FORUM_SORT_ORDERS.CREATION_DATE
        ? normalizeTimestamp(a.createdAtTimestamp)
        : normalizeTimestamp(a.lastActivityTimestamp);
    const timeB =
      sortOrder === FORUM_SORT_ORDERS.CREATION_DATE
        ? normalizeTimestamp(b.createdAtTimestamp)
        : normalizeTimestamp(b.lastActivityTimestamp);

    if (timeA !== timeB) {
      return timeB - timeA;
    }

    return String(b.id ?? "").localeCompare(String(a.id ?? ""));
  });

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
  throw new TypeError("options.container must provide ownerDocument.createElement");
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
 * Синхронизирует свойство и атрибут `checked` у чекбокса.
 *
 * @param {any} checkboxEl
 * @param {boolean} isChecked
 */
function setCheckboxChecked(checkboxEl, isChecked) {
  checkboxEl.checked = isChecked;
  if (isChecked) {
    checkboxEl.setAttribute("checked", "");
  } else {
    checkboxEl.removeAttribute("checked");
  }
}

/**
 * Синхронизирует свойство и атрибут `disabled` у элемента.
 *
 * @param {any} el
 * @param {boolean} isDisabled
 */
function setElementDisabled(el, isDisabled) {
  el.disabled = isDisabled;
  if (isDisabled) {
    el.setAttribute("disabled", "");
  } else {
    el.removeAttribute("disabled");
  }
}

/**
 * Создаёт контроллер представления форум-канала и формы создания публикации.
 *
 * @param {ForumViewControllerOptions} options
 */
export function createForumViewController(options) {
  if (!options || !options.container) {
    throw new TypeError("options.container is required");
  }

  const container = options.container;
  const doc = resolveDocument(container);

  /** @type {ForumPostItem[]} */
  let posts = Array.isArray(options.posts) ? options.posts.map(cloneForumPost) : [];
  /** @type {ForumTagItem[]} */
  let availableTags = Array.isArray(options.availableTags)
    ? options.availableTags
        .filter((t) => t && typeof t.id === "string")
        .map((t) => ({ id: t.id, name: typeof t.name === "string" ? t.name : t.id }))
    : [];
  /** @type {string[]} */
  let selectedTagIds = Array.isArray(options.selectedTagIds)
    ? options.selectedTagIds.filter((id) => typeof id === "string" && id.length > 0)
    : [];
  /** @type {string} */
  let searchQuery = typeof options.searchQuery === "string" ? options.searchQuery : "";
  /** @type {number} */
  let sortOrder = normalizeSortOrder(options.sortOrder);
  /** @type {number} */
  let layoutView = normalizeLayoutView(options.layoutView);
  /** @type {string[]} */
  let composerTagIds = [];
  /** @type {string} */
  let draftTitle = "";
  /** @type {string} */
  let draftContent = "";
  let destroyed = false;

  const root = doc.createElement("section");
  root.setAttribute("data-forum-view", "");
  root.setAttribute("role", "region");
  root.setAttribute("aria-label", "Форум");

  const filterBar = doc.createElement("div");
  filterBar.setAttribute("data-forum-tag-filter-bar", "");
  filterBar.setAttribute("role", "group");
  filterBar.setAttribute("aria-label", "Фильтр по тегам");

  const postListEl = doc.createElement("div");
  postListEl.setAttribute("data-forum-post-list", "");

  const composerForm = doc.createElement("form");
  composerForm.setAttribute("data-forum-new-post-form", "");

  /** @type {Map<string, any>} */
  const composerCheckboxMap = new Map();

  /**
   * Синхронизирует `checked` и `disabled` у чекбоксов тегов в форме создания публикации.
   */
  function syncComposerCheckboxes() {
    const capReached = composerTagIds.length >= MAX_FORUM_POST_TAGS;
    for (const [tagId, checkboxEl] of composerCheckboxMap.entries()) {
      const isSelected = composerTagIds.includes(tagId);
      setCheckboxChecked(checkboxEl, isSelected);
      setElementDisabled(checkboxEl, !isSelected && capReached);
    }
  }

  /**
   * Применяет выбор/снятие тега в форме новой публикации с соблюдением лимита `MAX_FORUM_POST_TAGS`.
   *
   * @param {string} tagId
   * @param {boolean} wantSelected
   */
  function applyComposerTagSelection(tagId, wantSelected) {
    if (wantSelected) {
      if (!composerTagIds.includes(tagId)) {
        if (composerTagIds.length >= MAX_FORUM_POST_TAGS) {
          syncComposerCheckboxes();
          return;
        }
        composerTagIds = [...composerTagIds, tagId];
      }
    } else {
      composerTagIds = composerTagIds.filter((id) => id !== tagId);
    }
    syncComposerCheckboxes();
  }

  /**
   * Перерисовывает панель фильтрации по тегам.
   */
  function renderFilterBar() {
    /** @type {any[]} */
    const buttons = [];
    for (const tag of availableTags) {
      const isActive = selectedTagIds.includes(tag.id);
      const btn = doc.createElement("button");
      btn.setAttribute("type", "button");
      btn.setAttribute("data-forum-tag-filter", tag.id);
      btn.setAttribute("aria-pressed", isActive ? "true" : "false");
      btn.textContent = tag.name;

      btn.addEventListener("click", () => {
        if (selectedTagIds.includes(tag.id)) {
          selectedTagIds = selectedTagIds.filter((id) => id !== tag.id);
        } else {
          selectedTagIds = [...selectedTagIds, tag.id];
        }
        renderFilterBar();
        renderPostList();
      });

      buttons.push(btn);
    }
    replaceElementChildren(filterBar, buttons);
  }

  /**
   * Перерисовывает список отфильтрованных и отсортированных карточек публикаций.
   */
  function renderPostList() {
    root.setAttribute("data-sort-order", String(sortOrder));
    root.setAttribute("data-layout-view", String(layoutView));

    const visiblePosts = sortAndFilterForumPosts(posts, {
      sortOrder,
      selectedTagIds,
      searchQuery,
    });

    /** @type {any[]} */
    const cards = [];
    for (const post of visiblePosts) {
      const postId = String(post.id);
      const isPinned = post.pinned === true;
      const msgCount =
        typeof post.messageCount === "number" &&
        Number.isFinite(post.messageCount) &&
        post.messageCount >= 0
          ? Math.floor(post.messageCount)
          : 0;

      const card = doc.createElement("article");
      card.setAttribute("data-forum-post-id", postId);
      if (isPinned) {
        card.setAttribute("data-pinned", "true");
        const pinnedBadge = doc.createElement("span");
        pinnedBadge.setAttribute("data-pinned", "true");
        pinnedBadge.textContent = "Закреплено";
        card.appendChild(pinnedBadge);
      }

      const titleEl = doc.createElement("h3");
      titleEl.setAttribute("data-forum-post-title", "");
      titleEl.textContent = typeof post.title === "string" ? post.title : "";
      card.appendChild(titleEl);

      const countEl = doc.createElement("span");
      countEl.setAttribute("data-forum-post-message-count", String(msgCount));
      countEl.textContent = String(msgCount);
      card.appendChild(countEl);

      card.addEventListener("click", () => {
        if (typeof options.onSelectPost === "function") {
          options.onSelectPost(postId);
        }
      });

      cards.push(card);
    }

    replaceElementChildren(postListEl, cards);
  }

  /**
   * Перерисовывает форму создания новой публикации `[data-forum-new-post-form]`.
   */
  function renderComposer() {
    composerCheckboxMap.clear();

    const titleInput = doc.createElement("input");
    titleInput.setAttribute("type", "text");
    titleInput.setAttribute("data-forum-title-input", "");
    titleInput.setAttribute("placeholder", "Заголовок публикации");
    titleInput.value = draftTitle;
    titleInput.addEventListener("input", (/** @type {any} */ event) => {
      draftTitle = String(event?.target?.value ?? titleInput.value ?? "");
    });

    const bodyInput = doc.createElement("textarea");
    bodyInput.setAttribute("data-forum-body-input", "");
    bodyInput.setAttribute("placeholder", "Напишите сообщение...");
    bodyInput.value = draftContent;
    bodyInput.addEventListener("input", (/** @type {any} */ event) => {
      draftContent = String(event?.target?.value ?? bodyInput.value ?? "");
    });

    const tagsGroup = doc.createElement("div");
    tagsGroup.setAttribute("data-forum-composer-tags", "");

    for (const tag of availableTags) {
      const labelEl = doc.createElement("label");
      labelEl.setAttribute("data-forum-composer-tag-label", tag.id);

      const checkbox = doc.createElement("input");
      checkbox.setAttribute("type", "checkbox");
      checkbox.setAttribute("data-forum-composer-tag", tag.id);
      checkbox.value = tag.id;

      checkbox.addEventListener("click", () => {
        const wantSelected = !composerTagIds.includes(tag.id);
        applyComposerTagSelection(tag.id, wantSelected);
      });

      checkbox.addEventListener("change", (/** @type {any} */ event) => {
        const explicitChecked = event?.target?.checked;
        const wantSelected =
          typeof explicitChecked === "boolean"
            ? explicitChecked
            : !composerTagIds.includes(tag.id);
        applyComposerTagSelection(tag.id, wantSelected);
      });

      const nameSpan = doc.createElement("span");
      nameSpan.textContent = tag.name;

      labelEl.appendChild(checkbox);
      labelEl.appendChild(nameSpan);
      tagsGroup.appendChild(labelEl);
      composerCheckboxMap.set(tag.id, checkbox);
    }

    const submitBtn = doc.createElement("button");
    submitBtn.setAttribute("type", "button");
    submitBtn.setAttribute("data-forum-submit-btn", "");
    submitBtn.textContent = "Опубликовать";

    function handleCreatePost() {
      const trimmedTitle = String(titleInput.value ?? draftTitle ?? "").trim();
      const trimmedContent = String(bodyInput.value ?? draftContent ?? "").trim();
      if (trimmedTitle.length === 0 || trimmedContent.length === 0) {
        return;
      }

      const appliedTags = composerTagIds.slice(0, MAX_FORUM_POST_TAGS);
      if (typeof options.onCreatePost === "function") {
        options.onCreatePost({
          title: trimmedTitle,
          content: trimmedContent,
          appliedTags,
        });
      }

      draftTitle = "";
      draftContent = "";
      titleInput.value = "";
      bodyInput.value = "";
      composerTagIds = [];
      syncComposerCheckboxes();
    }

    submitBtn.addEventListener("click", () => {
      handleCreatePost();
    });

    composerForm.addEventListener("submit", (/** @type {any} */ event) => {
      if (event && typeof event.preventDefault === "function") {
        event.preventDefault();
      }
      handleCreatePost();
    });

    replaceElementChildren(composerForm, [titleInput, bodyInput, tagsGroup, submitBtn]);
    syncComposerCheckboxes();
  }

  /**
   * Полностью перерисовывает представление форума.
   */
  function renderAll() {
    if (destroyed) return;
    renderFilterBar();
    renderPostList();
    renderComposer();
    replaceElementChildren(root, [filterBar, postListEl, composerForm]);
  }

  /**
   * Возвращает снимок текущего состояния контроллера форума.
   *
   * @returns {ForumViewStateSnapshot}
   */
  function getState() {
    return {
      posts: posts.map(cloneForumPost),
      filteredPosts: sortAndFilterForumPosts(posts, {
        sortOrder,
        selectedTagIds,
        searchQuery,
      }),
      availableTags: availableTags.map((t) => ({ id: t.id, name: t.name })),
      selectedTagIds: [...selectedTagIds],
      searchQuery,
      sortOrder,
      layoutView,
      composerTagIds: [...composerTagIds],
    };
  }

  /**
   * Частично обновляет состояние контроллера форума и перерисовывает DOM.
   *
   * @param {ForumViewStatePatch} [patch]
   * @returns {ForumViewStateSnapshot}
   */
  function setState(patch) {
    if (patch && typeof patch === "object") {
      if (Array.isArray(patch.posts)) {
        posts = patch.posts.map(cloneForumPost);
      }
      if (Array.isArray(patch.availableTags)) {
        availableTags = patch.availableTags
          .filter((t) => t && typeof t.id === "string")
          .map((t) => ({ id: t.id, name: typeof t.name === "string" ? t.name : t.id }));
      }
      if (Array.isArray(patch.selectedTagIds)) {
        selectedTagIds = patch.selectedTagIds.filter(
          (id) => typeof id === "string" && id.length > 0,
        );
      }
      if (typeof patch.searchQuery === "string") {
        searchQuery = patch.searchQuery;
      }
      if ("sortOrder" in patch) {
        sortOrder = normalizeSortOrder(patch.sortOrder);
      }
      if ("layoutView" in patch) {
        layoutView = normalizeLayoutView(patch.layoutView);
      }
    }
    renderFilterBar();
    renderPostList();
    renderComposer();
    return getState();
  }

  /**
   * Размонтирует контроллер форума и очищает контейнер.
   */
  function destroy() {
    destroyed = true;
    if (root.parentNode === container && typeof container.removeChild === "function") {
      container.removeChild(root);
    } else {
      replaceElementChildren(container, []);
    }
  }

  renderAll();
  replaceElementChildren(container, [root]);

  return {
    setState,
    getState,
    destroy,
  };
}
