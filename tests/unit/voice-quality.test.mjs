// @ts-check
import assert from "node:assert/strict";
import test from "node:test";

import {
  MIN_VOICE_BITRATE_BPS,
  DEFAULT_VOICE_BITRATE_BPS,
  MAX_VOICE_BITRATE_BPS,
  VIDEO_QUALITY_MODES_RU,
  clampVoiceBitrateBps,
  formatVoiceChannelQualityRu,
} from "../../rust/gui-spike/ui/features/voice-quality.js";

test("exports documented bitrate constants", () => {
  assert.equal(MIN_VOICE_BITRATE_BPS, 8000);
  assert.equal(DEFAULT_VOICE_BITRATE_BPS, 64000);
  assert.equal(MAX_VOICE_BITRATE_BPS, 384000);
});

test("VIDEO_QUALITY_MODES_RU is frozen and carries Discord labels", () => {
  assert.ok(Object.isFrozen(VIDEO_QUALITY_MODES_RU));
  assert.deepEqual({ ...VIDEO_QUALITY_MODES_RU }, { 1: "Авто", 2: "720p (Чёткое)" });
});

test("clampVoiceBitrateBps clamps the range and rounds", () => {
  assert.equal(clampVoiceBitrateBps(64000), 64000);
  assert.equal(clampVoiceBitrateBps(1000), MIN_VOICE_BITRATE_BPS);
  assert.equal(clampVoiceBitrateBps(999999), MAX_VOICE_BITRATE_BPS);
  assert.equal(clampVoiceBitrateBps(63999.6), 64000);
  assert.equal(clampVoiceBitrateBps("48000"), 48000);
});

test("clampVoiceBitrateBps defaults non-finite input to 64000", () => {
  assert.equal(clampVoiceBitrateBps(undefined), DEFAULT_VOICE_BITRATE_BPS);
  // null coerces to 0 (finite), so the spec's clamp path gives MIN, not DEFAULT.
  assert.equal(clampVoiceBitrateBps(null), MIN_VOICE_BITRATE_BPS);
  assert.equal(clampVoiceBitrateBps(Number.NaN), DEFAULT_VOICE_BITRATE_BPS);
  assert.equal(clampVoiceBitrateBps(Number.POSITIVE_INFINITY), DEFAULT_VOICE_BITRATE_BPS);
  assert.equal(clampVoiceBitrateBps("abc"), DEFAULT_VOICE_BITRATE_BPS);
});

test("formatVoiceChannelQualityRu renders a full channel in Russian", () => {
  const result = formatVoiceChannelQualityRu({
    bitrateBps: 96000,
    userLimit: 10,
    connectedCount: 10,
    rtcRegion: "  frankfurt  ",
    videoQualityMode: 2,
  });

  assert.deepEqual(result, {
    bitrateLabel: "96 кбит/с",
    capacityLabel: "10/10",
    regionLabel: "frankfurt",
    videoModeLabel: "720p (Чёткое)",
    isFull: true,
  });
});

test("formatVoiceChannelQualityRu uses defaults for an empty channel", () => {
  const result = formatVoiceChannelQualityRu({});

  assert.deepEqual(result, {
    bitrateLabel: "64 кбит/с",
    capacityLabel: "Без лимита",
    regionLabel: "Автоматический выбор",
    videoModeLabel: "Авто",
    isFull: false,
  });
});

test("formatVoiceChannelQualityRu treats blank region and 0 limit as defaults", () => {
  const result = formatVoiceChannelQualityRu({
    bitrateBps: 0,
    userLimit: 0,
    connectedCount: 3,
    rtcRegion: "   ",
    videoQualityMode: 99,
  });

  assert.equal(result.bitrateLabel, "8 кбит/с");
  assert.equal(result.capacityLabel, "Без лимита");
  assert.equal(result.regionLabel, "Автоматический выбор");
  assert.equal(result.videoModeLabel, "Авто");
  assert.equal(result.isFull, false);
});

test("formatVoiceChannelQualityRu never reports negative capacity and clamps overflow", () => {
  const negative = formatVoiceChannelQualityRu({ userLimit: 5, connectedCount: -4 });
  assert.equal(negative.capacityLabel, "0/5");
  assert.equal(negative.isFull, false);

  const overflow = formatVoiceChannelQualityRu({ userLimit: 2, connectedCount: 7 });
  assert.equal(overflow.capacityLabel, "7/2");
  assert.equal(overflow.isFull, true);

  const huge = formatVoiceChannelQualityRu({ bitrateBps: 512000 });
  assert.equal(huge.bitrateLabel, "384 кбит/с");
});
