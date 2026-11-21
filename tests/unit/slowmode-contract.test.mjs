// @ts-check
/**
 * tests/unit/slowmode-contract.test.mjs
 *
 * Automated QA Boundary Value Analysis & Contract Test for `slowmode.js`.
 * Validates:
 * 1. Discord Protocol Constants: MAX_SLOWMODE_SECONDS (21600s = 6h) & Preset Ladder.
 * 2. Boundary Value Analysis (BVA): Clamping [-∞, -1, 0, 1, 21600, 21601, +∞, NaN, Non-numeric].
 * 3. Human-Readable Localization: formatSlowmodeIntervalRu with seconds, minutes, and hours.
 * 4. Remaining Time Calculation: computeSlowmodeRemainingSeconds with delta timestamps.
 * 5. Moderator Bypass: bypassSlowmode flag guarantees 0s delay.
 * 6. Resiliency & Type Safety: Invalid timestamps, non-finite values and nullish args.
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import {
  MAX_SLOWMODE_SECONDS,
  SLOWMODE_PRESETS_SECONDS,
  clampSlowmodeSeconds,
  formatSlowmodeIntervalRu,
  computeSlowmodeRemainingSeconds,
} from '../../rust/gui-spike/ui/features/slowmode.js';

describe('QA Contract — Feature: slowmode', () => {
  it('1. Constants: Discord protocol limits and presets', () => {
    assert.equal(MAX_SLOWMODE_SECONDS, 21600, 'Max slowmode must be exactly 6 hours (21600s)');
    assert.ok(Array.isArray(SLOWMODE_PRESETS_SECONDS), 'Presets must be an array');
    assert.ok(Object.isFrozen(SLOWMODE_PRESETS_SECONDS), 'Presets array must be immutable');
    assert.equal(SLOWMODE_PRESETS_SECONDS[0], 0);
    assert.equal(SLOWMODE_PRESETS_SECONDS[SLOWMODE_PRESETS_SECONDS.length - 1], 21600);
    assert.deepEqual(
      [...SLOWMODE_PRESETS_SECONDS],
      [0, 5, 10, 15, 30, 60, 120, 300, 600, 900, 1800, 3600, 7200, 21600]
    );
  });

  it('2. BVA Clamping: Clamps boundaries into [0, 21600] and truncates float', () => {
    // Ниже нуля
    assert.equal(clampSlowmodeSeconds(-100), 0);
    assert.equal(clampSlowmodeSeconds(-1), 0);
    assert.equal(clampSlowmodeSeconds(0), 0);

    // Допустимый диапазон
    assert.equal(clampSlowmodeSeconds(5), 5);
    assert.equal(clampSlowmodeSeconds(15.9), 15, 'Floats must be truncated to integer');
    assert.equal(clampSlowmodeSeconds(21600), 21600);

    // Превышение максимума
    assert.equal(clampSlowmodeSeconds(21601), 21600);
    assert.equal(clampSlowmodeSeconds(999999), 21600);

    // Невалидные типы и нечисловые значения
    assert.equal(clampSlowmodeSeconds(NaN), 0);
    assert.equal(clampSlowmodeSeconds(Infinity), 0);
    assert.equal(clampSlowmodeSeconds(-Infinity), 0);
    assert.equal(clampSlowmodeSeconds(null), 0);
    assert.equal(clampSlowmodeSeconds(undefined), 0);
    assert.equal(clampSlowmodeSeconds('15'), 0);
    assert.equal(clampSlowmodeSeconds({}), 0);
  });

  it('3. Human Localization (RU): Formats seconds, minutes and hours', () => {
    assert.equal(formatSlowmodeIntervalRu(0), 'Медленный режим выключен');
    assert.equal(formatSlowmodeIntervalRu(-5), 'Медленный режим выключен');

    // Секунды (< 60s)
    assert.equal(formatSlowmodeIntervalRu(5), '5 сек.');
    assert.equal(formatSlowmodeIntervalRu(15), '15 сек.');
    assert.equal(formatSlowmodeIntervalRu(30), '30 сек.');

    // Минуты (< 3600s)
    assert.equal(formatSlowmodeIntervalRu(60), '1 мин.');
    assert.equal(formatSlowmodeIntervalRu(120), '2 мин.');
    assert.equal(formatSlowmodeIntervalRu(300), '5 мин.');
    assert.equal(formatSlowmodeIntervalRu(900), '15 мин.');

    // Часы (>= 3600s)
    assert.equal(formatSlowmodeIntervalRu(3600), '1 ч.');
    assert.equal(formatSlowmodeIntervalRu(7200), '2 ч.');
    assert.equal(formatSlowmodeIntervalRu(21600), '6 ч.');
  });

  it('4. Remaining Time Calculation: Accurately computes remaining countdown', () => {
    const rateLimit = 15; // 15 секунд
    const now = 1_700_000_010_000; // ms

    // Сообщение отправлено 4 секунды назад -> осталось 11 секунд
    const sent4sAgo = now - 4000;
    assert.equal(computeSlowmodeRemainingSeconds(rateLimit, sent4sAgo, now), 11);

    // Сообщение отправлено 15 секунд назад -> осталось 0 секунд
    const sent15sAgo = now - 15000;
    assert.equal(computeSlowmodeRemainingSeconds(rateLimit, sent15sAgo, now), 0);

    // Сообщение отправлено 30 секунд назад -> осталось 0 секунд (не отрицательное)
    const sent30sAgo = now - 30000;
    assert.equal(computeSlowmodeRemainingSeconds(rateLimit, sent30sAgo, now), 0);
  });

  it('5. Moderator Bypass: Permission flag completely overrides cooldown', () => {
    const rateLimit = 120; // 2 минуты
    const now = 1_700_000_005_000;
    const sent1sAgo = now - 1000;

    // Обычный пользователь: ждет 119 сек
    assert.equal(computeSlowmodeRemainingSeconds(rateLimit, sent1sAgo, now, false), 119);

    // Модератор (bypass = true): мгновенный доступ (0 сек)
    assert.equal(computeSlowmodeRemainingSeconds(rateLimit, sent1sAgo, now, true), 0);
  });

  it('6. Resiliency & Type Safety: Safe with missing timestamps and null values', () => {
    assert.equal(computeSlowmodeRemainingSeconds(0, Date.now(), Date.now()), 0);
    assert.equal(computeSlowmodeRemainingSeconds(10, null, Date.now()), 0);
    assert.equal(computeSlowmodeRemainingSeconds(10, undefined, Date.now()), 0);
    assert.equal(computeSlowmodeRemainingSeconds(10, -1, Date.now()), 0);
    assert.equal(computeSlowmodeRemainingSeconds(10, NaN, Date.now()), 0);
  });
});
