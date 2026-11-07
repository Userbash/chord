// @ts-check
/**
 * T-207 (UI unread bar): контракт баннера непрочитанных сообщений и кнопки
 * «Перейти к последним сообщениям».
 *
 * Проверяет: порог прокрутки 300 px, точные русские подписи, счётчик `99+`,
 * обработку нулевых/нечисловых значений и усечение дробных счётчиков.
 */
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import {
  JUMP_TO_PRESENT_THRESHOLD_PX,
  computeUnreadBannerState,
  computeJumpToPresentState,
} from "../../rust/gui-spike/ui/features/unread-bar.js";

describe("JUMP_TO_PRESENT_THRESHOLD_PX", () => {
  it("равен 300", () => {
    assert.equal(JUMP_TO_PRESENT_THRESHOLD_PX, 300);
  });
});

describe("computeUnreadBannerState", () => {
  it("скрывает баннер на нуле, отрицательных и нечисловых значениях", () => {
    for (const value of [0, -1, -99, NaN, Infinity, -Infinity]) {
      assert.deepEqual(computeUnreadBannerState(value), {
        visible: false,
        count: 0,
        textRu: "",
        actionLabelRu: "Отметить как прочитанное",
      });
    }
  });

  it("показывает баннер без подписи начала отсчёта", () => {
    assert.deepEqual(computeUnreadBannerState(5), {
      visible: true,
      count: 5,
      textRu: "Новых сообщений: 5",
      actionLabelRu: "Отметить как прочитанное",
    });
  });

  it("показывает баннер с подписью начала отсчёта", () => {
    assert.deepEqual(computeUnreadBannerState(12, "вчера"), {
      visible: true,
      count: 12,
      textRu: "Новых сообщений: 12 с вчера",
      actionLabelRu: "Отметить как прочитанное",
    });
  });

  it("усекает дробный счётчик и игнорирует пустую подпись", () => {
    assert.deepEqual(computeUnreadBannerState(7.9, ""), {
      visible: true,
      count: 7,
      textRu: "Новых сообщений: 7",
      actionLabelRu: "Отметить как прочитанное",
    });
  });

  it("меньше единицы после усечения считается нулём", () => {
    assert.deepEqual(computeUnreadBannerState(0.5), {
      visible: false,
      count: 0,
      textRu: "",
      actionLabelRu: "Отметить как прочитанное",
    });
  });
});

describe("computeJumpToPresentState", () => {
  it("скрыт у нижней границы и без новых сообщений", () => {
    assert.deepEqual(computeJumpToPresentState(0), {
      visible: false,
      badgeText: "",
      labelRu: "Перейти к последним сообщениям",
      actionLabelRu: "К настоящему моменту",
    });
    assert.deepEqual(computeJumpToPresentState(299), {
      visible: false,
      badgeText: "",
      labelRu: "Перейти к последним сообщениям",
      actionLabelRu: "К настоящему моменту",
    });
  });

  it("виден на пороге 300 px", () => {
    assert.deepEqual(computeJumpToPresentState(300), {
      visible: true,
      badgeText: "",
      labelRu: "Перейти к последним сообщениям",
      actionLabelRu: "К настоящему моменту",
    });
  });

  it("виден при новых сообщениях даже у нижней границы", () => {
    assert.deepEqual(computeJumpToPresentState(0, 3), {
      visible: true,
      badgeText: "3",
      labelRu: "Перейти к последним сообщениям",
      actionLabelRu: "К настоящему моменту",
    });
  });

  it("считает 99 максимумом счётчика и показывает 99+ дальше", () => {
    assert.equal(computeJumpToPresentState(500, 99).badgeText, "99");
    assert.equal(computeJumpToPresentState(500, 100).badgeText, "99+");
    assert.equal(computeJumpToPresentState(500, 1234).badgeText, "99+");
  });

  it("показывает счётчик только при положительных новых сообщениях", () => {
    assert.equal(computeJumpToPresentState(400, 0).badgeText, "");
    assert.equal(computeJumpToPresentState(400, -5).badgeText, "");
    assert.equal(computeJumpToPresentState(400, 1).badgeText, "1");
  });
});
