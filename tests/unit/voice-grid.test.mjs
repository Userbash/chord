// @ts-check
/**
 * T-191 (G2 UI voice grid layout): контракт раскладки голосовых плиток.
 *
 * Проверяет: предел 25 видимых плиток, переходы колонок по диапазонам
 * (1, 2, 2, 3, 4, 5) и приоритет участников (фокус -> стрим -> видео ->
 * говорит -> без mute -> алфавит).
 */
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import {
  MAX_VISIBLE_VOICE_TILES,
  computeVoiceGridDimensions,
  prioritizeVoiceParticipants,
} from "../../rust/gui-spike/ui/features/voice-grid.js";

describe("voice-grid: MAX_VISIBLE_VOICE_TILES", () => {
  it("равен 25", () => {
    assert.equal(MAX_VISIBLE_VOICE_TILES, 25);
  });
});

describe("voice-grid: computeVoiceGridDimensions", () => {
  it("0 плиток — пустая сетка", () => {
    assert.deepEqual(computeVoiceGridDimensions(0), {
      columns: 0,
      rows: 0,
      visibleCount: 0,
      overflowCount: 0,
    });
  });

  it("1 плитка — одна колонка", () => {
    assert.deepEqual(computeVoiceGridDimensions(1), {
      columns: 1,
      rows: 1,
      visibleCount: 1,
      overflowCount: 0,
    });
  });

  it("2 плитки — две колонки в один ряд", () => {
    assert.deepEqual(computeVoiceGridDimensions(2), {
      columns: 2,
      rows: 1,
      visibleCount: 2,
      overflowCount: 0,
    });
  });

  it("3..=4 плитки — 2x2", () => {
    for (const n of [3, 4]) {
      assert.deepEqual(computeVoiceGridDimensions(n), {
        columns: 2,
        rows: 2,
        visibleCount: n,
        overflowCount: 0,
      });
    }
  });

  it("5..=9 плиток — три колонки", () => {
    for (let n = 5; n <= 9; n++) {
      assert.deepEqual(computeVoiceGridDimensions(n), {
        columns: 3,
        rows: Math.ceil(n / 3),
        visibleCount: n,
        overflowCount: 0,
      });
    }
  });

  it("10..=16 плиток — четыре колонки", () => {
    for (let n = 10; n <= 16; n++) {
      assert.deepEqual(computeVoiceGridDimensions(n), {
        columns: 4,
        rows: Math.ceil(n / 4),
        visibleCount: n,
        overflowCount: 0,
      });
    }
  });

  it("17..=25 плиток — пять колонок", () => {
    for (let n = 17; n <= 25; n++) {
      assert.deepEqual(computeVoiceGridDimensions(n), {
        columns: 5,
        rows: Math.ceil(n / 5),
        visibleCount: n,
        overflowCount: 0,
      });
    }
  });

  it("больше 25 — 5x5 и счётчик переполнения", () => {
    assert.deepEqual(computeVoiceGridDimensions(26), {
      columns: 5,
      rows: 5,
      visibleCount: 25,
      overflowCount: 1,
    });
    assert.deepEqual(computeVoiceGridDimensions(40), {
      columns: 5,
      rows: 5,
      visibleCount: 25,
      overflowCount: 15,
    });
  });

  it("отрицательные и нечисловые значения прижимаются к 0", () => {
    for (const bad of [-1, -100, Number.NaN]) {
      assert.deepEqual(computeVoiceGridDimensions(bad), {
        columns: 0,
        rows: 0,
        visibleCount: 0,
        overflowCount: 0,
      });
    }
  });
});

describe("voice-grid: prioritizeVoiceParticipants", () => {
  /** @param {Partial<{id: string, speaking: boolean, streaming: boolean, videoOn: boolean, muted: boolean, displayName: string}>} p @returns {any} */
  const p = (p) => ({
    id: p.id ?? "x",
    speaking: p.speaking ?? false,
    streaming: p.streaming ?? false,
    videoOn: p.videoOn ?? false,
    muted: p.muted ?? false,
    displayName: p.displayName,
  });

  it("пустой список даёт пустые visible/overflow и нулевую сетку", () => {
    assert.deepEqual(prioritizeVoiceParticipants([]), {
      visible: [],
      overflow: [],
      grid: { columns: 0, rows: 0, visibleCount: 0, overflowCount: 0 },
    });
  });

  it("фокус всегда первый, даже без флагов", () => {
    const participants = [
      p({ id: "a", displayName: "Alice", streaming: true }),
      p({ id: "f", displayName: "Zoe" }),
    ];
    const { visible } = prioritizeVoiceParticipants(participants, "f");
    assert.deepEqual(
      visible.map((x) => x.id),
      ["f", "a"],
    );
  });

  it("порядок весов: стрим -> видео -> говорит -> без mute -> алфавит", () => {
    const participants = [
      p({ id: "s", displayName: "Aaron", speaking: false }),
      p({ id: "v", displayName: "Bob", videoOn: true }),
      p({ id: "t", displayName: "Carl", speaking: true }),
      p({ id: "u", displayName: "Dave", muted: true }),
      p({ id: "m", displayName: "Eve" }),
      p({ id: "str", displayName: "Zed", streaming: true }),
    ];
    const { visible } = prioritizeVoiceParticipants(participants);
    assert.deepEqual(
      visible.map((x) => x.id),
      ["str", "v", "t", "s", "m", "u"],
    );
  });

  it("алфавит по displayName, затем по id при равенстве/отсутствии имени", () => {
    const participants = [
      p({ id: "b" }),
      p({ id: "a", displayName: "Bob" }),
      p({ id: "c", displayName: "Alice" }),
    ];
    const { visible } = prioritizeVoiceParticipants(participants);
    assert.deepEqual(
      visible.map((x) => x.id),
      ["c", "a", "b"],
    );
  });

  it("visible обрезается до 25, остальные уходят в overflow", () => {
    const participants = Array.from({ length: 30 }, (_, i) =>
      p({ id: `p${String(i).padStart(2, "0")}`, displayName: `User ${String(i).padStart(2, "0")}` }),
    );
    const { visible, overflow, grid } = prioritizeVoiceParticipants(participants);
    assert.equal(visible.length, 25);
    assert.equal(overflow.length, 5);
    assert.deepEqual(grid, {
      columns: 5,
      rows: 5,
      visibleCount: 25,
      overflowCount: 5,
    });
    assert.deepEqual(
      visible.map((x) => x.id),
      participants.slice(0, 25).map((x) => x.id),
    );
  });

  it("не мутирует входной массив", () => {
    const participants = [
      p({ id: "b", displayName: "Bob" }),
      p({ id: "a", displayName: "Alice" }),
    ];
    const snapshot = participants.map((x) => x.id);
    prioritizeVoiceParticipants(participants);
    assert.deepEqual(
      participants.map((x) => x.id),
      snapshot,
    );
  });
});
