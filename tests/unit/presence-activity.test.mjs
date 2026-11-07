// @ts-check
/**
 * T-230 (FE-P2-25): контракт форматтера активности присутствия и выбора
 * первичной активности. Проверяет точные русские строки, приоритеты
 * CUSTOM > STREAMING > первый валидный, пустые/битые входы и лимит 128.
 */
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import {
  ACTIVITY_TYPES,
  MAX_CUSTOM_STATUS_CHARS,
  formatActivityLineRu,
  selectPrimaryActivity,
} from "../../rust/gui-spike/ui/features/presence-activity.js";

describe("task-230 presence activity", () => {
  it("exports frozen activity types and 128-char limit", () => {
    assert.deepEqual(
      { ...ACTIVITY_TYPES },
      { PLAYING: 0, STREAMING: 1, LISTENING: 2, WATCHING: 3, CUSTOM: 4, COMPETING: 5 },
    );
    assert.equal(Object.isFrozen(ACTIVITY_TYPES), true);
    assert.equal(MAX_CUSTOM_STATUS_CHARS, 128);
  });

  it("returns empty string for missing activity", () => {
    assert.equal(formatActivityLineRu(undefined), "");
    assert.equal(formatActivityLineRu(null), "");
  });

  it("formats every known type with exact Russian prefix", () => {
    assert.equal(formatActivityLineRu({ type: 0, name: "Dota 2" }), "Играет в Dota 2");
    assert.equal(formatActivityLineRu({ type: 1, name: "Speedrun" }), "Стримит Speedrun");
    assert.equal(formatActivityLineRu({ type: 2, name: "Spotify" }), "Слушает Spotify");
    assert.equal(formatActivityLineRu({ type: 3, name: "Anime" }), "Смотрит Anime");
    assert.equal(formatActivityLineRu({ type: 5, name: "CS2" }), "Соревнуется в CS2");
  });

  it("formats custom status from emoji and state", () => {
    assert.equal(formatActivityLineRu({ type: 4, emoji: "🎮", state: "в игре" }), "🎮 в игре");
    assert.equal(formatActivityLineRu({ type: 4, state: "только текст" }), "только текст");
    assert.equal(formatActivityLineRu({ type: 4, emoji: "🎧" }), "🎧");
  });

  it("slices custom status to 128 chars and returns empty for empty custom", () => {
    const long = "я".repeat(300);
    const out = formatActivityLineRu({ type: 4, state: long });
    assert.equal(out.length, MAX_CUSTOM_STATUS_CHARS);
    assert.equal(out, "я".repeat(128));
    assert.equal(formatActivityLineRu({ type: 4, state: "" }), "");
    assert.equal(formatActivityLineRu({ type: 4 }), "");
  });

  it("returns empty string for unknown type or unknown name", () => {
    assert.equal(formatActivityLineRu({ type: 99, name: "X" }), "");
    assert.equal(formatActivityLineRu({ type: 0 }), "Играет в ");
  });

  it("selects custom first, then streaming, then first valid", () => {
    const custom = { type: 4, state: "привет" };
    assert.equal(
      selectPrimaryActivity([{ type: 0, name: "A" }, custom, { type: 1, name: "S" }]),
      custom,
    );
    const streaming = { type: 1, name: "S" };
    assert.equal(selectPrimaryActivity([{ type: 0, name: "A" }, streaming]), streaming);
    const playing = { type: 0, name: "A" };
    assert.equal(selectPrimaryActivity([playing, { type: 2, name: "L" }]), playing);
  });

  it("ignores empty custom and invalid entries when selecting", () => {
    const playing = { type: 0, name: "A" };
    assert.equal(
      selectPrimaryActivity([{ type: 4, state: "" }, playing, { type: 2, name: "L" }]),
      playing,
    );
    assert.equal(selectPrimaryActivity([{ type: 4, emoji: "" }, { type: 1, name: "S" }])?.type, 1);
    assert.equal(selectPrimaryActivity([]), null);
    assert.equal(selectPrimaryActivity(null), null);
    assert.equal(selectPrimaryActivity(undefined), null);
    assert.equal(selectPrimaryActivity([{ type: 99 }, null]), null);
  });
});
