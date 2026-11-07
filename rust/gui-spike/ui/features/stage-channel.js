// @ts-check
/**
 * Контроллер канала-трибуны (Stage Channel) и очереди спикеров (task-238 / FE-P1-26).
 *
 * Отвечает за:
 * - разбиение участников трибуны на группы `speakers`, `requesting`, `audience`
 *   с сортировкой по русскому алфавиту (`localeCompare("ru")`);
 * - рендер региона `[data-stage-channel]` (`role="region"`, `aria-label="Трибуна"`),
 *   заголовка темы `[data-stage-topic]` (фолбэк «Трибуна без темы»),
 *   секций «Спикеры (N)», «Запросы на выступление (N)», «Слушатели (N)»
 *   и нижней панели управления (`[data-stage-hand-btn]`, `[data-stage-leave-btn]`).
 *
 * @module ui/features/stage-channel
 */

/**
 * Роли участников трибуны (Stage Channel).
 * @type {Readonly<{ SPEAKER: "speaker", AUDIENCE: "audience", REQUESTING: "requesting" }>}
 */
export const STAGE_ROLES = Object.freeze({
  SPEAKER: "speaker",
  AUDIENCE: "audience",
  REQUESTING: "requesting",
});

/** Тема трибуны по умолчанию, если тема не задана. */
const DEFAULT_STAGE_TOPIC_RU = "Трибуна без темы";

/**
 * @typedef {"speaker" | "audience" | "requesting"} StageRole
 */

/**
 * @typedef {object} StageParticipant
 * @property {string} userId
 * @property {string} displayName
 * @property {StageRole | string} [role]
 * @property {boolean} [speaking]
 * @property {boolean} [muted]
 * @property {string | null} [raisedHandAt]
 */

/**
 * @typedef {object} PartitionedStageParticipants
 * @property {StageParticipant[]} speakers
 * @property {StageParticipant[]} requesting
 * @property {StageParticipant[]} audience
 */

/**
 * @typedef {object} StageChannelStatePatch
 * @property {string} [topic]
 * @property {ReadonlyArray<StageParticipant>} [participants]
 * @property {string} [currentUserId]
 * @property {boolean} [isModerator]
 */

/**
 * @typedef {object} StageChannelStateSnapshot
 * @property {string} topic
 * @property {string} currentUserId
 * @property {boolean} isModerator
 * @property {StageParticipant[]} participants
 * @property {StageParticipant[]} speakers
 * @property {StageParticipant[]} requesting
 * @property {StageParticipant[]} audience
 */

/**
 * @typedef {object} StageChannelControllerOptions
 * @property {any} container
 * @property {string} [topic]
 * @property {ReadonlyArray<StageParticipant>} [participants]
 * @property {string} [currentUserId]
 * @property {boolean} [isModerator]
 * @property {(raised: boolean) => void} [onToggleHand]
 * @property {(userId: string) => void} [onApproveSpeaker]
 * @property {(userId: string) => void} [onMoveToAudience]
 * @property {() => void} [onLeaveStage]
 */

/**
 * Нормализует роль участника трибуны: неизвестные значения по умолчанию приводятся к `"audience"`.
 *
 * @param {unknown} rawRole
 * @returns {StageRole}
 */
function normalizeStageRole(rawRole) {
  if (rawRole === STAGE_ROLES.SPEAKER) return STAGE_ROLES.SPEAKER;
  if (rawRole === STAGE_ROLES.REQUESTING) return STAGE_ROLES.REQUESTING;
  return STAGE_ROLES.AUDIENCE;
}

/**
 * Сравнивает двух участников по `displayName` через `localeCompare("ru")`.
 *
 * @param {StageParticipant} a
 * @param {StageParticipant} b
 * @returns {number}
 */
function compareStageParticipantsRu(a, b) {
  const nameA = typeof a?.displayName === "string" ? a.displayName : "";
  const nameB = typeof b?.displayName === "string" ? b.displayName : "";
  const byName = nameA.localeCompare(nameB, "ru");
  if (byName !== 0) return byName;
  const idA = typeof a?.userId === "string" ? a.userId : "";
  const idB = typeof b?.userId === "string" ? b.userId : "";
  return idA.localeCompare(idB, "ru");
}

/**
 * Разбивает список участников трибуны на спикеров (`speakers`),
 * запросивших слово (`requesting`) и слушателей (`audience`).
 * Каждая группа сортируется по `displayName.localeCompare("ru")`.
 * Неизвестные роли по умолчанию попадают в `audience`. Входной массив не мутируется.
 *
 * @param {ReadonlyArray<StageParticipant> | null | undefined} participants
 * @returns {PartitionedStageParticipants}
 */
