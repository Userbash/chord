// @ts-check
/**
 * T-213 (UI voice input mode): контракт режима ввода голоса.
 *
 * Проверяет: замороженный набор режимов, клампы порога VAD (−100..0 дБ) и
 * задержки отпускания PTT (0..2000 мс) с фолбэками на нечислах, правило
 * передачи (`muted`/`deafened` глушат всё, PTT смотрит на `pttPressed`,
 * VOICE_ACTIVITY сравнивает уровень с порогом) и русскую сводку режима.
 */
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import {
  VOICE_INPUT_MODES,
  clampVadThresholdDb,
  clampPttReleaseDelayMs,
  isVoiceTransmitting,
  formatVoiceInputSummaryRu,
} from "../../rust/gui-spike/ui/features/voice-input-mode.js";

describe("VOICE_INPUT_MODES", () => {
  it("содержит ровно два замороженных режима", () => {
    assert.deepEqual(Object.keys(VOICE_INPUT_MODES), ["VOICE_ACTIVITY", "PUSH_TO_TALK"]);
    assert.equal(VOICE_INPUT_MODES.VOICE_ACTIVITY, "VOICE_ACTIVITY");
    assert.equal(VOICE_INPUT_MODES.PUSH_TO_TALK, "PUSH_TO_TALK");
    assert.equal(Object.isFrozen(VOICE_INPUT_MODES), true);
  });
});

describe("clampVadThresholdDb", () => {
  it("округляет конечные числа до целого внутри −100..0", () => {
    assert.equal(clampVadThresholdDb(-45), -45);
    assert.equal(clampVadThresholdDb(-45.4), -45);
    assert.equal(clampVadThresholdDb(-45.6), -46);
    assert.equal(clampVadThresholdDb(0), 0);
    assert.equal(clampVadThresholdDb(-100), -100);
  });

  it("зажимает выход за границы", () => {
    assert.equal(clampVadThresholdDb(-500), -100);
    assert.equal(clampVadThresholdDb(12), 0);
    assert.equal(clampVadThresholdDb(0.4), 0);
  });

  it("возвращает −60 на нечислах и отсутствии значения", () => {
    assert.equal(clampVadThresholdDb(undefined), -60);
    assert.equal(clampVadThresholdDb(NaN), -60);
    assert.equal(clampVadThresholdDb(Infinity), -60);
    assert.equal(clampVadThresholdDb(-Infinity), -60);
    assert.equal(clampVadThresholdDb("‑40"), -60);
    assert.equal(clampVadThresholdDb(null), -60);
  });
});

describe("clampPttReleaseDelayMs", () => {
  it("округляет конечные числа до целого внутри 0..2000", () => {
    assert.equal(clampPttReleaseDelayMs(0), 0);
    assert.equal(clampPttReleaseDelayMs(250), 250);
    assert.equal(clampPttReleaseDelayMs(249.5), 250);
    assert.equal(clampPttReleaseDelayMs(2000), 2000);
  });

  it("зажимает выход за границы", () => {
    assert.equal(clampPttReleaseDelayMs(-1), 0);
    assert.equal(clampPttReleaseDelayMs(5000), 2000);
    assert.equal(clampPttReleaseDelayMs(-0.4), 0);
  });

  it("возвращает 200 на нечислах и отсутствии значения", () => {
    assert.equal(clampPttReleaseDelayMs(undefined), 200);
    assert.equal(clampPttReleaseDelayMs(NaN), 200);
    assert.equal(clampPttReleaseDelayMs(Infinity), 200);
    assert.equal(clampPttReleaseDelayMs("300"), 200);
  });
});

