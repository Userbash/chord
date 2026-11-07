// @ts-check
/**
 * Карточка опроса и контроллер результатов голосования в реальном времени (task-241 / FE-P1-27).
 *
 * Отвечает за:
 * - расчёт процентного распределения голосов и определение лидирующих вариантов
 *   (`computePollPercentages`);
 * - рендер карточки опроса `[data-poll-card]` (`role="group"`), заголовка вопроса
 *   `[data-poll-question]`, вариантов ответа `[data-poll-answer-id]` (`role="radio"`
 *   или `role="checkbox"`), кнопки `[data-poll-vote-btn]` («Голосовать») и статуса
 *   `[data-poll-status]` («Опрос завершён» / «Всего голосов: N»).
 *
 * @module ui/features/poll-card
 */

/** Максимальное допустимое количество вариантов ответа в опросе Discord. */
export const MAX_POLL_ANSWERS = 10;

/** Максимальная длина текста вопроса опроса в символах. */
export const MAX_POLL_QUESTION_CHARS = 300;

/**
 * @typedef {object} PollAnswerInput
 * @property {number | string} answerId
 * @property {string} text
 * @property {number} [votes]
 */

/**
 * @typedef {object} PollAnswerComputed
 * @property {string} answerId
 * @property {string} text
 * @property {number} votes
 * @property {number} percentage
 * @property {boolean} isLeading
 */

/**
 * @typedef {object} PollCardStatePatch
 * @property {string} [question]
 * @property {ReadonlyArray<PollAnswerInput>} [answers]
 * @property {boolean} [allowMultiselect]
 * @property {boolean} [isExpired]
 * @property {ReadonlyArray<string | number>} [selectedAnswerIds]
 */

/**
 * @typedef {object} PollCardStateSnapshot
 * @property {string} question
 * @property {PollAnswerComputed[]} answers
 * @property {number} totalVotes
 * @property {boolean} allowMultiselect
 * @property {boolean} isExpired
 * @property {string[]} selectedAnswerIds
 */

/**
 * @typedef {object} PollCardControllerOptions
 * @property {any} container
 * @property {string} [question]
 * @property {ReadonlyArray<PollAnswerInput>} [answers]
 * @property {boolean} [allowMultiselect]
 * @property {boolean} [isExpired]
 * @property {ReadonlyArray<string | number>} [selectedAnswerIds]
 * @property {(answerIds: string[]) => void} [onSubmitVote]
 */

/**
 * Нормализует число голосов в неотрицательное целое число.
 *
 * @param {unknown} rawVotes
 * @returns {number}
 */
function normalizeVoteCount(rawVotes) {
  if (typeof rawVotes !== "number" || !Number.isFinite(rawVotes) || rawVotes <= 0) {
    return 0;
  }
  return Math.trunc(rawVotes);
}

/**
 * Вычисляет процент голосов и флаг лидерства (`isLeading`) для каждого варианта опроса.
 * Не мутирует входной массив и его элементы.
 *
 * @param {ReadonlyArray<PollAnswerInput> | null | undefined} answers
 * @param {number} [totalVotes]
 * @returns {PollAnswerComputed[]}
 */