export function partitionStageParticipants(participants) {
  const list = Array.isArray(participants) ? participants : [];
  /** @type {StageParticipant[]} */
  const speakers = [];
  /** @type {StageParticipant[]} */
  const requesting = [];
  /** @type {StageParticipant[]} */
  const audience = [];

  for (const item of list) {
    if (!item || typeof item !== "object") continue;
    const copy = { ...item };
    const role = normalizeStageRole(item.role);
    if (role === STAGE_ROLES.SPEAKER) {
      speakers.push(copy);
    } else if (role === STAGE_ROLES.REQUESTING) {
      requesting.push(copy);
    } else {
      audience.push(copy);
    }
  }

  speakers.sort(compareStageParticipantsRu);
  requesting.sort(compareStageParticipantsRu);
  audience.sort(compareStageParticipantsRu);

  return { speakers, requesting, audience };
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
 * Форматирует заголовок темы трибуны с фолбэком «Трибуна без темы».
 *
 * @param {unknown} rawTopic
 * @returns {string}
 */
function resolveStageTopicText(rawTopic) {
  if (typeof rawTopic === "string") {
    const trimmed = rawTopic.trim();
    if (trimmed.length > 0) {
      return trimmed;
    }
  }
  return DEFAULT_STAGE_TOPIC_RU;
}

/**
 * Создаёт контроллер канала-трибуны (Stage Channel) внутри `options.container`.
 *
 * @param {StageChannelControllerOptions} options
 */
export function createStageChannelController(options) {
  if (!options || !options.container) {
    throw new TypeError("options.container is required");
  }

  const container = options.container;
  const doc = resolveDocument(container);

  /** @type {string} */
  let topic = typeof options.topic === "string" ? options.topic : "";
  /** @type {StageParticipant[]} */
  let participants = Array.isArray(options.participants)
    ? options.participants.map((p) => ({ ...p }))
    : [];
  /** @type {string} */
  let currentUserId = typeof options.currentUserId === "string" ? options.currentUserId : "";
  /** @type {boolean} */
  let isModerator = options.isModerator === true;
  /** @type {boolean} */
  let fallbackHandRaised = false;
  let destroyed = false;

  const root = doc.createElement("section");
  root.setAttribute("data-stage-channel", "");
  root.setAttribute("role", "region");
  root.setAttribute("aria-label", "Трибуна");

  /**
   * Определяет, поднята ли рука у текущего пользователя.
   *
   * @returns {boolean}
   */
  function isCurrentUserHandRaised() {
    if (currentUserId.length > 0) {
      const current = participants.find((p) => p && p.userId === currentUserId);
      if (current) {
        return normalizeStageRole(current.role) === STAGE_ROLES.REQUESTING;
      }
    }
    return fallbackHandRaised;
  }

  /**
   * Перерисовывает содержимое региона трибуны.
   */
  function renderDom() {
    if (destroyed) return;

    const grouped = partitionStageParticipants(participants);

    // 1. Шапка трибуны с темой
    const headerEl = doc.createElement("header");
    headerEl.setAttribute("data-stage-header", "");

    const topicEl = doc.createElement("h2");
    topicEl.setAttribute("data-stage-topic", "");
    topicEl.textContent = resolveStageTopicText(topic);
    headerEl.appendChild(topicEl);

    // 2. Секция спикеров
    const speakersSection = doc.createElement("section");
    speakersSection.setAttribute("data-stage-speakers", "");

    const speakersTitle = doc.createElement("h3");
    speakersTitle.textContent = `Спикеры (${grouped.speakers.length})`;
    speakersSection.appendChild(speakersTitle);

    for (const speaker of grouped.speakers) {
      const itemEl = doc.createElement("div");
      itemEl.setAttribute("data-stage-speaker-id", String(speaker.userId));
      if (speaker.speaking === true) {
        itemEl.setAttribute("data-speaking", "true");
      }

      const nameEl = doc.createElement("span");
      nameEl.textContent = speaker.displayName;
      itemEl.appendChild(nameEl);

      if (isModerator) {
        const moveBtn = doc.createElement("button");
        moveBtn.setAttribute("type", "button");
        moveBtn.setAttribute("data-move-to-audience", String(speaker.userId));
        moveBtn.textContent = "В слушатели";
        moveBtn.addEventListener("click", () => {
          const targetId = String(speaker.userId);
          participants = participants.map((p) =>
            p && p.userId === targetId
              ? { ...p, role: STAGE_ROLES.AUDIENCE, speaking: false }
              : p,
          );
          renderDom();
          if (typeof options.onMoveToAudience === "function") {
            options.onMoveToAudience(targetId);
          }
        });
        itemEl.appendChild(moveBtn);
      }

      speakersSection.appendChild(itemEl);
    }

    // 3. Секция запросов на выступление
    const requestsSection = doc.createElement("section");
    requestsSection.setAttribute("data-stage-requests", "");

    const requestsTitle = doc.createElement("h3");
    requestsTitle.textContent = `Запросы на выступление (${grouped.requesting.length})`;
    requestsSection.appendChild(requestsTitle);

    for (const req of grouped.requesting) {
      const itemEl = doc.createElement("div");
      itemEl.setAttribute("data-stage-request-id", String(req.userId));

      const nameEl = doc.createElement("span");
      nameEl.textContent = req.displayName;
      itemEl.appendChild(nameEl);

      if (isModerator) {
        const approveBtn = doc.createElement("button");
        approveBtn.setAttribute("type", "button");
        approveBtn.setAttribute("data-approve-speaker", String(req.userId));
        approveBtn.textContent = "Одобрить";
        approveBtn.addEventListener("click", () => {
          const targetId = String(req.userId);
          participants = participants.map((p) =>
            p && p.userId === targetId ? { ...p, role: STAGE_ROLES.SPEAKER } : p,
          );
          renderDom();
          if (typeof options.onApproveSpeaker === "function") {
            options.onApproveSpeaker(targetId);
          }
        });
        itemEl.appendChild(approveBtn);
      }

      requestsSection.appendChild(itemEl);
    }

    // 4. Секция слушателей
    const audienceSection = doc.createElement("section");
    audienceSection.setAttribute("data-stage-audience", "");

    const audienceTitle = doc.createElement("h3");
    audienceTitle.textContent = `Слушатели (${grouped.audience.length})`;
    audienceSection.appendChild(audienceTitle);

    for (const listener of grouped.audience) {
      const itemEl = doc.createElement("div");
      itemEl.setAttribute("data-stage-listener-id", String(listener.userId));

      const nameEl = doc.createElement("span");
      nameEl.textContent = listener.displayName;
      itemEl.appendChild(nameEl);

      audienceSection.appendChild(itemEl);
    }

    // 5. Нижняя панель управления
    const controlsBar = doc.createElement("footer");
    controlsBar.setAttribute("data-stage-controls", "");

    const handRaised = isCurrentUserHandRaised();
    const handBtn = doc.createElement("button");
    handBtn.setAttribute("type", "button");
    handBtn.setAttribute("data-stage-hand-btn", "");
    handBtn.setAttribute("aria-pressed", handRaised ? "true" : "false");
    handBtn.textContent = handRaised ? "Опустить руку" : "Поднять руку";
    handBtn.addEventListener("click", () => {
      const nextRaised = !isCurrentUserHandRaised();
      fallbackHandRaised = nextRaised;
      if (currentUserId.length > 0) {
        let found = false;
        participants = participants.map((p) => {
          if (p && p.userId === currentUserId) {
            found = true;
            return {
              ...p,
              role: nextRaised ? STAGE_ROLES.REQUESTING : STAGE_ROLES.AUDIENCE,
            };
          }
          return p;
        });
        if (!found) {
          fallbackHandRaised = nextRaised;
        }
      }
      renderDom();
      if (typeof options.onToggleHand === "function") {
        options.onToggleHand(nextRaised);
      }
    });
    controlsBar.appendChild(handBtn);

    const leaveBtn = doc.createElement("button");
    leaveBtn.setAttribute("type", "button");
    leaveBtn.setAttribute("data-stage-leave-btn", "");
    leaveBtn.textContent = "Покинуть трибуну";
    leaveBtn.addEventListener("click", () => {
      if (typeof options.onLeaveStage === "function") {
        options.onLeaveStage();
      }
    });
    controlsBar.appendChild(leaveBtn);

    replaceElementChildren(root, [
      headerEl,
      speakersSection,
      requestsSection,
      audienceSection,
      controlsBar,
    ]);
  }

  /**
   * Возвращает снимок текущего состояния контроллера трибуны.
   *
   * @returns {StageChannelStateSnapshot}
   */
  function getState() {
    const grouped = partitionStageParticipants(participants);
    return {
      topic,
      currentUserId,
      isModerator,
      participants: participants.map((p) => ({ ...p })),
      speakers: grouped.speakers,
      requesting: grouped.requesting,
      audience: grouped.audience,
    };
  }

  /**
   * Частично обновляет состояние трибуны и перерисовывает DOM.
   *
   * @param {StageChannelStatePatch} [patch]
   * @returns {StageChannelStateSnapshot}
   */
  function setState(patch) {
    if (patch && typeof patch === "object") {
      if ("topic" in patch) {
        topic = typeof patch.topic === "string" ? patch.topic : "";
      }
      if (Array.isArray(patch.participants)) {
        participants = patch.participants.map((p) => ({ ...p }));
      }
      if (typeof patch.currentUserId === "string") {
        currentUserId = patch.currentUserId;
      }
      if (typeof patch.isModerator === "boolean") {
        isModerator = patch.isModerator;
      }
    }
    renderDom();
    return getState();
  }

  /**
   * Размонтирует контроллер и очищает контейнер.
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
