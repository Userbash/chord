// @ts-check
/**
 * task-268 (FE-INT-11): единый фасад помощников заголовка канала и композера.
 *
 * Модуль не трогает DOM и сеть: он только переиспользует чистые помощники
 * `thread-badge.js` (бейдж непрочитанных, срок автоархивации, флаги ветки) и
 * `guild-boost.js` (уровень бустов и пороги), собирая из них одну модель для
 * рендера строки заголовка канала.
 *
 * @module ui/features/chat-helpers-bridge
 */

import {
  formatUnreadCountBadge,
  formatThreadStatusSummaryRu
} from "./thread-badge.js";

import {
  BOOST_TIER_THRESHOLDS,
  resolveBoostTierFromCount
} from "./guild-boost.js";

/**
 * Метаданные ветки, влияющие на бейджи заголовка.
 * @typedef {object} ThreadMetadataInput
 * @property {boolean} [archived]
 * @property {boolean} [locked]
 * @property {number} [autoArchiveDuration]
 */

/**
 * Вход фасада: то, что уже есть у вызывающего кода о канале и ветке.
 * @typedef {object} ChannelHeaderBadgesOptions
 * @property {number} [unreadCount]
 * @property {ThreadMetadataInput | null} [threadMetadata]
 * @property {number} [boostCount]
 */

/**
 * Готовая модель бейджей строки заголовка канала.
 * @typedef {Readonly<{
 *   unreadBadge: string,
 *   threadDurationLabel: string,
 *   isThreadLocked: boolean,
 *   isThreadArchived: boolean,
 *   boostTier: number,
 *   boostProgressPercent: number
 * }>} ChannelHeaderBadgesModel
 */

/** @type {number} */
const DEFAULT_TOPIC_MAX_CHARS = 160;

/**
 * Нормализует количество бустов в неотрицательное целое.
 * @param {unknown} value
 * @returns {number}
 */
function normalizeCount(value) {
  return Math.max(0, Math.trunc(Number(value) || 0));
}

/**
 * Считает процент прогресса до следующего уровня бустов.
 *
 * База — текущий уровень: 0 -> порог TIER_1, 1 -> TIER_2, 2 -> TIER_3.
 * На максимальном уровне (3) прогресс всегда 100. Значение округляется и
 * зажимается в диапазон 0..100.
 *
 * @param {unknown} boostCount
 * @returns {number}
 */
function resolveBoostProgressPercent(boostCount) {
  const count = normalizeCount(boostCount);
  const tier = resolveBoostTierFromCount(count);
  if (tier >= 3) return 100;

  /** @type {number} */
  let nextThreshold;
  if (tier === 0) nextThreshold = BOOST_TIER_THRESHOLDS.TIER_1;
  else if (tier === 1) nextThreshold = BOOST_TIER_THRESHOLDS.TIER_2;
  else nextThreshold = BOOST_TIER_THRESHOLDS.TIER_3;

  const percent = Math.round((count / nextThreshold) * 100);
  return Math.min(100, Math.max(0, percent));
}

/**
 * Собирает модель бейджей заголовка канала из помощников ветки и бустов.
 *
 * @param {ChannelHeaderBadgesOptions | null | undefined} [options]
 * @returns {ChannelHeaderBadgesModel}
 */
export function buildChannelHeaderBadgesModel(options) {
  const unreadCount = options?.unreadCount;
  const threadMetadata = options?.threadMetadata ?? undefined;
  const boostCount = options?.boostCount;

  const threadSummary = formatThreadStatusSummaryRu(
    threadMetadata
      ? {
          autoArchiveDuration: threadMetadata.autoArchiveDuration,
          archived: threadMetadata.archived,
          locked: threadMetadata.locked
        }
      : undefined
  );

  return Object.freeze({
    unreadBadge: formatUnreadCountBadge(unreadCount),
    threadDurationLabel: threadSummary.archiveLabel,
    isThreadLocked: Boolean(threadMetadata?.locked),
    isThreadArchived: Boolean(threadMetadata?.archived),
    boostTier: resolveBoostTierFromCount(boostCount),
    boostProgressPercent: resolveBoostProgressPercent(boostCount)
  });
}

/**
 * Готовит тему канала для тултипа: обрезает края и, если тема длиннее лимита,
 * режет её до `maxChars - 1` символа и добавляет многоточие.
 *
 * Не-строка и пустая (после trim) тема дают пустую строку — тултип тогда не
 * показывается.
 *
 * @param {unknown} topic
 * @param {number} [maxChars] максимум символов вместе с многоточием, по умолчанию 160
 * @returns {string}
 */
export function formatChannelTopicTooltip(topic, maxChars = DEFAULT_TOPIC_MAX_CHARS) {
  if (typeof topic !== "string") return "";
  const trimmed = topic.trim();
  if (!trimmed) return "";

  const limit = normalizeCount(maxChars);
  if (trimmed.length <= limit) return trimmed;

  return trimmed.slice(0, Math.max(0, limit - 1)) + "…";
}