describe("isVoiceTransmitting", () => {
  it("muted/deafened всегда запрещают передачу", () => {
    const ptt = { mode: "PUSH_TO_TALK" };
    assert.equal(isVoiceTransmitting(ptt, { pttPressed: true, muted: true }), false);
    assert.equal(isVoiceTransmitting(ptt, { pttPressed: true, deafened: true }), false);
    assert.equal(
      isVoiceTransmitting({ mode: "VOICE_ACTIVITY", vadThresholdDb: -100 }, { currentLevelDb: 0, muted: true }),
      false,
    );
    assert.equal(
      isVoiceTransmitting({ mode: "VOICE_ACTIVITY", vadThresholdDb: -100 }, { currentLevelDb: 0, deafened: true }),
      false,
    );
  });

  it("PUSH_TO_TALK передаёт только при нажатой кнопке", () => {
    const config = { mode: "PUSH_TO_TALK" };
    assert.equal(isVoiceTransmitting(config, { pttPressed: true }), true);
    assert.equal(isVoiceTransmitting(config, { pttPressed: false }), false);
    assert.equal(isVoiceTransmitting(config, {}), false);
    assert.equal(isVoiceTransmitting(config, { pttPressed: "yes" }), true);
    // Уровень сигнала в PTT не участвует.
    assert.equal(isVoiceTransmitting(config, { pttPressed: false, currentLevelDb: 0 }), false);
  });

  it("VOICE_ACTIVITY сравнивает уровень с клампленым порогом", () => {
    const config = { mode: "VOICE_ACTIVITY", vadThresholdDb: -40 };
    assert.equal(isVoiceTransmitting(config, { currentLevelDb: -40 }), true);
    assert.equal(isVoiceTransmitting(config, { currentLevelDb: -39.5 }), true);
    assert.equal(isVoiceTransmitting(config, { currentLevelDb: -40.5 }), false);
    assert.equal(isVoiceTransmitting(config, { currentLevelDb: -80 }), false);
  });

  it("VOICE_ACTIVITY: нечисловой уровень не передаёт, порог по умолчанию −60", () => {
    assert.equal(isVoiceTransmitting({ mode: "VOICE_ACTIVITY" }, {}), false);
    assert.equal(isVoiceTransmitting({ mode: "VOICE_ACTIVITY" }, { currentLevelDb: NaN }), false);
    assert.equal(isVoiceTransmitting({ mode: "VOICE_ACTIVITY" }, { currentLevelDb: Infinity }), false);
    assert.equal(isVoiceTransmitting({ mode: "VOICE_ACTIVITY" }, { currentLevelDb: -60 }), true);
    assert.equal(isVoiceTransmitting({ mode: "VOICE_ACTIVITY" }, { currentLevelDb: -61 }), false);
  });

  it("неизвестный режим ведёт себя как VOICE_ACTIVITY", () => {
    assert.equal(isVoiceTransmitting({ mode: "SOMETHING" }, { currentLevelDb: 0, pttPressed: true }), true);
    assert.equal(isVoiceTransmitting({ mode: "SOMETHING" }, { currentLevelDb: -90, pttPressed: true }), false);
  });
});

describe("formatVoiceInputSummaryRu", () => {
  it("описывает режим рации с клампленой задержкой", () => {
    assert.equal(
      formatVoiceInputSummaryRu({ mode: "PUSH_TO_TALK", pttReleaseDelayMs: 300 }),
      "Режим рации (задержка 300 мс)",
    );
    assert.equal(
      formatVoiceInputSummaryRu({ mode: "PUSH_TO_TALK", pttReleaseDelayMs: 9000 }),
      "Режим рации (задержка 2000 мс)",
    );
    assert.equal(formatVoiceInputSummaryRu({ mode: "PUSH_TO_TALK" }), "Режим рации (задержка 200 мс)");
  });

  it("описывает активацию по голосу с клампленым порогом", () => {
    assert.equal(
      formatVoiceInputSummaryRu({ mode: "VOICE_ACTIVITY", vadThresholdDb: -35 }),
      "Активация по голосу (-35 дБ)",
    );
    assert.equal(
      formatVoiceInputSummaryRu({ mode: "VOICE_ACTIVITY", vadThresholdDb: 40 }),
      "Активация по голосу (0 дБ)",
    );
    assert.equal(formatVoiceInputSummaryRu({}), "Активация по голосу (-60 дБ)");
    assert.equal(formatVoiceInputSummaryRu(undefined), "Активация по голосу (-60 дБ)");
  });
});
