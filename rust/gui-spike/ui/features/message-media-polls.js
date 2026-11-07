// @ts-check
/**
 * Рендер карточек вложений сообщений (с проверкой безопасных URL и спойлерами)
 * и интерактивного виджета опросов.
 * @module ui/features/message-media-polls
 */

import {
  classifyAttachmentKind,
  formatFileSizeRu,
  isSpoilerFilename,
} from './attachment-validator.js';
import {
  computePollResults,
  formatPollExpiryRu,
  togglePollAnswerSelection,
} from './poll-progress.js';

/**
 * @typedef {{
 *   id?: string | number,
 *   filename?: string,
 *   name?: string,
 *   size?: number,
 *   url?: string,
 *   contentType?: string,
 *   content_type?: string,
 *   spoiler?: boolean,
 * }} MessageAttachmentInput
 *
 * @typedef {{
 *   onDownload?: (attachment: MessageAttachmentInput) => void,
 * }} RenderAttachmentsOptions
 *
 * @typedef {{
 *   question?: string | { text?: string },
 *   answers?: Array<{ answerId: string, text: string }>,
 *   counts?: Array<{ id: string, count?: number, meVoted?: boolean }>,
 *   allowMultiselect?: boolean,
 *   maxChoices?: number,
 *   expiryMs?: number | null,
 *   selectedAnswerIds?: string[],
 * }} PollWidgetData
 *
 * @typedef {{
 *   onVote?: (selectedAnswerIds: string[]) => void,
 *   nowMs?: number,
 * }} PollWidgetOptions
 */

/**
 * Проверяет, что URL вложения использует безопасную схему (`https://`, `blob:` или `asset:`).
 * Схемы `javascript:`, `data:`, `vbscript:`, `http://` и пустые строки отвергаются.
 *
 * @param {string | null | undefined} url
 * @returns {boolean}
 */
export function isSafeAttachmentUrl(url) {
  if (typeof url !== 'string') return false;
  const trimmed = url.trim();
  if (trimmed.length === 0 || /[\u0000-\u001f\u007f]/.test(trimmed)) return false;

  const lower = trimmed.toLowerCase();
  if (lower.startsWith('https://')) return trimmed.length > 'https://'.length;
  if (lower.startsWith('asset:')) return trimmed.length > 'asset:'.length;
  if (lower.startsWith('blob:')) {
    const rest = lower.slice('blob:'.length).trim();
    if (rest.length === 0 || rest.startsWith('javascript:') || rest.startsWith('data:')) {
      return false;
    }
    return true;
  }
  return false;
}

/**
 * Возвращает фабрику DOM-элементов из контейнера или глобального документа.
 *
 * @param {any} containerEl
 * @returns {{ createElement: (tag: string) => any }}
 */
function resolveDocument(containerEl) {
  if (containerEl && containerEl.ownerDocument && typeof containerEl.ownerDocument.createElement === 'function') {
    return containerEl.ownerDocument;
  }
  if (containerEl && typeof containerEl.createElement === 'function') {
    return containerEl;
  }
  const globalDoc = /** @type {any} */ (globalThis).document;
  if (globalDoc && typeof globalDoc.createElement === 'function') {
    return globalDoc;
  }
  throw new TypeError('Container element must provide ownerDocument.createElement');
}

/**
 * Очищает дочерние узлы контейнера и вставляет новые.
 *
 * @param {any} containerEl
 * @param {any[]} nodes
 */
function replaceContainerChildren(containerEl, nodes) {
  if (typeof containerEl.replaceChildren === 'function') {
    containerEl.replaceChildren(...nodes);
    return;
  }
  if (Array.isArray(containerEl.children)) {
    containerEl.children.length = 0;
  }
  for (const node of nodes) {
    containerEl.appendChild(node);
  }
}

/**
 * Выставляет состояние `disabled` как свойством, так и атрибутом.
 *
 * @param {any} buttonEl
 * @param {boolean} isDisabled
 */
function setButtonDisabled(buttonEl, isDisabled) {
  buttonEl.disabled = isDisabled;
  if (isDisabled) {
    if (typeof buttonEl.setAttribute === 'function') buttonEl.setAttribute('disabled', '');
  } else if (typeof buttonEl.removeAttribute === 'function') {
    buttonEl.removeAttribute('disabled');
  }
}

/**
 * Рендерит карточки вложений сообщения внутри `containerEl`.
 *
 * @param {any} containerEl
 * @param {ReadonlyArray<MessageAttachmentInput>} attachments
 * @param {RenderAttachmentsOptions} [options]
 * @returns {any[]}
 */
