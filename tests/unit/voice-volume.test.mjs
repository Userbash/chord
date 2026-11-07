// @ts-check
/**
 * task-211 (UI voice volume helper): контракт нормализации пользовательской и
 * устройственной громкости, множителя усиления аудиоэлемента и русской подписи.
 *
 * Проверяет: границы 0..200 для пользователя и 0..100 для устройства, округление
 * к целому, откат к 100 для нечисловых входов, нулевой gain при mute/deaf и
 * метку «Без звука».
 */
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import {
  DEFAULT_USER_VOLUME,
  MAX_DEVICE_VOLUME,
  MAX_USER_VOLUME,
  MIN_USER_VOLUME,
  clampDeviceVolume,
  clampUserVolume,
  computeAudioGainMultiplier,
  formatVolumeLabelRu,
} from "../../rust/gui-spike/ui/features/voice-volume.js";

describe("voice-volume: константы", () => {
  it("границы пользовательской и устройственной громкости", () => {
    assert.equal(MIN_USER_VOLUME, 0);
    assert.equal(DEFAULT_USER_VOLUME, 100);
    assert.equal(MAX_USER_VOLUME, 200);
    assert.equal(MAX_DEVICE_VOLUME, 100);
  });
});

describe("voice-volume: clampUserVolume", () => {
  it("пропускает значения внутри диапазона", () => {
    assert.equal(clampUserVolume(0), 0);
    assert.equal(clampUserVolume(55), 55);
    assert.equal(clampUserVolume(200), 200);
  });

  it("обрезает выход за диапазон", () => {
    assert.equal(clampUserVolume(-10), 0);
    assert.equal(clampUserVolume(250), 200);
    assert.equal(clampUserVolume(1e9), 200);
  });

  it("округляет дробные значения до целого", () => {
    assert.equal(clampUserVolume(49.4), 49);
    assert.equal(clampUserVolume(49.5), 50);
    assert.equal(clampUserVolume(199.7), 200);
  });

  it("нечисловые входы дают значение по умолчанию", () => {
    for (const bad of [NaN, Infinity, -Infinity, null, undefined, "50", {}, []]) {
      assert.equal(clampUserVolume(bad), DEFAULT_USER_VOLUME, String(bad));
    }
  });
});

describe("voice-volume: clampDeviceVolume", () => {
  it("пропускает и обрезает по 0..100", () => {
    assert.equal(clampDeviceVolume(0), 0);
    assert.equal(clampDeviceVolume(60), 60);
    assert.equal(clampDeviceVolume(100), 100);
    assert.equal(clampDeviceVolume(-5), 0);
    assert.equal(clampDeviceVolume(150), 100);
  });

  it("округляет дробные значения", () => {
    assert.equal(clampDeviceVolume(33.3), 33);
    assert.equal(clampDeviceVolume(66.6), 67);
  });

  it("нечисловые входы дают 100", () => {
    for (const bad of [NaN, Infinity, null, undefined, "90"]) {
      assert.equal(clampDeviceVolume(bad), 100, String(bad));
    }
  });
});

describe("voice-volume: computeAudioGainMultiplier", () => {
  it("переводит проценты в множитель", () => {
    assert.equal(computeAudioGainMultiplier(100), 1);
    assert.equal(computeAudioGainMultiplier(0), 0);
    assert.equal(computeAudioGainMultiplier(200), 2);
    assert.equal(computeAudioGainMultiplier(50), 0.5);
  });

  it("обрезает и округляет вход", () => {
    assert.equal(computeAudioGainMultiplier(150), 1.5);
    assert.equal(computeAudioGainMultiplier(999), 2);
    assert.equal(computeAudioGainMultiplier(-20), 0);
    assert.equal(computeAudioGainMultiplier(49.6), 0.5);
  });

  it("нечисловой вход трактуется как 100%", () => {
    assert.equal(computeAudioGainMultiplier(null), 1);
    assert.equal(computeAudioGainMultiplier(undefined), 1);
    assert.equal(computeAudioGainMultiplier(NaN), 1);
  });

  it("mute или deaf дают 0", () => {
    assert.equal(computeAudioGainMultiplier(100, true), 0);
    assert.equal(computeAudioGainMultiplier(100, false, true), 0);
    assert.equal(computeAudioGainMultiplier(200, true, true), 0);
    assert.equal(computeAudioGainMultiplier(100, false, false), 1);
  });
});

describe("voice-volume: formatVolumeLabelRu", () => {
  it("обычный процент", () => {
    assert.equal(formatVolumeLabelRu(100), "100%");
    assert.equal(formatVolumeLabelRu(0), "Без звука");
    assert.equal(formatVolumeLabelRu(200), "200%");
    assert.equal(formatVolumeLabelRu(49.6), "50%");
  });

  it("mute даёт «Без звука» даже при ненулевом объёме", () => {
    assert.equal(formatVolumeLabelRu(100, true), "Без звука");
    assert.equal(formatVolumeLabelRu(200, true), "Без звука");
  });

  it("нечисловой вход как 100%", () => {
    assert.equal(formatVolumeLabelRu(null), "100%");
    assert.equal(formatVolumeLabelRu(undefined), "100%");
  });
});
