// @ts-check
/**
 * T-189 (UI plural formatter): контракт русских плюральных форм.
 *
 * Проверяет: категории CLDR для 'one'/'few'/'many' с точными границами
 * (11, 111, 12 суффиксов, 0), fallback 'other' для нечисловых/дробных входов,
 * подстановку '{count}' санитизированным целым и пресеты UI-строк.
 */
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import {
  RU_PLURAL_PRESETS,
  formatPluralRu,
  formatPresetRu,
  selectRussianPluralCategory,
} from "../../rust/gui-spike/ui/features/plural-format.js";

describe("selectRussianPluralCategory", () => {
  it("возвращает 'one' для n%10===1, кроме 11 и его серии", () => {
    for (const n of [1, 21, 31, 101, 1001, 2001]) {
      assert.equal(selectRussianPluralCategory(n), "one", `n=${n}`);
    }
    for (const n of [11, 111, 211, 1011]) {
      assert.notEqual(selectRussianPluralCategory(n), "one", `n=${n}`);
    }
  });

  it("возвращает 'few' для 2..4 и серий, кроме 12..14", () => {
    for (const n of [2, 3, 4, 22, 23, 24, 32, 43, 102, 1003]) {
      assert.equal(selectRussianPluralCategory(n), "few", `n=${n}`);
    }
    for (const n of [12, 13, 14, 112, 113, 114, 1012]) {
      assert.equal(selectRussianPluralCategory(n), "many", `n=${n}`);
    }
  });

  it("возвращает 'many' для 0, 5..20 и прочих целых", () => {
    for (const n of [0, 5, 6, 9, 10, 11, 20, 25, 100, 111, 1000, 11111]) {
      assert.equal(selectRussianPluralCategory(n), "many", `n=${n}`);
    }
  });

  it("возвращает 'other' для отрицательных, дробных и нечисловых входов", () => {
    for (const value of [-1, -21, 1.5, 2.9, NaN, Infinity, -Infinity, "abc", null, undefined, {}, []]) {
      assert.equal(selectRussianPluralCategory(value), "other", `value=${String(value)}`);
    }
  });
});

describe("formatPluralRu", () => {
  const forms = {
    one: "{count} сообщение",
    few: "{count} сообщения",
    many: "{count} сообщений",
    other: "{count} сообщений (other)",
  };

  it("выбирает форму по категории и подставляет счётчик", () => {
    assert.equal(formatPluralRu(1, forms), "1 сообщение");
    assert.equal(formatPluralRu(21, forms), "21 сообщение");
    assert.equal(formatPluralRu(3, forms), "3 сообщения");
    assert.equal(formatPluralRu(22, forms), "22 сообщения");
    assert.equal(formatPluralRu(5, forms), "5 сообщений");
    assert.equal(formatPluralRu(11, forms), "11 сообщений");
    assert.equal(formatPluralRu(111, forms), "111 сообщений");
  });

  it("санитизирует счётчик: Math.max(0, Math.trunc(Number(x) || 0))", () => {
    assert.equal(formatPluralRu(0, forms), "0 сообщений");
    assert.equal(formatPluralRu(-5, forms), "0 сообщений");
    assert.equal(formatPluralRu(2.9, forms), "2 сообщения");
    assert.equal(formatPluralRu("7", forms), "7 сообщений");
    assert.equal(formatPluralRu("abc", forms), "0 сообщений");
    assert.equal(formatPluralRu(NaN, forms), "0 сообщений");
    assert.equal(formatPluralRu(21.9, forms), "21 сообщение");
    assert.equal(formatPluralRu(22.9, forms), "22 сообщения");
  });

  it("заменяет все вхождения '{count}'", () => {
    assert.equal(formatPluralRu(2, { few: "{count} из {count}" }), "2 из 2");
  });

  it("без подходящей формы или шаблона возвращает пустую строку", () => {
    assert.equal(formatPluralRu(5, {}), "");
    assert.equal(formatPluralRu(5, undefined), "");
    assert.equal(formatPluralRu(5, { one: "{count} штука" }), "");
    assert.equal(formatPluralRu(5, /** @type {any} */ ({ many: 42 })), "");
  });
});

describe("RU_PLURAL_PRESETS", () => {
  it("содержит точные шаблоны четырёх UI-строк", () => {
    assert.deepEqual(Object.keys(RU_PLURAL_PRESETS).sort(), [
      "attachmentsCount",
      "membersOnline",
      "typingUsers",
      "unreadMessages",
    ]);
    assert.deepEqual(RU_PLURAL_PRESETS.unreadMessages, {
      one: "{count} непрочитанное сообщение",
      few: "{count} непрочитанных сообщения",
      many: "{count} непрочитанных сообщений",
    });
    assert.deepEqual(RU_PLURAL_PRESETS.membersOnline, {
      one: "{count} участник в сети",
      few: "{count} участника в сети",
      many: "{count} участников в сети",
    });
    assert.deepEqual(RU_PLURAL_PRESETS.typingUsers, {
      one: "{count} пользователь печатает...",
      few: "{count} пользователя печатают...",
      many: "{count} пользователей печатают...",
    });
    assert.deepEqual(RU_PLURAL_PRESETS.attachmentsCount, {
      one: "{count} вложение",
      few: "{count} вложения",
      many: "{count} вложений",
    });
  });

  it("formatPresetRu форматирует пресеты и безопасен на неизвестном ключе", () => {
    assert.equal(formatPresetRu("unreadMessages", 1), "1 непрочитанное сообщение");
    assert.equal(formatPresetRu("unreadMessages", 3), "3 непрочитанных сообщения");
    assert.equal(formatPresetRu("unreadMessages", 11), "11 непрочитанных сообщений");
    assert.equal(formatPresetRu("membersOnline", 5), "5 участников в сети");
    assert.equal(formatPresetRu("typingUsers", 2), "2 пользователя печатают...");
    assert.equal(formatPresetRu("attachmentsCount", 21), "21 вложение");
    assert.equal(formatPresetRu("attachmentsCount", 0), "0 вложений");
    assert.equal(formatPresetRu("unknownPreset", 5), "");
  });
});