export function renderMessageAttachments(containerEl, attachments, options = {}) {
  if (!containerEl) return [];
  const doc = resolveDocument(containerEl);
  const list = Array.isArray(attachments) ? attachments : [];
  /** @type {any[]} */
  const cards = [];

  for (let index = 0; index < list.length; index += 1) {
    const attachment = list[index];
    if (!attachment || typeof attachment !== 'object') continue;

    const id = String(attachment.id ?? `att-${index}`);
    const filename = String(attachment.filename ?? attachment.name ?? '');
    const contentType = String(attachment.contentType ?? attachment.content_type ?? '');
    const kind = classifyAttachmentKind(filename, contentType);
    const sizeLabel = formatFileSizeRu(Number(attachment.size ?? 0));
    const rawUrl = typeof attachment.url === 'string' ? attachment.url.trim() : '';
    const urlSafe = isSafeAttachmentUrl(rawUrl);
    const spoiler = attachment.spoiler === true || isSpoilerFilename(filename);

    const card = doc.createElement('div');
    card.setAttribute('data-attachment-id', id);
    card.setAttribute('data-attachment-kind', kind);
    card.setAttribute('data-url-safe', urlSafe ? 'true' : 'false');

    const nameEl = doc.createElement('span');
    nameEl.setAttribute('data-attachment-name', '');
    nameEl.textContent = filename;
    card.appendChild(nameEl);

    const sizeEl = doc.createElement('span');
    sizeEl.setAttribute('data-attachment-size', '');
    sizeEl.textContent = sizeLabel;
    card.appendChild(sizeEl);

    if (urlSafe) {
      card.setAttribute('data-attachment-url', rawUrl);
      if (kind === 'image') {
        const img = doc.createElement('img');
        img.setAttribute('data-attachment-media', 'image');
        img.setAttribute('src', rawUrl);
        img.setAttribute('alt', filename);
        card.appendChild(img);
      } else if (kind === 'video') {
        const video = doc.createElement('video');
        video.setAttribute('data-attachment-media', 'video');
        video.setAttribute('src', rawUrl);
        video.setAttribute('controls', 'true');
        card.appendChild(video);
      } else if (kind === 'audio') {
        const audio = doc.createElement('audio');
        audio.setAttribute('data-attachment-media', 'audio');
        audio.setAttribute('src', rawUrl);
        audio.setAttribute('controls', 'true');
        card.appendChild(audio);
      }
    }

    const downloadBtn = doc.createElement('button');
    downloadBtn.setAttribute('type', 'button');
    downloadBtn.setAttribute('data-action', 'download-attachment');
    downloadBtn.textContent = 'Скачать';
    setButtonDisabled(downloadBtn, !urlSafe);
    downloadBtn.addEventListener('click', (/** @type {any} */ event) => {
      if (event && typeof event.stopPropagation === 'function') {
        event.stopPropagation();
      }
      if (!urlSafe) return;
      if (typeof options.onDownload === 'function') {
        options.onDownload(attachment);
      }
    });
    card.appendChild(downloadBtn);

    if (spoiler) {
      card.setAttribute('data-spoiler', 'hidden');
      card.setAttribute('aria-expanded', 'false');

      const badge = doc.createElement('span');
      badge.setAttribute('data-spoiler-badge', '');
      badge.textContent = 'СПОЙЛЕР';

      const toggleSpoiler = () => {
        const isHidden = card.getAttribute('data-spoiler') === 'hidden';
        card.setAttribute('data-spoiler', isHidden ? 'revealed' : 'hidden');
        card.setAttribute('aria-expanded', isHidden ? 'true' : 'false');
      };

      badge.addEventListener('click', (/** @type {any} */ event) => {
        if (event && typeof event.stopPropagation === 'function') {
          event.stopPropagation();
        }
        toggleSpoiler();
      });
      card.addEventListener('click', (/** @type {any} */ event) => {
        if (event && (event.target === badge || event.target === downloadBtn)) {
          return;
        }
        toggleSpoiler();
      });
      card.appendChild(badge);
    }

    cards.push(card);
  }

  replaceContainerChildren(containerEl, cards);
  return cards;
}

/**
 * Рендерит интерактивный виджет опроса внутри `containerEl`.
 *
 * @param {any} containerEl
 * @param {PollWidgetData} pollData
 * @param {PollWidgetOptions} [options]
 */
