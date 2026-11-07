// @ts-check
import test from 'node:test';
import assert from 'node:assert/strict';

import {
  MAX_SLOWMODE_SECONDS,
  SLOWMODE_PRESETS_SECONDS,
  clampSlowmodeSeconds,
  formatSlowmodeIntervalRu,
  computeSlowmodeRemainingSeconds,
} from '../../rust/gui-spike/ui/features/slowmode.js';

test('MAX_SLOWMODE_SECONDS = 21600 (6h)', () => {
  assert.equal(MAX_SLOWMODE_SECONDS, 21600);
});

test('SLOWMODE_PRESETS_SECONDS is frozen preset ladder', () => {
  assert.ok(Object.isFrozen(SLOWMODE_PRESETS_SECONDS));
  assert.deepEqual([...SLOWMODE_PRESETS_SECONDS], [0, 5, 10, 15, 30, 60, 120, 300, 600, 900, 1800, 3600, 7200, 21600]);
  assert.throws(() => {
    // @ts-expect-error frozen array is not writable
    SLOWMODE_PRESETS_SECONDS.push(1);
  }, TypeError);
});

test('clampSlowmodeSeconds clamps into [0, 21600]', () => {
  assert.equal(clampSlowmodeSeconds(-5), 0);
  assert.equal(clampSlowmodeSeconds(0), 0);
  assert.equal(clampSlowmodeSeconds(10.9), 10);
  assert.equal(clampSlowmodeSeconds(21600), 21600);
  assert.equal(clampSlowmodeSeconds(999999), 21600);
});

test('clampSlowmodeSeconds defaults non-finite input to 0', () => {
  assert.equal(clampSlowmodeSeconds(NaN), 0);
  assert.equal(clampSlowmodeSeconds(Infinity), 0);
  assert.equal(clampSlowmodeSeconds(-Infinity), 0);
  assert.equal(clampSlowmodeSeconds(undefined), 0);
  assert.equal(clampSlowmodeSeconds(null), 0);
  assert.equal(clampSlowmodeSeconds('30'), 0);
});

test('clampSlowmodeSeconds truncates to integer', () => {
  assert.equal(clampSlowmodeSeconds(59.99), 59);
  assert.equal(clampSlowmodeSeconds(0.9), 0);
  assert.equal(Number.isInteger(clampSlowmodeSeconds(3600.75)), true);
});

test('formatSlowmodeIntervalRu: zero means disabled', () => {
  assert.equal(formatSlowmodeIntervalRu(0), 'Медленный режим выключен');
  assert.equal(formatSlowmodeIntervalRu(-30), 'Медленный режим выключен');
  assert.equal(formatSlowmodeIntervalRu(NaN), 'Медленный режим выключен');
});

test('formatSlowmodeIntervalRu: seconds below a minute', () => {
  assert.equal(formatSlowmodeIntervalRu(5), '5 сек.');
  assert.equal(formatSlowmodeIntervalRu(10), '10 сек.');
  assert.equal(formatSlowmodeIntervalRu(59.9), '59 сек.');
});

test('formatSlowmodeIntervalRu: minutes below an hour', () => {
  assert.equal(formatSlowmodeIntervalRu(60), '1 мин.');
  assert.equal(formatSlowmodeIntervalRu(120), '2 мин.');
  assert.equal(formatSlowmodeIntervalRu(3599), '59 мин.');
  assert.equal(formatSlowmodeIntervalRu(1800), '30 мин.');
});

test('formatSlowmodeIntervalRu: hours from one hour up', () => {
  assert.equal(formatSlowmodeIntervalRu(3600), '1 ч.');
  assert.equal(formatSlowmodeIntervalRu(7200), '2 ч.');
  assert.equal(formatSlowmodeIntervalRu(21600), '6 ч.');
  assert.equal(formatSlowmodeIntervalRu(999999), '6 ч.');
});

test('computeSlowmodeRemainingSeconds: zero when slowmode disabled', () => {
  assert.equal(computeSlowmodeRemainingSeconds(0, 1000, 5000), 0);
});

test('computeSlowmodeRemainingSeconds: zero when bypassed', () => {
  assert.equal(computeSlowmodeRemainingSeconds(60, 1000, 5000, true), 0);
});

test('computeSlowmodeRemainingSeconds: zero without valid last message timestamp', () => {
  assert.equal(computeSlowmodeRemainingSeconds(60, NaN, 5000), 0);
  assert.equal(computeSlowmodeRemainingSeconds(60, 0, 5000), 0);
  assert.equal(computeSlowmodeRemainingSeconds(60, -1, 5000), 0);
  assert.equal(computeSlowmodeRemainingSeconds(60, undefined, 5000), 0);
});

test('computeSlowmodeRemainingSeconds counts down from last message', () => {
  const last = 1_700_000_000_000;
  assert.equal(computeSlowmodeRemainingSeconds(60, last, last), 60);
  assert.equal(computeSlowmodeRemainingSeconds(60, last, last + 15_000), 45);
  assert.equal(computeSlowmodeRemainingSeconds(60, last, last + 59_999), 1);
});

test('computeSlowmodeRemainingSeconds never returns negative', () => {
  const last = 1_700_000_000_000;
  assert.equal(computeSlowmodeRemainingSeconds(60, last, last + 60_000), 0);
  assert.equal(computeSlowmodeRemainingSeconds(60, last, last + 3_600_000), 0);
});

test('computeSlowmodeRemainingSeconds: clamps rate limit and treats now as 0 when invalid', () => {
  const last = 1_700_000_000_000;
  assert.equal(computeSlowmodeRemainingSeconds(999999, last, last), 21600);
  // now defaults to 0, so elapsed clamps to 0 and the full cooldown remains
  assert.equal(computeSlowmodeRemainingSeconds(60, last, NaN), 60);
  assert.equal(computeSlowmodeRemainingSeconds(60, last, undefined), 60);
});

test('computeSlowmodeRemainingSeconds: last message in the future clamps elapsed to 0', () => {
  const last = 1_700_000_000_000;
  assert.equal(computeSlowmodeRemainingSeconds(60, last, last - 30_000), 60);
});
