// @ts-check
/**
 * task-255: чистые помощники панели бустов сервера (уровни, остаток, перки).
 * Без DOM и сети — только арифметика уровней по количеству бустов.
 */

/**
 * Пороги количества бустов для перехода на уровень.
 * @type {Readonly<{ TIER_1: number, TIER_2: number, TIER_3: number }>}
 */
export const BOOST_TIER_THRESHOLDS = Object.freeze({ TIER_1: 2, TIER_2: 7, TIER_3: 14 });

/**
 * @typedef {Readonly<{ emojiSlots: number, maxUploadMb: number, maxBitrateKbps: number }>} BoostTierPerks
 */

/**
 * Перки по уровню буста: 0 — без уровня.
 * @type {Readonly<Record<0 | 1 | 2 | 3, BoostTierPerks>>}
 */
export const BOOST_TIER_PERKS_RU = Object.freeze({
  0: Object.freeze({ emojiSlots: 50, maxUploadMb: 25, maxBitrateKbps: 96 }),
  1: Object.freeze({ emojiSlots: 100, maxUploadMb: 25, maxBitrateKbps: 128 }),
  2: Object.freeze({ emojiSlots: 150, maxUploadMb: 50, maxBitrateKbps: 256 }),
  3: Object.freeze({ emojiSlots: 250, maxUploadMb: 100, maxBitrateKbps: 384 })
});

/**
 * Нормализует вход в неотрицательное целое.
 * @param {unknown} value
 * @returns {number}
 */
function normalizeBoostCount(value) {
  return Math.max(0, Math.trunc(Number(value) || 0));
}

/**
 * Уровень буста по количеству бустов: >=14 -> 3, >=7 -> 2, >=2 -> 1, иначе 0.
 * @param {unknown} boostCount
 * @returns {0 | 1 | 2 | 3}
 */
export function resolveBoostTierFromCount(boostCount) {
  const count = normalizeBoostCount(boostCount);
  if (count >= BOOST_TIER_THRESHOLDS.TIER_3) return 3;
  if (count >= BOOST_TIER_THRESHOLDS.TIER_2) return 2;
  if (count >= BOOST_TIER_THRESHOLDS.TIER_1) return 1;
  return 0;
}

/**
 * @typedef {Object} BoostProgressSummary
 * @property {0 | 1 | 2 | 3} tier
 * @property {string} tierLabel
 * @property {number} boostsRemaining
 * @property {BoostTierPerks} perks
 */

/**
 * Сводка прогресса бустов для панели: уровень, подпись, сколько бустов до следующего.
 * @param {unknown} boostCount
 * @returns {BoostProgressSummary}
 */
export function formatBoostProgressSummaryRu(boostCount) {
  const count = normalizeBoostCount(boostCount);
  const tier = resolveBoostTierFromCount(count);
  /** @type {number | null} */
  let nextThreshold = null;
  if (tier === 0) nextThreshold = BOOST_TIER_THRESHOLDS.TIER_1;
  else if (tier === 1) nextThreshold = BOOST_TIER_THRESHOLDS.TIER_2;
  else if (tier === 2) nextThreshold = BOOST_TIER_THRESHOLDS.TIER_3;
  return {
    tier,
    tierLabel: tier === 0 ? "Без уровня" : "Уровень " + tier,
    boostsRemaining: nextThreshold === null ? 0 : Math.max(0, nextThreshold - count),
    perks: BOOST_TIER_PERKS_RU[tier]
  };
}