export function computePollPercentages(answers, totalVotes) {
  const list = Array.isArray(answers) ? answers : [];

  /** @type {Array<{ answerId: string, text: string, votes: number }>} */
  const normalized = [];
  for (const item of list) {
    if (!item || typeof item !== "object") continue;
    const answerId =
      item.answerId !== undefined && item.answerId !== null ? String(item.answerId) : "";
    const text = typeof item.text === "string" ? item.text : "";
    const votes = normalizeVoteCount(item.votes);
    normalized.push({ answerId, text, votes });
  }

  const maxVotes = normalized.reduce((max, item) => (item.votes > max ? item.votes : max), 0);
  const safeTotalVotes =
    typeof totalVotes === "number" && Number.isFinite(totalVotes) && totalVotes > 0
      ? totalVotes
      : 0;

  return normalized.map((item) => ({
    answerId: item.answerId,
    text: item.text,
    votes: item.votes,
    percentage: safeTotalVotes > 0 ? Math.round((item.votes * 100) / safeTotalVotes) : 0,
    isLeading: item.votes > 0 && item.votes === maxVotes,
  }));
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
 * Нормализует текст вопроса опроса с ограничением до `MAX_POLL_QUESTION_CHARS`.
 *
 * @param {unknown} rawQuestion
 * @returns {string}
 */
function normalizePollQuestion(rawQuestion) {
  if (typeof rawQuestion !== "string") return "";
  return rawQuestion.slice(0, MAX_POLL_QUESTION_CHARS);
}

/**
 * Нормализует входной список вариантов ответа.
 *
 * @param {ReadonlyArray<PollAnswerInput> | null | undefined} rawAnswers
 * @returns {Array<{ answerId: string, text: string, votes: number }>}
 */
function normalizePollAnswers(rawAnswers) {
  if (!Array.isArray(rawAnswers)) return [];
  /** @type {Array<{ answerId: string, text: string, votes: number }>} */
  const result = [];
  for (const item of rawAnswers) {
    if (!item || typeof item !== "object") continue;
    result.push({
      answerId: item.answerId !== undefined && item.answerId !== null ? String(item.answerId) : "",
      text: typeof item.text === "string" ? item.text : "",
      votes: normalizeVoteCount(item.votes),
    });
  }
  return result;
}

/**
 * Нормализует массив выбранных `answerId` с учётом режима мультивыбора и лимита `MAX_POLL_ANSWERS`.
 *
 * @param {ReadonlyArray<string | number> | null | undefined} rawSelected
 * @param {boolean} allowMultiselect
 * @returns {string[]}
 */
function normalizeSelectedAnswerIds(rawSelected, allowMultiselect) {
  if (!Array.isArray(rawSelected)) return [];
  const limit = allowMultiselect ? MAX_POLL_ANSWERS : 1;
  /** @type {string[]} */
  const unique = [];
  for (const id of rawSelected) {
    if (id === undefined || id === null) continue;
    const strId = String(id);
    if (strId.length === 0 || unique.includes(strId)) continue;
    unique.push(strId);
    if (unique.length >= limit) break;
  }
  return unique;
}

/**
 * Переключает выбор варианта ответа в опросе.
 *
 * @param {ReadonlyArray<string>} currentSelected
 * @param {string} answerId
 * @param {boolean} allowMultiselect
 * @returns {string[]}
 */
function toggleSelectedAnswer(currentSelected, answerId, allowMultiselect) {
  const id = String(answerId);
  if (!allowMultiselect) {
    return currentSelected.includes(id) ? [] : [id];
  }
  if (currentSelected.includes(id)) {
    return currentSelected.filter((existing) => existing !== id);
  }
  if (currentSelected.length >= MAX_POLL_ANSWERS) {
    return currentSelected.slice();
  }
  return [...currentSelected, id];
}

/**
 * Создаёт контроллер карточки опроса и результатов голосования внутри `options.container`.
 *
 * @param {PollCardControllerOptions} options
 */
export function createPollCardController(options) {
  if (!options || !options.container) {
    throw new TypeError("options.container is required");
  }

  const container = options.container;
  const doc = resolveDocument(container);

  /** @type {string} */
  let question = normalizePollQuestion(options.question);
  /** @type {Array<{ answerId: string, text: string, votes: number }>} */
  let answers = normalizePollAnswers(options.answers);
  /** @type {boolean} */
  let allowMultiselect = options.allowMultiselect === true;
  /** @type {boolean} */
  let isExpired = options.isExpired === true;
  /** @type {string[]} */
  let selectedAnswerIds = normalizeSelectedAnswerIds(
    options.selectedAnswerIds,
    allowMultiselect,
  );
  let destroyed = false;

  const root = doc.createElement("div");
  root.setAttribute("data-poll-card", "");
  root.setAttribute("role", "group");

  /**
   * Вычисляет общее количество голосов по всем вариантам.
   *
   * @returns {number}
   */
  function getTotalVotes() {
    return answers.reduce((sum, item) => sum + item.votes, 0);
  }

  /**
   * Перерисовывает DOM карточки опроса.
   */
  function renderDom() {
    if (destroyed) return;

    const totalVotes = getTotalVotes();
    const computedAnswers = computePollPercentages(answers, totalVotes);

    const questionEl = doc.createElement("h3");
    questionEl.setAttribute("data-poll-question", "");
    questionEl.textContent = question;

    const answersListEl = doc.createElement("div");
    answersListEl.setAttribute("data-poll-answers", "");

    const optionRole = allowMultiselect ? "checkbox" : "radio";

    for (const item of computedAnswers) {
      const answerEl = doc.createElement("button");
      answerEl.setAttribute("type", "button");
      answerEl.setAttribute("data-poll-answer-id", item.answerId);
      answerEl.setAttribute("role", optionRole);

      const isChecked = selectedAnswerIds.includes(item.answerId);
      answerEl.setAttribute("aria-checked", isChecked ? "true" : "false");
      answerEl.setAttribute("data-poll-percent", `${item.percentage}%`);
      if (item.isLeading) {
        answerEl.setAttribute("data-leading", "true");
      }

      const textSpan = doc.createElement("span");
      textSpan.textContent = item.text;
      answerEl.appendChild(textSpan);

      const percentSpan = doc.createElement("span");
      percentSpan.textContent = ` (${item.percentage}%)`;
      answerEl.appendChild(percentSpan);

      answerEl.addEventListener("click", () => {
        if (isExpired) return;
        selectedAnswerIds = toggleSelectedAnswer(
          selectedAnswerIds,
          item.answerId,
          allowMultiselect,
        );
        renderDom();
      });

      answersListEl.appendChild(answerEl);
    }

    const footerEl = doc.createElement("div");
    footerEl.setAttribute("data-poll-footer", "");

    const statusEl = doc.createElement("span");
    statusEl.setAttribute("data-poll-status", "");
    statusEl.textContent = isExpired
      ? "Опрос завершён"
      : `Всего голосов: ${totalVotes}`;
    footerEl.appendChild(statusEl);

    const voteBtn = doc.createElement("button");
    voteBtn.setAttribute("type", "button");
    voteBtn.setAttribute("data-poll-vote-btn", "");
    voteBtn.textContent = "Голосовать";
    const voteDisabled = isExpired || selectedAnswerIds.length === 0;
    voteBtn.disabled = voteDisabled;
    if (voteDisabled) {
      voteBtn.setAttribute("disabled", "");
    }

    voteBtn.addEventListener("click", () => {
      if (isExpired || selectedAnswerIds.length === 0) return;
      if (typeof options.onSubmitVote === "function") {
        options.onSubmitVote(selectedAnswerIds.slice());
      }
    });
    footerEl.appendChild(voteBtn);

    replaceElementChildren(root, [questionEl, answersListEl, footerEl]);
  }

  /**
   * Возвращает снимок текущего состояния контроллера карточки опроса.
   *
   * @returns {PollCardStateSnapshot}
   */
  function getState() {
    const totalVotes = getTotalVotes();
    return {
      question,
      answers: computePollPercentages(answers, totalVotes),
      totalVotes,
      allowMultiselect,
      isExpired,
      selectedAnswerIds: selectedAnswerIds.slice(),
    };
  }

  /**
   * Частично обновляет состояние опроса и перерисовывает DOM.
   *
   * @param {PollCardStatePatch} [patch]
   * @returns {PollCardStateSnapshot}
   */
  function setState(patch) {
    if (patch && typeof patch === "object") {
      if ("question" in patch) {
        question = normalizePollQuestion(patch.question);
      }
      if (Array.isArray(patch.answers)) {
        answers = normalizePollAnswers(patch.answers);
      }
      if (typeof patch.allowMultiselect === "boolean") {
        allowMultiselect = patch.allowMultiselect;
      }
      if (typeof patch.isExpired === "boolean") {
        isExpired = patch.isExpired;
      }
      if (Array.isArray(patch.selectedAnswerIds)) {
        selectedAnswerIds = normalizeSelectedAnswerIds(
          patch.selectedAnswerIds,
          allowMultiselect,
        );
      } else if ("allowMultiselect" in patch && !allowMultiselect) {
        selectedAnswerIds = normalizeSelectedAnswerIds(selectedAnswerIds, false);
      }
    }
    renderDom();
    return getState();
  }

  /**
   * Размонтирует карточку опроса и очищает контейнер.
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
    destroy,
  };
}