export function createPollWidget(containerEl, pollData = {}, options = {}) {
  const doc = resolveDocument(containerEl);
  const nowMs =
    typeof options.nowMs === 'number' && Number.isFinite(options.nowMs)
      ? options.nowMs
      : Date.now();

  const rawQuestion = pollData.question;
  const questionText =
    typeof rawQuestion === 'string'
      ? rawQuestion
      : typeof rawQuestion?.text === 'string'
        ? rawQuestion.text
        : '';

  const answers = Array.isArray(pollData.answers) ? pollData.answers : [];
  const counts = Array.isArray(pollData.counts) ? pollData.counts : [];
  const allowMultiselect = Boolean(pollData.allowMultiselect);
  const maxChoices =
    typeof pollData.maxChoices === 'number' && Number.isFinite(pollData.maxChoices)
      ? pollData.maxChoices
      : undefined;

  const results = computePollResults(answers, counts);
  const expiryText = formatPollExpiryRu(pollData.expiryMs, nowMs);
  const isExpired = expiryText === 'Опрос завершён';

  /** @type {string[]} */
  let selectedAnswerIds = Array.isArray(pollData.selectedAnswerIds)
    ? pollData.selectedAnswerIds.map((id) => String(id))
    : [];

  const root = doc.createElement('div');
  root.setAttribute('data-poll-widget', '');
  root.setAttribute('data-multiselect', allowMultiselect ? 'true' : 'false');
  root.setAttribute('data-expired', isExpired ? 'true' : 'false');

  const questionEl = doc.createElement('h3');
  questionEl.setAttribute('data-poll-question', '');
  questionEl.textContent = questionText;
  root.appendChild(questionEl);

  const answersContainer = doc.createElement('div');
  answersContainer.setAttribute('data-poll-answers', '');
  answersContainer.setAttribute('role', allowMultiselect ? 'group' : 'radiogroup');
  root.appendChild(answersContainer);

  /** @type {Map<string, any>} */
  const optionElements = new Map();

  const footer = doc.createElement('div');
  footer.setAttribute('data-poll-footer', '');

  const expiryEl = doc.createElement('span');
  expiryEl.setAttribute('data-poll-expiry', '');
  expiryEl.textContent = expiryText;
  footer.appendChild(expiryEl);

  const totalVotesEl = doc.createElement('span');
  totalVotesEl.setAttribute('data-poll-total-votes', '');
  totalVotesEl.textContent = String(results.totalVotes);
  footer.appendChild(totalVotesEl);

  const submitBtn = doc.createElement('button');
  submitBtn.setAttribute('type', 'button');
  submitBtn.setAttribute('data-action', 'poll-vote');
  submitBtn.setAttribute('data-poll-submit', '');
  submitBtn.textContent = 'Голосовать';
  footer.appendChild(submitBtn);
  root.appendChild(footer);

  function syncSelectionState() {
    for (const [answerId, optionBtn] of optionElements.entries()) {
      const isSelected = selectedAnswerIds.includes(answerId);
      optionBtn.setAttribute('data-selected', isSelected ? 'true' : 'false');
      optionBtn.setAttribute('aria-checked', isSelected ? 'true' : 'false');
    }
    setButtonDisabled(submitBtn, isExpired || selectedAnswerIds.length === 0);
  }

  for (const item of results.items) {
    const optionBtn = doc.createElement('button');
    optionBtn.setAttribute('type', 'button');
    optionBtn.setAttribute('data-poll-answer', '');
    optionBtn.setAttribute('data-answer-id', item.answerId);
    optionBtn.setAttribute('data-leading', item.isLeading ? 'true' : 'false');
    optionBtn.setAttribute('data-me-voted', item.meVoted ? 'true' : 'false');
    optionBtn.setAttribute('role', allowMultiselect ? 'checkbox' : 'radio');
    setButtonDisabled(optionBtn, isExpired);

    const textEl = doc.createElement('span');
    textEl.setAttribute('data-poll-answer-text', '');
    textEl.textContent = item.text;
    optionBtn.appendChild(textEl);

    const countEl = doc.createElement('span');
    countEl.setAttribute('data-poll-answer-count', '');
    countEl.textContent = String(item.count);
    optionBtn.appendChild(countEl);

    const pctEl = doc.createElement('span');
    pctEl.setAttribute('data-poll-answer-percentage', '');
    pctEl.textContent = `${item.percentage}%`;
    optionBtn.appendChild(pctEl);

    optionBtn.addEventListener('click', () => {
      if (isExpired) return;
      selectedAnswerIds = togglePollAnswerSelection(
        selectedAnswerIds,
        item.answerId,
        allowMultiselect,
        maxChoices,
      );
      syncSelectionState();
    });

    optionElements.set(item.answerId, optionBtn);
    answersContainer.appendChild(optionBtn);
  }

  submitBtn.addEventListener('click', () => {
    if (isExpired || selectedAnswerIds.length === 0) return;
    if (typeof options.onVote === 'function') {
      options.onVote([...selectedAnswerIds]);
    }
  });

  syncSelectionState();
  replaceContainerChildren(containerEl, [root]);

  return {
    element: root,
    /** @returns {string[]} */
    getSelectedAnswerIds() {
      return [...selectedAnswerIds];
    },
    /** @returns {boolean} */
    isExpired() {
      return isExpired;
    },
  };
}
